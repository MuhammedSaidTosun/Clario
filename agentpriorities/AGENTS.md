# AGENTS.md

This file defines the rules for AI coding agents working on this repository.

Project: Cross-platform PDF Reader + Editor + Converter
Stack: Tauri + Rust + Svelte

Target platforms:

* macOS
* Windows
* Linux

The application must run fully offline.

---

# Development Philosophy

1. Rust performs all heavy operations.
2. Svelte handles UI only.
3. Communication between UI and backend must use Tauri commands.
4. The system must be modular.
5. Every feature must be implemented incrementally.

---

# High Level Goal

Build a professional desktop application capable of:

* Viewing PDFs
* Editing PDFs
* Converting multiple formats to PDF
* Exporting PDFs to other formats

Supported formats:

Import → PDF

* TXT
* MD
* EPUB
* DOCX
* PPTX

Export from PDF

* TXT
* DOCX
* PPTX
* MD
* EPUB

---

# Project Structure

Frontend:

src/
components/
Toolbar.svelte
Sidebar.svelte
PdfViewer.svelte

Backend:

src-tauri/
src/
main.rs
commands/
pdf/
converters/

---

# Communication Model

Frontend must communicate with Rust using:

invoke()

Example:

invoke("render_page")

---

# Code Style Rules

Rust:

* Use Result instead of panic
* Avoid unwrap()
* Prefer small modules
* Keep files under ~300 lines if possible

Svelte:

* Components must be small
* UI must not contain business logic
* State must be centralized

---

# Dependency Rules

AI agents must NOT:

* add new dependencies without explaining why
* replace existing frameworks
* introduce heavy frameworks

---

# Logging

Application logs must be written to:

app_data/logs/app.log

---

# Development Workflow

When implementing a feature:

1. read the project structure
2. propose a short plan
3. implement minimal working code
4. explain changed files

---

# Safety

AI agents must NOT:

* delete project structure
* rewrite large parts of the system
* change architecture decisions
