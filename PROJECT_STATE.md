# Project State

## Verified As Of 2026-04-15

This workspace builds and tests offline. The active path remains `app -> RenderScene -> render-api -> render-vulkan`, and the project now has both a stronger Doom-style renderer foundation and a minimal first playable loop.

## Rendering

### Working
- WAD palette, wall texture, flat, and sprite patch decoding in `wad/`
- Shared opening/portal semantics in `level/` used by visibility, hitscan, movement traces, and coarse sprite rejection
- Vulkan swapchain, render pass, framebuffers, command recording, and texture upload in `render-vulkan/`
- BSP/front-to-back subsector traversal in `app/`
- Opening-aware angular clipping groundwork: solid walls consume angle intervals, portals pass through them
- Scene submission now uses visible subsectors and visible segs instead of near-whole-map submission
- Floor and ceiling generation from visible subsector polygons with ear-clipping-style triangulation and a fallback only when needed
- Wall submission split into upper, lower, solid middle, and masked middle sections
- Masked middle textures routed through the alpha path and excluded from solid occlusion
- Sprite submission filtered by BSP/angle occlusion plus shared line-of-sight rejection so sprites do not blindly render through solid walls
- The intended textured world renderer is now the active path (FORCE_DEBUG_TRIANGLE disabled)

### Partial
- Visibility is meaningfully better, but still approximate rather than full classic Doom portal/occlusion correctness
- Flat triangulation is materially better, but still depends on subsector polygon quality and still has fallback cases
- Texture pegging/alignment now has clearer groundwork and basic flag handling, but not all Doom rules are implemented
- Sprite clipping and placement are improved, but still not Doom-accurate
- Masked midtexture behavior is coherent groundwork, not final Doom masked rendering
- Two-sided wall openings now affect movement, hitscan, and sprite rejection, but vertical opening correctness is still coarse

### Broken / Missing
- First visible pixels are still not confirmed on a real runtime; the current blocker is in the final presentation/pipeline bring-up stage
- No full classic Doom vertical clipping/portal correctness yet
- No verified runtime result in this environment because there is no Wayland compositor here

## Gameplay

### Working
- Fixed-tick game loop
- Player movement and turning
- Mouse look (horizontal)
- Keyboard and left-mouse fire input
- One minimal hitscan weapon with cooldown
- Monster spawning from `THINGS`, with a fallback enemy spawn if the map has none
- Basic enemy chase/attack/death behavior
- Enemy HP and death state
- Hitscan damage against enemies with side-aware opening checks and shared line tracing
- Player and monster movement now consult shared opening traces before crossing linedef boundaries
- Actor-actor collision is implemented

## Packaging

- The previous `wad::Archive` tarball verification failure is fixed
- `cargo package -p wad --allow-dirty --offline` succeeds
- `cargo package --workspace --allow-dirty --no-verify --offline` succeeds
- `cargo package -p level --allow-dirty --offline` now only stops because the environment cannot fetch uncached crates.io data offline

## Validation

- `cargo build --workspace --offline`
- `cargo test --workspace --offline`
- `cargo package -p wad --allow-dirty --offline`
- `cargo package -p level --allow-dirty --offline`
- `cargo package --workspace --allow-dirty --no-verify --offline`
- `cargo run -p app -- --wad-path doom.wad --map E1M1` still cannot be runtime-validated here due missing Wayland compositor
