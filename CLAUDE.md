# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust-based reimplementation of GZDoom (the classic Doom source port), targeting Wayland + Vulkan on Linux. The engine loads original Doom WAD files and renders classic maps with modern graphics APIs. Currently a pre-playable proof-of-concept with textured rendering and basic gameplay loop.

## Build & Run Commands

All builds must be done offline (dependencies are locally cached):

```bash
# Build entire workspace
cargo build --workspace --offline

# Run the game (requires a real Wayland compositor and a Doom IWAD)
cargo run -p app -- --wad-path doom.wad --map E1M1

# Run all tests (currently only doc/unit tests; no formal test suite exists)
cargo test --workspace --offline

# Check compilation without building
cargo check --workspace --offline

# Minimal Wayland window smoke-test
cargo run -p wayland-bootstrap
```

## Workspace Structure

11 crates in a layered dependency graph:

```
app (binary)
├── engine-core    — fixed-tick game loop + Game state orchestration
│   └── gameplay   — Actor/AI system (migrating to hecs ECS)
├── wad            — WAD archive parser (palettes, textures, patches)
├── level          — Doom map format parser + BSP tree
├── render-vulkan  — Wayland/Vulkan backend (~1500 lines)
│   └── render-api — renderer trait + scene data structures (backend-agnostic)
├── input          — InputState abstraction (keyboard + mouse)
├── fixed-point    — deterministic i64 fixed-point math (16-bit fractional)
└── config         — cvar system (skeleton only)
```

## Key Architecture Decisions

**Fixed-point math (`fixed-point` crate)**: Actor positions and velocities use `i64` with a 16-bit fractional part, matching classic Doom's representation for determinism. Floating-point is used only in the renderer.

**Trait-based renderer**: `render-api` defines a `Renderer` trait and `RenderScene` type. `render-vulkan` implements it. This decouples game logic from graphics backend.

**RenderScene submission model**: The app collects `FlatTriangle`, `WallQuad`, and `Sprite` primitives separately, then submits them in a single `RenderScene`. The Vulkan backend processes them in painter's algorithm order (flats → walls → sprites).

**CPU-side 3D→2D projection**: Vertex projection happens on the CPU in `app/src/main.rs` rather than in shaders. This was chosen to work around Naga shader compilation limitations and allows rapid iteration.

**Fixed-tick loop**: `engine-core` uses an accumulator pattern — real time accumulates and the game ticks at a fixed rate. Rendering happens every frame with interpolation data.

**ECS migration in progress**: `gameplay` is transitioning from a monolithic `Actor` struct to `hecs` ECS. The migration is partial — new monster AI work should use ECS components.

**Visibility**: BSP traversal in `app/src/main.rs` drives scene submission. Sector/linedef "openings" are the shared model for visibility, collision, and hitscan — but this is still approximate, not portal-perfect.

**Aspect ratio**: The renderer applies 1.2× vertical stretch to match classic Doom's intended 4:3 look on the original hardware pixels.

## Rendering Pipeline (render-vulkan)

- Dual render pass: opaque geometry (depth test enabled) then alpha-blended sprites
- Texture atlas: all game textures packed into a single GPU atlas; descriptor sets for sampling
- Shaders: SPIR-V blobs in `render-vulkan/shaders/`, compiled from GLSL via Naga
- Swapchain management handles resize and recreation

## Current State

**Working**: WAD/map loading, BSP traversal, textured wall/flat/sprite rendering, player movement + mouse-look, basic hitscan combat, monster chase/attack AI, actor-actor collision.

**Approximate/partial**: Texture pegging alignment, sprite clipping, masked textures, vertical opening checks.

**Not yet implemented**: Sector actions (doors/elevators), audio, full portal-correct visibility, formal test suite.

Validation currently relies on visual inspection against reference screenshots (`1.png`, `2.png`) and console output (e.g., `"Hit actor X! HP left: Y"`). No automated test infrastructure exists.

## Planning Documents

- `PROJECT_STATE.md` — rendering/gameplay checklist as of 2026-04-15
- `ROADMAP.md` — 5-milestone roadmap (Pixels → Textures → Visibility → Gameplay → Validation)
- `TODO.md` — comprehensive task checklist with current blocking gaps
- `HANDOFF_TO_CODEX.md` — recent breakthroughs and immediate next steps
