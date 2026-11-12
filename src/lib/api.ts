import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface ScanResult {
  path: string;
  count: number;
}

export interface TemplatePreview {
  dataUrl: string;
  width: number;
  height: number;
  fileSize: number;
}

export interface ProgressPayload {
  current: number;
  total: number;
  fileName: string;
}

export interface FileError {
  file: string;
  message: string;
}

export interface ProcessSummary {
  total: number;
  processed: number;
  skipped: number;
  failed: number;
  outputDir: string;
  durationMs: number;
  errors: FileError[];
}

export interface ProcessRequest {
  sourceDir: string;
  templatePath: string;
  outputDir: string;
  outputFormat: "png" | "jpeg";
  quality: number;
  placement: "center" | "stretch" | "fit";
  resizeEnabled: boolean;
  maxWidth: number;
  maxHeight: number;
  suffix: string;
  overwrite: boolean;
}

export function scanSourceFolder(path: string): Promise<ScanResult> {
  return invoke("scan_source_folder", { path });
}

export function readTemplatePreview(path: string): Promise<TemplatePreview> {
  return invoke("read_template_preview", { path });
}

export function processBatch(request: ProcessRequest): Promise<ProcessSummary> {
  return invoke("process_batch", { request });
}

export function listenProgress(
  handler: (payload: ProgressPayload) => void
): Promise<() => void> {
  return listen<ProgressPayload>("progress", (event) => handler(event.payload));
}
