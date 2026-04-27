# Handoff

## What Was Improved In This Pass

- Added opening-aware angular clipping groundwork to app-side BSP traversal
- Visibility now tracks visible segs as well as visible subsectors, and solid walls contribute occlusion while portals do not
- Sprite submission now uses the same coarse occlusion state plus shared line-of-sight rejection instead of bypassing visibility entirely
- Flat generation now stays on the subsector path and uses cleaned polygons with ear-clipping-style triangulation before falling back
- Wall generation remains RenderScene-driven but now carries clearer Doom semantics for upper/lower/solid-middle/masked-middle sections
- Added shared opening helpers and line-trace helpers in `level/` so visibility, hitscan, movement, and sprite rejection interpret walls through the same boundary
- Added a minimal playable loop: keyboard/mouse fire input, opening-aware hitscan, simple enemy chase/attack/death, and fallback enemy spawn
- Added a forced first-pixel bring-up mode in `render-vulkan`: NDC debug triangle/quad/wall, depth disabled, solid-color shaders, and runtime prints for vertex layout, vertex counts, viewport size, and `cmd_draw`
- Confirmed the active black-screen work is now isolated to the final live renderer path rather than WAD/map/gameplay setup

## What Remains Approximate

- Angular clipping is a real structural step, but not full classic Doom occlusion
- Portal/opening reasoning is still coarse, especially vertically and around portal transitions
- Sprite clipping is still only coarse rejection, not exact clipping
- Masked middle textures are cleaner structurally but not final Doom behavior
- The gameplay loop is intentionally minimal and still prototype-grade
- Runtime Wayland/Vulkan validation was not possible in this environment
- First visible pixels are not yet confirmed; the live renderer is still in a black-screen bring-up state

## Packaging Reality

- The old `wad::Archive` package verification failure remains fixed
- `cargo package -p wad --allow-dirty --offline` succeeds
- `cargo package -p level --allow-dirty --offline` still fails only because uncached crates.io data is unavailable offline
- `cargo package --workspace --allow-dirty --no-verify --offline` succeeds

## Verified Commands

- `cargo build --workspace --offline`
- `cargo test --workspace --offline`
- `cargo package -p wad --allow-dirty --offline`
- `cargo package -p level --allow-dirty --offline`
- `cargo package --workspace --allow-dirty --no-verify --offline`
