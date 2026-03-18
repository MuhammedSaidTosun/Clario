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
    renderedZoom: number;
  };

  type VisibleBand = {
    startPage: number;
    endPage: number;
  };

  type ScrollTarget = {
    page: number;
    token: number;
  };

  type NavigationTransition = {
    targetPage: number;
    token: number;
    startedAtMs: number;
  };

  type RenderRequestKeyParts = {
    filePath: string;
    page: number;
    zoom: number;
    devicePixelRatio: number;
    generation: number;
  };

  const MIN_ZOOM = 0.5;
  const MAX_ZOOM = 3.0;
  const ZOOM_STEP = 0.25;
  const MIN_DEVICE_PIXEL_RATIO = 1;
  const MAX_DEVICE_PIXEL_RATIO = 4;
  const INITIAL_RENDER_WINDOW_SIZE = 24;
  const LOAD_MORE_WINDOW_SIZE = 6;
  const PREFETCH_TRIGGER_AHEAD_PAGES = 7;
  const PREFETCH_COOLDOWN_MS = 280;
  const PREFETCH_BATCH_SIZE = 3;
  const PREFETCH_FAST_SCROLL_BACKOFF_MS = 700;
  const NAVIGATION_TRANSITION_TIMEOUT_MS = 1500;
  const ZOOM_VISIBLE_BAND_MARGIN = 1;
  const STALE_REFRESH_COOLDOWN_MS = 200;

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
  let lastPrefetchAt = 0;
  let lastActivePageChangeAtMs = 0;
  let fastForwardPrefetchBackoffUntilMs = 0;
  let nonEssentialWorkEpoch = 0;
  let navigationTransition = $state<NavigationTransition | null>(null);
  let renderRequestGeneration = 0;
  let pendingRenderRequests = new Map<string, Promise<PdfPageRenderResponse>>();
  let completedRenderRequests = new Map<string, PdfPageRenderResponse>();
  let isStaleRefreshRunning = false;
  let lastStaleRefreshAtMs = 0;
  let currentVisibleBand = $state<VisibleBand | null>(null);

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
    navigationTransition = null;
    renderError = null;
    lastPrefetchAt = 0;
    lastActivePageChangeAtMs = 0;
    fastForwardPrefetchBackoffUntilMs = 0;
    nonEssentialWorkEpoch = 0;
    currentVisibleBand = null;
    pendingRenderRequests.clear();
    completedRenderRequests.clear();
    isStaleRefreshRunning = false;
    lastStaleRefreshAtMs = 0;
  }

  function normalizeForKey(value: number): number {
    if (!Number.isFinite(value)) {
      return 1;
    }

    return Math.round(value * 100) / 100;
  }

  function renderRequestKey(parts: RenderRequestKeyParts): string {
    return [
      parts.generation,
      parts.filePath,
      parts.page,
      normalizeForKey(parts.zoom),
      normalizeForKey(parts.devicePixelRatio)
    ].join("|");
  }

  function clampPageWithinLoaded(page: number, loadedThrough: number): number {
    return Math.min(loadedThrough, Math.max(1, page));
  }

  function buildPageRange(startPage: number, endPage: number): number[] {
    if (endPage < startPage) {
      return [];
    }

    const pages: number[] = [];

    for (let page = startPage; page <= endPage; page += 1) {
      pages.push(page);
    }

    return pages;
  }

  function visibleBandWindowForLoaded(
    loadedThrough: number,
    fallbackPage: number
  ): { startPage: number; endPage: number } {
    const clampedLoadedThrough = Math.max(1, loadedThrough);
    const fallback = clampPageWithinLoaded(fallbackPage, clampedLoadedThrough);
    const band = currentVisibleBand;

    if (band === null) {
      return {
        startPage: fallback,
        endPage: fallback
      };
    }

    const startPage = clampPageWithinLoaded(Math.min(band.startPage, band.endPage), clampedLoadedThrough);
    const endPage = clampPageWithinLoaded(Math.max(band.startPage, band.endPage), clampedLoadedThrough);

    return {
      startPage,
      endPage
    };
  }

  function beginRenderGeneration(): void {
    renderRequestGeneration += 1;
    pendingRenderRequests.clear();
    completedRenderRequests.clear();
    isStaleRefreshRunning = false;
    lastStaleRefreshAtMs = 0;
  }

  function handleVisibleBandChange(startPage: number, endPage: number): void {
    if (!Number.isFinite(startPage) || !Number.isFinite(endPage)) {
      return;
    }

    const normalizedStart = Math.max(1, Math.floor(startPage));
    const normalizedEnd = Math.max(normalizedStart, Math.floor(endPage));
    const existingBand = currentVisibleBand;

    if (
      existingBand !== null &&
      existingBand.startPage === normalizedStart &&
      existingBand.endPage === normalizedEnd
    ) {
      return;
    }

    currentVisibleBand = {
      startPage: normalizedStart,
      endPage: normalizedEnd
    };

    void maybeRefreshStalePagesInBand(normalizedStart, normalizedEnd);
  }

  function requestViewerScrollToPage(page: number): void {
    scrollTargetToken += 1;

    navigationTransition = {
      targetPage: page,
      token: scrollTargetToken,
      startedAtMs: Date.now()
    };

    scrollTarget = {
      page,
      token: scrollTargetToken
    };
  }

  function maybePrefetchAhead(focusPage: number): void {
    if (!currentPdfPath || isRendering || isStaleRefreshRunning || pageCount < 1) {
      return;
    }

    if (loadedThroughPage >= pageCount) {
      return;
    }

    const remainingLoadedAhead = loadedThroughPage - focusPage;

    if (remainingLoadedAhead > PREFETCH_TRIGGER_AHEAD_PAGES) {
      return;
    }

    const now = Date.now();

    if (now < fastForwardPrefetchBackoffUntilMs && remainingLoadedAhead > 1) {
      return;
    }

    if (now - lastPrefetchAt < PREFETCH_COOLDOWN_MS) {
      return;
    }

    lastPrefetchAt = now;
    void loadNextPageWindow(PREFETCH_BATCH_SIZE, "prefetch", nonEssentialWorkEpoch);
  }

  function handleActivePageChange(page: number): void {
    if (!Number.isFinite(page)) {
      return;
    }

    if (page < 1 || page > pageCount) {
      return;
    }

    const transition = navigationTransition;

    if (transition !== null) {
      if (page === transition.targetPage) {
        navigationTransition = null;
      } else {
        const isTransitionExpired = Date.now() - transition.startedAtMs >= NAVIGATION_TRANSITION_TIMEOUT_MS;

        if (!isTransitionExpired) {
          return;
        }

        navigationTransition = null;
      }
    }

    const movement = page - currentPage;
    const now = Date.now();
    lastActivePageChangeAtMs = now;

    if (movement > 0 && movement >= 2) {
      fastForwardPrefetchBackoffUntilMs = now + PREFETCH_FAST_SCROLL_BACKOFF_MS;
      nonEssentialWorkEpoch += 1;
    }

    if (currentPage !== page) {
      currentPage = page;
    }

    maybePrefetchAhead(page);
  }

  async function renderSinglePdfPage(
    filePath: string,
    pageToRender: number,
    zoomToRender: number,
    devicePixelRatio: number
  ): Promise<PdfPageRenderResponse> {
    const generation = renderRequestGeneration;
    const key = renderRequestKey({
      filePath,
      page: pageToRender,
      zoom: zoomToRender,
      devicePixelRatio,
      generation
    });

    const completed = completedRenderRequests.get(key);

    if (completed) {
      return completed;
    }

    const pending = pendingRenderRequests.get(key);

    if (pending) {
      return pending;
    }

    const request = invoke<PdfPageRenderResponse>("render_pdf_page", {
      filePath,
      page: pageToRender,
      zoom: zoomToRender,
      devicePixelRatio
    })
      .then((response) => {
        if (generation === renderRequestGeneration) {
          completedRenderRequests.set(key, response);
        }

        return response;
      })
      .finally(() => {
        pendingRenderRequests.delete(key);
      });

    pendingRenderRequests.set(key, request);
    return request;
  }

  async function renderNavigationTargetPage(targetPage: number): Promise<boolean> {
    if (!currentPdfPath) {
      return false;
    }

    if (targetPage < 1 || targetPage > pageCount) {
      return false;
    }

    if (targetPage <= loadedThroughPage) {
      return true;
    }

    try {
      statusText = `Preparing target page ${targetPage}...`;
      const response = await renderSinglePdfPage(
        currentPdfPath,
        targetPage,
        zoom,
        renderDevicePixelRatio
      );

      renderedPages = mergeRenderedPages(renderedPages, [
        {
          page: response.page,
          imagePath: response.image_path,
          renderedZoom: response.zoom
        }
      ]);
      pageCount = response.page_count;
      zoom = response.zoom;

      // Navigation target at the current boundary is promoted first.
      if (response.page === loadedThroughPage + 1) {
        loadedThroughPage = response.page;
      }

      return true;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
      return false;
    }
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
        imagePath: response.image_path,
        renderedZoom: response.zoom
      });
    }

    return {
      pages: rangePages,
      totalPages,
      resolvedZoom
    };
  }

  function computeZoomPriorityPages(focusPage: number, loadedThrough: number): number[] {
    const clampedLoadedThrough = Math.max(1, loadedThrough);
    const visibleBand = visibleBandWindowForLoaded(clampedLoadedThrough, focusPage);
    const expandedStart = Math.max(1, visibleBand.startPage - ZOOM_VISIBLE_BAND_MARGIN);
    const expandedEnd = Math.min(clampedLoadedThrough, visibleBand.endPage + ZOOM_VISIBLE_BAND_MARGIN);
    return buildPageRange(expandedStart, expandedEnd);
  }

  async function renderPageList(
    filePath: string,
    pagesToRender: number[],
    zoomToRender: number,
    devicePixelRatio: number,
    stageLabel: string
  ): Promise<{ pages: RenderedPdfPage[]; totalPages: number; resolvedZoom: number }> {
    const nextPages: RenderedPdfPage[] = [];
    let totalPages = pageCount;
    let resolvedZoom = zoomToRender;

    for (const page of pagesToRender) {
      statusText = `${stageLabel} page ${page}${totalPages > 0 ? ` / ${totalPages}` : ""}...`;
      const response = await renderSinglePdfPage(filePath, page, zoomToRender, devicePixelRatio);
      totalPages = response.page_count;
      resolvedZoom = response.zoom;
      nextPages.push({
        page: response.page,
        imagePath: response.image_path,
        renderedZoom: response.zoom
      });
    }

    return {
      pages: nextPages,
      totalPages,
      resolvedZoom
    };
  }

  async function rerenderLoadedPagesForZoom(
    filePath: string,
    zoomToRender: number,
    targetLoadedThrough: number,
    preferredFocusPage: number
  ): Promise<void> {
    isRendering = true;
    renderError = null;
    const devicePixelRatio = getDevicePixelRatio();
    beginRenderGeneration();
    zoom = zoomToRender;
    renderDevicePixelRatio = devicePixelRatio;

    try {
      const nextLoadedThrough = Math.min(pageCount, Math.max(1, targetLoadedThrough));
      const focusPage = Math.min(nextLoadedThrough, Math.max(1, preferredFocusPage));
      const priorityPages = computeZoomPriorityPages(focusPage, nextLoadedThrough);

      const priorityResponse = await renderPageList(
        filePath,
        priorityPages,
        zoomToRender,
        devicePixelRatio,
        "Zooming visible region:"
      );

      renderedPages = mergeRenderedPages(renderedPages, priorityResponse.pages);
      pageCount = priorityResponse.totalPages;
      zoom = priorityResponse.resolvedZoom;
      loadedThroughPage = Math.min(nextLoadedThrough, priorityResponse.totalPages);

      if (currentPage < 1 || currentPage > pageCount) {
        currentPage = Math.min(Math.max(1, currentPage), pageCount);
      }

      statusText = `Loaded pages 1-${loadedThroughPage} of ${pageCount} at ${zoom.toFixed(2)}x (visible region first).`;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isRendering = false;
    }
  }

  function normalizeZoomForComparison(value: number): number {
    if (!Number.isFinite(value)) {
      return 1;
    }

    return Math.round(value * 100) / 100;
  }

  function findStalePagesInBand(bandStart: number, bandEnd: number): number[] {
    const currentZoomNormalized = normalizeZoomForComparison(zoom);
    const stalePages: number[] = [];

    for (let page = bandStart; page <= bandEnd; page += 1) {
      const rendered = renderedPages.find((rp) => rp.page === page);

      if (!rendered) {
        continue;
      }

      if (normalizeZoomForComparison(rendered.renderedZoom) !== currentZoomNormalized) {
        stalePages.push(page);
      }
    }

    return stalePages;
  }

  async function maybeRefreshStalePagesInBand(bandStart: number, bandEnd: number): Promise<void> {
    if (!currentPdfPath || isRendering || isStaleRefreshRunning) {
      return;
    }

    const now = Date.now();

    if (now - lastStaleRefreshAtMs < STALE_REFRESH_COOLDOWN_MS) {
      return;
    }

    const stalePages = findStalePagesInBand(bandStart, bandEnd);

    if (stalePages.length < 1) {
      return;
    }

    const generation = renderRequestGeneration;
    const epochAtStart = nonEssentialWorkEpoch;
    isStaleRefreshRunning = true;
    lastStaleRefreshAtMs = now;

    try {
      const response = await renderPageList(
        currentPdfPath,
        stalePages,
        zoom,
        renderDevicePixelRatio,
        "Refreshing visible pages:"
      );

      if (generation !== renderRequestGeneration || epochAtStart !== nonEssentialWorkEpoch) {
        return;
      }

      renderedPages = mergeRenderedPages(renderedPages, response.pages);
      pageCount = response.totalPages;
      zoom = response.resolvedZoom;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isStaleRefreshRunning = false;
    }
  }

  async function renderPdfDocument(
    filePath: string,
    zoomToRender: number,
    targetLoadedThrough: number = INITIAL_RENDER_WINDOW_SIZE
  ): Promise<void> {
    isRendering = true;
    renderError = null;
    const devicePixelRatio = getDevicePixelRatio();
    beginRenderGeneration();

    try {
      statusText = "Rendering initial pages...";
      const firstResponse = await renderSinglePdfPage(filePath, 1, zoomToRender, devicePixelRatio);
      const totalPages = firstResponse.page_count;
      const nextLoadedThrough = Math.min(totalPages, Math.max(1, targetLoadedThrough));

      const initialPages: RenderedPdfPage[] = [
        {
          page: firstResponse.page,
          imagePath: firstResponse.image_path,
          renderedZoom: firstResponse.zoom
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

  async function loadNextPageWindow(
    requestedWindowSize: number = LOAD_MORE_WINDOW_SIZE,
    source: "prefetch" | "demand" = "demand",
    nonEssentialEpochAtStart: number | null = null
  ): Promise<void> {
    if (!currentPdfPath || isRendering || loadedThroughPage >= pageCount) {
      return;
    }

    isRendering = true;
    renderError = null;

    try {
      const normalizedWindowSize = Math.max(1, Math.min(LOAD_MORE_WINDOW_SIZE, Math.floor(requestedWindowSize)));
      const startPage = loadedThroughPage + 1;
      const endPage = Math.min(pageCount, loadedThroughPage + normalizedWindowSize);
      const rangeResponse = await renderPageRange(
        currentPdfPath,
        startPage,
        endPage,
        zoom,
        renderDevicePixelRatio
      );

      if (
        source === "prefetch" &&
        nonEssentialEpochAtStart !== null &&
        nonEssentialEpochAtStart !== nonEssentialWorkEpoch
      ) {
        return;
      }

      renderedPages = mergeRenderedPages(renderedPages, rangeResponse.pages);
      loadedThroughPage = endPage;
      pageCount = rangeResponse.totalPages;
      zoom = rangeResponse.resolvedZoom;
      statusText =
        source === "prefetch"
          ? `Prefetched pages ${startPage}-${endPage} of ${pageCount}.`
          : `Loaded pages ${startPage}-${endPage} of ${pageCount}.`;
    } catch (error) {
      const message = `PDF render error: ${formatError(error)}`;
      renderError = message;
      statusText = message;
    } finally {
      isRendering = false;
    }
  }

  function handleLazyLoadNext(): void {
    maybePrefetchAhead(Math.max(1, currentPage));
  }

  async function ensurePageLoaded(targetPage: number, prioritizeNavigationTarget: boolean = false): Promise<boolean> {
    if (prioritizeNavigationTarget && targetPage === loadedThroughPage + 1) {
      const prioritized = await renderNavigationTargetPage(targetPage);

      if (!prioritized) {
        return false;
      }
    }

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
    const loaded = await ensurePageLoaded(targetPage, true);

    if (!loaded) {
      statusText = `Failed to prepare page ${targetPage}.`;
      return;
    }

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
    const loaded = await ensurePageLoaded(targetPage, true);

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

    const focusPage = currentPage;
    const nextZoom = Math.min(MAX_ZOOM, Number((zoom + ZOOM_STEP).toFixed(2)));
    if (nextZoom === zoom) {
      statusText = `Maximum zoom is ${MAX_ZOOM.toFixed(2)}x.`;
      return;
    }

    await rerenderLoadedPagesForZoom(currentPdfPath, nextZoom, loadedThroughPage, focusPage);
  }

  async function handleZoomOut(): Promise<void> {
    if (!currentPdfPath || isRendering) {
      return;
    }

    const focusPage = currentPage;
    const nextZoom = Math.max(MIN_ZOOM, Number((zoom - ZOOM_STEP).toFixed(2)));
    if (nextZoom === zoom) {
      statusText = `Minimum zoom is ${MIN_ZOOM.toFixed(2)}x.`;
      return;
    }

    await rerenderLoadedPagesForZoom(currentPdfPath, nextZoom, loadedThroughPage, focusPage);
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
    onLazyLoadNext={handleLazyLoadNext}
    onActivePageChange={handleActivePageChange}
    onVisibleBandChange={handleVisibleBandChange}
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
