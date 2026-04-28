# UZDoom Parity TODO

This file tracks what remains to make this Rust Doom/GZDoom-like renderer and runtime work reliably and move toward UZDoom-style behavior.

Scope constraints for this project:

- Rust only.
- Vulkan only.
- Linux / Wayland only.
- Keep the current renderer architecture.
- Keep BSP clipping, lighting, and geometry systems incremental.
- Use local UZDoom source as a behavioral reference, not as copied code.

## Current Baseline

The current project has a functional Rust workspace, WAD loading, classic binary Doom map parsing, BSP leaf polygon clipping for flats, wall section generation, basic sprites, a Vulkan renderer, dynamic point lighting, renderer debug modes, and a minimal gameplay loop.

The project is not yet UZDoom-like. It still needs substantial work in rendering correctness, Doom compatibility, resource handling, map format support, gameplay simulation, presentation, diagnostics, and validation.

## Critical Next Priorities

- [ ] Runtime-check all new renderer debug modes on a real Wayland/Vulkan session.
- [ ] Capture screenshots for `solid`, `normals`, `uv`, `light-only`, `texture-only`, and `lit` modes.
- [ ] Use `uv` mode to confirm wall U is continuous across split segs.
- [ ] Use `texture-only` mode to isolate texture sampling from lighting.
- [ ] Use `normals` mode to confirm floors, ceilings, and walls have stable normals.
- [ ] Confirm no missing walls or flats in E1M1 after recent UV changes.
- [ ] Confirm no black voids remain from missing flats in common IWAD maps.
- [ ] Confirm texture repeat is working for UVs outside `[0, 1]`.
- [ ] Confirm the Vulkan sampler uses repeat addressing for all texture axes.
- [ ] Confirm texture-only output is not modulated by vertex color or lighting.
- [ ] Confirm light-only output does not sample textures.
- [ ] Confirm solid output does not sample textures.
- [ ] Confirm dynamic lighting still works after UV changes.
- [ ] Record the current visual state with known-good screenshots.

## Rendering: Wall UV And Texture Alignment

- [x] Use flat world-space UVs based on map coordinates.
- [x] Avoid local polygon UVs for flats.
- [x] Avoid normalizing UVs to `[0, 1]`.
- [x] Use linedef-relative wall U basis instead of per-seg reset.
- [x] Project seg endpoints onto the parent linedef direction.
- [x] Preserve reversed seg U direction naturally.
- [ ] Compare current wall UVs against UZDoom `hw_walls.cpp` behavior on several E1M1 walls.
- [ ] Confirm sidedef X offsets match classic Doom expectations.
- [ ] Confirm sidedef Y offsets match classic Doom expectations.
- [ ] Confirm upper texture alignment on doors.
- [ ] Confirm lower texture alignment on lifts and stairs.
- [ ] Confirm middle texture alignment on one-sided walls.
- [ ] Confirm masked middle texture alignment on windows and grates.
- [ ] Add exact upper unpegged behavior.
- [ ] Add exact lower unpegged behavior.
- [ ] Add exact middle texture pegging behavior.
- [ ] Handle `ML_DONTPEGTOP`.
- [ ] Handle `ML_DONTPEGBOTTOM`.
- [ ] Handle upper/lower pegging with neighboring sector heights.
- [ ] Handle one-sided middle textures with correct ceiling/floor anchors.
- [ ] Handle two-sided masked middle textures with correct top/bottom anchors.
- [ ] Handle wall texture clipping without distorting UVs.
- [ ] Handle negative sidedef offsets.
- [ ] Handle very large sidedef offsets.
- [ ] Handle segs whose endpoints extend beyond linedef due node splitting.
- [ ] Handle non-axis-aligned walls.
- [ ] Handle very short walls without UV explosions.
- [ ] Handle zero-length linedefs safely.
- [ ] Handle unknown wall texture dimensions with `64x64` fallback.
- [ ] Track wall texture size fallback counts per texture name, not only total count.
- [ ] Add a debug dump for one selected linedef's UV inputs and outputs.
- [ ] Add unit tests for reversed seg U coordinates.
- [ ] Add unit tests for negative sidedef X offsets.
- [ ] Add unit tests for sidedef Y offsets.
- [ ] Add unit tests for upper wall pegging.
- [ ] Add unit tests for lower wall pegging.
- [ ] Add unit tests for masked middle texture placement.
- [ ] Add regression maps or fixtures for split linedefs.
- [ ] Add regression fixtures for reversed segs.
- [ ] Add visual comparison screenshots for common wall cases.

