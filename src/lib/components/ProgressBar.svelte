<script lang="ts">
  import type { ProgressPayload } from "../api";

  let { progress }: { progress: ProgressPayload } = $props();

  const percent = $derived(
    progress.total > 0 ? (progress.current / progress.total) * 100 : 0
  );
</script>

<div
  class="progress-track"
  role="progressbar"
  aria-valuenow={progress.current}
  aria-valuemin={0}
  aria-valuemax={progress.total}
>
  <div
    class="progress-fill"
    style="width: {Math.min(100, Math.max(0, percent))}%"
  ></div>
</div>
<p class="progress-label mono">
  {progress.current} / {progress.total} — {progress.fileName}
</p>
