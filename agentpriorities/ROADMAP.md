# ROADMAP.md

Sequential development roadmap for this repository.

Core rules:
- Follow steps in order.
- Do not skip steps.
- Do not start a later step before current step exit criteria are met.
- Keep the app fully offline on macOS, Windows, and Linux.

## Current Status (Authoritative)
- Active phase: Phase 1 (Reader Core)
- Active step: Step 8 (Text Layer Extraction)
- Active sub-focus: Step 8.1 (Bounded Page-Level Extraction + Selectable Layer Foundation)
- Step 9+: NOT STARTED and blocked until Step 8 exit criteria pass

---

## Phase 1 - Reader Core
Goal: stable, responsive PDF reader foundation for all later work.

### 1. Repo + App Skeleton (Completed)
- Tauri + Svelte bootstrap
- Rust command infrastructure
- temp/cache/log directories
- open-file flow

### 2. PDF Reader MVP (Completed)
- PDFium integration
- open PDF
- render page
- previous/next navigation
- basic zoom

### 3. Reader Performance + Viewer Layout (Completed)
- render cache
- fit-to-width
- fit-to-page
- improved viewer layout
- bounded initial render window (viewport-near pages only, not full document)

### 4. Continuous Scroll Viewer (Completed)
- vertical continuous page flow
- scroll continuity across pages
- previous/next still available

### 5. Visible Page Tracking (Completed)
- active page detection during scroll
- page indicator updates

### 6. Lazy Page Rendering (Completed)
- render visible/nearby pages first
- defer far-page work

### 7. Reader Performance Hardening (Completed)
Scope: viewport-first responsiveness, safer zoom, render work hardening, and virtualization-safe behavior.

#### 7.1 Light Prefetch (Completed)
- nearby page prefetch only
- no aggressive global speculative work
- keep visible region priority

#### 7.2 Render Dedup (Completed)
- deduplicate equivalent render requests
- suppress unnecessary repeated work

#### 7.3 Navigation Target Stabilization (Completed)
- stabilize active page target during programmatic navigation
- prevent flicker from stale tracking updates

#### 7.4 Safer Zoom with Visible-Pages-First (Completed)
- prioritize visible pages during zoom transitions
- avoid broad rerender behavior when bounded alternatives work

#### 7.5 Bounded Scheduling / Hardening (Completed)
- cap frontend scheduling pressure
- suppress stale non-essential work
- keep interaction latency ahead of global refresh eagerness

#### 7.6 Virtualization Hardening (Implemented, Ongoing Polish)
- keep a small mounted page-image window around viewport
- preserve continuous flow with spacers/placeholders
- allow temporarily stale far pages when needed
- do not revert to mounting full loaded spans as active images

#### 7.7 Zoom UX + Visible-Band Consistency (Completed)
- pinch-first zoom direction
- immediate visual zoom during gesture
- deferred bounded real rerender after gesture settle
- zoom rerender scoped to visible band + margin only (not all loaded pages)
- stale-zoom pages show dimmed old content, not blank placeholders
- pages refresh at new zoom when scrolled into visible band
- unacceptable steady state: mixed zoom-state pages inside active visible band with no automatic recovery

#### 7.8 Step 7 Completion Sweep (Completed)
- finalize tuning across fast scroll, pinch zoom, and programmatic navigation
- remove/adjust conflicting zoom controls only if they hurt pinch-first model
- validate no regressions to Steps 1-6 behavior

### Step 7 Exit Criteria (Gate for Step 8)
All items must be true before Step 8 starts:
- fast scrolling remains responsive without major freezes
- visible-region-first behavior remains the default
- virtualization is active and preserved
- small mounted window around viewport is maintained
- stale far pages may exist temporarily without UX collapse
- no unacceptable mixed zoom-state steady state in active visible band
- navigation target stabilization remains intact
- no heavy global render queues/orchestration introduced
- no regressions in completed Steps 1-6

### 8. Text Layer Extraction (In Progress)
- page-level text extraction command boundary (Rust -> Tauri -> Svelte)
- structured segment-level text geometry for future search/navigation groundwork
- bounded extraction policy tied to mounted window + active-local visible-near pages
- virtualization-safe text-layer mounting discipline
- optional experimental invisible selectable overlay (off by default)
- explicit non-goal: no Step 9 search/match/highlight behavior in this step

### 9. Search in PDF (Blocked)
- search text
- navigate matches
- highlight match results

---

## Phase 2 - Editor Foundations (Not Started)
Goal: editable PDF workflows layered on top of reader architecture.

### 10. DocumentIR / Internal Editing Model
### 11. Overlay Layer System
### 12. Selection Engine
### 13. Annotation System
### 14. Basic Editing Tools
### 15. Undo / Redo System
### 16. Save Pipeline - Overlay Save
### 17. Save Pipeline - Flatten Save

---

## Phase 3 - Export From PDF (Not Started)
Goal: export PDF content into supported formats.

### 18. PDF -> TXT
### 19. PDF -> MD
### 20. PDF -> EPUB
### 21. PDF -> DOCX / PPTX (Fidelity Mode)

---

## Phase 4 - Import To PDF (Not Started)
Goal: import supported formats and generate PDF offline.

### 22. TXT -> PDF Engine
### 23. MD -> PDF Engine
### 24. EPUB -> PDF Engine
### 25. PPTX -> PDF Engine
### 26. DOCX -> PDF Engine

---

## Phase 5 - Unified Converter (Not Started)
Goal: unified cross-platform offline conversion platform.

### 27. Unified Conversion API
### 28. Batch Conversion Pipeline
### 29. CLI / Automation Support
### 30. Packaging + Cross-Platform Runtime Bundling

---

## Non-Negotiable Guardrails
- Do not start Step 9+ while Step 8 is active.
- Do not shift rendering ownership from Rust to Svelte.
- Do not replace virtualization with full-range mounted image spans.
- Do not solve zoom consistency via broad full-range rerenders.
- Do not introduce heavy global queue/orchestration systems for reader flow.