## Rendering: Flat UVs And Plane Texturing

- [x] Use world-space flat mapping.
- [x] Use repeated UVs instead of clamping.
- [x] Use `flat_u = world_x / 64.0`.
- [x] Use `flat_v = -world_z / 64.0`.
- [ ] Verify floor orientation against UZDoom `hw_vertexbuilder.cpp`.
- [ ] Verify ceiling orientation against UZDoom `hw_vertexbuilder.cpp`.
- [ ] Confirm floor/ceiling texture continuity across subsector borders.
- [ ] Confirm no seams from BSP clipped polygon boundaries.
- [ ] Confirm flat UVs do not move with camera.
- [ ] Confirm flat UVs behave on negative world coordinates.
- [ ] Confirm flat UVs behave on large world coordinates.
- [ ] Support actual non-64 flat dimensions if/when supported by resource format.
- [ ] Support flat panning metadata when map format supports it.
- [ ] Support flat scaling metadata when map format supports it.
- [ ] Support flat rotation metadata when map format supports it.
- [ ] Support UDMF plane offsets.
- [ ] Support UDMF plane scaling.
- [ ] Support UDMF plane rotation.
- [ ] Support sloped plane UV behavior.
- [ ] Support sky flat behavior separately from normal ceiling flat behavior.
- [ ] Add visual checker for flat UV continuity.
- [ ] Add unit tests for negative-coordinate flat UVs.
- [ ] Add unit tests for cross-subsector flat UV continuity.

## Rendering: BSP, Subsector Polygons, And Flats

- [x] Replace seg-loop floor reconstruction with BSP leaf polygon clipping.
- [x] Generate floors/ceilings for subsector leaf polygons.
- [x] Avoid requiring subsector segs to form closed loops.
- [ ] Validate BSP leaf polygons against UZDoom or a known node-builder result.
- [ ] Verify leaf polygon clipping with malformed or unusual nodes.
- [ ] Verify clipping with maps containing very large coordinates.
- [ ] Verify clipping with maps containing tiny sliver subsectors.
- [ ] Improve degenerate polygon rejection diagnostics.
- [ ] Track skipped polygons by subsector index.
- [ ] Add optional wireframe overlay for leaf polygons.
- [ ] Add optional subsector ID color visualization.
- [ ] Add optional sector ID color visualization.
- [ ] Add optional overdraw visualization.
- [ ] Avoid overfilling outside actual sector boundaries.
- [ ] Investigate whether BSP leaf clipping should be constrained by map lines after node clipping.
- [ ] Improve handling of subsectors whose inferred sector is wrong.
- [ ] Derive subsector sector from authoritative seg/sidedef data consistently.
- [ ] Add tests for node clipping front/back semantics.
- [ ] Add tests for leaf assignment order.
- [ ] Add tests for maps with no nodes.
- [ ] Add tests for maps with GL nodes or ZNODES once parsed.

## Rendering: Wall Geometry

