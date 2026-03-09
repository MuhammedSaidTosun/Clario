# AGENTS.md

This file defines the rules for AI coding agents working on this repository.

Project: Cross-platform PDF Reader + Editor + Converter  
Stack: Tauri + Rust + Svelte

Target platforms:
- macOS
- Windows
- Linux

The application must run fully offline.

---

# Development Philosophy

1. Rust performs all heavy operations.
2. Svelte handles UI only.
3. Communication between UI and backend must use Tauri commands.
4. The system must be modular.
5. Every feature must be implemented incrementally.
6. The roadmap must be followed strictly in order.
7. Completed steps must not be broken by later work.

---

# High Level Goal

Build a professional desktop application capable of:

- Viewing PDFs
- Editing PDFs
- Converting multiple formats to PDF
- Exporting PDFs to other formats

Supported formats:

Import → PDF
- TXT
- MD
- EPUB
- DOCX
- PPTX

Export from PDF
- TXT
- DOCX
- PPTX
- MD
- EPUB

---

# Project Structure

Frontend:
- `src/`
- `src/components/`
- `Toolbar.svelte`
- `Sidebar.svelte`
- `PdfViewer.svelte`

Backend:
- `src-tauri/`
- `src-tauri/src/`
- `src-tauri/src/main.rs`
- `src-tauri/src/commands/`
- `src-tauri/src/pdf/`
- `src-tauri/src/converters/`

This structure may grow, but agents must keep it modular and avoid unnecessary restructuring.

---

# Communication Model

Frontend must communicate with Rust using Tauri commands via `invoke()`.

Example:
```ts
invoke("render_pdf_page")