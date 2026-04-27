# Handoff: Rust GZDoom Implementation Details

## Status Summary
The engine has successfully achieved **"First Textured Pixels"** and is now capable of rendering a recognizable, textured E1M1. Movement (keyboard) and turning (mouse) are fully functional. The project is currently transitioning from a monolithic actor system to a deterministic ECS architecture. The `gameplay` crate build has been restored.

## Current Architecture
- **`render-vulkan`**: High-performance Vulkan backend using `ash`.
    - **Stable Path**: Currently using **2D SPIR-V shaders** (`tex.vert.spv`, `tex.frag.spv`).
    - **CPU Projection**: Performs manual 3D-to-2D projection in `render_scene` to bypass Naga's current GLSL/WGSL parsing limitations.
    - **Painter's Algorithm**: Renders opaque batches back-to-front to handle layering without a hardware depth buffer.
    - **Aspect Ratio**: Implements a `1.2x` vertical stretch to match Doom's original 320x200 (stretched to 4:3) presentation.
- **`fixed-point`**: New crate providing deterministic `Fixed` point math (i64 with 16-bit fractional part).
- **`gameplay`**: Transitioning to `hecs` ECS.
    - **Actor**: Refactored to use `[Fixed; 2]` for positions/velocity.
    - **WorldState**: Basic `hecs::World` integration started with `MonsterAI` components and an `update_ai_systems` stub.
- **`app`**: Main entry point; handles WAD loading, BSP traversal, and input (including mouse look).

## Recent Breakthroughs
- **Binding Conflict Fixed**: Resolved a critical bug where textures were colliding with uniform buffers at Binding 0, causing visual noise.
- **Painter's Algorithm**: Fixed world geometry overlapping issues by reversing the BSP traversal order for the opaque pass.
- **Vertical Stretching**: Adjusted FOV projection to fix the "squashed" appearance of the world.
- **Build Restored**: Fixed brace imbalance and ECS query pattern errors in `gameplay/src/lib.rs`.

## Pending Work / Known Issues
1. **ECS Migration**: Monster AI (`update_ai_systems`) is partially drafted in ECS; the logic needs to be fully ported from the old `Actor::think` method and integrated into the main loop.
2. **Sector Logic (Phase 3)**:
    - Implement `Thinkers` for doors and elevators.
    - Add sector lighting interpolation (flickering, blinking).
3. **Audio (Phase 5)**: Integrate `cpal` to load and play `DS` lumps from the WAD.

## Immediate Next Steps for Codex/Claude
1. **Complete ECS Monster AI**: Finish the `update_ai_systems` logic to move monsters toward the player using the `Position` and `Velocity` components.
2. **Integrate AI into Game Loop**: Call `update_ai_systems` from `engine-core` or `app`.
3. **Sector Actions**: Implement the first door-opening trigger using the `level` crate's line-activation logic.
4. **Hardware Clipping**: Consider porting a more advanced clipper if performance becomes a bottleneck.

## Verification
- Run with `./app --wad-path doom.wad --map E1M1`.
- Verify visuals against `2.png` (latest stable render).
- Check console output for hitscan damage logs ("Hit actor X! HP left: Y").
