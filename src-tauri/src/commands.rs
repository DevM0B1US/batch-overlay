//! Tauri IPC commands: folder scanning, template preview and batch processing.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use base64::Engine;
use image::{DynamicImage, GenericImageView};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::overlay::{self, OutFormat, Placement};

/// File extensions accepted as source images.
const SUPPORTED_EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

/// Templates up to this size (and 4096px) are inlined into the preview as-is;
/// anything bigger is thumbnailed first so the IPC stays fast.
const PREVIEW_INLINE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub path: String,
    pub count: u32,
    pub files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub current: u32,
    pub total: u32,
    pub file_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileError {
    pub file: String,
    pub message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSummary {
    pub total: u32,
    pub processed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub output_dir: String,
    pub duration_ms: u64,
    pub errors: Vec<FileError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    pub source_dir: String,
    pub template_path: String,
    pub output_dir: String,
    pub output_format: String,
    pub quality: u8,
    pub placement: String,
    pub resize_enabled: bool,
    pub max_width: u32,
    pub max_height: u32,
    pub suffix: String,
    pub overwrite: bool,
}

fn list_supported_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Cannot read folder {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Cannot read directory entry: {e}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        if SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
            files.push(path);
        }
    }
    files.sort_by_key(|p| {
        p.file_name()
            .map(|n| n.to_ascii_lowercase())
            .unwrap_or_default()
    });
    Ok(files)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

/// Decode an image file by content, not extension, so mislabeled files
/// (e.g. JPEG bytes saved as .png) still process correctly.
fn decode_image_file(path: &Path) -> Result<DynamicImage, String> {
    image::ImageReader::open(path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?
        .decode()
        .map_err(|e| format!("cannot decode {}: {e}", path.display()))
}

#[tauri::command]
pub fn scan_source_folder(path: String) -> Result<ScanResult, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("Not a folder: {path}"));
    }
    let files = list_supported_files(&dir)?;
    let names = files.iter().map(|p| file_name_of(p)).collect();
    Ok(ScanResult {
        path,
        count: files.len() as u32,
        files: names,
    })
}

#[tauri::command]
pub fn read_template_preview(path: String) -> Result<TemplatePreview, String> {
    let file = PathBuf::from(&path);
    if !file.is_file() {
        return Err(format!("Not a file: {path}"));
    }
    let bytes = fs::read(&file).map_err(|e| format!("Cannot read template: {e}"))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Cannot decode template: {e}"))?;
    let (width, height) = img.dimensions();

    let mime = match file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };

    let data_url = if bytes.len() <= PREVIEW_INLINE_LIMIT && width.max(height) <= 4096 {
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )
    } else {
        let thumb = img.thumbnail(1024, 1024);
        let mut png = Vec::new();
        thumb
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| format!("Cannot build preview: {e}"))?;
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png)
        )
    };

    Ok(TemplatePreview {
        data_url,
        width,
        height,
        file_size: bytes.len() as u64,
    })
}

/// Keep only characters that are safe in filenames on Windows and Linux.
fn sanitize_suffix(suffix: &str) -> String {
    let cleaned: String = suffix
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "_overlay".to_string()
    } else {
        cleaned
    }
}

fn run_batch(app: AppHandle, request: ProcessRequest) -> Result<ProcessSummary, String> {
    let started = Instant::now();

    let format = OutFormat::parse(&request.output_format)
        .ok_or_else(|| format!("Unsupported output format: {}", request.output_format))?;
    let placement = Placement::parse(&request.placement)
        .ok_or_else(|| format!("Unknown placement mode: {}", request.placement))?;

    let source_dir = PathBuf::from(&request.source_dir);
    if !source_dir.is_dir() {
        return Err(format!("Source folder does not exist: {}", request.source_dir));
    }
    let template_path = PathBuf::from(&request.template_path);
    if !template_path.is_file() {
        return Err(format!("Template file does not exist: {}", request.template_path));
    }
    if request.output_dir.trim().is_empty() {
        return Err("Output folder is empty".to_string());
    }
    let output_dir = PathBuf::from(&request.output_dir);
    fs::create_dir_all(&output_dir).map_err(|e| format!("Cannot create output folder: {e}"))?;

    let files = list_supported_files(&source_dir)?;
    let total = files.len() as u32;

    // Decode the template once and share it across worker threads.
    let template = decode_image_file(&template_path)
        .map_err(|e| format!("Cannot decode template: {e}"))?;

    let suffix = sanitize_suffix(&request.suffix);
    let ext = format.extension();
    let quality = request.quality.clamp(0, 100);
    let resize_enabled = request.resize_enabled && request.max_width > 0 && request.max_height > 0;

    let processed = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let errors: Mutex<Vec<FileError>> = Mutex::new(Vec::new());

    files.par_iter().for_each(|path| {
        let file_name = file_name_of(path);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image")
            .to_string();
        let out_path = output_dir.join(format!("{stem}{suffix}.{ext}"));

        if out_path.exists() && !request.overwrite {
            skipped.fetch_add(1, Ordering::Relaxed);
        } else {
            let result = (|| -> Result<(), String> {
                let img = decode_image_file(path).map_err(|e| format!("decode failed: {e}"))?;
                let img = if resize_enabled {
                    overlay::resize_to_fit(&img, request.max_width, request.max_height)
                } else {
                    img
                };
                let composed = overlay::apply_overlay(&img, &template, placement);
                let bytes = overlay::encode_image(&composed, format, quality)?;
                fs::write(&out_path, bytes).map_err(|e| format!("write failed: {e}"))?;
                Ok(())
            })();

            match result {
                Ok(()) => {
                    processed.fetch_add(1, Ordering::Relaxed);
                }
                Err(message) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                    errors
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .push(FileError { file: file_name.clone(), message });
                }
            };
        }

        let current = done.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = app.emit(
            "progress",
            ProgressPayload {
                current: current as u32,
                total,
                file_name,
            },
        );
    });

    let errors = errors.into_inner().unwrap_or_else(|p| p.into_inner());
    Ok(ProcessSummary {
        total,
        processed: processed.load(Ordering::Relaxed) as u32,
        skipped: skipped.load(Ordering::Relaxed) as u32,
        failed: failed.load(Ordering::Relaxed) as u32,
        output_dir: output_dir.to_string_lossy().into_owned(),
        duration_ms: started.elapsed().as_millis() as u64,
        errors,
    })
}

/// Runs the batch on a blocking worker thread so the UI never freezes, and
/// reports per-file progress through the `progress` event.
#[tauri::command]
pub async fn process_batch(app: AppHandle, request: ProcessRequest) -> Result<ProcessSummary, String> {
    tauri::async_runtime::spawn_blocking(move || run_batch(app, request))
        .await
        .map_err(|e| format!("Worker crashed: {e}"))?
}
