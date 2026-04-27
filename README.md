# rust-gzdoom

Rust workspace for an early Linux-first, Wayland-first, Vulkan-first Doom/GZDoom-style engine port.

## Current Reality

- `app/` loads an IWAD/PWAD, parses a Doom map, builds a render scene, and runs the fixed-tick loop
- `render-vulkan/` is the active renderer and now draws textured walls, flats, and sprites through a single Vulkan pipeline
- keyboard movement and turning are live
- monster spawning, simple AI, and basic hitscan combat are live

The project is still pre-playable. Rendering is partial, visibility is approximate, and runtime validation has not been completed in this environment because there is no Wayland compositor here.

## Workspace

- `wad/`: WAD archive parsing, palettes, patches, textures, flats
- `level/`: Doom map parsing (`VERTEXES`, `LINEDEFS`, `SIDEDEFS`, `SECTORS`, `SEGS`, `SSECTORS`, `NODES`, `THINGS`)
- `gameplay/`: actor state, movement, simple AI, combat
- `input/`: input state
- `engine-core/`: fixed-tick loop and game orchestration
- `render-api/`: renderer-facing scene and trait definitions
- `render-vulkan/`: Wayland/Vulkan renderer backend
- `app/`: main executable
- `wayland-bootstrap/`: minimal bootstrap binary

## Commands

- `cargo build --workspace --offline`
- `cargo test --workspace --offline`
- `cargo run -p app -- --wad-path doom.wad --map E1M1`
- `cargo run -p wayland-bootstrap`
- `cargo package --workspace --allow-dirty --no-verify --offline`

## Packaging Notes

The workspace packages offline, but manifests still need `repository`, `homepage`, and `documentation` metadata.
# rust-gzdoom
