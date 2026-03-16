<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onDestroy } from "svelte";

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
    onLazyLoadNext: () => void;
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
    onLazyLoadNext,
    onActivePageChange
  }: Props = $props();

  const VIEWPORT_PADDING = 24;
  const NEAR_END_SCROLL_THRESHOLD_PX = 480;
  const PAGE_STACK_VERTICAL_PADDING_PX = 12;
  const PAGE_STAGE_GAP_PX = 16;
  const DEFAULT_LOGICAL_PAGE_WIDTH = 800;
  const DEFAULT_LOGICAL_PAGE_HEIGHT = 1131;
  const VIRTUAL_VISIBLE_EXTRA_BEFORE = 1;
  const VIRTUAL_VISIBLE_EXTRA_AFTER = 1;
  const VIRTUAL_MOUNT_HARD_CAP = 8;
  const PINCH_WHEEL_DELTA_STEP = 90;
  const PINCH_FLUSH_DELAY_MS = 70;
  const PINCH_APPLY_COOLDOWN_MS = 140;

  let fitMode = $state<FitMode>("manual");
  let logicalPageSizeByNumber = $state<Record<number, { width: number; height: number }>>({});
  let viewportElement = $state<HTMLDivElement | null>(null);
  let viewportWidth = $state(0);
  let viewportHeight = $state(0);
  let pendingReadingAnchor = $state<ReadingAnchor | null>(null);
  let handledScrollTargetToken = $state(0);
  let pageSizeSessionFirstPath = $state<string | null>(null);
  let pendingViewportScanRaf = $state<number | null>(null);
  let pendingVirtualWindowRaf = $state<number | null>(null);
  let activeVisiblePage = $state<number | null>(null);
  let virtualPageStart = $state(1);
  let virtualPageEnd = $state(0);
  let virtualTopSpacerPx = $state(0);
  let virtualBottomSpacerPx = $state(0);
  let virtualMountedPages = $state<number[]>([]);
  let estimatedPageTopByNumber = $state<Record<number, number>>({});
  let estimatedDisplayHeightByNumber = $state<Record<number, number>>({});
  let pinchDeltaAccumulator = 0;
  let pendingPinchZoomDirection: "in" | "out" | null = null;
  let pinchFlushTimeoutId: number | null = null;
  let lastPinchZoomAppliedAtMs = 0;

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
    scheduleVirtualWindowUpdate();
  }

  function averageLogicalPageSize(): { width: number; height: number } {
    const sizes = Object.values(logicalPageSizeByNumber);

    if (sizes.length < 1) {
      return {
        width: DEFAULT_LOGICAL_PAGE_WIDTH,
        height: DEFAULT_LOGICAL_PAGE_HEIGHT
      };
    }

    let totalWidth = 0;
    let totalHeight = 0;

    for (const size of sizes) {
      totalWidth += size.width;
      totalHeight += size.height;
    }

    return {
      width: Math.max(1, Math.round(totalWidth / sizes.length)),
      height: Math.max(1, Math.round(totalHeight / sizes.length))
    };
  }

  function computeDisplayWidthForLogicalPage(
    logicalWidth: number,
    logicalHeight: number,
    mode: FitMode,
    viewportClientWidth: number,
    viewportClientHeight: number
  ): number {
    const safeLogicalWidth = Math.max(1, logicalWidth);
    const safeLogicalHeight = Math.max(1, logicalHeight);

    if (mode === "manual") {
      return safeLogicalWidth;
    }

    const availableWidth = Math.max(1, viewportClientWidth - VIEWPORT_PADDING);
    const availableHeight = Math.max(1, viewportClientHeight - VIEWPORT_PADDING);

    if (mode === "width") {
      return availableWidth;
    }

    const widthScale = availableWidth / safeLogicalWidth;
    const heightScale = availableHeight / safeLogicalHeight;
    const fitScale = Math.min(widthScale, heightScale);

    return Math.max(1, Math.round(safeLogicalWidth * fitScale));
  }

  function estimateDisplayHeightForPage(
    pageNumber: number,
    mode: FitMode,
    viewportClientWidth: number,
    viewportClientHeight: number,
    fallbackLogicalSize: { width: number; height: number }
  ): number {
    const pageSize = logicalPageSizeByNumber[pageNumber] ?? fallbackLogicalSize;
    const displayWidth = computeDisplayWidthForLogicalPage(
      pageSize.width,
      pageSize.height,
      mode,
      viewportClientWidth,
      viewportClientHeight
    );
    const scale = displayWidth / Math.max(1, pageSize.width);
    return Math.max(1, Math.round(pageSize.height * scale));
  }

  function recomputeVirtualWindow(): void {
    const totalLoadedPages = Math.max(0, loadedThroughPage);

    if (totalLoadedPages < 1) {
      virtualPageStart = 1;
      virtualPageEnd = 0;
      virtualTopSpacerPx = 0;
      virtualBottomSpacerPx = 0;
      virtualMountedPages = [];
      estimatedPageTopByNumber = {};
      estimatedDisplayHeightByNumber = {};
      return;
    }

    const viewport = viewportElement;
    const viewportClientWidth = Math.max(1, viewport?.clientWidth ?? viewportWidth ?? 1);
    const viewportClientHeight = Math.max(1, viewport?.clientHeight ?? viewportHeight ?? 1);
    const fallbackLogicalSize = averageLogicalPageSize();
    const prefixBlockHeights: number[] = [0];
    const pageTopByNumber: Record<number, number> = {};
    const displayHeightByNumber: Record<number, number> = {};

    for (let page = 1; page <= totalLoadedPages; page += 1) {
      const displayHeight = estimateDisplayHeightForPage(
        page,
        fitMode,
        viewportClientWidth,
        viewportClientHeight,
        fallbackLogicalSize
      );
      const blockHeight = displayHeight + PAGE_STAGE_GAP_PX;
      displayHeightByNumber[page] = displayHeight;
      pageTopByNumber[page] = PAGE_STACK_VERTICAL_PADDING_PX + prefixBlockHeights[page - 1];
      prefixBlockHeights[page] = prefixBlockHeights[page - 1] + blockHeight;
    }

    estimatedPageTopByNumber = pageTopByNumber;
    estimatedDisplayHeightByNumber = displayHeightByNumber;

    const scrollTop = viewport?.scrollTop ?? 0;
    const viewportBottom = scrollTop + (viewport?.clientHeight ?? viewportClientHeight);
    let visibleStart = totalLoadedPages;
    let visibleEnd = totalLoadedPages;
    let foundVisibleStart = false;
    let foundVisibleEnd = false;

    for (let page = 1; page <= totalLoadedPages; page += 1) {
      const pageTop = pageTopByNumber[page];
      const pageBottom = pageTop + displayHeightByNumber[page] + PAGE_STAGE_GAP_PX;

      if (!foundVisibleStart && pageBottom >= scrollTop) {
        visibleStart = page;
        foundVisibleStart = true;
      }

      if (pageTop <= viewportBottom) {
        visibleEnd = page;
        foundVisibleEnd = true;
      } else if (foundVisibleEnd) {
        break;
      }
    }

    if (!foundVisibleStart) {
      visibleStart = totalLoadedPages;
      visibleEnd = totalLoadedPages;
    }

    if (visibleEnd < visibleStart) {
      visibleEnd = visibleStart;
    }

    const visibleCount = visibleEnd - visibleStart + 1;
    const maxMountedPages = Math.max(VIRTUAL_MOUNT_HARD_CAP, visibleCount);
    let mountedStart = Math.max(1, visibleStart - VIRTUAL_VISIBLE_EXTRA_BEFORE);
    let mountedEnd = Math.min(totalLoadedPages, visibleEnd + VIRTUAL_VISIBLE_EXTRA_AFTER);

    if (mountedEnd - mountedStart + 1 > maxMountedPages) {
      mountedStart = visibleStart;
      mountedEnd = visibleEnd;
    }

    while (mountedEnd - mountedStart + 1 < maxMountedPages && (mountedStart > 1 || mountedEnd < totalLoadedPages)) {
      if (mountedEnd < totalLoadedPages) {
        mountedEnd += 1;
      }

      if (mountedEnd - mountedStart + 1 >= maxMountedPages) {
        break;
      }

      if (mountedStart > 1) {
        mountedStart -= 1;
      }
    }

    const nextMountedPages: number[] = [];

    for (let page = mountedStart; page <= mountedEnd; page += 1) {
      nextMountedPages.push(page);
    }

    const nextTopSpacerPx = Math.max(0, prefixBlockHeights[mountedStart - 1]);
    const nextBottomSpacerPx = Math.max(0, prefixBlockHeights[totalLoadedPages] - prefixBlockHeights[mountedEnd]);
    const didWindowChange = mountedStart !== virtualPageStart || mountedEnd !== virtualPageEnd;

    virtualPageStart = mountedStart;
    virtualPageEnd = mountedEnd;
    virtualTopSpacerPx = nextTopSpacerPx;
    virtualBottomSpacerPx = nextBottomSpacerPx;
    virtualMountedPages = nextMountedPages;

    if (didWindowChange) {
      scheduleVisiblePageScan();
    }
  }

  function scheduleVirtualWindowUpdate(): void {
    if (pendingVirtualWindowRaf !== null) {
      return;
    }

    pendingVirtualWindowRaf = requestAnimationFrame(() => {
      pendingVirtualWindowRaf = null;
      recomputeVirtualWindow();
    });
  }

  function renderedPageByNumber(pageNumber: number): RenderedPdfPage | null {
    for (const renderedPage of renderedPages) {
      if (renderedPage.page === pageNumber) {
        return renderedPage;
      }
    }

    return null;
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

    if (page < 1 || page > loadedThroughPage) {
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
    scheduleVirtualWindowUpdate();
    const visiblePage = determineVisiblePage();
    if (visiblePage !== null) {
      reportActivePage(visiblePage);
    }
    scheduleVisiblePageScan();
    maybeRequestLazyLoad();
  }

  function schedulePinchFlush(delayMs: number): void {
    if (pinchFlushTimeoutId !== null) {
      return;
    }

    pinchFlushTimeoutId = window.setTimeout(() => {
      pinchFlushTimeoutId = null;
      flushPendingPinchZoom();
    }, Math.max(PINCH_FLUSH_DELAY_MS, delayMs));
  }

  function queuePinchZoom(direction: "in" | "out"): void {
    pendingPinchZoomDirection = direction;
    schedulePinchFlush(PINCH_FLUSH_DELAY_MS);
  }

  function flushPendingPinchZoom(): void {
    if (pendingPinchZoomDirection === null) {
      return;
    }

    const now = Date.now();
    const elapsedMs = now - lastPinchZoomAppliedAtMs;

    if (isBusy) {
      schedulePinchFlush(PINCH_FLUSH_DELAY_MS);
      return;
    }

    if (elapsedMs < PINCH_APPLY_COOLDOWN_MS) {
      schedulePinchFlush(PINCH_APPLY_COOLDOWN_MS - elapsedMs);
      return;
    }

    const direction = pendingPinchZoomDirection;
    pendingPinchZoomDirection = null;
    lastPinchZoomAppliedAtMs = now;

    if (direction === "in") {
      handleZoomInClick();
      return;
    }

    handleZoomOutClick();
  }

  function handleViewportWheel(event: WheelEvent): void {
    if (!hasPdfLoaded || event.deltaY === 0) {
      return;
    }

    const isTrackpadPinch = event.ctrlKey;

    if (!isTrackpadPinch) {
      return;
    }

    event.preventDefault();
    pinchDeltaAccumulator += event.deltaY;

    if (Math.abs(pinchDeltaAccumulator) < PINCH_WHEEL_DELTA_STEP) {
      return;
    }

    const direction: "in" | "out" = pinchDeltaAccumulator < 0 ? "in" : "out";
    pinchDeltaAccumulator = 0;
    queuePinchZoom(direction);
  }

  function maybeRequestLazyLoad(): void {
    const viewport = viewportElement;

    if (!viewport || !hasMorePages || isBusy) {
      return;
    }

    const remainingScrollPx = viewport.scrollHeight - (viewport.scrollTop + viewport.clientHeight);

    if (remainingScrollPx <= NEAR_END_SCROLL_THRESHOLD_PX) {
      onLazyLoadNext();
    }
  }

  function computeDisplayWidthForPage(
    pageNumber: number,
    mode: FitMode,
    viewportClientWidth: number,
    viewportClientHeight: number
  ): number {
    const fallbackLogicalSize = averageLogicalPageSize();
    const pageSize = logicalPageSizeByNumber[pageNumber] ?? fallbackLogicalSize;

    return computeDisplayWidthForLogicalPage(
      pageSize.width,
      pageSize.height,
      mode,
      viewportClientWidth,
      viewportClientHeight
    );
  }

  function imageStyleForPage(pageNumber: number): string {
    const displayWidth = computeDisplayWidthForPage(
      pageNumber,
      fitMode,
      viewportWidth,
      viewportHeight
    );

    return `width: ${displayWidth}px;`;
  }

  function placeholderStyleForPage(pageNumber: number): string {
    const displayWidth = computeDisplayWidthForPage(pageNumber, fitMode, viewportWidth, viewportHeight);
    const displayHeight = Math.max(1, estimatedDisplayHeightByNumber[pageNumber] ?? DEFAULT_LOGICAL_PAGE_HEIGHT);
    return `width: ${displayWidth}px; height: ${displayHeight}px;`;
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
    const preferredPage = determineVisiblePage();
    let targetElement: HTMLElement | null = null;

    if (preferredPage !== null) {
      targetElement =
        pageElements.find((pageElement) => Number(pageElement.dataset.page) === preferredPage) ?? null;
    }

    if (!targetElement) {
      for (const pageElement of pageElements) {
        const pageRect = pageElement.getBoundingClientRect();

        if (pageRect.bottom <= viewportRect.top) {
          continue;
        }

        const pageNumber = Number(pageElement.dataset.page);

        if (!Number.isFinite(pageNumber)) {
          continue;
        }

        targetElement = pageElement;
        break;
      }
    }

    if (!targetElement) {
      return null;
    }

    const pageNumber = Number(targetElement.dataset.page);

    if (!Number.isFinite(pageNumber)) {
      return null;
    }

    const pageRect = targetElement.getBoundingClientRect();
    const pageHeight = Math.max(1, pageRect.height);
    const readingLineY = viewportRect.top + viewportRect.height * 0.35;
    const rawOffsetPx = readingLineY - pageRect.top;
    const offsetPx = Math.min(pageHeight, Math.max(0, rawOffsetPx));
    const offsetRatio = Math.min(1, offsetPx / pageHeight);

    return {
      page: pageNumber,
      offsetRatio
    };
  }

  function restoreReadingAnchor(anchor: ReadingAnchor, behavior: ScrollBehavior): boolean {
    const viewport = viewportElement;

    if (!viewport) {
      return false;
    }

    const target = viewport.querySelector<HTMLElement>(`[data-page="${anchor.page}"]`);

    if (!target) {
      const estimatedPageTop = estimatedPageTopByNumber[anchor.page];
      const estimatedPageHeight = estimatedDisplayHeightByNumber[anchor.page];

      if (!Number.isFinite(estimatedPageTop) || !Number.isFinite(estimatedPageHeight)) {
        return false;
      }

      const estimatedOffset = anchor.offsetRatio * Math.max(1, estimatedPageHeight);
      const estimatedScrollTop = Math.max(0, estimatedPageTop + estimatedOffset);
      viewport.scrollTo({
        top: estimatedScrollTop,
        behavior
      });
      scheduleVirtualWindowUpdate();

      requestAnimationFrame(() => {
        const exactTarget = viewport.querySelector<HTMLElement>(`[data-page="${anchor.page}"]`);

        if (!exactTarget) {
          return;
        }

        const retryViewportRect = viewport.getBoundingClientRect();
        const retryTargetRect = exactTarget.getBoundingClientRect();

        if (retryTargetRect.height < 1) {
          return;
        }

        const absoluteRetryTop = viewport.scrollTop + (retryTargetRect.top - retryViewportRect.top);
        const retryOffset = anchor.offsetRatio * retryTargetRect.height;
        viewport.scrollTo({
          top: Math.max(0, absoluteRetryTop + retryOffset),
          behavior: "auto"
        });
        scheduleVisiblePageScan();
      });

      return true;
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
    loadedThroughPage;
    logicalPageSizeByNumber;
    fitMode;
    viewportWidth;
    viewportHeight;
    isBusy;
    const anchor = pendingReadingAnchor;

    if (!anchor) {
      return;
    }

    requestAnimationFrame(() => {
      const restored = restoreReadingAnchor(anchor, "auto");

      if (restored) {
        reportActivePage(anchor.page);
      }

      if (restored && pendingReadingAnchor === anchor && !isBusy) {
        pendingReadingAnchor = null;
      }
    });
  });

  $effect(() => {
    renderedPages;
    loadedThroughPage;
    fitMode;
    logicalPageSizeByNumber;
    viewportWidth;
    viewportHeight;
    scheduleVirtualWindowUpdate();
  });

  $effect(() => {
    renderedPages;
    virtualMountedPages;
    scheduleVisiblePageScan();
    requestAnimationFrame(() => {
      maybeRequestLazyLoad();
    });
  });

  $effect(() => {
    isBusy;

    if (!isBusy) {
      flushPendingPinchZoom();
    }
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

    const hasTargetPage = target.page >= 1 && target.page <= loadedThroughPage;

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
        handledScrollTargetToken = target.token;
        scheduleVisiblePageScan();
      }
    });
  });

  onDestroy(() => {
    if (pinchFlushTimeoutId !== null) {
      clearTimeout(pinchFlushTimeoutId);
    }
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
  <p><strong>Loaded pages:</strong> {loadedThroughPage > 0 ? `1 - ${loadedThroughPage}` : "0"}</p>

  {#if renderedPages.length > 0}
    <div class="viewport" bind:this={viewportElement} onscroll={handleViewportScroll} onwheel={handleViewportWheel}>
      <div class="pages-stack">
        {#if virtualTopSpacerPx > 0}
          <div class="page-spacer" style={`height: ${virtualTopSpacerPx}px;`} aria-hidden="true"></div>
        {/if}

        {#each virtualMountedPages as pageNumber (pageNumber)}
          {@const renderedPage = renderedPageByNumber(pageNumber)}
          <div class="page-stage" data-page={pageNumber}>
            <div class="page-surface">
              {#if renderedPage}
                <img
                  src={convertFileSrc(renderedPage.imagePath)}
                  alt={"Rendered PDF page " + pageNumber}
                  onload={(event) => handleImageLoad(pageNumber, event)}
                  style={imageStyleForPage(pageNumber)}
                />
              {:else}
                <div
                  class="page-placeholder"
                  style={placeholderStyleForPage(pageNumber)}
                  aria-label={"Placeholder PDF page " + pageNumber}
                ></div>
              {/if}
            </div>
          </div>
        {/each}

        {#if virtualBottomSpacerPx > 0}
          <div class="page-spacer" style={`height: ${virtualBottomSpacerPx}px;`} aria-hidden="true"></div>
        {/if}
      </div>
    </div>

    {#if hasMorePages}
      <p class="lazy-load-hint">More pages load automatically while you scroll.</p>
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
    padding: 0.75rem;
    box-sizing: border-box;
  }

  .page-stage {
    width: 100%;
    display: flex;
    justify-content: center;
    padding-bottom: 1rem;
    box-sizing: border-box;
  }

  .page-spacer {
    width: 100%;
    min-height: 1px;
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

  .page-placeholder {
    display: block;
    background:
      linear-gradient(180deg, #f3f5f8, #eceff4),
      repeating-linear-gradient(90deg, #e2e7ee 0 12px, #eef2f7 12px 24px);
  }

  .lazy-load-hint {
    margin-top: 0.75rem;
    font-size: 0.88rem;
    color: #435468;
  }
</style>
