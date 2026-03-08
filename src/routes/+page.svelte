<script lang="ts">
  import PdfViewer from "../components/PdfViewer.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type AppDirs = {
    temp_dir: string;
    cache_dir: string;
    log_file: string;
  };

  type PdfPageRenderResponse = {
    image_path: string;
    page: number;
    page_count: number;
    zoom: number;
  };

  const MIN_ZOOM = 0.5;
  const MAX_ZOOM = 3.0;
  const ZOOM_STEP = 0.25;
  const MIN_DEVICE_PIXEL_RATIO = 1;
  const MAX_DEVICE_PIXEL_RATIO = 4;

  let selectedFilePath = $state("No file selected.");
  let debugDirs = $state<AppDirs | null>(null);
  let statusText = $state("Starting...");
  let backendPing = $state("");
  let currentPdfPath = $state<string | null>(null);
  let currentPage = $state(1);
  let pageCount = $state(0);
  let zoom = $state(1.0);
  let renderDevicePixelRatio = $state(1.0);
  let renderedImagePath = $state<string | null>(null);
  let renderError = $state<string | null>(null);
  let isRendering = $state(false);

  function formatError(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }

    return String(error);
  }

  async function loadDebugPanel(): Promise<void> {
    debugDirs = await invoke<AppDirs>("app_dirs");
    backendPing = await invoke<string>("ping");
  }

  function isPdfPath(path: string): boolean {
    return path.toLowerCase().endsWith(".pdf");
  }

  function getDevicePixelRatio(): number {
    if (typeof window === "undefined") {
      return 1.0;
    }

    const value = window.devicePixelRatio;

    if (!Number.isFinite(value) || value <= 0) {
      return 1.0;
    }

    const clamped = Math.min(MAX_DEVICE_PIXEL_RATIO, Math.max(MIN_DEVICE_PIXEL_RATIO, value));
    return Number(clamped.toFixed(2));
  }

  function resetPdfState(pdfPath: string): void {
    currentPdfPath = pdfPath;
    // The UI and backend command API are both 1-based for page indexing.
    currentPage = 1;
    zoom = 1.0;
    pageCount = 0;
    renderedImagePath = null;
    renderError = null;
  }

  async function renderPdfPage(filePath: string, pageToRender: number, zoomToRender: number): Promise<void> {
    isRendering = true;
    renderError = null;
    const devicePixelRatio = getDevicePixelRatio();

    try {
      const response = await invoke<PdfPageRenderResponse>("render_pdf_page", {
        filePath,
        page: pageToRender,
        zoom: zoomToRender,
        devicePixelRatio
      });

      currentPdfPath = filePath;
      currentPage = response.page;
      pageCount = response.page_count;
      zoom = response.zoom;
      renderDevicePixelRatio = devicePixelRatio;
      renderedImagePath = response.image_path;
      statusText = `Showing page ${response.page} / ${response.page_count} at ${response.zoom.toFixed(2)}x`;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isRendering = false;
    }
  }

  async function handlePrevPage(): Promise<void> {
    if (!currentPdfPath || isRendering) {
      return;
    }

    if (currentPage <= 1) {
      statusText = "Already at page 1.";
      return;
    }

    await renderPdfPage(currentPdfPath, currentPage - 1, zoom);
  }

  async function handleNextPage(): Promise<void> {
    if (!currentPdfPath || isRendering) {
      return;
    }

    if (currentPage >= pageCount) {
      statusText = "Already at the last page.";
      return;
    }

    await renderPdfPage(currentPdfPath, currentPage + 1, zoom);
  }

  async function handleZoomIn(): Promise<void> {
    if (!currentPdfPath || isRendering) {
      return;
    }

    const nextZoom = Math.min(MAX_ZOOM, Number((zoom + ZOOM_STEP).toFixed(2)));
    if (nextZoom === zoom) {
      statusText = `Maximum zoom is ${MAX_ZOOM.toFixed(2)}x.`;
      return;
    }

    await renderPdfPage(currentPdfPath, currentPage, nextZoom);
  }

  async function handleZoomOut(): Promise<void> {
    if (!currentPdfPath || isRendering) {
      return;
    }

    const nextZoom = Math.max(MIN_ZOOM, Number((zoom - ZOOM_STEP).toFixed(2)));
    if (nextZoom === zoom) {
      statusText = `Minimum zoom is ${MIN_ZOOM.toFixed(2)}x.`;
      return;
    }

    await renderPdfPage(currentPdfPath, currentPage, nextZoom);
  }

  onMount(async () => {
    try {
      await invoke("ensure_app_dirs");
      await invoke("log_info", { message: "Application started" });
      await loadDebugPanel();
      renderDevicePixelRatio = getDevicePixelRatio();
      statusText = "Ready";
    } catch (error) {
      statusText = `Startup error: ${formatError(error)}`;
    }
  });

  async function openFile(): Promise<void> {
    try {
      const selection = await invoke<string | string[] | null>("plugin:dialog|open", {
        options: {
          title: "Open File",
          multiple: false,
          directory: false
        }
      });

      if (selection === null || Array.isArray(selection)) {
        statusText = "File selection canceled.";
        return;
      }

      selectedFilePath = selection;
      await invoke("log_info", { message: `File selected: ${selection}` });
      statusText = "File selected and logged.";

      if (!isPdfPath(selection)) {
        statusText = "Selected file is not a PDF. Viewer unchanged.";
        return;
      }

      resetPdfState(selection);
      statusText = "Loading PDF page 1...";
      await renderPdfPage(selection, 1, 1.0);
    } catch (error) {
      statusText = `Open file error: ${formatError(error)}`;
    }
  }
