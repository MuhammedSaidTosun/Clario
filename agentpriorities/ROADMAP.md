# ROADMAP.md

This file defines the development order of the project.

The steps must be followed sequentially.

Completed steps must remain working as later steps are added.

---

# Phase 1 — Reader Core

1. Repo + App Skeleton ✅
   - Tauri + Svelte setup
   - Rust command infrastructure
   - temp / cache / log directories
   - open file flow

2. PDF Reader MVP ✅
   - PDFium integration
   - open PDF
   - render page
   - next / previous
   - zoom

3. Reader Performance + Viewer Layout ✅
   - render cache
   - fit to width
   - fit to page
   - improved viewer layout

4. Continuous Scroll Viewer ✅
   - pages displayed vertically
   - scrolling continues across pages
   - next / previous remain available

5. Visible Page Tracking
   - determine active page during scroll
   - update page indicator accordingly

6. Lazy Page Rendering
   - render only visible and nearby pages
   - avoid rendering distant pages immediately

7. Prefetch + Render Queue
   - pre-render nearby pages
   - serialize and control render work

8. Text Layer Extraction
   - extract selectable text layer from PDFs
   - prepare for copy/search workflows

9. Search in PDF
   - search text
   - navigate matches
   - highlight results in viewer

Goal:
A strong, usable, high-quality PDF reader.

---

# Phase 2 — Editor Foundations

10. DocumentIR / Internal Editing Model
    - single source of truth for document editing state

11. Overlay Layer System
    - separate editable overlay above rendered PDF pages

12. Selection Engine
    - select page objects
    - bounding boxes
    - handles
    - active object tracking

13. Annotation System
    - highlight
    - underline
    - notes
    - freehand drawing

14. Basic Editing Tools
    - text box
    - image placement
    - shape placement
    - move / resize

15. Undo / Redo System
    - reversible editor operations

16. Save Pipeline — Overlay Save
    - preserve original PDF
    - write annotation/edit overlay data

17. Save Pipeline — Flatten Save
    - flatten pages when compatibility is needed

Goal:
Editable PDF documents built on top of the reader.

---

# Phase 3 — Export From PDF

18. PDF → TXT
    - strong extraction for own PDFs
    - best-effort extraction for external PDFs

19. PDF → MD
    - text extraction plus markdown structure

20. PDF → EPUB
    - fixed-layout EPUB
    - reflowable EPUB

21. PDF → DOCX / PPTX (Fidelity Mode)
    - preserve appearance through fidelity-oriented export

Goal:
Export PDF content into supported output formats.

---

# Phase 4 — Import To PDF

22. TXT → PDF Engine
    - internal layout engine path

23. MD → PDF Engine
    - markdown parsing
    - layout
    - PDF output

24. EPUB → PDF Engine
    - EPUB parse
    - HTML/intermediate layout
    - PDF output

25. PPTX → PDF Engine
    - slide-based positioned rendering

26. DOCX → PDF Engine
    - paragraph / run / table / image / pagination handling

Goal:
Import supported document formats into PDF through the app’s own conversion engine.

---

# Phase 5 — Unified Converter

27. Unified Conversion API
    - one conversion service for:
      - import
      - export
      - mode control

28. Batch Conversion Pipeline
    - multiple-file conversion flow

29. CLI / Automation Support
    - command-line access to conversion engine

30. Packaging + Cross-Platform Runtime Bundling
    - macOS
    - Windows
    - Linux
    - PDFium runtime management
    - stable distribution packaging

Goal:
A complete cross-platform offline document conversion platform.

---

# Rule

No step may be skipped.

Each phase must produce a working result before moving forward.