- [x] Split wall submission into upper, lower, middle solid, and masked middle sections.
- [ ] Verify upper wall generation against front/back ceiling heights.
- [ ] Verify lower wall generation against front/back floor heights.
- [ ] Verify one-sided middle walls span floor-to-ceiling.
- [ ] Verify masked middle walls only span the open window area when needed.
- [ ] Handle closed two-sided walls correctly.
- [ ] Handle self-referencing sectors.
- [ ] Handle missing top/bottom textures gracefully.
- [ ] Handle "no texture" marker `-` consistently.
- [ ] Avoid generating invisible zero-height walls.
- [ ] Avoid generating walls with missing sectors.
- [ ] Add diagnostics for missing wall textures by texture name.
- [ ] Add wall section counters by section kind.
- [ ] Add debug draw colors by wall section kind.
- [ ] Fix exact wall normal direction per linedef side.
- [ ] Ensure wall normals face the visible/front side.
- [ ] Validate wall culling behavior if backface culling is ever re-enabled.
- [ ] Support sloped wall top/bottom edges when slopes exist.
- [ ] Support sky wall suppression rules.
- [ ] Support horizon lines and special sky behavior.

## Rendering: Masked Textures And Transparency

- [x] Route masked middle textures through an alpha-capable path.
- [ ] Confirm texture alpha is decoded correctly from patch transparency.
- [ ] Confirm masked wall sorting against opaque walls.
- [ ] Confirm masked wall sorting against sprites.
- [ ] Confirm masked wall depth writes are correct.
- [ ] Confirm alpha pass does not break lighting.
- [ ] Implement proper masked midtexture clipping to floor/ceiling openings.
- [ ] Implement Doom-style midtexture vertical placement.
- [ ] Handle two-sided line translucency flags if present.
- [ ] Handle additive translucent surfaces later if supported.
- [ ] Handle grates and fences without sorting artifacts.
- [ ] Add debug mode that renders alpha geometry in a distinct color.
- [ ] Add counters for opaque wall draws vs alpha wall draws.
- [ ] Avoid per-frame draw spam unless trace mode is enabled.

## Rendering: Sprites

- [ ] Implement full Doom sprite frame selection.
- [ ] Implement sprite rotation selection based on camera angle.
- [ ] Implement mirrored sprite frames.
- [ ] Implement sprite offsets using patch left/top offsets.
- [ ] Implement correct sprite origin and vertical placement.
- [ ] Implement weapon sprites separately from world sprites.
- [ ] Implement full actor sprite state animation.
- [ ] Implement sprite clipping against solid walls.
- [ ] Implement sprite clipping against two-sided openings.
- [ ] Implement sprite vertical clipping against floors/ceilings.
- [ ] Implement sprite sorting with masked midtextures.
- [ ] Implement sprite lighting from sector light level.
- [ ] Integrate dynamic lights with sprites.
- [ ] Handle partially transparent sprite pixels.
- [ ] Handle fullbright sprite frames.
- [ ] Handle corpse sprites and death states.
- [ ] Handle pickup sprites.
- [ ] Handle projectile sprites.
- [ ] Add sprite debug bounding boxes.
- [ ] Add sprite debug origin markers.
- [ ] Add regression screenshots for enemies and items.

## Rendering: Sky

- [ ] Replace `F_SKY1` flat fallback with real sky rendering.
- [ ] Implement classic Doom sky texture selection.
- [ ] Implement sky projection behavior.
- [ ] Implement sky ceiling behavior.
- [ ] Implement sky floor behavior where relevant.
- [ ] Implement sky wall behavior between sky sectors.
- [ ] Implement "do not draw sky walls" style behavior where supported.
- [ ] Implement sky transfer linedefs.
- [ ] Implement double sky if supported later.
- [ ] Implement tiled sky vs stretched sky modes.
- [ ] Parse MAPINFO sky definitions if supported later.
- [ ] Parse UDMF sky settings if supported later.
- [ ] Avoid z-fighting between sky and ceiling fallback geometry.
- [ ] Add sky debug mode.
- [ ] Add screenshots comparing sky behavior with UZDoom.

## Rendering: Lighting

