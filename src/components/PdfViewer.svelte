<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";

  type FitMode = "manual" | "width" | "page";
  type RenderedPdfPage = {
    page: number;
    imagePath: string;
  };
  type ScrollTarget = {
    page: number;
    token: number;
  };
  type ReadingAnchor = {
    page: number;
    offsetRatio: number;
  };

  type Props = {
    renderedPages: RenderedPdfPage[];
    currentPage: number;
    scrollTarget: ScrollTarget | null;
    pageCount: number;
    loadedThroughPage: number;
    zoom: number;
    renderDevicePixelRatio: number;
    statusText: string;
    hasPdfLoaded: boolean;
    hasMorePages: boolean;
    isBusy: boolean;
    onPrev: () => void;
    onNext: () => void;
    onZoomIn: () => void;
    onZoomOut: () => void;
    onLoadMore: () => void;
    onActivePageChange: (page: number) => void;
  };

  let {
    renderedPages,
    currentPage,
    scrollTarget,
    pageCount,
    loadedThroughPage,
    zoom,
    renderDevicePixelRatio,
    statusText,
    hasPdfLoaded,
    hasMorePages,
    isBusy,
    onPrev,
    onNext,
    onZoomIn,
    onZoomOut,
    onLoadMore,
    onActivePageChange
  }: Props = $props();

  const VIEWPORT_PADDING = 24;

  let fitMode = $state<FitMode>("manual");
  let logicalPageSizeByNumber = $state<Record<number, { width: number; height: number }>>({});
  let viewportElement = $state<HTMLDivElement | null>(null);
  let viewportWidth = $state(0);
  let viewportHeight = $state(0);
  let pendingReadingAnchor = $state<ReadingAnchor | null>(null);
  let handledScrollTargetToken = $state(0);
  let pageSizeSessionFirstPath = $state<string | null>(null);
  let pendingViewportScanRaf = $state<number | null>(null);
  let activeVisiblePage = $state<number | null>(null);

  $effect(() => {
    const firstPath = renderedPages[0]?.imagePath ?? null;

    if (firstPath !== pageSizeSessionFirstPath) {
      logicalPageSizeByNumber = {};
      pageSizeSessionFirstPath = firstPath;
    }
  });

  $effect(() => {
    const element = viewportElement;

    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }

    const updateViewportSize = (): void => {
      viewportWidth = element.clientWidth;
      viewportHeight = element.clientHeight;
    };

    updateViewportSize();

    const observer = new ResizeObserver(() => {
      updateViewportSize();
    });

    observer.observe(element);

    return () => {
      observer.disconnect();
    };
  });

  function handleImageLoad(pageNumber: number, event: Event): void {
    if (!(event.currentTarget instanceof HTMLImageElement)) {
      return;
    }

    const safeDevicePixelRatio = renderDevicePixelRatio > 0 ? renderDevicePixelRatio : 1;
    logicalPageSizeByNumber = {
      ...logicalPageSizeByNumber,
      [pageNumber]: {
        width: Math.max(1, Math.round(event.currentTarget.naturalWidth / safeDevicePixelRatio)),
        height: Math.max(1, Math.round(event.currentTarget.naturalHeight / safeDevicePixelRatio))
      }
    };
  }

  function determineVisiblePage(): number | null {
    const viewport = viewportElement;

    if (!viewport) {
      return null;
    }

    const viewportRect = viewport.getBoundingClientRect();
    const viewportCenterY = viewportRect.top + viewportRect.height * 0.35;
    const pageElements = Array.from(viewport.querySelectorAll<HTMLElement>("[data-page]"));

    let bestPage: number | null = null;
    let bestDistance = Number.POSITIVE_INFINITY;

    for (const pageElement of pageElements) {
      const pageNumber = Number(pageElement.dataset.page);

      if (!Number.isFinite(pageNumber)) {
        continue;
      }

      const pageRect = pageElement.getBoundingClientRect();
      const isVisible = pageRect.bottom > viewportRect.top && pageRect.top < viewportRect.bottom;

      if (!isVisible) {
        continue;
      }

      const distance = Math.abs(pageRect.top - viewportCenterY);

      if (distance < bestDistance) {
        bestDistance = distance;
        bestPage = pageNumber;
      }
    }

    return bestPage;
  }

  function reportActivePage(page: number): void {
    if (!Number.isFinite(page)) {
      return;
    }

    if (!renderedPages.some((renderedPage) => renderedPage.page === page)) {
      return;
    }

    if (activeVisiblePage === page) {
      return;
    }

    activeVisiblePage = page;
    onActivePageChange(page);
  }

  function scheduleVisiblePageScan(): void {
    if (pendingViewportScanRaf !== null) {
      return;
    }

    pendingViewportScanRaf = requestAnimationFrame(() => {
      pendingViewportScanRaf = null;
      const visiblePage = determineVisiblePage();

      if (visiblePage !== null) {
        reportActivePage(visiblePage);
      }
    });
  }

  function handleViewportScroll(): void {
    const visiblePage = determineVisiblePage();
    if (visiblePage !== null) {
      reportActivePage(visiblePage);
    }
    scheduleVisiblePageScan();
  }

  function computeDisplayWidthForPage(
    pageNumber: number,
    mode: FitMode,
    viewportClientWidth: number,
    viewportClientHeight: number
  ): number | null {
    const pageSize = logicalPageSizeByNumber[pageNumber];
    if (!pageSize) {
      return null;
    }

    const { width, height } = pageSize;

    if (width === null || height === null) {
      return null;
    }

    if (mode === "manual") {
      return width;
    }

    const availableWidth = Math.max(1, viewportClientWidth - VIEWPORT_PADDING);
    const availableHeight = Math.max(1, viewportClientHeight - VIEWPORT_PADDING);

    if (mode === "width") {
      return availableWidth;
    }

    const widthScale = availableWidth / width;
    const heightScale = availableHeight / height;
    const fitScale = Math.min(widthScale, heightScale);

    return Math.max(1, Math.round(width * fitScale));
  }

  function imageStyleForPage(pageNumber: number): string {
    const displayWidth = computeDisplayWidthForPage(
      pageNumber,
      fitMode,
      viewportWidth,
      viewportHeight
    );

    return displayWidth === null ? "" : `width: ${displayWidth}px;`;
  }

  function setFitToWidth(): void {
    pendingReadingAnchor = captureReadingAnchor();
    fitMode = "width";
  }

  function setFitToPage(): void {
    pendingReadingAnchor = captureReadingAnchor();
    fitMode = "page";
  }

  function handleZoomInClick(): void {
    pendingReadingAnchor = captureReadingAnchor();
    fitMode = "manual";
    onZoomIn();
  }

  function handleZoomOutClick(): void {
    pendingReadingAnchor = captureReadingAnchor();
    fitMode = "manual";
    onZoomOut();
  }

  function captureReadingAnchor(): ReadingAnchor | null {
    const viewport = viewportElement;

    if (!viewport) {
      return null;
    }

    const viewportRect = viewport.getBoundingClientRect();
    const pageElements = Array.from(viewport.querySelectorAll<HTMLElement>("[data-page]"));

    for (const pageElement of pageElements) {
      const pageRect = pageElement.getBoundingClientRect();

      if (pageRect.bottom <= viewportRect.top) {
        continue;
      }

      const pageNumber = Number(pageElement.dataset.page);

      if (!Number.isFinite(pageNumber)) {
        continue;
      }

      const pageHeight = Math.max(1, pageRect.height);
      const offsetPx = Math.max(0, viewportRect.top - pageRect.top);
      const offsetRatio = Math.min(1, offsetPx / pageHeight);

      return {
        page: pageNumber,
        offsetRatio
      };
    }

    return null;
  }

  function restoreReadingAnchor(anchor: ReadingAnchor, behavior: ScrollBehavior): boolean {
    const viewport = viewportElement;

    if (!viewport) {
      return false;
    }

    const target = viewport.querySelector<HTMLElement>(`[data-page="${anchor.page}"]`);

    if (!target) {
      return false;
    }

    const viewportRect = viewport.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();

    if (targetRect.height < 1) {
      return false;
    }

    const absoluteTargetTop = viewport.scrollTop + (targetRect.top - viewportRect.top);
    const targetOffset = anchor.offsetRatio * targetRect.height;
    const nextScrollTop = Math.max(0, absoluteTargetTop + targetOffset);

    viewport.scrollTo({
      top: nextScrollTop,
      behavior
    });

    return true;
  }

  $effect(() => {
    pendingReadingAnchor;
    renderedPages;
    logicalPageSizeByNumber;
    fitMode;
    const anchor = pendingReadingAnchor;

    if (!anchor) {
      return;
    }

    requestAnimationFrame(() => {
      const restored = restoreReadingAnchor(anchor, "auto");

      if (restored) {
        reportActivePage(anchor.page);
      }

      if (restored && pendingReadingAnchor === anchor) {
        pendingReadingAnchor = null;
      }
    });
  });

  $effect(() => {
    renderedPages;
    scheduleVisiblePageScan();
  });

  $effect(() => {
    scrollTarget;
    renderedPages;
    const viewport = viewportElement;
    const target = scrollTarget;

    if (!viewport || !target) {
      return;
    }

    if (target.token <= handledScrollTargetToken) {
      return;
    }

    const hasTargetPage = renderedPages.some((page) => page.page === target.page);

    if (!hasTargetPage) {
      return;
    }

    requestAnimationFrame(() => {
      const scrolled = restoreReadingAnchor(
        {
          page: target.page,
          offsetRatio: 0
        },
        "smooth"
      );

      if (scrolled && handledScrollTargetToken < target.token) {
        reportActivePage(target.page);
        handledScrollTargetToken = target.token;
      }
    });
  });
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
    <button type="button" onclick={handleZoomOutClick} disabled={!hasPdfLoaded || isBusy}>Zoom out</button>
    <button type="button" onclick={handleZoomInClick} disabled={!hasPdfLoaded || isBusy}>Zoom in</button>
    <button
      type="button"
      onclick={setFitToWidth}
      class:active={fitMode === "width"}
      aria-pressed={fitMode === "width"}
      disabled={!hasPdfLoaded}
    >
      Fit to width
    </button>
    <button
      type="button"
      onclick={setFitToPage}
      class:active={fitMode === "page"}
      aria-pressed={fitMode === "page"}
      disabled={!hasPdfLoaded}
    >
      Fit to page
    </button>
  </div>

  <p><strong>Page:</strong> {currentPage} / {pageCount}</p>
  <p><strong>Zoom:</strong> {zoom.toFixed(2)}x</p>
  <p><strong>Loaded pages:</strong> 1 - {loadedThroughPage}</p>

  {#if renderedPages.length > 0}
    <div class="viewport" bind:this={viewportElement} onscroll={handleViewportScroll}>
      <div class="pages-stack">
        {#each renderedPages as renderedPage (renderedPage.page)}
          <div class="page-stage" data-page={renderedPage.page}>
            <div class="page-surface">
              <img
                src={convertFileSrc(renderedPage.imagePath)}
                alt={"Rendered PDF page " + renderedPage.page}
                onload={(event) => handleImageLoad(renderedPage.page, event)}
                style={imageStyleForPage(renderedPage.page)}
              />
            </div>
          </div>
        {/each}
      </div>
    </div>

    {#if hasMorePages}
      <div class="load-more-row">
        <button type="button" onclick={onLoadMore} disabled={isBusy}>Load more pages</button>
      </div>
    {/if}
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

  .controls button.active {
    background: #1f3c5f;
  }

  .viewport {
    margin-top: 0.8rem;
    border: 1px solid #d9d9d9;
    background: #e9edf3;
    border-radius: 8px;
    overflow: auto;
    height: min(76vh, 900px);
  }

  .pages-stack {
    min-height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding: 0.75rem;
    box-sizing: border-box;
  }

  .page-stage {
    width: 100%;
    display: flex;
    justify-content: center;
  }

  .page-surface {
    background: #fff;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.16);
    border: 1px solid #dadde3;
  }

  .page-surface img {
    display: block;
    max-width: none;
    height: auto;
  }

  .load-more-row {
    margin-top: 0.75rem;
    display: flex;
    justify-content: center;
  }
</style>
