# FileViewer

A file viewer application built with GPUI (Graphical Processing User Interface) framework in Rust.

## Overview

FileViewer is a desktop application for viewing files, built using the GPUI framework which provides a modern, GPU-accelerated UI toolkit for Rust.

## Supported Formats

| Format | Extensions | Parse | Format / Minify | Syntax Highlighting | Tree View |
| ------ | ----------- | :---: | :--------------: | :-----------------: | :-------: |
| JSON   | `.json`     |  ✅   |       ✅        |          ✅          |     ✅     |
| JSONC  | `.jsonc`    |  ✅   |       ✅        |          ✅          |     ✅     |
| JSONL  | `.jsonl`, `.ndjson` |  ✅   |       ✅        |          ✅          |     ✅     |
| TOML   | `.toml`     |  ✅   |       ✅        |          ✅          |     ✅     |

The format is detected from the file extension when a document is opened, and
falls back to JSON for anything unrecognized.

JSONC documents accept `//` and `/* */` comments and allow trailing commas.
JSONL documents are parsed as a sequence of values, one per line, and appear in
the tree as a synthetic root array with one entry per value. Formatting and
minification for both are line-oriented and never reserialize the value tree,
so comments and blank lines survive round-tripping. UTF-8 BOMs are stripped
before formatting, and JSONL source spans are offset by the BOM length so
highlighting and cursor sync still land on the right text.

## Features

- Two-way synchronization between the editor and the tree view
- Pretty-printing and minification per format
- Syntax highlighting with precise source spans and line/column tracking
- Validation diagnostics with source context and a jump-to-error action
- In-document find, symbol navigation, word wrap, and undo/redo
- Modern GPU-accelerated UI
- Cross-platform support (Linux, macOS, Windows)

## Keyboard Shortcuts
| Shortcut           | Action                     |
| ------------------ | -------------------------- |
| `Ctrl+O`           | Open a file                |
| `Ctrl+S`           | Save                       |
| `Ctrl+Shift+F`     | Format document            |
| `Ctrl+F`           | Find                       |
| `Ctrl+Z` / `Ctrl+Y` | Undo / redo               |
| `Ctrl+T`           | Toggle tree panel          |
| `Ctrl+Shift+E`     | Expand all tree nodes       |
| `Ctrl+Shift+C`     | Collapse all tree nodes     |
| `Alt+Z`            | Toggle word wrap           |
| `Alt+Down` / `Alt+Up` | Jump to next / previous symbol |

## Getting Started

Build the project, then open a file by passing its path as an argument or with
`Ctrl+O`:

```bash
cargo run --release -- config.jsonc
```

### Building

```bash
cargo build --release
```

### Running

```bash
cargo run --release
```

Or pass a path to open it directly:

```bash
./target/release/file-viewer config.toml
```

## Project Structure

```
FileViewer/
├── src/           # Source code
├── tests/         # Integration tests
├── Cargo.toml     # Project configuration
└── build.rs       # Build script
```

## Dependencies

- `gpui` - GPU-accelerated UI framework
- `serde` - Serialization framework
- `serde_json` - JSON serialization with preserve_order feature
- `toml` - TOML parsing

## Testing

```bash
cargo test
```

## License

MIT License