- [x] Add vertex normals.
- [x] Add point light uniform buffer.
- [x] Add simple forward dynamic lighting.
- [x] Add ambient fallback so maps stay readable.
- [x] Add light-only debug mode.
- [ ] Implement classic Doom sector light behavior more accurately.
- [ ] Apply sector light level to walls, flats, and sprites consistently.
- [ ] Support light diminishing with distance like Doom software renderer if desired.
- [ ] Support extra light levels.
- [ ] Support fullbright textures/sprites.
- [ ] Support glowing flats or emissive textures later.
- [ ] Extract simple lights from known Doom things.
- [ ] Extract lights from bright sectors as an optional mode.
- [ ] Add debug visualization for point light positions.
- [ ] Add light radius visualization.
- [ ] Add per-surface light contribution debug mode.
- [ ] Add lighting toggle in config or CLI.
- [ ] Avoid saturating textures too aggressively.
- [ ] Add gamma/brightness controls.
- [ ] Add color correction configuration.
- [ ] Support sector color maps later.
- [ ] Support fog/fade tables later.
- [ ] Support UZDoom-style dynamic light definitions later.
- [ ] Add tests for light uniform packing.
- [ ] Add shader validation in build or CI.

## Rendering: Vulkan Pipeline

- [x] Use Vulkan only.
- [x] Use Wayland window creation only.
- [x] Use depth test/write for opaque map geometry.
- [x] Use repeat sampler addressing.
- [x] Add debug modes in shader.
- [ ] Remove stale forced-debug bring-up code if no longer needed.
- [ ] Remove stale inline shader strings if SPIR-V source is authoritative.
- [ ] Decide on shader source/SPIR-V generation workflow.
- [ ] Add a script to compile GLSL to SPIR-V.
- [ ] Add shader validation command to docs.
- [ ] Add CI/test check that SPIR-V matches shader sources if possible.
- [ ] Add validation layer support in debug builds.
- [ ] Add optional Vulkan debug messenger.
- [ ] Improve swapchain recreation behavior.
- [ ] Handle minimize/resize edge cases.
- [ ] Track GPU memory allocations.
- [ ] Avoid recreating too much state unnecessarily.
- [ ] Add per-frame descriptor/resource lifecycle checks.
- [ ] Add staging buffer reuse.
- [ ] Add texture upload batching.
- [ ] Add mipmapping if wanted.
- [ ] Add anisotropic filtering if supported and desired.
- [ ] Add nearest/linear filter toggle.
- [ ] Add palette-preserving texture path if needed for classic look.
- [ ] Add renderdoc-friendly labels if debug utils are available.
- [ ] Add a screenshot capture path for regression testing.

## Rendering: Vertex Layout And Debugging

- [x] Add position, UV, color, world position, and normal to render vertices.
- [x] Assert vertex size and offsets.
- [x] Add solid debug mode.
- [x] Add normal visualization mode.
- [x] Add UV visualization mode.
- [x] Add light-only mode.
- [x] Add texture-only mode.
- [ ] Add vertex attribute layout unit/integration check.
- [ ] Add startup print for active vertex layout only in trace mode.
- [ ] Add option to dump first N vertices of a selected draw call.
- [ ] Add option to dump draw calls grouped by texture.
- [ ] Add option to isolate one texture name.
- [ ] Add option to isolate one wall section kind.
- [ ] Add option to isolate flats only.
- [ ] Add option to isolate walls only.
- [ ] Add option to isolate sprites only.
- [ ] Add option to freeze camera and frame submission for debugging.
- [ ] Add camera coordinate debug output.

## Rendering: Visibility And Clipping

- [ ] Implement more accurate Doom BSP traversal.
- [ ] Implement proper portal/solid wall visibility semantics.
- [ ] Improve angular clipper precision.
- [ ] Handle partially open two-sided walls.
- [ ] Handle vertical visibility through windows and doors.
- [ ] Handle occlusion from one-sided solid walls robustly.
- [ ] Handle occlusion from closed two-sided sectors.
- [ ] Avoid over-culling masked midtextures.
- [ ] Avoid under-culling sprites through walls.
- [ ] Implement wall/sprite clipping closer to Doom drawsegs.
- [ ] Track visible subsector count by traversal.
- [ ] Add visibility debug overlay.
- [ ] Add line-of-sight debug traces.
- [ ] Add tests for solid wall occlusion.
- [ ] Add tests for open portal visibility.
- [ ] Add tests for window visibility.
- [ ] Add tests for door visibility during movement.

