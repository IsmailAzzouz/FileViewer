# FileViewer

A file viewer application built with GPUI (Graphical Processing User Interface) framework in Rust.

## Overview

FileViewer is a desktop application for viewing files, built using the GPUI framework which provides a modern, GPU-accelerated UI toolkit for Rust.

## Features

- File browsing and viewing
- Modern GPU-accelerated UI
- Cross-platform support (Linux, macOS, Windows)

## Getting Started

### Prerequisites

- Rust (latest stable)
- Cargo

### Building

```bash
cargo build --release
```

### Running

```bash
cargo run --release
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

## License

MIT License