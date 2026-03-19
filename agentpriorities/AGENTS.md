# AGENTS.md

Operational rules for coding agents in this repository.

Project:
Offline cross-platform PDF Reader + Editor + Converter

Stack:
- Tauri
- Rust
- Svelte

Target platforms:
- macOS
- Windows
- Linux

## 1) Mandatory Pre-Read (No Exceptions)
Before writing or editing code, read these files in order:
1. `agentpriorities/AGENTS.md`
2. `agentpriorities/ROADMAP.md`
3. `agentpriorities/ARCHITECTURE.md`

If they conflict, resolve by priority:
1. `AGENTS.md`
2. `ROADMAP.md`
3. `ARCHITECTURE.md`

## 2) Current Active Scope
Current active focus:
- Phase 1, Step 8 (Text Layer Extraction Foundation)

Step 9 and beyond:
- NOT started
- NOT allowed to be started

Editor and converter work:
- Out of scope until roadmap says otherwise

## 3) Scope Discipline
Allowed work now:
- Reader behavior that implements Step 8 goals while preserving Step 7 responsiveness
- Tight bug fixes/refactors in Steps 1-7 that are directly required for Step 8 safety
- Policy/document updates that reduce future implementation risk

Not allowed now:
- Step 9+ features (search/navigation/highlighting, editor, converter)
- Refactors that change roadmap phase boundaries without Step 8 need
- Repository-wide restructuring unrelated to Step 8

## 4) Ownership Boundaries (Must Preserve)
Rust (reader engine) owns:
- PDF rendering
- render/cache primitives
- rendering-related backend commands

Svelte (UI) owns:
- layout and presentation
- viewport/visible-band tracking
- mounted page-window decisions
- interaction and gesture state

Boundary:
- Svelte <-> Rust communication only through Tauri commands
- Do not move rendering ownership from Rust into Svelte

## 5) Reader Rules During Step 8 (Authoritative)
Treat these as implementation constraints, not suggestions.

### 5.1 Visible-Region-First
- Prioritize responsiveness in/near the viewport over global freshness.
- Prefer bounded local correctness in the visible band over eager whole-document catch-up.

### 5.2 Virtualization Is Required
- Keep a small mounted page-image window around the viewport.
- Do not mount full loaded ranges as active page images.
- Use spacers/placeholders/shells to preserve scroll continuity.
- Temporarily stale far pages are acceptable.

### 5.3 Stale-Work Suppression
- Drop/supersede obsolete work when newer state makes it irrelevant.
- Avoid draining stale queues if it hurts interaction latency.
- Dedup render requests by page + zoom-state semantics.

### 5.4 Zoom Policy (Pinch-First Direction)
Target zoom behavior:
- Immediate visual zoom feedback during pinch/gesture
- Deferred, bounded real rerender after gesture settles
- Visible-band rerender priority over distant pages

Unacceptable steady state:
- Mixed zoom-state pages inside the active visible band

Do not use:
- Broad full-range rerenders as the primary zoom consistency strategy

### 5.5 Navigation Stability
- Programmatic next/previous navigation must keep active-target behavior stable.
- Avoid indicator flicker and stale transition overrides during navigation settle periods.

### 5.6 Text Layer Safety
- Selectable text overlay is experimental and must be off by default.
- When text selection mode is off, extraction scheduling and text-layer mounting should be dormant.
- When enabled, extraction and mounting must stay bounded to mounted active-local pages only.
- If text-layer behavior conflicts with reader responsiveness, preserve reader responsiveness.

## 6) Forbidden Directions
Do NOT introduce:
- Heavy global render queues
- Broad orchestration subsystems
- Large task state machines
- Full-range eager rerender frameworks
- Strategies that revert virtualization
- Work that starts Step 9+

## 7) Change Process Rules
For every change:
1. Confirm it is within current roadmap scope.
2. Verify it preserves Rust/Svelte ownership boundaries.
3. Prefer the smallest change that improves visible-band responsiveness/stability.
4. Avoid speculative architecture expansion.
5. Keep behavior modular and incremental.

## 8) Documentation Update Rule
If behavior/policy for reader Step 8 changes materially, update all relevant source-of-truth docs in the same change:
- `agentpriorities/AGENTS.md`
- `agentpriorities/ROADMAP.md`
- `agentpriorities/ARCHITECTURE.md`

Do not leave policy drift for later.

## 9) Quality Gate Before Closing Work
Before marking Step 8 work done, verify:
- No Step 9+ scope leakage
- Virtualization still active
- Visible-band responsiveness preserved
- No unacceptable mixed zoom-state steady state in visible band
- Text extraction remains dormant by default and bounded when experimental mode is enabled
- Selectable text layer does not regress reader interaction latency in default mode
- No regressions to earlier completed reader steps