## WAD And Resource Loading

- [x] Parse WAD directory and lumps.
- [x] Load palette.
- [x] Load PNAMES/TEXTURE1/TEXTURE2.
- [x] Load patch-based textures.
- [x] Load flats.
- [x] Load sprites from sprite ranges.
- [ ] Support PWAD overlay order correctly.
- [ ] Support multiple WAD files.
- [ ] Support IWAD + PWAD command line.
- [ ] Support Doom, Doom II, Ultimate Doom, Freedoom naming differences.
- [ ] Improve namespace handling.
- [ ] Handle duplicate lump names correctly.
- [ ] Handle texture replacement correctly.
- [ ] Handle flat replacement correctly.
- [ ] Handle sprite replacement correctly.
- [ ] Parse ANIMATED lump.
- [ ] Parse SWITCHES lump.
- [ ] Apply animated wall textures.
- [ ] Apply animated flat textures.
- [ ] Apply switch texture changes during line use.
- [ ] Support PNAMES/TEXTURE edge cases.
- [ ] Support patches with unusual offsets.
- [ ] Preserve patch transparency.
- [ ] Add tests for patch decoding.
- [ ] Add tests for multi-patch texture composition.
- [ ] Add tests for transparent columns/posts.
- [ ] Add tests for duplicate texture names.
- [ ] Add tests for PWAD replacement ordering.

## Map Format Support

- [x] Parse classic Doom binary map lumps.
- [x] Parse THINGS, LINEDEFS, SIDEDEFS, VERTEXES, SEGS, SSECTORS, NODES, SECTORS.
- [ ] Support Doom II map names like `MAP01`.
- [ ] Support Hexen-format maps if desired.
- [ ] Support UDMF `TEXTMAP`.
- [ ] Support BEHAVIOR detection beyond reporting.
- [ ] Support ZNODES.
- [ ] Support GL nodes.
- [ ] Support extended node formats.
- [ ] Support BLOCKMAP.
- [ ] Support REJECT.
- [ ] Parse sidedef/sector extended fields from UDMF.
- [ ] Parse line specials beyond current classic subset.
- [ ] Parse thing flags by game/version.
- [ ] Parse skill flags.
- [ ] Parse multiplayer flags.
- [ ] Parse ambush/deaf flags.
- [ ] Parse Boom generalized linedefs.
- [ ] Parse Boom sector specials.
- [ ] Parse MBF/MBF21 extensions if in scope.
- [ ] Validate lump sizes and report malformed maps clearly.
- [ ] Add map format detection.
- [ ] Add map load diagnostics by format.
- [ ] Add tests for E1M1.
- [ ] Add tests for MAP01.
- [ ] Add tests for missing optional lumps.
- [ ] Add tests for malformed lump sizes.

## Gameplay: Player

- [x] Basic movement.
- [x] Basic turning.
- [x] Basic fire input.
- [ ] Implement Doom-accurate player radius and height.
- [ ] Implement acceleration/friction closer to Doom.
- [ ] Implement running/walking controls.
- [ ] Implement strafing correctly.
- [ ] Implement use action behavior.
- [ ] Implement weapon bob.
- [ ] Implement view bob.
- [ ] Implement pain flashes.
- [ ] Implement pickup flashes/messages.
- [ ] Implement status bar.
- [ ] Implement health/armor/ammo.
- [ ] Implement death state.
- [ ] Implement respawn/restart.
- [ ] Implement intermission.
- [ ] Implement automap.
- [ ] Implement save/load eventually.

