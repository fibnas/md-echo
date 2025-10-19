# md-echo
**Minimalist dual-pane Markdown editor & previewer built with egui and eframe**

[![Crates.io](https://img.shields.io/crates/v/md-echo.svg)](https://crates.io/crates/md-echo)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org)

---

`md-echo` is a fast, native Markdown editor written in Rust using [`eframe`](https://docs.rs/eframe) and [`egui`](https://docs.rs/egui).  
It displays **side-by-side editing and live preview** of Markdown with CommonMark compliance — perfect for quick note editing, technical docs, or journaling.

---

## Features

- Dual-Pane Interface: Edit on the left, preview on the right.
- Live Markdown Rendering: Powered by [`egui_commonmark`](https://crates.io/crates/egui_commonmark).
- File Management: New, Open, Save, Save As, and Exit.
- Unsaved Changes Protection: Confirms before discarding or exiting unsaved work.
- Hotkeys:
  - Ctrl+N — New file
  - Ctrl+O — Open file
  - Ctrl+S — Save
  - Ctrl+Shift+S — Save As
  - Ctrl+Q — Quit
- Status Bar: Shows file name, save state, and character count.

---

## Installation

You’ll need the latest stable Rust toolchain and GTK development headers:

**Fedora / RHEL:**
```bash
sudo dnf install gtk3-devel
```

**Debian / Ubuntu:**
```bash
sudo apt install libgtk-3-dev
```

Then install the crate:
```bash
cargo install md-echo
```

Or build locally:
```bash
git clone https://github.com/fibnas/md-echo
cd md-echo
cargo run --release
```

---

## Usage

Run the app:
```bash
md-echo
```

Then:
- Type Markdown in the left pane.
- Watch the formatted output update instantly in the right pane.
- Use the File menu or shortcuts to open/save documents.

---

## Tech Stack

- [`eframe`](https://docs.rs/eframe) — Native app framework for egui
- [`egui_commonmark`](https://crates.io/crates/egui_commonmark) — CommonMark renderer
- [`egui_extras`](https://docs.rs/egui_extras) — Layout helpers
- [`rfd`](https://crates.io/crates/rfd) — Native file dialogs
- Standard library I/O for fast read/write

---

## License

Licensed under the [MIT License](./LICENSE).

---

## Contributing

Pull requests are welcome.
Feel free to fork, tinker, and open an issue or PR if you have improvements.

---

> "Write, preview, and echo your thoughts — all in one window."
