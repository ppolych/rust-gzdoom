# TODO

## Revalidated Checklist

- [x] Set up Rust workspace with core crates (`engine-core`, `wad`, `config`, `level`, `gameplay`, `input`, `render-api`, `render-vulkan`, `app`)
- [x] Implement WAD file parsing and basic resource loading
- [x] Implement Doom map format parsing (`VERTEXES`, `LINEDEFS`, `SIDEDEFS`, `SECTORS`, `SEGS`, `SSECTORS`, `NODES`, `THINGS`)
- [x] Create level data structures and basic validation
- [x] Implement fixed-tick game loop with basic player movement
- [x] Implement Vulkan renderer with swapchain, render pass, frame buffering, and depth support
- [x] Integrate rendering with game loop in main application
- [x] Add proper texture loading and management
- [x] Implement camera system and GPU-side projection handling
- [x] Add real input handling (keyboard and left mouse fire)
- [~] Implement wall/flat/sprite rendering in Vulkan backend
- [~] Implement Doom-like visibility and occlusion behavior
- [~] Implement gameplay loop (minimal weapon, shooting, one simple enemy)
- [~] Complete first-pixel Vulkan bring-up and restore the intended textured shader path

## Current Blocking Gaps

1. Make any visible pixel appear on a real Wayland/Vulkan run using the forced debug triangle path.
2. Restore the intended textured shader path after the forced debug path is confirmed visible.
3. Improve portal/opening-aware visibility beyond the current angular clipper and coarse eye-height checks.
4. Tighten sprite clipping against visible wall openings.
5. Runtime-validate on a real Wayland session.
