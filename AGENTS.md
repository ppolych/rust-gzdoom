# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust workspace for a Linux-first, Wayland-first, Vulkan-only Doom/GZDoom-style engine port. `wad/` handles archive parsing and texture resources, `level/` parses Doom map lumps, `gameplay/` contains actor logic, `input/` holds input state, `engine-core/` drives the fixed-tick loop, `render-api/` defines the render scene boundary and renderer trait, `render-vulkan/` is the active ash-based backend, `app/` assembles the runtime world and render scene, and `wayland-bootstrap/` is a minimal platform smoke test.

## Build, Test, and Development Commands
- `cargo build --workspace --offline`: build all crates without network access.
- `cargo test --workspace --offline`: run all unit and doc tests.
- `cargo run -p app -- --wad-path doom.wad --map E1M1`: load a WAD and run the current prototype.
- `cargo run -p wayland-bootstrap`: test Wayland/Vulkan bootstrap separately.
- `cargo package --workspace --allow-dirty --no-verify --offline`: verify local packaging.

## Coding Style & Naming Conventions
Keep the code `cargo fmt`-clean. Use `snake_case` for functions/modules and `CamelCase` for types. Keep parsing in `wad/` and `level/`, simulation in `gameplay/` and `engine-core/`, and Vulkan/platform work in `render-vulkan/`. Extend the existing renderer incrementally; do not add parallel render paths.

## Testing Guidelines
Add crate-local `#[cfg(test)]` modules beside the code they cover. Prioritize parsing tests in `wad/` and `level/`, and deterministic simulation tests in `gameplay/` and `engine-core/`. There is still no runtime rendering test harness, so renderer claims should be limited to build/test verification unless a Wayland run was actually checked.

## Commit & Pull Request Guidelines
Use short imperative commit subjects such as `render-api: add render scene` or `render-vulkan: enable sprite blending`. Keep PRs focused, list the exact commands you ran, and say whether the result was build-tested only or runtime-tested on a Wayland session.

## Current Implementation Notes
`app/` builds a `RenderScene`, and `render-vulkan/` contains the live ash renderer. During the current black-screen bring-up, the active startup path is temporarily forced into a debug pipeline that renders hardcoded NDC geometry with solid-color shaders and runtime draw logging. The intended textured wall/flat/sprite path still exists in the same backend but is not yet trusted as the first visible-pixel path. `render-vulkan/src/wall_rendering.rs` is still dead placeholder code and should not be treated as the live path.

## Packaging Notes
The workspace builds, tests, and packages offline. `cargo package` still warns about missing `documentation`, `homepage`, and `repository` metadata across crates.
