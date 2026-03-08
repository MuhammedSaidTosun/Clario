<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

  type Props = {
    imagePath: string | null;
    currentPage: number;
    pageCount: number;
    zoom: number;
    renderDevicePixelRatio: number;
    statusText: string;
    hasPdfLoaded: boolean;
    isBusy: boolean;
    onPrev: () => void;
    onNext: () => void;
    onZoomIn: () => void;
    onZoomOut: () => void;
  };

  let {
    imagePath,
    currentPage,
    pageCount,
    zoom,
    renderDevicePixelRatio,
    statusText,
    hasPdfLoaded,
    isBusy,
    onPrev,
    onNext,
    onZoomIn,
    onZoomOut
  }: Props = $props();

  let logicalImageWidth = $state<number | null>(null);

  $effect(() => {
    imagePath;
    logicalImageWidth = null;
  });

  function handleImageLoad(event: Event): void {
    if (!(event.currentTarget instanceof HTMLImageElement)) {
      return;
    }

    const safeDevicePixelRatio = renderDevicePixelRatio > 0 ? renderDevicePixelRatio : 1;
    logicalImageWidth = Math.max(1, Math.round(event.currentTarget.naturalWidth / safeDevicePixelRatio));
  }

  const imageStyle = $derived(logicalImageWidth === null ? "" : `width: ${logicalImageWidth}px;`);
  const imageUrl = $derived(imagePath ? convertFileSrc(imagePath) : null);
</script>

<section class="panel viewer">
  <h2>PDF Viewer</h2>

  <div class="controls">
    <button type="button" onclick={onPrev} disabled={!hasPdfLoaded || currentPage <= 1 || isBusy}>
      Previous page
    </button>
    <button
      type="button"
      onclick={onNext}
      disabled={!hasPdfLoaded || currentPage >= pageCount || isBusy}
    >
      Next page
    </button>
    <button type="button" onclick={onZoomOut} disabled={!hasPdfLoaded || isBusy}>Zoom out</button>
    <button type="button" onclick={onZoomIn} disabled={!hasPdfLoaded || isBusy}>Zoom in</button>
  </div>

  <p><strong>Page:</strong> {currentPage} / {pageCount}</p>
  <p><strong>Zoom:</strong> {zoom.toFixed(2)}x</p>

  {#if imagePath && imageUrl}
    <div class="image-wrap">
      <img src={imageUrl} alt={"Rendered PDF page " + currentPage} onload={handleImageLoad} style={imageStyle} />
    </div>
  {:else}
    <p>No PDF page rendered yet.</p>
  {/if}

  <p><strong>Viewer status:</strong> {statusText}</p>
</section>

<style>
  .viewer {
    margin-top: 1rem;
  }

  .controls {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 0.8rem;
  }

  .controls button {
    border: 1px solid #2a4f7a;
    background: #2a4f7a;
    color: #fff;
    border-radius: 8px;
    padding: 0.5rem 0.8rem;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .controls button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .image-wrap {
    margin-top: 0.8rem;
    border: 1px solid #d9d9d9;
    background: #fff;
    border-radius: 8px;
    padding: 0.5rem;
    overflow: auto;
    max-height: 70vh;
  }

  .image-wrap img {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 0 auto;
  }
</style>
