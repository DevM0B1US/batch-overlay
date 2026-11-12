<script lang="ts">
  interface Settings {
    outputFormat: "png" | "jpeg";
    quality: number;
    outputDir: string;
    outputDirTouched: boolean;
    placement: "center" | "stretch" | "fit";
    resizeEnabled: boolean;
    maxWidth: number;
    maxHeight: number;
    suffix: string;
    overwrite: boolean;
  }

  let {
    settings,
    busy,
    sourceDir,
    onBrowseOutput,
    onResetOutput,
  }: {
    settings: Settings;
    busy: boolean;
    sourceDir: string;
    onBrowseOutput: () => void;
    onResetOutput: () => void;
  } = $props();

  const qualityDisabled = $derived(busy || settings.outputFormat !== "jpeg");
  const resizeDisabled = $derived(busy || !settings.resizeEnabled);
</script>

<aside class="card settings">
  <h2>Settings</h2>

  <div class="field">
    <label class="field-label" for="fmt">Output format</label>
    <select id="fmt" bind:value={settings.outputFormat} disabled={busy}>
      <option value="png">PNG</option>
      <option value="jpeg">JPEG</option>
    </select>
  </div>

  <div class="field">
    <label class="field-label" for="q">Quality · {settings.quality}</label>
    <input
      id="q"
      type="range"
      min="0"
      max="100"
      bind:value={settings.quality}
      disabled={qualityDisabled}
    />
    <p class="hint">Applies to JPEG output only. PNG is always lossless.</p>
  </div>

  <div class="field">
    <label class="field-label" for="out">Output folder</label>
    <input
      id="out"
      type="text"
      bind:value={settings.outputDir}
      oninput={() => (settings.outputDirTouched = true)}
      disabled={busy}
      spellcheck="false"
    />
    <div class="row" style="margin-top:6px">
      <button class="btn" onclick={onBrowseOutput} disabled={busy}>
        Browse…
      </button>
      <button
        class="btn"
        onclick={onResetOutput}
        disabled={busy || !sourceDir}
      >
        Reset to default
      </button>
    </div>
    <p class="hint">Default: an “output” subfolder inside the source folder.</p>
  </div>

  <div class="field">
    <label class="field-label" for="place">Template placement</label>
    <select id="place" bind:value={settings.placement} disabled={busy}>
      <option value="center">Center</option>
      <option value="stretch">Stretch</option>
      <option value="fit">Fit</option>
    </select>
    <p class="hint">
      Center keeps the template’s own size · Stretch fills the whole image ·
      Fit scales uniformly to the largest size that fits.
    </p>
  </div>

  <div class="field">
    <label class="check">
      <input type="checkbox" bind:checked={settings.resizeEnabled} disabled={busy} />
      Resize source images before overlay
    </label>
    <div class="two-col" style="margin-top:8px">
      <div>
        <label class="field-label" for="mw">Max width</label>
        <input
          id="mw"
          type="number"
          min="1"
          max="20000"
          bind:value={settings.maxWidth}
          disabled={resizeDisabled}
        />
      </div>
      <div>
        <label class="field-label" for="mh">Max height</label>
        <input
          id="mh"
          type="number"
          min="1"
          max="20000"
          bind:value={settings.maxHeight}
          disabled={resizeDisabled}
        />
      </div>
    </div>
    <p class="hint">Aspect ratio is preserved; images are only scaled down.</p>
  </div>

  <div class="field">
    <label class="field-label" for="suffix">Filename suffix</label>
    <input
      id="suffix"
      type="text"
      bind:value={settings.suffix}
      disabled={busy}
      spellcheck="false"
      placeholder="_overlay"
    />
    <p class="hint">Output name: original name + suffix, e.g. photo_overlay.jpg</p>
  </div>

  <div class="field">
    <label class="check">
      <input type="checkbox" bind:checked={settings.overwrite} disabled={busy} />
      Overwrite existing files
    </label>
    <p class="hint">
      When off, files that already exist in the output folder are skipped.
    </p>
  </div>
</aside>