</script>

<main class="page">
  <h1>Clario</h1>
  <button type="button" onclick={openFile}>Open File</button>

  <section class="panel">
    <h2>Selected File</h2>
    <p>{selectedFilePath}</p>
  </section>

  <PdfViewer
    imagePath={renderedImagePath}
    currentPage={currentPage}
    pageCount={pageCount}
    zoom={zoom}
    renderDevicePixelRatio={renderDevicePixelRatio}
    statusText={renderError ?? statusText}
    hasPdfLoaded={currentPdfPath !== null && pageCount > 0}
    isBusy={isRendering}
    onPrev={handlePrevPage}
    onNext={handleNextPage}
    onZoomIn={handleZoomIn}
    onZoomOut={handleZoomOut}
  />

  <section class="panel debug">
    <h2>Debug Panel</h2>
    {#if debugDirs}
      <p><strong>temp_dir:</strong> {debugDirs.temp_dir}</p>
      <p><strong>cache_dir:</strong> {debugDirs.cache_dir}</p>
      <p><strong>log_file:</strong> {debugDirs.log_file}</p>
      <p><strong>ping:</strong> {backendPing}</p>
    {:else}
      <p>Loading paths...</p>
    {/if}
    <p><strong>status:</strong> {statusText}</p>
  </section>
</main>

<style>
  .page {
    min-height: 100vh;
    padding: 2rem 1rem;
    max-width: 900px;
    margin: 0 auto;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  h1 {
    margin: 0 0 1rem;
  }

  button {
    border: 1px solid #2a4f7a;
    background: #2a4f7a;
    color: #fff;
    border-radius: 8px;
    padding: 0.6rem 1rem;
    cursor: pointer;
    font-size: 0.95rem;
  }

  button:hover {
    background: #1f3c5f;
  }

  .panel {
    margin-top: 1rem;
    padding: 0.9rem;
    border: 1px solid #d9d9d9;
    border-radius: 8px;
    background: #fafafa;
  }

  .panel h2 {
    margin: 0 0 0.5rem;
    font-size: 1rem;
  }

  .panel p {
    margin: 0.35rem 0;
    word-break: break-word;
  }

  .debug {
    font-size: 0.9rem;
  }
</style>
