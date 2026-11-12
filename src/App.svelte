<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { join } from "@tauri-apps/api/path";
  import {
    scanSourceFolder,
    readTemplatePreview,
    processBatch,
    listenProgress,
    type TemplatePreview,
    type ProcessSummary,
    type ProgressPayload,
  } from "./lib/api";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import ProgressBar from "./lib/components/ProgressBar.svelte";

  let sourceDir = $state("");
  let sourceCount = $state<number | null>(null);
  let templatePath = $state("");
  let template = $state<TemplatePreview | null>(null);

  let busy = $state(false);
  let progress = $state<ProgressPayload | null>(null);
  let summary = $state<ProcessSummary | null>(null);
  let errorMsg = $state("");

  let settings = $state({
    outputFormat: "png" as "png" | "jpeg",
    quality: 90,
    outputDir: "",
    outputDirTouched: false,
    placement: "center" as "center" | "stretch" | "fit",
    resizeEnabled: false,
    maxWidth: 1920,
    maxHeight: 1920,
    suffix: "_overlay",
    overwrite: false,
  });

  const canStart = $derived(sourceDir !== "" && templatePath !== "" && !busy);

  onMount(() => {
    const promise = listenProgress((p) => {
      // Keep the bar monotonic even if worker events arrive slightly out of order.
      const prev = progress;
      const current =
        prev && prev.total === p.total && p.current < prev.current
          ? prev.current
          : p.current;
      progress = { ...p, current };
    });
    return () => {
      promise.then((unlisten) => unlisten());
    };
  });

  function resetResults() {
    summary = null;
    errorMsg = "";
    progress = null;
  }

  async function pickSourceFolder() {
    const selection = await open({
      directory: true,
      multiple: false,
      title: "Select source folder",
    });
    if (typeof selection !== "string") return;
    resetResults();
    try {
      const result = await scanSourceFolder(selection);
      sourceDir = result.path;
      sourceCount = result.count;
      if (!settings.outputDirTouched) {
        settings.outputDir = await join(result.path, "output");
      }
    } catch (e) {
      errorMsg = String(e);
      sourceDir = "";
      sourceCount = null;
    }
  }

  async function pickTemplate() {
    const selection = await open({
      multiple: false,
      title: "Select overlay template",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg"] }],
    });
    if (typeof selection !== "string") return;
    resetResults();
    try {
      template = await readTemplatePreview(selection);
      templatePath = selection;
    } catch (e) {
      errorMsg = String(e);
      template = null;
      templatePath = "";
    }
  }

  async function browseOutputDir() {
    const selection = await open({
      directory: true,
      multiple: false,
      title: "Select output folder",
    });
    if (typeof selection !== "string") return;
    settings.outputDir = selection;
    settings.outputDirTouched = true;
  }

  async function resetOutputDir() {
    if (!sourceDir) return;
    settings.outputDir = await join(sourceDir, "output");
    settings.outputDirTouched = false;
  }

  function clampDimension(value: number): number | null {
    return Number.isFinite(value) && value >= 1 ? Math.floor(value) : null;
  }

  async function start() {
    if (!canStart) return;
    if (!settings.outputDir.trim()) {
      errorMsg = "Choose an output folder first.";
      return;
    }
    if (settings.resizeEnabled) {
      const w = clampDimension(settings.maxWidth);
      const h = clampDimension(settings.maxHeight);
      if (w === null || h === null) {
        errorMsg = "Enter a valid max width and height (minimum 1 px).";
        return;
      }
      settings.maxWidth = w;
      settings.maxHeight = h;
    }
    resetResults();
    busy = true;
    try {
      summary = await processBatch({
        sourceDir,
        templatePath,
        outputDir: settings.outputDir,
        outputFormat: settings.outputFormat,
        quality: settings.outputFormat === "jpeg" ? settings.quality : 100,
        placement: settings.placement,
        resizeEnabled: settings.resizeEnabled,
        maxWidth: Math.max(1, Math.floor(settings.maxWidth) || 1),
        maxHeight: Math.max(1, Math.floor(settings.maxHeight) || 1),
        suffix: settings.suffix,
        overwrite: settings.overwrite,
      });
    } catch (e) {
      errorMsg = String(e);
    } finally {
      busy = false;
      progress = null;
    }
  }
</script>

<main class="app">
  <section class="col-main">
    <div class="card">
      <h2>1 · Source folder</h2>
      <div class="row">
        <button class="btn" onclick={pickSourceFolder} disabled={busy}>
          Select Source Folder
        </button>
        {#if sourceCount !== null}
          <span class="meta">
            {sourceCount} supported image{sourceCount === 1 ? "" : "s"} found
            (jpg / jpeg / png)
          </span>
        {/if}
      </div>
      {#if sourceDir}
        <p class="meta mono" style="margin-top:8px">{sourceDir}</p>
      {/if}
    </div>

    <div class="card">
      <h2>2 · Template</h2>
      <div class="row">
        <button class="btn" onclick={pickTemplate} disabled={busy}>
          Select Template
        </button>
        {#if template}
          <span class="meta">
            {template.width} × {template.height} px ·
            {(template.fileSize / 1024).toFixed(0)} KB
          </span>
        {/if}
      </div>
      {#if template}
        <div class="preview-box checkerboard">
          <img src={template.dataUrl} alt="Template preview" />
        </div>
        <p class="meta mono" style="margin-top:6px">{templatePath}</p>
      {/if}
    </div>

    <div class="card">
      <h2>3 · Process</h2>
      <button class="btn-primary" onclick={start} disabled={!canStart}>
        {busy ? "Processing…" : "Start Batch Processing"}
      </button>
      {#if busy && progress}
        <div style="margin-top:12px">
          <ProgressBar {progress} />
        </div>
      {/if}
      {#if summary}
        <div class="summary" role="status">
          <p class="summary-line ok">
            Done — {summary.processed} processed · {summary.skipped} skipped ·
            {summary.failed} failed in {(summary.durationMs / 1000).toFixed(1)}s
          </p>
          <p class="meta mono">Saved to {summary.outputDir}</p>
          {#if summary.errors.length > 0}
            <ul class="error-list">
              {#each summary.errors.slice(0, 8) as err}
                <li><span class="mono">{err.file}</span> — {err.message}</li>
              {/each}
              {#if summary.errors.length > 8}
                <li>… and {summary.errors.length - 8} more</li>
              {/if}
            </ul>
          {/if}
        </div>
      {/if}
      {#if errorMsg && !busy}
        <div class="error-box" style="margin-top:10px">{errorMsg}</div>
      {/if}
    </div>
  </section>

  <SettingsPanel
    {settings}
    {busy}
    {sourceDir}
    onBrowseOutput={browseOutputDir}
    onResetOutput={resetOutputDir}
  />
</main>