## Gameplay: Weapons

- [x] Minimal hitscan weapon.
- [ ] Implement fist.
- [ ] Implement pistol.
- [ ] Implement shotgun.
- [ ] Implement chaingun.
- [ ] Implement rocket launcher.
- [ ] Implement plasma rifle.
- [ ] Implement BFG.
- [ ] Implement chainsaw.
- [ ] Implement weapon switching.
- [ ] Implement ammo consumption.
- [ ] Implement weapon pickup.
- [ ] Implement muzzle flash sprites/states.
- [ ] Implement projectile spawning.
- [ ] Implement splash damage.
- [ ] Implement bullet spread.
- [ ] Implement autoaim if desired.
- [ ] Implement weapon sound triggers.
- [ ] Implement weapon animation states.

## Gameplay: Actors And Monsters

- [x] Minimal enemy behavior.
- [x] Basic HP/death.
- [ ] Implement Doom actor state machine.
- [ ] Implement thing definitions.
- [ ] Implement monster wake-up behavior.
- [ ] Implement line-of-sight checks closer to Doom.
- [ ] Implement sound propagation.
- [ ] Implement monster pathing.
- [ ] Implement melee attacks.
- [ ] Implement ranged attacks.
- [ ] Implement projectiles.
- [ ] Implement pain chance.
- [ ] Implement corpse states.
- [ ] Implement item pickups.
- [ ] Implement barrels.
- [ ] Implement decorations.
- [ ] Implement teleport fog.
- [ ] Implement spawn filters by skill.
- [ ] Implement difficulty settings.
- [ ] Implement boss death triggers.
- [ ] Add actor collision tests.
- [ ] Add hitscan tests.
- [ ] Add projectile collision tests.

## Gameplay: Movement, Collision, And Physics

- [x] Basic wall crossing checks.
- [x] Actor-actor collision.
- [ ] Implement BLOCKMAP for efficient collision.
- [ ] Implement sliding along walls.
- [ ] Implement step-up behavior.
- [ ] Implement drop-off rules.
- [ ] Implement floor/ceiling crushing.
- [ ] Implement moving platform collision.
- [ ] Implement door collision.
- [ ] Implement projectile collision.
- [ ] Implement thing blocking flags.
- [ ] Implement pass-through flags where relevant.
- [ ] Implement teleporter movement.
- [ ] Implement sector damage.
- [ ] Implement friction/special sectors if supported.
- [ ] Improve vertical opening checks.
- [ ] Improve collision against two-sided lines.
- [ ] Add deterministic movement tests.

## Gameplay: Sectors And Line Specials

- [x] Some door/floor special groundwork.
- [ ] Implement common Doom door specials.
- [ ] Implement common Doom lift specials.
- [ ] Implement common Doom floor specials.
- [ ] Implement common Doom ceiling specials.
- [ ] Implement crushers.
- [ ] Implement stairs.
- [ ] Implement exits.
- [ ] Implement keys and locked doors.
- [ ] Implement switches.
- [ ] Implement walk-over triggers.
- [ ] Implement shoot triggers.
- [ ] Implement repeatable vs one-shot triggers.
- [ ] Implement tagged sector lookup robustly.
- [ ] Implement sector light changes.
- [ ] Implement secret sector counting.
- [ ] Implement damaging sectors.
- [ ] Implement teleporters.
- [ ] Implement scrolling floors/walls later if in scope.
- [ ] Implement Boom generalized actions later.
- [ ] Add tests for each implemented line special.
- [ ] Add tests for sector movement timing.

## Audio

- [ ] Load Doom sound lumps.
- [ ] Decode sound formats.
- [ ] Implement sound playback backend for Linux.
- [ ] Keep audio backend compatible with project constraints.
- [ ] Implement positional sounds.
- [ ] Implement sector sound propagation.
- [ ] Implement weapon sounds.
- [ ] Implement monster sounds.
- [ ] Implement item pickup sounds.
- [ ] Implement door/platform sounds.
- [ ] Implement music playback.
- [ ] Support MUS to MIDI or an alternative.
- [ ] Add volume controls.
- [ ] Add mute controls.

