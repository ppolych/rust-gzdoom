# Roadmap

## Current Phase

Late Phase 3 stalled in final first-pixel bring-up, with Phase 4 gameplay/scene work waiting behind it.

The project now has BSP/subsector-driven scene submission, opening-aware angular clipping groundwork, clearer Doom-style wall sections, and a minimal playable loop with shooting and a basic enemy, but the active renderer is temporarily forced into a debug bring-up path until first visible pixels are confirmed.

## Milestone 1: First Visible Pixels

- Confirm the forced NDC debug triangle appears on screen
- Confirm viewport/scissor, draw submission, and shader I/O are correct
- Restore the textured path only after the forced debug path is visible

## Milestone 2: Restore Textured Geometry

- Restore the intended SPIR-V textured path after first-pixel validation
- Make the debug path optional instead of active by default
- Confirm debug wall/world geometry is visible before trusting full scene output

## Milestone 3: Stronger Doom Visibility And Surfaces

- Improve the angular clipper into stronger opening-aware portal reasoning
- Reduce remaining sprite-through-opening and sprite-through-wall edge cases
- Improve subsector polygon reconstruction and reduce triangulation fallback cases
- Expand texture pegging/alignment toward fuller Doom correctness
- Improve masked midtexture behavior

## Milestone 4: Strengthen The Playable Loop

- Improve vertical opening checks for hitscan and collision on two-sided openings
- Add mouse look
- Improve enemy placement, reaction, and rendering correctness

## Milestone 5: Runtime Validation And Hardening

- Runtime-test on an actual Wayland session with Doom/Freedoom IWADs
- Add missing crate metadata: `repository`, `homepage`, `documentation`
- Add focused tests in `wad/`, `level/`, `gameplay/`, and visibility code
