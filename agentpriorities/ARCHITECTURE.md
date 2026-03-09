
---

# `ARCHITECTURE.md`

```md
# ARCHITECTURE.md

This document describes the architecture of the application.

---

# Overview

The application consists of five main layers:

1. UI Layer
2. Reader Engine Layer
3. Editor Layer
4. Conversion Layer
5. Persistence / Save Layer

Each layer must evolve incrementally and remain compatible with the roadmap order.

---

# UI Layer

Technology:
- Svelte

Responsibilities:
- rendering the interface
- user interaction
- displaying PDF pages
- viewer controls
- editor overlays
- reader layout modes
- status and debug feedback

Important components:
- `Toolbar`
- `Sidebar`
- `PdfViewer`

The UI layer must not own heavy rendering, parsing, or conversion logic.

---

# Reader Engine Layer

Technology:
- Rust

Responsibilities:
- PDF rendering
- page image generation
- cache-aware page rendering
- file management
- command handling
- page navigation support
- reader performance primitives

Important modules:
- `commands/`
- `pdf/`

This layer is responsible for opening PDFs and rendering pages for the viewer.

---

# Editor Layer

Technology:
- Rust + Svelte

Responsibilities:
- DocumentIR-based editing state
- overlay system
- object selection
- annotations
- editing tools
- undo / redo

Editing must follow a layered model:

1. PDF render layer
2. interaction / overlay layer
3. UI control layer

User changes update the shared document model.

---

# Conversion Layer

Technology:
- Rust

Responsibilities:
- importing formats
- exporting formats
- layout engine
- document transformation
- format-specific adapters

Converters:
- TXT converter
- MD converter
- EPUB converter
- DOCX converter
- PPTX converter

This layer is the long-term core of the converter engine.

---

# Persistence / Save Layer

Technology:
- Rust

Responsibilities:
- overlay save
- flatten save
- export file generation
- cache file persistence
- output file consistency

Two save modes exist:

## Overlay Save
Keeps the original PDF and writes edits/annotations on top.

## Flatten Save
Renders pages to images and rebuilds the PDF for compatibility.

---

# Document Model

All editable document data must be stored in:

`DocumentIR`

`DocumentIR` is the single source of truth between:

UI ↔ Rust

It will be introduced in the editor phase and used for later editing and conversion workflows.

---

# Data Flow

## Reader flow
PDF → Rust PDF engine → rendered page image → Svelte viewer

## Import flow
File → Parser → DocumentIR / layout model → PDF

## Export flow
PDF → Extraction → Converter → Target Format

---

# Rendering

PDF pages are rendered using a Rust PDF engine.

Rendered pages are displayed in the Svelte viewer.

Current rendering principles:
- backend-controlled rendering
- cache-backed page output
- high-DPI-aware rendering
- lossless page output where required for quality

---

# Continuous Reading Model

The reader will evolve from:
- single-page navigation

toward:
- continuous scroll viewer
- visible page tracking
- lazy page rendering
- prefetch and render queue support

These are reader-layer concerns, not editor concerns.

---

# Architecture Rules

1. Rust owns rendering, parsing, conversion, and save logic.
2. Svelte owns layout, controls, and presentation.
3. Tauri commands are the backend boundary.
4. The reader and converter systems must remain separable.
5. The editor system must build on top of the reader, not replace it.