## Input And Platform

- [x] Wayland-first window creation.
- [x] Keyboard movement.
- [x] Mouse button firing.
- [ ] Improve raw mouse input.
- [ ] Add configurable key bindings.
- [ ] Add mouse sensitivity config.
- [ ] Add fullscreen support.
- [ ] Add window resize behavior.
- [ ] Add cursor grab/release behavior.
- [ ] Add pause/focus behavior.
- [ ] Add controller support only if wanted.
- [ ] Document Wayland-only platform assumptions.
- [ ] Avoid accidental X11/SDL fallback.

## UI, HUD, And Menus

- [ ] Implement status bar.
- [ ] Implement face widget.
- [ ] Implement health/armor/ammo display.
- [ ] Implement key display.
- [ ] Implement weapon display.
- [ ] Implement pickup messages.
- [ ] Implement menu framework.
- [ ] Implement options menu.
- [ ] Implement episode/map selection.
- [ ] Implement pause menu.
- [ ] Implement end-level tally.
- [ ] Implement automap.
- [ ] Implement console only if in scope.
- [ ] Implement debug overlay.

## UZDoom Feature Parity Areas

These are large feature families in UZDoom/GZDoom. They should be treated as long-term compatibility work, not short renderer fixes.

- [ ] DECORATE/ZScript actor definitions.
- [ ] MAPINFO.
- [ ] GLDEFS.
- [ ] ANIMDEFS.
- [ ] SNDINFO.
- [ ] LANGUAGE.
- [ ] SBARINFO or modern status bar definitions.
- [ ] UDMF maps.
- [ ] ACS/BEHAVIOR scripts.
- [ ] Polyobjects.
- [ ] Portals.
- [ ] 3D floors.
- [ ] Slopes.
- [ ] Dynamic lights from definitions.
- [ ] Model support.
- [ ] Particle systems.
- [ ] Advanced translucency.
- [ ] Material definitions.
- [ ] Brightmaps.
- [ ] Normal/specular maps if desired.
- [ ] Hardware light modes.
- [ ] Sector color/fog/fade.
- [ ] Compatibility flags.
- [ ] DeHackEd/BEX support.
- [ ] MBF/MBF21 support if desired.

## Diagnostics And Tooling

- [x] Startup render scene summary.
- [x] Texture UV mode summary.
- [x] Renderer debug modes.
- [ ] Add `--dump-scene-stats`.
- [ ] Add `--dump-visible-segs`.
- [ ] Add `--dump-textures`.
- [ ] Add `--dump-missing-textures`.
- [ ] Add `--dump-map-info`.
- [ ] Add `--debug-linedef <id>`.
- [ ] Add `--debug-sector <id>`.
- [ ] Add `--debug-subsector <id>`.
- [ ] Add screenshot capture.
- [ ] Add frame capture marker/log.
- [ ] Add deterministic camera start options.
- [ ] Add benchmark mode.
- [ ] Add validation mode that loads maps without opening a window.
- [ ] Add render smoke tests if headless rendering becomes available.
- [ ] Add asset/resource validation command.

## Testing

- [x] Workspace builds offline.
- [x] Workspace tests run offline.
- [ ] Add more WAD parser tests.
- [ ] Add more level parser tests.
- [ ] Add more wall UV tests.
- [ ] Add flat UV tests for negative coordinates.
- [ ] Add BSP clipping tests for more node layouts.
- [ ] Add visibility tests.
- [ ] Add collision tests.
- [ ] Add line special tests.
- [ ] Add actor behavior tests.
- [ ] Add renderer data packing tests.
- [ ] Add shader compilation/validation test or script.
- [ ] Add screenshot comparison tests eventually.
- [ ] Test Doom E1M1/E1M2/E1M3.
- [ ] Test Doom II MAP01/MAP02.
- [ ] Test Freedoom maps.
- [ ] Test simple custom maps built for UV and visibility regressions.
- [ ] Test maps with reversed segs.
- [ ] Test maps with split linedefs.
- [ ] Test maps with many transparent midtextures.
- [ ] Test maps with many doors/lifts.

