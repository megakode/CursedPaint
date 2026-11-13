# CursedPaint

A native, cross-platform (macOS/Linux) desktop pixel-art editor for Commodore 64 graphics that
enforces the real C64 colour limits, so the artist never has to think about them. Modelled on
**ArcPaint64** (browser-based, MIT licence, by Stello Doussis), which is the reference for features and
look. The reference copy lives at `/Users/peter/Downloads/arcPaint64-v1.3.1/index.html` (one 2.5 MB
file; README.txt and CHANGELOG.txt beside it document its features). Its dither/brush tables were
ported from there (keep the MIT attribution in comments). We write the core fresh from the C64
hardware rules and use ArcPaint's behaviour as the reference.

The folder is still named `retropaint` (the app was renamed to CursedPaint; folder kept on purpose).
Not a git repository yet.

## Stack and commands

Rust workspace, egui/eframe UI. Toolchain: Rust 1.100 (the user updated it; `Cargo.lock` was first
resolved on 1.85, so some crates are older than latest, e.g. eframe 0.32, rfd 0.15).

```bash
cargo run --release          # run the app (binary: target/release/cursedpaint)
cargo test --release         # all tests (core + app), currently 47
cargo build --release        # note: `cargo test` does NOT rebuild the app binary
```

- `crates/c64gfx`: GUI-free core, fully unit-tested. **Put C64 rules and logic here.**
  - `image.rs`: `Image` stored exactly as hardware sees it (bitmap bytes, screen RAM, colour RAM,
    background). `Mode::{Hires, Multicolor}`. `paint_hardware` (write a slot) and `paint_auto`
    (paint a colour, find a legal slot; `force` recolours the least-used slot, otherwise nearest
    colour). `region_at` is flood-fill. Coordinates are *logical* pixels (160 wide in multicolor).
  - `palette.rs`: Pepto and Colodore RGB, `SortMode` (display order only: C64, Luminance, Hue wheel,
    Gradient ramps).
  - `dither.rs`: 40 patterns per mode (ArcPaint tables), invert/flipX/flipY/anchored.
  - `brush.rs`: 8 preset brushes per mode.
  - `shapes.rs`: line, rect, ellipse point generators.
  - `io.rs`: Koala, Art Studio, native project (`.rpnt`, magic `RPNT`), KickAssembler ASM export.
- `crates/app` (package `cursedpaint`): egui app.
  - `app.rs`: `App` state, input handling, side panel, canvas, status bar, shortcuts, and the app
    tests (they drive strokes via a `stroke()` helper, no GUI needed).
  - `icons.rs`: toolbar icons drawn with the painter (no image assets), `Icon`, `button()`.

## Features that exist

Hires and multicolor bitmap modes, any size from 1x1 to 40x25 cells; tools: pencil, line, rect,
filled rect, ellipse, filled ellipse, fill, colour picker; Automatic and Hardware colour modes;
dither section (40 patterns, Orig./Invert, Global/Anchor, FlipX, FlipY); brushes section (8 presets,
stamped by pencil/line/rect/ellipse only); floating brush preview in the active colour (matches what a
click paints, dither included); palette fixed to Pepto with a Sort dropdown; status bar with a
fixed-position 4-slot indicator of the hovered cell's colour slots (struck-through = free slot);
undo/redo (snapshot-based, 200 deep); cell grid, pixel grid, zoom (scroll), pan (middle drag or
Space-drag), fit.

Shortcuts: B L R E G I tools (Shift+R / Shift+E filled), `[` `]` previous/next colour in the
*displayed* order (wraps), `-` `=` previous/next brush (wraps), X swap colours, F fit, Cmd+Z / Cmd+Shift+Z,
Cmd+S, Cmd+O. Right-click paints the second colour.

Import/export: open `.rpnt`, `.koa/.kla` (Koala), `.art/.aas/.ocp` (Art Studio); export Koala (multicolor,
40x25 only), Art Studio (hires, 40x25 only), KickAssembler `.asm`. Project save is `.rpnt`.

## Not built yet (ideas, roughly in the order discussed)

- Sprite modes (standard/multicolor sprites, 24x21 units) and sprite animation; charset/tilemap editing
  (the user is making C64 *games*, so these matter more than full bitmaps).
- Selections, copy/paste, captured custom brushes, transforms (flip, rotate, scale, wrap shift).
- Custom dither editor, gradients and luminance-distance, colour locking, "Hardware colour remap".
- Overlay reference image, animation frames with onion skin, 1:1 preview window, snap to grid.
- PRG / Paint Magic / BMP import (PRG needs a 6510 emulator), BMP/PNG/BIN export.
- ArcPaint's `I` hotkey for dither invert is not bound (`I` is the colour picker here).
- Packaging (macOS .app/dmg, Linux AppImage), CI. Linux has never been built or tested.

## Gotchas and decisions

- **Only the logical pixel grid exists.** Multicolor pixels are drawn 2x wide (`Mode::aspect_x`).
  ArcPaint's multicolor brush/dither tables use 8-bit rows with 2-bit pairs; we convert to 4 logical
  columns (centre x=1 for MC brushes, x=3 for hires).
- Dither semantics: a set cell paints the active colour, a clear cell leaves the pixel untouched.
  The solid pattern ignores Invert.
- Sorting the palette must never change colour numbers or image data (tested); only swatch order.
- The preview and the stroke logic share `hover_preview_points` / `put_all`; if you change how a
  stroke expands (brush, dither), update both or the preview will lie (a test guards this).
- Project file format is unversioned in practice (`RPNT` v1); don't break loading of saved files
  without a migration.
- Changing the palette combo was removed on request; the app uses `PALETTES[0]` (Pepto). Colodore
  is still in the core.

## Verification tips

- The user runs the app themselves from their terminal and reports back; I cannot click it
  (macOS accessibility is blocked, and synthetic clicks land in whichever window is frontmost, which
  once meant the Claude app; do not retry that).
- Screenshots of just the app window work: find the window id of the biggest window for the process
  with `CGWindowListCopyWindowInfo`, then `screencapture -x -o -l <id> out.png`. Helper Swift sources
  were in the old session scratchpad (`drv/wid.swift`); they are easy to recreate.
- Tests cover logic only; anything visual or mouse-driven has been checked by the user by hand.
- No Node on this machine, so the ArcPaint HTML can't be run or syntax-checked with it.

## Other work in this session (outside this repo)

The user also had ArcPaint64 modified to add an **x2** preview button next to the 1:1 Preview toggle
(`/Users/peter/Downloads/arcPaint64-v1.3.1/index.html`; original backed up in the old session
scratchpad as `index.html.bak`). That is separate from CursedPaint.
