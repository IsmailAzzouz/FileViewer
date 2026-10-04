<p align="center">
  <img src="assets/header.svg" alt="FileViewer — Precise by Construction" width="100%" />
</p>

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
| YAML   | `.yaml`, `.yml` |  ✅ |       ✅        |          ✅          |     ✅     |

The format is detected from the file extension when a document is opened, and
falls back to JSON for anything unrecognized.

JSONC documents accept `//` and `/* */` comments and allow trailing commas.
JSONL documents are parsed as a sequence of values, one per line, and appear in
the tree as a synthetic root array with one entry per value. Formatting and
minification for both are line-oriented and never reserialize the value tree,
so comments and blank lines survive round-tripping. UTF-8 BOMs are stripped
before formatting, and JSONL source spans are offset by the BOM length so
highlighting and cursor sync still land on the right text.

YAML support covers block mappings and sequences, flow collections, plain and
quoted scalars, block scalars with chomping and explicit-indent indicators,
anchors with aliases, merge keys, tags, and multi-document streams. An alias
expands to a clone of the anchored node, so the tree shows resolved data; a key
written directly in a mapping wins over a merged one. A multi-document stream
appears as a synthetic root array, matching JSONL. Alias expansion is bounded by
a node budget, so a self-referential or deliberately explosive document reports
a diagnostic instead of exhausting memory. Complex keys (`?`) are rejected
rather than silently dropped, and timestamps are typed as strings.

`samples/feature-tour.yml` exercises these constructs and is useful for a quick
visual check of the tree and the editor.

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

On Linux the GPUI runtime needs the XCB and XKB development libraries at link
time (`libxcb1-dev`, `libxkbcommon-dev`, `libxkbcommon-x11-dev`). Without them the
build fails at the final link with `rust-lld: error: unable to find library
-lxcb`. Any distro package manager works; the crates themselves compile fine
because GPUI loads its platform libraries dynamically.

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
├── assets/         # README header artwork
├── samples/        # Sample documents, including a YAML feature tour
├── src/            # Source code
├── tests/          # Integration tests
├── Cargo.toml      # Project configuration
└── build.rs        # Build script
```

## Dependencies

- `gpui` - GPU-accelerated UI framework
- `serde` - Serialization framework
- `serde_json` - JSON serialization with preserve_order feature
- `toml` - TOML parsing

The JSON, JSONC, JSONL, and TOML parsers are hand-written for exact source spans.
YAML adds no dependency for the same reason: the parser is hand-written rather
than adapting a library whose scanner reports start positions only.

## Testing

```bash
cargo test
```

## License

[MIT](LICENSE)