## Documentation

- [ ] Update `README.md` with current run commands.
- [ ] Document `--render-debug-mode`.
- [ ] Document WAD path expectations.
- [ ] Document Linux/Wayland/Vulkan-only scope.
- [ ] Document required Vulkan tools for shader rebuilds.
- [ ] Document shader source/SPIR-V workflow.
- [ ] Document current supported map formats.
- [ ] Document unsupported UZDoom features.
- [ ] Document current renderer architecture.
- [ ] Document level geometry generation.
- [ ] Document texture coordinate rules.
- [ ] Document dynamic lighting limitations.
- [ ] Document debug workflows for UV issues.
- [ ] Document debug workflows for geometry holes.
- [ ] Document debug workflows for missing textures.

## Packaging And Repository Hygiene

- [ ] Decide whether IWAD/PWAD files should be tracked.
- [ ] Remove accidental WAD deletes/adds from git status if unintended.
- [ ] Add `.gitignore` entries for local WADs if appropriate.
- [ ] Add `.gitignore` entry for `target/` if missing.
- [ ] Add crate metadata: repository, homepage, documentation.
- [ ] Ensure `cargo package --workspace --allow-dirty --no-verify --offline` still works.
- [ ] Add CI once the repository is ready.
- [ ] Add formatting check to CI.
- [ ] Add test check to CI.
- [ ] Add shader validation to CI where tools are available.

## Suggested Milestones

### Milestone A: Visual Baseline Lock

- [ ] Capture debug-mode screenshots.
- [ ] Confirm geometry is correct in solid mode.
- [ ] Confirm normals are correct in normal mode.
- [ ] Confirm UV continuity in UV mode.
- [ ] Confirm texture sampling in texture-only mode.
- [ ] Confirm lighting in light-only mode.
- [ ] Fix any remaining vertex attribute or shader mismatch.

### Milestone B: Doom Texture Correctness

- [ ] Finish wall pegging rules.
- [ ] Finish wall offset behavior.
- [ ] Finish masked middle texture placement.
- [ ] Finish sky rendering.
- [ ] Add visual regression maps.

### Milestone C: Visibility Correctness

- [ ] Improve angular clipping.
- [ ] Improve portal visibility.
- [ ] Improve vertical opening clipping.
- [ ] Fix sprite/wall/masked clipping interactions.

### Milestone D: Playable Doom Loop

- [ ] Implement real weapon states.
- [ ] Implement core monster states.
- [ ] Implement core pickups.
- [ ] Implement common line specials.
- [ ] Implement HUD/status bar.
- [ ] Implement sound.

### Milestone E: UZDoom-Like Compatibility

- [ ] Add UDMF support.
- [ ] Add MAPINFO.
- [ ] Add ANIMDEFS/ANIMATED/SWITCHES support.
- [ ] Add DECORATE/ZScript or a scoped actor-definition subset.
- [ ] Add advanced sector/render features.

## Notes On "Like UZDoom"

UZDoom/GZDoom is not only a renderer. It includes decades of compatibility behavior, content definition languages, map format extensions, software and hardware renderer semantics, actor systems, script systems, audio, UI, menus, savegames, and compatibility flags.

For this project, "like UZDoom" should be approached in layers:

1. Make classic Doom IWAD maps render correctly.
2. Make classic Doom movement, collision, combat, and specials work.
3. Add debugging and tests so renderer changes do not regress.
4. Add Boom/MBF/UDMF compatibility only after the classic baseline is stable.
5. Add UZDoom/GZDoom content-definition features deliberately, one format at a time.

