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

  type RenderedPdfPage = {
    page: number;
    imagePath: string;
  };

  type ScrollTarget = {
    page: number;
    token: number;
  };

  const MIN_ZOOM = 0.5;
  const MAX_ZOOM = 3.0;
  const ZOOM_STEP = 0.25;
  const MIN_DEVICE_PIXEL_RATIO = 1;
  const MAX_DEVICE_PIXEL_RATIO = 4;
  const INITIAL_RENDER_WINDOW_SIZE = 24;
  const LOAD_MORE_WINDOW_SIZE = 8;

  let selectedFilePath = $state("No file selected.");
  let debugDirs = $state<AppDirs | null>(null);
  let statusText = $state("Starting...");
  let backendPing = $state("");
  let currentPdfPath = $state<string | null>(null);
  let currentPage = $state(1);
  let pageCount = $state(0);
  let zoom = $state(1.0);
  let renderDevicePixelRatio = $state(1.0);
  let renderedPages = $state<RenderedPdfPage[]>([]);
  let loadedThroughPage = $state(0);
  let scrollTarget = $state<ScrollTarget | null>(null);
  let scrollTargetToken = 0;
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
    renderedPages = [];
    loadedThroughPage = 0;
    scrollTarget = null;
    renderError = null;
  }

  function requestViewerScrollToPage(page: number): void {
    scrollTargetToken += 1;
    scrollTarget = {
      page,
      token: scrollTargetToken
    };
  }

  function handleActivePageChange(page: number): void {
    if (!Number.isFinite(page)) {
      return;
    }

    if (page < 1 || page > pageCount) {
      return;
    }

    if (currentPage !== page) {
      currentPage = page;
    }
  }

  async function renderSinglePdfPage(
    filePath: string,
    pageToRender: number,
    zoomToRender: number,
    devicePixelRatio: number
  ): Promise<PdfPageRenderResponse> {
    return invoke<PdfPageRenderResponse>("render_pdf_page", {
      filePath,
      page: pageToRender,
      zoom: zoomToRender,
      devicePixelRatio
    });
  }

  function mergeRenderedPages(existingPages: RenderedPdfPage[], incomingPages: RenderedPdfPage[]): RenderedPdfPage[] {
    const byPage = new Map<number, RenderedPdfPage>();

    for (const renderedPage of existingPages) {
      byPage.set(renderedPage.page, renderedPage);
    }

    for (const renderedPage of incomingPages) {
      byPage.set(renderedPage.page, renderedPage);
    }

    return Array.from(byPage.values()).sort((left, right) => left.page - right.page);
  }

  async function renderPageRange(
    filePath: string,
    startPage: number,
    endPage: number,
    zoomToRender: number,
    devicePixelRatio: number
  ): Promise<{ pages: RenderedPdfPage[]; totalPages: number; resolvedZoom: number }> {
    const rangePages: RenderedPdfPage[] = [];
    let totalPages = pageCount;
    let resolvedZoom = zoomToRender;

    for (let page = startPage; page <= endPage; page += 1) {
      statusText = `Rendering page ${page}${totalPages > 0 ? ` / ${totalPages}` : ""}...`;
      const response = await renderSinglePdfPage(filePath, page, zoomToRender, devicePixelRatio);

      totalPages = response.page_count;
      resolvedZoom = response.zoom;
      rangePages.push({
        page: response.page,
        imagePath: response.image_path
      });
    }

    return {
      pages: rangePages,
      totalPages,
      resolvedZoom
    };
  }

  async function renderPdfDocument(
    filePath: string,
    zoomToRender: number,
    targetLoadedThrough: number = INITIAL_RENDER_WINDOW_SIZE
  ): Promise<void> {
    isRendering = true;
    renderError = null;
    const devicePixelRatio = getDevicePixelRatio();

    try {
      statusText = "Rendering initial pages...";
      const firstResponse = await renderSinglePdfPage(filePath, 1, zoomToRender, devicePixelRatio);
      const totalPages = firstResponse.page_count;
      const nextLoadedThrough = Math.min(totalPages, Math.max(1, targetLoadedThrough));

      const initialPages: RenderedPdfPage[] = [
        {
          page: firstResponse.page,
          imagePath: firstResponse.image_path
        }
      ];

      if (nextLoadedThrough > 1) {
        const rangeResponse = await renderPageRange(
          filePath,
          2,
          nextLoadedThrough,
          firstResponse.zoom,
          devicePixelRatio
        );
        initialPages.push(...rangeResponse.pages);
      }

      currentPdfPath = filePath;
      pageCount = totalPages;
      zoom = firstResponse.zoom;
      renderDevicePixelRatio = devicePixelRatio;
      renderedPages = initialPages;
      loadedThroughPage = nextLoadedThrough;

      if (currentPage < 1 || currentPage > totalPages) {
        currentPage = 1;
      }

      statusText = `Loaded pages 1-${nextLoadedThrough} of ${totalPages} at ${firstResponse.zoom.toFixed(2)}x`;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isRendering = false;
    }
  }

  async function loadNextPageWindow(): Promise<void> {
    if (!currentPdfPath || isRendering || loadedThroughPage >= pageCount) {
      return;
    }

    isRendering = true;
    renderError = null;

    try {
      const startPage = loadedThroughPage + 1;
      const endPage = Math.min(pageCount, loadedThroughPage + LOAD_MORE_WINDOW_SIZE);
      const rangeResponse = await renderPageRange(
        currentPdfPath,
        startPage,
        endPage,
        zoom,
        renderDevicePixelRatio
      );

      renderedPages = mergeRenderedPages(renderedPages, rangeResponse.pages);
      loadedThroughPage = endPage;
      pageCount = rangeResponse.totalPages;
      zoom = rangeResponse.resolvedZoom;
      statusText = `Loaded pages ${startPage}-${endPage} of ${pageCount}.`;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isRendering = false;
    }
  }

  async function ensurePageLoaded(targetPage: number): Promise<boolean> {
    if (targetPage <= loadedThroughPage) {
      return true;
    }

    while (loadedThroughPage < targetPage) {
      await loadNextPageWindow();

      if (renderError !== null || loadedThroughPage < 1) {
        return false;
      }
    }

    return true;
  }

  async function handlePrevPage(): Promise<void> {
    if (!currentPdfPath || renderedPages.length === 0 || isRendering) {
      return;
    }

    const basePage = currentPage;
    if (basePage <= 1) {
      statusText = "Already at page 1.";
      return;
    }

    const targetPage = basePage - 1;
    currentPage = targetPage;
    requestViewerScrollToPage(targetPage);
    statusText = `Jumped to page ${targetPage}.`;
  }

  async function handleNextPage(): Promise<void> {
    if (!currentPdfPath || renderedPages.length === 0 || isRendering) {
      return;
    }

    const basePage = currentPage;

    if (basePage >= pageCount) {
      statusText = "Already at the last page.";
      return;
    }

    const targetPage = basePage + 1;
    const loaded = await ensurePageLoaded(targetPage);

    if (!loaded) {
      return;
    }

    currentPage = targetPage;
    requestViewerScrollToPage(targetPage);
    statusText = `Jumped to page ${targetPage}.`;
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

    await renderPdfDocument(currentPdfPath, nextZoom, loadedThroughPage);
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

    await renderPdfDocument(currentPdfPath, nextZoom, loadedThroughPage);
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
      statusText = "Loading document...";
      await renderPdfDocument(selection, 1.0);
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
    renderedPages={renderedPages}
    currentPage={currentPage}
    scrollTarget={scrollTarget}
    pageCount={pageCount}
    loadedThroughPage={loadedThroughPage}
    zoom={zoom}
    renderDevicePixelRatio={renderDevicePixelRatio}
    statusText={renderError ?? statusText}
    hasPdfLoaded={currentPdfPath !== null && pageCount > 0 && renderedPages.length > 0}
    hasMorePages={currentPdfPath !== null && loadedThroughPage < pageCount}
    isBusy={isRendering}
    onPrev={handlePrevPage}
    onNext={handleNextPage}
    onZoomIn={handleZoomIn}
    onZoomOut={handleZoomOut}
    onLoadMore={loadNextPageWindow}
    onActivePageChange={handleActivePageChange}
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
