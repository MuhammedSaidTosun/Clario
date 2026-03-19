# ARCHITECTURE.md

Architecture source of truth for this offline Tauri + Rust + Svelte application.

This document defines ownership boundaries and system constraints.
It must stay compatible with roadmap order.

## Architectural Principles
- Offline-first on macOS, Windows, Linux
- Modular layers with explicit ownership
- Incremental evolution (no broad rewrites during active reader hardening)
- Viewport-first responsiveness over global eager consistency

---

## 1. Layer Model

### UI Layer (Svelte)
Owns:
- layout and presentation
- viewer composition and scroll structure
- viewport measurement and visible-band state
- mounted-window and placeholder/spacer decisions
- interaction state (scroll, gesture, navigation transitions)

Does not own:
- PDF parsing
- page rendering engine internals
- conversion pipelines
- save/flatten pipelines
- heavy backend-like orchestration systems

### Reader Engine Layer (Rust)
Owns:
- PDF opening/parsing primitives
- page rendering
- render output and cache primitives
- backend command handlers
- rendering performance primitives

Does not move to UI:
- render ownership
- PDF rendering correctness logic

### Editor Layer (Rust + Svelte, Future)
- Builds on reader output
- Adds overlay/interactions without replacing reader rendering

### Conversion Layer (Rust, Future)
- Import/export engines remain separate from reader interaction loop

### Persistence Layer (Rust, Future)
- Overlay save, flatten save, export outputs

---

## 2. Boundary Contract

Svelte <-> Rust boundary:
- Tauri commands only

Contract intent:
- UI decides what is worth requesting now (viewport/presentation priorities)
- Rust executes rendering and returns outputs
- UI never re-implements rendering ownership

---

## 3. Reader Architecture (Current Direction)

Phase 1 Step 8 is active.
Reader architecture must stay aligned to these rules.

### 3.1 Viewport-First Decision Model
- Prioritize the active visible region and nearby pages.
- Avoid broad global synchronization when it hurts interaction latency.
- Local visible correctness has higher priority than far-page freshness.
- Initial document load should render only viewport-near pages; far pages load via prefetch on demand.

### 3.2 Virtualized Continuous Scroll Model
- Keep only a small mounted page-image window around viewport.
- Preserve continuity with spacers/placeholders/page shells.
- Do not mount full loaded ranges as active images.
- Temporarily stale far pages are acceptable when bounded and recoverable.

### 3.3 Stale-Work Suppression
- Obsolete work may be dropped/superseded.
- Dedup equivalent render intents.
- Prefer responsiveness over completing stale global work.

### 3.4 Navigation Stability
- Programmatic next/previous must keep navigation targets stable.
- Prevent indicator flicker from stale intermediate updates.

### 3.5 Zoom Architecture Policy (Pinch-First)
Target behavior:
- immediate visual zoom feedback during gesture
- deferred bounded real rerender after gesture settles
- visible-band rerender priority before distant catch-up

Steady-state requirement:
- mixed zoom-state pages in active visible band are unacceptable

Non-goal:
- broad full-range rerender as the default zoom consistency mechanism

Stale-zoom page behavior:
- stale-zoom pages in the visible band show dimmed old content, not blank placeholders
- zoom rerender is scoped to visible band + margin, not all loaded pages
- pages refresh at the new zoom level when scrolled into the visible band

### 3.6 Text Layer Extraction Policy (Step 8)
- Reader remains image-render-based at its core.
- Rust owns page text extraction and normalization.
- Svelte mounts text presentation only in experimental mode, bounded to mounted active-local pages.
- Text extraction must remain bounded, deduplicated, and stale-safe.
- Extraction should defer under heavy active rendering and retry once rendering settles.
- Selectable text layer is allowed as a foundation for copy/search preparation, but is off by default.
- Step 8 must not introduce Step 9 search query, match navigation, or match highlighting behavior.

---

## 4. Discouraged Architectural Patterns
Do not introduce reader-wide systems that centralize excessive orchestration, including:
- heavy global render queues
- broad orchestration hubs/controllers
- large task state machines coordinating the full document

Reason:
- they increase latency and complexity,
- conflict with viewport-first bounded behavior,
- and raise regression risk during Step 7 hardening.

---

## 5. Data Flow (Current + Future)

Reader flow:
- PDF -> Rust reader engine -> rendered page output -> Svelte viewer

Future editor flow:
- reader render layer + overlay interaction layer + editor controls

Future conversion flow:
- format parser/extractor -> Rust conversion pipeline -> output format

---

## 6. Architecture Rules (Non-Negotiable)
1. Rust owns rendering, parsing, conversion, and persistence logic.
2. Svelte owns layout, presentation, interaction, and viewport-driven request policy.
3. Tauri commands are the only app-layer boundary.
4. Virtualization is part of intended reader architecture and must be preserved.
5. Viewport-first bounded behavior is preferred over global eager consistency.
6. Step 8 text-layer work must not be used to start Step 9+ scope.
7. Reader changes must avoid heavy queue/orchestration expansion.
