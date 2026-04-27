# Next Steps

## Immediate Technical Tasks

1. Confirm the forced debug triangle appears on a real Wayland/Vulkan run and capture the new runtime prints from `render-vulkan`.
2. Verify viewport/scissor, `cmd_draw` execution, and swapchain presentation on the live machine.
3. Once the forced debug triangle appears, restore the textured shader path and confirm the debug quad/wall are visible.
4. After the textured debug path works, restore actual scene geometry and confirm visible walls/flats/sprites.
5. Only then resume Doom-correct visibility and collision work.

## Short Follow-Up Tasks

1. Add mouse look and thread it through `input::InputState`.
2. Add actor-actor collision and stop rebuilding temporary actor vectors in update paths.
3. Remove or archive `render-vulkan/src/wall_rendering.rs` so the repo no longer carries a dead renderer path.
4. Add tests for visibility traversal, triangulation, and deterministic gameplay updates.
5. Expand the shared opening model to cover more precise sprite clipping and vertical portal reasoning after first-pixel bring-up is solved.
