# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

**cosmic-comp** is the Wayland compositor for the COSMIC desktop environment (System76). It's a full-featured compositor built on **Smithay** (Rust Wayland compositor framework), handling window management, rendering, input, and display output. Licensed GPL-3.0.

## Build Commands

```bash
# Build (release)
make                        # or: cargo build --release

# Build (debug)
make DEBUG=1                # or: cargo build

# Check compilation without building
cargo check
cargo check --no-default-features
cargo check --features debug

# Format
cargo fmt --all -- --check  # check only
cargo fmt --all             # apply

# Test
cargo test --all-features   # requires libseat and system deps

# Install to system
sudo make install           # binary + keybindings + tiling config
sudo make install-bare-session  # also installs systemd units + wayland session

# Clean
make clean

# Nix
nix build
nix develop                 # dev shell with all system deps
```

**Rust toolchain:** 1.90 (pinned in `rust-toolchain.toml`), edition 2024, rustfmt style_edition 2024.

**System dependencies** (for building on Debian/Ubuntu):
`libudev-dev libgbm-dev libxkbcommon-dev libegl1-mesa-dev libwayland-dev libinput-dev libdbus-1-dev libsystemd-dev libseat-dev libdisplay-info-dev libpixman-1-dev`

## Cargo Features

- `default = ["systemd"]` — systemd/logind session integration
- `debug` — egui-based debug overlay UI
- `profile-with-tracy` — CPU profiling with Tracy
- `profile-with-tracy-gpu` — adds GPU profiling to Tracy

Release profile uses fat LTO. The `fastdebug` profile inherits release with debug symbols. Several hot dependencies (`tiny-skia`, `rustybuzz`, `ttf-parser`) have opt-level=2 even in dev builds.

## Architecture

### Entry Point

`src/main.rs` calls `cosmic_comp::run()` in `src/lib.rs`, which:
1. Parses CLI args, initializes logging (journald if available)
2. Creates the Calloop event loop and Wayland display
3. Builds the central `State` struct
4. Auto-selects backend: `COSMIC_BACKEND` env var forces `kms`/`x11`/`winit`; otherwise KMS if no display server detected, X11 with winit fallback if running nested

### Core Modules

- **`state.rs`** — Central `State` struct holding all compositor state. This is the largest file (~54KB) and the integration point for everything.
- **`backend/`** — Pluggable rendering backends:
  - `kms/` — Native DRM/GBM backend for direct GPU control (production backend). Handles multi-GPU, scanout, surface tiling.
  - `winit/` — Window-based backend for development/testing
  - `x11/` — Backend for running nested inside X11
  - `render/` — Rendering abstraction across GLES, Pixman (software), Vulkan. Custom shaders for shadows, rounded corners, clipping.
- **`shell/`** — Window management:
  - `layout/` — Tiling and floating window layouts with animation support
  - `workspace.rs` — Workspace management with groups
  - `focus/` — Keyboard/pointer focus tracking
  - `grabs/` — Input grabs for window move/resize/menus
  - `element/` — Renderable window elements
  - `zoom.rs` — Accessibility zoom/magnification
- **`wayland/`** — Wayland protocol implementations:
  - `handlers/` — Event handlers for standard + COSMIC protocols
  - `protocols/` — Custom protocol definitions (workspace management, overlap notify, image capture)
- **`input/`** — Keyboard, pointer, touch, and gesture processing
- **`config/`** — Dynamic configuration via `cosmic-config`
- **`dbus/`** — D-Bus integration (logind, accessibility, power management)
- **`xwayland.rs`** — XWayland support for X11 application compatibility (~46KB)

### Workspace Structure

Cargo workspace with two members:
- **`cosmic-comp`** (root) — The compositor binary
- **`cosmic-comp-config/`** — Configuration types library. Features: `output` (ron serialization + tracing), `randr` (cosmic-randr integration)

### Key Dependencies

- **`smithay`** — Wayland compositor framework (patched to specific commit via `[patch.crates-io]`)
- **`cosmic-protocols`** — COSMIC-specific Wayland protocols (patched to main branch)
- **`libcosmic`** / `cosmic-config` — COSMIC desktop framework and settings
- **`calloop`** — Event loop driving the compositor
- **`id_tree`** — Tree structure for tiling layout (patched fork)

### Runtime Data

`data/` contains:
- `keybindings.ron` — Default keyboard shortcuts
- `tiling-exceptions.ron` — Window tiling exception rules
- `cosmic.desktop` — Wayland session entry for display managers
- Systemd service/target units for session management

### i18n

Uses `i18n-embed-fl` with Fluent files in `resources/i18n/` (30+ languages). Fallback language is English.
