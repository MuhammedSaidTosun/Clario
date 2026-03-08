# ARCHITECTURE.md

This document describes the architecture of the application.

---

# Overview

The application consists of three main layers:

1. UI Layer
2. Engine Layer
3. Conversion Layer

---

# UI Layer

Technology:

Svelte

Responsibilities:

* rendering the interface
* user interaction
* displaying PDF pages
* editor overlays

Important components:

Toolbar
Sidebar
PdfViewer

---

# Engine Layer

Technology:

Rust

Responsibilities:

* PDF rendering
* file management
* command handling
* document state management

Important modules:

commands/
pdf/

---

# Conversion Layer

Technology:

Rust

Responsibilities:

* importing formats
* exporting formats
* layout engine

Converters:

TXT converter
MD converter
EPUB converter
DOCX converter
PPTX converter

---

# Document Model

All document data must be stored in:

DocumentIR

DocumentIR is the single source of truth between:

UI ↔ Rust

---

# Data Flow

Import:

File → Parser → DocumentIR → Layout Engine → PDF

Export:

PDF → Extraction → Converter → Target Format

---

# Rendering

PDF pages are rendered using a Rust PDF engine.

Rendered pages are displayed in the Svelte viewer.

---

# Editing

Editing uses a layered approach:

1. PDF render layer
2. interaction layer
3. UI layer

User changes update the DocumentIR model.

---

# Saving

Two save modes exist:

Overlay Save
Flatten Save

Overlay Save keeps original PDF.

Flatten Save renders pages to images and rebuilds the PDF.
