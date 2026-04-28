use anyhow::Result;
use clap::{Parser, ValueEnum};
use engine_core::game::Game;
use engine_core::GameLoop;
use glam::DVec2;
use level::Level;
use render_api::{
    FlatTriangle, PointLight, RenderDebugMode, RenderScene, Renderer, Sprite, TextureImage,
    ViewState, WallQuad, WallSectionKind, MAX_POINT_LIGHTS,
};
use render_vulkan::VulkanRenderer;
use std::collections::{HashMap, HashSet};
use wad::Archive;
use winit::{
    event::{Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    platform::wayland::WindowBuilderExtWayland,
    window::WindowBuilder,
};

const SKY_FALLBACK_TEXTURE: &str = "__sky";
const DEBUG_SOLID_FLATS: bool = false;
const DEBUG_SCENE_STATS: bool = false;
const SUBSECTOR_VERTEX_EPSILON: f64 = 0.01;
const DYNAMIC_LIGHTING_ENABLED: bool = true;
const AMBIENT_STRENGTH: f32 = 0.35;
const SKY_TRANSFER_SPECIALS: &[u16] = &[271, 272];

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    wad_path: String,
    #[arg(short, long, default_value = "E1M1")]
    map: String,
    #[arg(long, value_enum, default_value_t = RenderDebugModeArg::Lit)]
    render_debug_mode: RenderDebugModeArg,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RenderDebugModeArg {
    Lit,
    Solid,
    Normals,
    Uv,
    LightOnly,
    TextureOnly,
}

impl From<RenderDebugModeArg> for RenderDebugMode {
    fn from(value: RenderDebugModeArg) -> Self {
        match value {
            RenderDebugModeArg::Lit => Self::Lit,
            RenderDebugModeArg::Solid => Self::Solid,
            RenderDebugModeArg::Normals => Self::Normals,
            RenderDebugModeArg::Uv => Self::Uv,
            RenderDebugModeArg::LightOnly => Self::LightOnly,
            RenderDebugModeArg::TextureOnly => Self::TextureOnly,
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("Rust GZDoom")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .with_name("rust-gzdoom", "")
        .build(&event_loop)?;

    let mut renderer = VulkanRenderer::new(&window)?;
    load_missing_texture(&mut renderer)?;
    load_sky_fallback_texture(&mut renderer)?;

    let archive = Archive::load_wad(&args.wad_path)?;
    println!("Loaded {} lumps", archive.lumps.len());

    let palette_data = archive.get_lump_data("PLAYPAL")?;
    let palette = wad::Palette::from_lump(palette_data);

    let (textures, pnames) = archive.load_textures()?;
    println!("Loading {} textures...", textures.len());
    let mut available_sprites = HashSet::new();
    let mut texture_sizes = HashMap::new();

    for tex in textures {
        let mut data = vec![0u8; tex.width as usize * tex.height as usize * 4];

        for tp in &tex.patches {
            let patch_name = &pnames[tp.patch_idx];
            if let Ok(patch) = archive.load_patch(patch_name) {
                for (x_rel, col) in patch.columns.iter().enumerate() {
                    let x = tp.origin_x as i32 + x_rel as i32;
                    if x < 0 || x >= tex.width as i32 {
                        continue;
                    }

                    for post in col {
                        for (y_rel, &pixel_idx) in post.pixels.iter().enumerate() {
                            let y = tp.origin_y as i32 + post.top_delta as i32 + y_rel as i32;
                            if y < 0 || y >= tex.height as i32 {
                                continue;
                            }

                            let color = palette.colors[pixel_idx as usize];
                            let idx = (y as usize * tex.width as usize + x as usize) * 4;
                            data[idx] = color[0];
                            data[idx + 1] = color[1];
                            data[idx + 2] = color[2];
                            data[idx + 3] = 255;
                        }
                    }
                }
            }
        }

        let image = TextureImage {
            width: tex.width as u32,
            height: tex.height as u32,
            data,
        };
        renderer.load_texture(&tex.name, &image)?;
        texture_sizes.insert(tex.name.clone(), (tex.width as f32, tex.height as f32));
    }

    let mut sprite_lumps = archive.find_lumps_in_range("S_START", "S_END");
    sprite_lumps.extend(archive.find_lumps_in_range("SS_START", "SS_END"));
    println!("Loading {} sprites...", sprite_lumps.len());
    for &idx in &sprite_lumps {
        let lump = &archive.lumps[idx];
        if let Ok(patch) = archive.load_patch(&lump.name) {
            let mut data = vec![0u8; patch.width as usize * patch.height as usize * 4];
            for (x, col) in patch.columns.iter().enumerate() {
                for post in col {
                    for (y_rel, &pixel_idx) in post.pixels.iter().enumerate() {
                        let y = post.top_delta as usize + y_rel;
                        if y < patch.height as usize {
                            let color = palette.colors[pixel_idx as usize];
                            let idx = (y * patch.width as usize + x) * 4;
                            data[idx] = color[0];
                            data[idx + 1] = color[1];
                            data[idx + 2] = color[2];
                            data[idx + 3] = 255;
                        }
                    }
                }
            }

            let image = TextureImage {
                width: patch.width as u32,
                height: patch.height as u32,
                data,
            };
            renderer.load_texture(&lump.name, &image)?;
            available_sprites.insert(lump.name.clone());
        }
    }

    let flats = archive.load_flats()?;
    println!("Loading {} flats...", flats.len());
    let mut available_flats = HashSet::new();
    for (name, data) in flats {
        let mut rgba_data = vec![0u8; 64 * 64 * 4];
        for (i, &pixel_idx) in data.iter().enumerate() {
            let color = palette.colors[pixel_idx as usize];
            rgba_data[i * 4] = color[0];
            rgba_data[i * 4 + 1] = color[1];
            rgba_data[i * 4 + 2] = color[2];
            rgba_data[i * 4 + 3] = 255;
        }
        let image = TextureImage {
            width: 64,
            height: 64,
            data: rgba_data,
        };
        renderer.load_texture(&name, &image)?;
        texture_sizes.insert(name.clone(), (64.0, 64.0));
        available_flats.insert(name);
    }
    available_flats.insert("__missing".to_string());
    available_flats.insert(SKY_FALLBACK_TEXTURE.to_string());
    texture_sizes.insert("__missing".to_string(), (16.0, 16.0));
    texture_sizes.insert(SKY_FALLBACK_TEXTURE.to_string(), (64.0, 64.0));

    let level = level::load_level(&archive, &args.map)?;
    let map_feature_stats = collect_map_feature_stats(&archive, &level);
    println!("Loaded level {}:", args.map);
    println!("  Vertices: {}", level.vertices.len());
    println!("  Sectors: {}", level.sectors.len());
    println!("  Sidedefs: {}", level.sidedefs.len());
    println!("  Linedefs: {}", level.linedefs.len());
    println!("  Segs: {}", level.segs.len());
    println!("  Subsectors: {}", level.subsectors.len());
    println!("  Nodes: {}", level.nodes.len());

    let mut game = Game::new(level);
    let render_debug_mode = RenderDebugMode::from(args.render_debug_mode);
    let (_, startup_stats) = build_render_scene_with_stats(
        &game.level,
        game.player.pos_to_dvec2(),
        game.player.angle,
        41.0,
        &game.actors,
        &available_sprites,
        &available_flats,
        &texture_sizes,
        render_debug_mode,
    );
    print_scene_summary(&args.map, &game.level, &startup_stats, &map_feature_stats);
    let mut loop_ = GameLoop::new(35.0);

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                elwt.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if let Err(e) = renderer.resize(size.width, size.height) {
                    eprintln!("Resize error: {}", e);
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key,
                                state,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let pressed = state.is_pressed();
                if let PhysicalKey::Code(code) = physical_key {
                    match code {
                        KeyCode::KeyW => game.input.forward = pressed,
                        KeyCode::KeyS => game.input.backward = pressed,
                        KeyCode::KeyA => game.input.left = pressed,
                        KeyCode::KeyD => game.input.right = pressed,
                        KeyCode::ArrowLeft => game.input.turn_left = pressed,
                        KeyCode::ArrowRight => game.input.turn_right = pressed,
                        KeyCode::Space => game.input.fire = pressed,
                        KeyCode::KeyE => game.input.use_action = pressed,
                        _ => {}
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                if button == MouseButton::Left {
                    game.input.fire = state.is_pressed();
                }
            }
            Event::DeviceEvent {
                event: winit::event::DeviceEvent::MouseMotion { delta },
                ..
            } => {
                game.input.mouse_delta_x += delta.0;
            }
            Event::AboutToWait => {
                if let Err(e) = renderer.begin_frame() {
                    eprintln!("Render error: {}", e);
                }

                let view = ViewState {
                    position: game.player.pos_to_dvec2(),
                    angle: game.player.angle,
                    eye_height: 41.0,
                    fov_y_radians: std::f32::consts::FRAC_PI_2,
                };
                let (scene, scene_stats) = build_render_scene_with_stats(
                    &game.level,
                    game.player.pos_to_dvec2(),
                    game.player.angle,
                    41.0,
                    &game.actors,
                    &available_sprites,
                    &available_flats,
                    &texture_sizes,
                    render_debug_mode,
                );
                if DEBUG_SCENE_STATS {
                    print_scene_summary(&args.map, &game.level, &scene_stats, &map_feature_stats);
                }

                if let Err(e) = renderer.render_scene(&scene, &view) {
                    eprintln!("Render error: {}", e);
                }
                if let Err(e) = renderer.end_frame() {
                    eprintln!("Render error: {}", e);
                }

                if let Err(e) = loop_.update(|dt| {
                    game.tick(dt)?;
                    if game.completed.is_some() {
                        elwt.exit();
                    }
                    game.input.use_action = false;
                    game.input.mouse_delta_x = 0.0;
                    Ok(())
                }) {
                    eprintln!("Tick error: {}", e);
                }
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn load_missing_texture(renderer: &mut impl Renderer) -> Result<()> {
    let size = 16u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let magenta = ((x / 4) + (y / 4)) % 2 == 0;
            data[idx] = if magenta { 255 } else { 0 };
            data[idx + 1] = 0;
            data[idx + 2] = if magenta { 255 } else { 0 };
            data[idx + 3] = 255;
        }
    }
    renderer.load_texture(
        "__missing",
        &TextureImage {
            width: size,
            height: size,
            data,
        },
    )
}

fn load_sky_fallback_texture(renderer: &mut impl Renderer) -> Result<()> {
    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let t = y as f32 / (size - 1) as f32;
            data[idx] = (24.0 + 20.0 * t) as u8;
            data[idx + 1] = (42.0 + 28.0 * t) as u8;
            data[idx + 2] = (72.0 + 42.0 * t) as u8;
            data[idx + 3] = 255;
        }
    }
    renderer.load_texture(
        SKY_FALLBACK_TEXTURE,
        &TextureImage {
            width: size,
            height: size,
            data,
        },
    )
}

#[derive(Debug, Default)]
struct SceneBuildStats {
    total_subsectors: usize,
    visible_subsectors: usize,
    visible_segs: usize,
    flat_seg_count: usize,
    min_visible_subsector_segs: usize,
    max_visible_subsector_segs: usize,
    visible_subsectors_under_three_segs: usize,
    bsp_clipped_polygons_generated: usize,
    degenerate_polygons_skipped: usize,
    wall_triangles: usize,
    floor_triangles: usize,
    ceiling_triangles: usize,
    skipped_subsectors: usize,
    skipped_degenerate_triangles: usize,
    missing_flat_fallbacks: usize,
    wall_texture_size_fallbacks: usize,
    flat_texture_size_fallbacks: usize,
    dynamic_lighting_enabled: bool,
    ambient_strength: f32,
    point_lights: usize,
    render_debug_mode: RenderDebugMode,
}

#[derive(Default)]
struct MapFeatureStats {
    has_animated_lump: bool,
    has_switches_lump: bool,
    has_textmap_lump: bool,
    has_behavior_lump: bool,
    has_znodes_lump: bool,
    has_gl_nodes: bool,
    sky_transfer_lines: usize,
}

fn collect_map_feature_stats(archive: &Archive, level: &Level) -> MapFeatureStats {
    MapFeatureStats {
        has_animated_lump: archive.find_lump_index("ANIMATED").is_some(),
        has_switches_lump: archive.find_lump_index("SWITCHES").is_some(),
        has_textmap_lump: archive.find_lump_index("TEXTMAP").is_some(),
        has_behavior_lump: archive.find_lump_index("BEHAVIOR").is_some(),
        has_znodes_lump: archive.find_lump_index("ZNODES").is_some(),
        has_gl_nodes: archive.find_lump_index("GL_VERT").is_some()
            || archive.find_lump_index("GL_SEGS").is_some()
            || archive.find_lump_index("GL_SSECT").is_some()
            || archive.find_lump_index("GL_NODES").is_some(),
        sky_transfer_lines: level
            .linedefs
            .iter()
            .filter(|line| SKY_TRANSFER_SPECIALS.contains(&line.special))
            .count(),
    }
}

fn build_render_scene_with_stats(
    level: &Level,
    viewer_pos: DVec2,
    viewer_angle: f64,
    eye_height: f64,
    actors: &[gameplay::Actor],
    available_sprites: &HashSet<String>,
    available_flats: &HashSet<String>,
    texture_sizes: &HashMap<String, (f32, f32)>,
    render_debug_mode: RenderDebugMode,
) -> (RenderScene, SceneBuildStats) {
    let mut scene = RenderScene::default();
    let mut stats = SceneBuildStats::default();
    scene.dynamic_lighting_enabled = DYNAMIC_LIGHTING_ENABLED;
    scene.ambient_strength = AMBIENT_STRENGTH;
    scene.debug_mode = render_debug_mode;
    let visibility = build_visibility(level, viewer_pos, viewer_angle, eye_height);
    stats.total_subsectors = level.subsectors.len();
    stats.visible_subsectors = visibility.visible_subsectors.len();
    stats.visible_segs = visibility.visible_segs.len();
    let bsp_polygons = build_bsp_subsector_polygons(level);
    stats.bsp_clipped_polygons_generated = bsp_polygons
        .polygons
        .iter()
        .filter(|poly| poly.is_some())
        .count();
    stats.degenerate_polygons_skipped = bsp_polygons.degenerate_polygons_skipped;
    let viewer_floor = level
        .find_sector(viewer_pos)
        .and_then(|idx| level.sectors.get(idx))
        .map(|sector| sector.floor_height)
        .unwrap_or(0.0);
    let viewer_eye_z = viewer_floor + eye_height;
    scene.point_lights = generate_demo_lights(level, viewer_pos, viewer_angle, viewer_eye_z);
    stats.dynamic_lighting_enabled = scene.dynamic_lighting_enabled;
    stats.ambient_strength = scene.ambient_strength;
    stats.point_lights = scene.point_lights.len();
    stats.render_debug_mode = scene.debug_mode;

    for &subsector_index in &visibility.visible_subsectors {
        let subsector = &level.subsectors[subsector_index];
        let sector = &level.sectors[subsector.sector];
        let seg_count = subsector.num_segs as usize;
        stats.flat_seg_count += seg_count;
        if stats.min_visible_subsector_segs == 0 || seg_count < stats.min_visible_subsector_segs {
            stats.min_visible_subsector_segs = seg_count;
        }
        stats.max_visible_subsector_segs = stats.max_visible_subsector_segs.max(seg_count);
        if seg_count < 3 {
            stats.visible_subsectors_under_three_segs += 1;
        }
        if DEBUG_SCENE_STATS {
            println!(
                "  Subsector {}: first_seg={} seg_count={}",
                subsector_index, subsector.first_seg, subsector.num_segs
            );
        }
        let Some(Some(polygon)) = bsp_polygons.polygons.get(subsector_index) else {
            stats.skipped_subsectors += 1;
            continue;
        };

        let light = light_color(sector.light_level);
        let (triangles, skipped) = triangulate_convex_fan_with_stats(polygon);
        stats.skipped_degenerate_triangles += skipped;
        for [a, b, c] in triangles {
            let floor_texture =
                resolve_flat_texture(&sector.floor_texture, false, available_flats, &mut stats);
            scene.flats.push(make_flat_triangle(
                &floor_texture,
                [a, b, c],
                sector.floor_height as f32,
                false,
                light,
            ));
            stats.floor_triangles += 1;
            let ceiling_texture =
                resolve_flat_texture(&sector.ceiling_texture, true, available_flats, &mut stats);
            scene.flats.push(make_flat_triangle(
                &ceiling_texture,
                [a, b, c],
                sector.ceiling_height as f32,
                true,
                light,
            ));
            stats.ceiling_triangles += 1;
        }
    }

    let mut emitted_walls = HashSet::new();
    for &seg_index in &visibility.visible_segs {
        if !emitted_walls.insert(seg_index) {
            continue;
        }
        let seg = &level.segs[seg_index];
        let Some(linedef_index) = seg.linedef else {
            continue;
        };
        let linedef = &level.linedefs[linedef_index];
        let start = level.vertices[seg.v1].p;
        let end = level.vertices[seg.v2].p;
        let before = scene.walls.len();
        build_wall_sections_for_seg(
            level,
            linedef,
            seg,
            start,
            end,
            texture_sizes,
            &mut scene.walls,
        );
        stats.wall_triangles += (scene.walls.len() - before) * 2;
    }

    let visible_sectors: HashSet<usize> = visibility
        .visible_subsectors
        .iter()
        .map(|&idx| level.subsectors[idx].sector)
        .collect();
    for actor in actors {
        if matches!(
            actor.class,
            gameplay::ActorClass::Player | gameplay::ActorClass::Projectile
        ) || actor.is_dead
        {
            continue;
        }
        let actor_pos = actor.pos_to_dvec2();
        let Some(actor_sector_idx) = level.find_sector(actor_pos) else {
            continue;
        };
        let actor_sector = &level.sectors[actor_sector_idx];
        let actor_center_z = actor_sector.floor_height + actor.height_f64() * 0.5;
        if !actor_potentially_visible(
            actor_pos,
            actor.radius_f64(),
            viewer_pos,
            viewer_angle,
            &visibility.solid_intervals,
            actor_center_z,
        ) {
            continue;
        }
        if !level.line_of_sight_clear(viewer_pos, viewer_eye_z, actor_pos, actor_center_z) {
            continue;
        }
        if visible_sectors.contains(&actor_sector_idx) {
            let sector = actor_sector;
            let bottom_z = sector.floor_height as f32;
            let texture_name = actor_sprite_name(actor.type_id, available_sprites);
            scene.sprites.push(Sprite {
                position: actor_pos,
                bottom_z,
                texture_name,
                width: actor.radius_f64() as f32 * 2.0,
                height: actor.height_f64() as f32,
                color: [1.0, 1.0, 1.0, 1.0],
            });
        }
    }

    stats.wall_texture_size_fallbacks = scene
        .walls
        .iter()
        .filter(|wall| !texture_sizes.contains_key(&wall.texture_name))
        .count();
    stats.flat_texture_size_fallbacks = scene
        .flats
        .iter()
        .filter(|flat| !texture_sizes.contains_key(&flat.texture_name))
        .count();

    (scene, stats)
}

fn print_scene_summary(
    map_name: &str,
    level: &Level,
    stats: &SceneBuildStats,
    map_features: &MapFeatureStats,
) {
    println!("Render scene summary for {}:", map_name);
    println!("  Sectors: {}", level.sectors.len());
    println!("  Subsectors: {}", stats.total_subsectors);
    println!("  Visible subsectors: {}", stats.visible_subsectors);
    println!(
        "  BSP clipped polygons generated: {}",
        stats.bsp_clipped_polygons_generated
    );
    println!("  Visible segs: {}", stats.visible_segs);
    println!("  Flat segs used: {}", stats.flat_seg_count);
    let average_edges = if stats.visible_subsectors == 0 {
        0.0
    } else {
        stats.flat_seg_count as f64 / stats.visible_subsectors as f64
    };
    println!(
        "  Visible subsector seg count: min={} max={} under3={} avg={:.2}",
        stats.min_visible_subsector_segs,
        stats.max_visible_subsector_segs,
        stats.visible_subsectors_under_three_segs,
        average_edges
    );
    println!("  Wall triangles: {}", stats.wall_triangles);
    println!("  Floor triangles: {}", stats.floor_triangles);
    println!("  Ceiling triangles: {}", stats.ceiling_triangles);
    println!("  Skipped subsectors: {}", stats.skipped_subsectors);
    println!(
        "  Degenerate BSP polygons skipped: {}",
        stats.degenerate_polygons_skipped
    );
    println!(
        "  Skipped degenerate flat triangles: {}",
        stats.skipped_degenerate_triangles
    );
    println!("  Missing flat fallbacks: {}", stats.missing_flat_fallbacks);
    println!("Texture UVs:");
    println!("  Flat UV mode: UZDoom-style world x/64, -z/64");
    println!("  Wall UV mode: seg distance with simplified texture_top");
    println!("  Sidedef offsets applied: yes");
    println!("  Texture size fallback: 64x64");
    println!(
        "  Wall texture size fallbacks: {}",
        stats.wall_texture_size_fallbacks
    );
    println!(
        "  Flat texture size fallbacks: {}",
        stats.flat_texture_size_fallbacks
    );
    println!("Texture compatibility:");
    println!("  Exact wall pegging: TODO");
    println!("  Wrapped middle textures: sampler repeat");
    println!("  Texture scaling metadata: not present in parsed map format");
    println!("  Slopes/skew metadata: not present in parsed map format");
    println!(
        "  Sky transfer linedefs detected: {}",
        map_features.sky_transfer_lines
    );
    println!(
        "  ANIMATED lump: {}",
        present_text(map_features.has_animated_lump)
    );
    println!(
        "  SWITCHES lump: {}",
        present_text(map_features.has_switches_lump)
    );
    println!(
        "  UDMF TEXTMAP lump: {}",
        present_text(map_features.has_textmap_lump)
    );
    println!(
        "  BEHAVIOR lump: {}",
        present_text(map_features.has_behavior_lump)
    );
    println!(
        "  ZNODES/GL nodes: {}",
        present_text(map_features.has_znodes_lump || map_features.has_gl_nodes)
    );
    println!("Renderer lighting:");
    println!("  Render debug mode: {:?}", stats.render_debug_mode);
    println!(
        "  Dynamic lighting: {}",
        if stats.dynamic_lighting_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("  Point lights: {}", stats.point_lights);
    println!("  Ambient: {:.2}", stats.ambient_strength);
    println!("  Max lights: {}", MAX_POINT_LIGHTS);
}

fn present_text(present: bool) -> &'static str {
    if present {
        "present (not applied yet)"
    } else {
        "absent"
    }
}

fn generate_demo_lights(
    level: &Level,
    viewer_pos: DVec2,
    viewer_angle: f64,
    viewer_eye_z: f64,
) -> Vec<PointLight> {
    if !DYNAMIC_LIGHTING_ENABLED {
        return Vec::new();
    }

    let forward = DVec2::new(viewer_angle.cos(), viewer_angle.sin());
    let right = DVec2::new(-viewer_angle.sin(), viewer_angle.cos());
    let sample_height = |pos: DVec2, fallback_z: f64| {
        level
            .find_sector(pos)
            .and_then(|idx| level.sectors.get(idx))
            .map(|sector| sector.floor_height + 80.0)
            .unwrap_or(fallback_z)
    };

    let mut lights = Vec::new();
    let mut push_light = |map_pos: DVec2, z: f64, color: [f32; 3], intensity: f32, radius: f32| {
        if lights.len() >= MAX_POINT_LIGHTS {
            return;
        }
        lights.push(PointLight {
            position: [map_pos.x as f32, z as f32, map_pos.y as f32],
            color,
            intensity,
            radius,
        });
    };

    push_light(
        viewer_pos + forward * 96.0,
        viewer_eye_z + 32.0,
        [1.0, 0.72, 0.45],
        1.25,
        520.0,
    );

    let cool_pos = viewer_pos + forward * 420.0;
    push_light(
        cool_pos,
        sample_height(cool_pos, viewer_eye_z + 32.0),
        [0.45, 0.62, 1.0],
        0.85,
        420.0,
    );

    let green_pos = viewer_pos + right * 320.0 + forward * 220.0;
    push_light(
        green_pos,
        sample_height(green_pos, viewer_eye_z + 24.0),
        [0.35, 1.0, 0.45],
        0.7,
        360.0,
    );

    lights
}

struct VisibilitySet {
    visible_subsectors: Vec<usize>,
    visible_segs: Vec<usize>,
    solid_intervals: Vec<AngleInterval>,
}

fn build_visibility(
    level: &Level,
    viewer_pos: DVec2,
    viewer_angle: f64,
    eye_height: f64,
) -> VisibilitySet {
    let mut visible_subsectors = Vec::new();
    let mut visible_segs = Vec::new();
    if level.nodes.is_empty() {
        visible_subsectors.extend(0..level.subsectors.len());
        visible_segs.extend(0..level.segs.len());
        return VisibilitySet {
            visible_subsectors,
            visible_segs,
            solid_intervals: Vec::new(),
        };
    }

    let mut visited = HashSet::new();
    let mut clipper = AngleClipper::new(viewer_angle, std::f64::consts::FRAC_PI_2 * 0.7);
    traverse_bsp(
        level,
        level.nodes.len() - 1,
        viewer_pos,
        eye_height,
        &mut clipper,
        &mut visited,
        &mut visible_subsectors,
        &mut visible_segs,
    );

    VisibilitySet {
        visible_subsectors,
        visible_segs,
        solid_intervals: clipper.intervals,
    }
}

fn traverse_bsp(
    level: &Level,
    node_index: usize,
    viewer_pos: DVec2,
    eye_height: f64,
    clipper: &mut AngleClipper,
    visited: &mut HashSet<usize>,
    out_subsectors: &mut Vec<usize>,
    out_segs: &mut Vec<usize>,
) {
    let node = &level.nodes[node_index];
    let side = level.point_on_node_side(viewer_pos, node_index);
    let front_child = node.children[side];
    let back_child = node.children[1 - side];

    visit_bsp_child(
        level,
        front_child,
        viewer_pos,
        eye_height,
        clipper,
        visited,
        out_subsectors,
        out_segs,
    );

    if bbox_in_view(node.bbox[1 - side], viewer_pos, clipper) {
        visit_bsp_child(
            level,
            back_child,
            viewer_pos,
            eye_height,
            clipper,
            visited,
            out_subsectors,
            out_segs,
        );
    }
}

fn visit_bsp_child(
    level: &Level,
    child: u16,
    viewer_pos: DVec2,
    eye_height: f64,
    clipper: &mut AngleClipper,
    visited: &mut HashSet<usize>,
    out_subsectors: &mut Vec<usize>,
    out_segs: &mut Vec<usize>,
) {
    if (child & 0x8000) != 0 {
        let subsector_index = (child & 0x7fff) as usize;
        if visited.insert(subsector_index)
            && visit_subsector(
                level,
                subsector_index,
                viewer_pos,
                eye_height,
                clipper,
                out_segs,
            )
        {
            out_subsectors.push(subsector_index);
        }
    } else {
        traverse_bsp(
            level,
            child as usize,
            viewer_pos,
            eye_height,
            clipper,
            visited,
            out_subsectors,
            out_segs,
        );
    }
}

fn visit_subsector(
    level: &Level,
    subsector_index: usize,
    viewer_pos: DVec2,
    eye_height: f64,
    clipper: &mut AngleClipper,
    out_segs: &mut Vec<usize>,
) -> bool {
    let subsector = &level.subsectors[subsector_index];
    let mut any_visible = false;
    let Some(seg_indices) = subsector_seg_indices(level, subsector) else {
        eprintln!(
            "Skipping visibility for subsector {} (first_seg={} seg_count={}): seg range exceeds seg count {}",
            subsector_index,
            subsector.first_seg,
            subsector.num_segs,
            level.segs.len()
        );
        return false;
    };
    for seg_index in seg_indices {
        let seg = &level.segs[seg_index];
        let start = level.vertices[seg.v1].p;
        let end = level.vertices[seg.v2].p;
        let Some(interval) = segment_angle_interval(viewer_pos, start, end, clipper.viewer_angle)
        else {
            continue;
        };
        if clipper.is_fully_occluded(interval) {
            continue;
        }
        any_visible = true;
        out_segs.push(seg_index);
        if seg_occludes_at_height(level, seg, eye_height) {
            clipper.occlude(interval);
        }
    }
    any_visible
}

fn bbox_in_view(bbox: [i16; 4], viewer_pos: DVec2, clipper: &AngleClipper) -> bool {
    let top = bbox[0] as f64;
    let bottom = bbox[1] as f64;
    let left = bbox[2] as f64;
    let right = bbox[3] as f64;
    let corners = [
        DVec2::new(left, bottom),
        DVec2::new(left, top),
        DVec2::new(right, bottom),
        DVec2::new(right, top),
    ];
    let mut angles = Vec::new();
    corners.iter().for_each(|corner| {
        let rel = *corner - viewer_pos;
        if rel.length_squared() <= 4096.0 * 4096.0 {
            angles.push(angle_delta(rel.y.atan2(rel.x), clipper.viewer_angle));
        }
    });
    if angles.is_empty() {
        return false;
    }
    let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
    let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let interval = AngleInterval {
        start: min_angle,
        end: max_angle,
    };
    clipper.intersects_fov(interval) && !clipper.is_fully_occluded(interval)
}

fn subsector_seg_indices(level: &Level, subsector: &level::SubSector) -> Option<Vec<usize>> {
    let first_seg = subsector.first_seg;
    let last_seg = first_seg.checked_add(subsector.num_segs as usize)?;
    if last_seg > level.segs.len() {
        return None;
    }
    Some((first_seg..last_seg).collect())
}

struct BspSubsectorPolygons {
    polygons: Vec<Option<Vec<DVec2>>>,
    degenerate_polygons_skipped: usize,
}

fn build_bsp_subsector_polygons(level: &Level) -> BspSubsectorPolygons {
    let mut polygons = vec![None; level.subsectors.len()];
    let mut degenerate_polygons_skipped = 0;
    let Some(initial_polygon) = initial_bsp_clip_polygon(level) else {
        return BspSubsectorPolygons {
            polygons,
            degenerate_polygons_skipped,
        };
    };

    if level.nodes.is_empty() {
        if !level.subsectors.is_empty() && !is_degenerate_polygon(&initial_polygon) {
            polygons[0] = Some(initial_polygon);
        }
        return BspSubsectorPolygons {
            polygons,
            degenerate_polygons_skipped,
        };
    }

    clip_bsp_node_to_subsectors(
        level,
        level.nodes.len() - 1,
        initial_polygon,
        &mut polygons,
        &mut degenerate_polygons_skipped,
    );
    BspSubsectorPolygons {
        polygons,
        degenerate_polygons_skipped,
    }
}

fn initial_bsp_clip_polygon(level: &Level) -> Option<Vec<DVec2>> {
    let first = level.vertices.first()?.p;
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;
    for vertex in &level.vertices {
        min_x = min_x.min(vertex.p.x);
        max_x = max_x.max(vertex.p.x);
        min_y = min_y.min(vertex.p.y);
        max_y = max_y.max(vertex.p.y);
    }

    let width = (max_x - min_x).abs();
    let height = (max_y - min_y).abs();
    let pad = width.max(height).max(1024.0) * 0.25 + 64.0;
    Some(vec![
        DVec2::new(min_x - pad, min_y - pad),
        DVec2::new(max_x + pad, min_y - pad),
        DVec2::new(max_x + pad, max_y + pad),
        DVec2::new(min_x - pad, max_y + pad),
    ])
}

fn clip_bsp_node_to_subsectors(
    level: &Level,
    node_index: usize,
    polygon: Vec<DVec2>,
    out: &mut [Option<Vec<DVec2>>],
    degenerate_polygons_skipped: &mut usize,
) {
    if polygon.len() < 3 || is_degenerate_polygon(&polygon) {
        *degenerate_polygons_skipped += 1;
        return;
    }

    let node = &level.nodes[node_index];
    let front = clip_polygon_to_bsp_side(&polygon, node, 0);
    let back = clip_polygon_to_bsp_side(&polygon, node, 1);
    assign_bsp_child_polygon(
        level,
        node.children[0],
        front,
        out,
        degenerate_polygons_skipped,
    );
    assign_bsp_child_polygon(
        level,
        node.children[1],
        back,
        out,
        degenerate_polygons_skipped,
    );
}

fn assign_bsp_child_polygon(
    level: &Level,
    child: u16,
    polygon: Vec<DVec2>,
    out: &mut [Option<Vec<DVec2>>],
    degenerate_polygons_skipped: &mut usize,
) {
    if polygon.len() < 3 || is_degenerate_polygon(&polygon) {
        *degenerate_polygons_skipped += 1;
        return;
    }

    if (child & 0x8000) != 0 {
        let subsector_index = (child & 0x7fff) as usize;
        if let Some(slot) = out.get_mut(subsector_index) {
            *slot = Some(polygon);
        }
    } else {
        clip_bsp_node_to_subsectors(
            level,
            child as usize,
            polygon,
            out,
            degenerate_polygons_skipped,
        );
    }
}

fn clip_polygon_to_bsp_side(polygon: &[DVec2], node: &level::Node, side: usize) -> Vec<DVec2> {
    if polygon.is_empty() {
        return Vec::new();
    }

    let mut clipped = Vec::new();
    for i in 0..polygon.len() {
        let current = polygon[i];
        let next = polygon[(i + 1) % polygon.len()];
        let current_distance = bsp_line_signed_distance(node, current);
        let next_distance = bsp_line_signed_distance(node, next);
        let current_inside = bsp_side_contains_distance(side, current_distance);
        let next_inside = bsp_side_contains_distance(side, next_distance);

        match (current_inside, next_inside) {
            (true, true) => push_unique_polygon_vertex(&mut clipped, next),
            (true, false) => {
                push_unique_polygon_vertex(
                    &mut clipped,
                    line_segment_bsp_intersection(current, next, current_distance, next_distance),
                );
            }
            (false, true) => {
                push_unique_polygon_vertex(
                    &mut clipped,
                    line_segment_bsp_intersection(current, next, current_distance, next_distance),
                );
                push_unique_polygon_vertex(&mut clipped, next);
            }
            (false, false) => {}
        }
    }
    remove_closing_duplicate(&mut clipped);
    clipped
}

fn bsp_line_signed_distance(node: &level::Node, point: DVec2) -> f64 {
    let origin = DVec2::new(node.x as f64, node.y as f64);
    let direction = DVec2::new(node.dx as f64, node.dy as f64);
    let rel = point - origin;
    direction.x * rel.y - direction.y * rel.x
}

fn bsp_side_contains_distance(side: usize, distance: f64) -> bool {
    if side == 0 {
        distance <= SUBSECTOR_VERTEX_EPSILON
    } else {
        distance >= -SUBSECTOR_VERTEX_EPSILON
    }
}

fn line_segment_bsp_intersection(a: DVec2, b: DVec2, da: f64, db: f64) -> DVec2 {
    let denom = da - db;
    if denom.abs() < f64::EPSILON {
        return a;
    }
    let t = (da / denom).clamp(0.0, 1.0);
    a + (b - a) * t
}

fn push_unique_polygon_vertex(polygon: &mut Vec<DVec2>, vertex: DVec2) {
    if polygon
        .last()
        .is_some_and(|last| last.distance(vertex) < SUBSECTOR_VERTEX_EPSILON)
    {
        return;
    }
    polygon.push(vertex);
}

fn remove_closing_duplicate(polygon: &mut Vec<DVec2>) {
    if polygon.len() >= 2
        && polygon[0].distance(*polygon.last().expect("polygon has at least two vertices"))
            < SUBSECTOR_VERTEX_EPSILON
    {
        polygon.pop();
    }
}

fn is_degenerate_polygon(polygon: &[DVec2]) -> bool {
    if polygon.len() < 3 {
        return true;
    }
    polygon_signed_area(polygon).abs() < 0.001
}

fn polygon_signed_area(polygon: &[DVec2]) -> f64 {
    let mut area = 0.0;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

fn triangulate_convex_fan_with_stats(polygon: &[DVec2]) -> (Vec<[DVec2; 3]>, usize) {
    if polygon.len() < 3 {
        return (Vec::new(), 0);
    }
    if polygon.len() == 3 {
        let triangle = [polygon[0], polygon[1], polygon[2]];
        return if is_degenerate_triangle(triangle) {
            (Vec::new(), 1)
        } else {
            (vec![triangle], 0)
        };
    }

    let mut triangles = Vec::new();
    let mut skipped = 0;
    for i in 1..polygon.len() - 1 {
        let triangle = [polygon[0], polygon[i], polygon[i + 1]];
        if is_degenerate_triangle(triangle) {
            skipped += 1;
        } else {
            triangles.push(triangle);
        }
    }
    (triangles, skipped)
}

fn build_wall_sections_for_seg(
    level: &Level,
    linedef: &level::LineDef,
    seg: &level::Seg,
    start: DVec2,
    end: DVec2,
    texture_sizes: &HashMap<String, (f32, f32)>,
    walls: &mut Vec<WallQuad>,
) {
    let side_index = seg.side as usize;
    let Some(side_idx) = linedef.sidedef[side_index] else {
        return;
    };
    let side = &level.sidedefs[side_idx];
    let Some(front_sector_idx) = linedef.sectors[side_index] else {
        return;
    };
    let front_sector = &level.sectors[front_sector_idx];
    let back_sector = linedef.sectors[1 - side_index].map(|idx| &level.sectors[idx]);
    let Some(linedef_start) = level.vertices.get(linedef.v1).map(|vertex| vertex.p) else {
        return;
    };
    let Some(linedef_end) = level.vertices.get(linedef.v2).map(|vertex| vertex.p) else {
        return;
    };
    let linedef_delta = linedef_end - linedef_start;
    let linedef_len = linedef_delta.length();
    if linedef_len <= f64::EPSILON {
        return;
    }
    let line_dir = linedef_delta / linedef_len;
    let dist_start = (start - linedef_start).dot(line_dir) as f32;
    let dist_end = (end - linedef_start).dot(line_dir) as f32;
    let color = light_color(front_sector.light_level);

    if let Some(back_sector) = back_sector {
        if front_sector.ceiling_height > back_sector.ceiling_height {
            push_wall(
                walls,
                WallSectionKind::Upper,
                &side.top_texture,
                false,
                start,
                end,
                back_sector.ceiling_height as f32,
                front_sector.ceiling_height as f32,
                compute_wall_uvs(
                    WallUvParams {
                        texture_name: &side.top_texture,
                        u_start_world: side.texture_offset as f32 + dist_start,
                        u_end_world: side.texture_offset as f32 + dist_end,
                        bottom_z: back_sector.ceiling_height as f32,
                        top_z: front_sector.ceiling_height as f32,
                        row_offset_world: side.row_offset as f32,
                    },
                    texture_sizes,
                ),
                color,
            );
        }
        if front_sector.floor_height < back_sector.floor_height {
            push_wall(
                walls,
                WallSectionKind::Lower,
                &side.bottom_texture,
                false,
                start,
                end,
                front_sector.floor_height as f32,
                back_sector.floor_height as f32,
                compute_wall_uvs(
                    WallUvParams {
                        texture_name: &side.bottom_texture,
                        u_start_world: side.texture_offset as f32 + dist_start,
                        u_end_world: side.texture_offset as f32 + dist_end,
                        bottom_z: front_sector.floor_height as f32,
                        top_z: back_sector.floor_height as f32,
                        row_offset_world: side.row_offset as f32,
                    },
                    texture_sizes,
                ),
                color,
            );
        }
        if side.mid_texture != "-" {
            push_wall(
                walls,
                WallSectionKind::MiddleMasked,
                &side.mid_texture,
                true,
                start,
                end,
                back_sector.floor_height.max(front_sector.floor_height) as f32,
                back_sector.ceiling_height.min(front_sector.ceiling_height) as f32,
                compute_wall_uvs(
                    WallUvParams {
                        texture_name: &side.mid_texture,
                        u_start_world: side.texture_offset as f32 + dist_start,
                        u_end_world: side.texture_offset as f32 + dist_end,
                        bottom_z: back_sector.floor_height.max(front_sector.floor_height) as f32,
                        top_z: back_sector.ceiling_height.min(front_sector.ceiling_height) as f32,
                        row_offset_world: side.row_offset as f32,
                    },
                    texture_sizes,
                ),
                color,
            );
        }
    } else {
        let texture_name = if side.mid_texture == "-" {
            "__missing"
        } else {
            &side.mid_texture
        };
        push_wall(
            walls,
            WallSectionKind::MiddleSolid,
            texture_name,
            false,
            start,
            end,
            front_sector.floor_height as f32,
            front_sector.ceiling_height as f32,
            compute_wall_uvs(
                WallUvParams {
                    texture_name,
                    u_start_world: side.texture_offset as f32 + dist_start,
                    u_end_world: side.texture_offset as f32 + dist_end,
                    bottom_z: front_sector.floor_height as f32,
                    top_z: front_sector.ceiling_height as f32,
                    row_offset_world: side.row_offset as f32,
                },
                texture_sizes,
            ),
            color,
        );
    }
}

fn push_wall(
    walls: &mut Vec<WallQuad>,
    section_kind: WallSectionKind,
    texture_name: &str,
    masked: bool,
    start: DVec2,
    end: DVec2,
    bottom_z: f32,
    top_z: f32,
    (uv_min, uv_max): ([f32; 2], [f32; 2]),
    color: [f32; 4],
) {
    if texture_name.is_empty() || texture_name == "-" || top_z <= bottom_z {
        return;
    }
    // TODO: derive the exact Doom outward side for every linedef side. This stable
    // perpendicular is sufficient for first-pass side lighting with culling disabled.
    let normal = wall_normal(start, end);
    walls.push(WallQuad {
        texture_name: texture_name.to_string(),
        section_kind,
        masked,
        start,
        end,
        bottom_z,
        top_z,
        uv_min,
        uv_max,
        normal,
        color,
    });
}

fn wall_normal(start: DVec2, end: DVec2) -> [f32; 3] {
    let dir = end - start;
    let len = dir.length();
    if len <= f64::EPSILON {
        return [0.0, 0.0, 1.0];
    }
    let nx = (dir.y / len) as f32;
    let nz = (-dir.x / len) as f32;
    [nx, 0.0, nz]
}

fn make_flat_triangle(
    texture_name: &str,
    world: [DVec2; 3],
    z: f32,
    is_ceiling: bool,
    color: [f32; 4],
) -> FlatTriangle {
    let positions = [
        [world[0].x as f32, world[0].y as f32, z],
        [world[1].x as f32, world[1].y as f32, z],
        [world[2].x as f32, world[2].y as f32, z],
    ];
    let uvs = world.map(flat_world_uv);

    if is_ceiling {
        FlatTriangle {
            texture_name: missing_texture_name(texture_name),
            positions: [positions[0], positions[2], positions[1]],
            uvs: [uvs[0], uvs[2], uvs[1]],
            normal: [0.0, -1.0, 0.0],
            color: flat_color(color, true),
        }
    } else {
        FlatTriangle {
            texture_name: missing_texture_name(texture_name),
            positions,
            uvs,
            normal: [0.0, 1.0, 0.0],
            color: flat_color(color, false),
        }
    }
}

fn flat_world_uv(world: DVec2) -> [f32; 2] {
    // UZDoom's classic flat path emits world-space UVs as x/64 and -y/64.
    // Our map plane is XZ, represented here as DVec2(x, z), so both floors
    // and ceilings use the same repeated world-space mapping.
    [world.x as f32 / 64.0, -(world.y as f32) / 64.0]
}

fn flat_color(color: [f32; 4], is_ceiling: bool) -> [f32; 4] {
    if !DEBUG_SOLID_FLATS {
        return color;
    }
    if is_ceiling {
        [0.25, 0.45, 0.9, 1.0]
    } else {
        [0.35, 0.8, 0.35, 1.0]
    }
}

fn resolve_flat_texture(
    texture_name: &str,
    is_ceiling: bool,
    available_flats: &HashSet<String>,
    stats: &mut SceneBuildStats,
) -> String {
    let normalized = texture_name.to_uppercase();
    if is_ceiling && normalized == "F_SKY1" {
        // TODO: render Doom sky as a projected sky texture instead of a flat fallback.
        return SKY_FALLBACK_TEXTURE.to_string();
    }
    if normalized.is_empty() || normalized == "-" {
        stats.missing_flat_fallbacks += 1;
        return "__missing".to_string();
    }
    if available_flats.contains(&normalized) {
        normalized
    } else {
        stats.missing_flat_fallbacks += 1;
        "__missing".to_string()
    }
}

fn texture_size(texture_name: &str, texture_sizes: &HashMap<String, (f32, f32)>) -> (f32, f32) {
    texture_sizes
        .get(texture_name)
        .copied()
        .unwrap_or((64.0, 64.0))
}

struct WallUvParams<'a> {
    texture_name: &'a str,
    u_start_world: f32,
    u_end_world: f32,
    bottom_z: f32,
    top_z: f32,
    row_offset_world: f32,
}

fn compute_wall_uvs(
    params: WallUvParams<'_>,
    texture_sizes: &HashMap<String, (f32, f32)>,
) -> ([f32; 2], [f32; 2]) {
    let (texture_width, texture_height) = texture_size(params.texture_name, texture_sizes);
    let texture_width = texture_width.max(1.0);
    let texture_height = texture_height.max(1.0);
    let u0 = params.u_start_world / texture_width;
    let u1 = params.u_end_world / texture_width;

    // UZDoom's wall path keeps U continuous across split segs by measuring from
    // the parent linedef. This pass intentionally keeps stable local wall V and
    // leaves exact upper/lower/middle pegging rules as a TODO.
    let height = (params.top_z - params.bottom_z).max(0.0);
    let v0 = params.row_offset_world / texture_height;
    let v1 = (params.row_offset_world + height) / texture_height;

    ([u0, v0], [u1, v1])
}

fn signed_area(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn is_degenerate_triangle(triangle: [DVec2; 3]) -> bool {
    signed_area(triangle[0], triangle[1], triangle[2]).abs() < 0.001
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_square_level_with_edges(edges: Vec<(usize, usize)>) -> Level {
        test_level_with_vertices_and_edges(
            vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(64.0, 0.0),
                DVec2::new(64.0, 64.0),
                DVec2::new(0.0, 64.0),
            ],
            edges,
        )
    }

    fn test_level_with_vertices_and_edges(
        vertices: Vec<DVec2>,
        edges: Vec<(usize, usize)>,
    ) -> Level {
        let num_segs = edges.len() as u16;
        Level {
            vertices: vertices.into_iter().map(|p| level::Vertex { p }).collect(),
            sectors: vec![level::Sector {
                floor_height: 0.0,
                ceiling_height: 128.0,
                floor_texture: "FLOOR0_1".to_string(),
                ceiling_texture: "CEIL1_1".to_string(),
                light_level: 255,
                special: 0,
                tag: 0,
            }],
            sidedefs: Vec::new(),
            linedefs: Vec::new(),
            segs: edges
                .into_iter()
                .map(|(v1, v2)| level::Seg {
                    v1,
                    v2,
                    angle: 0,
                    linedef: None,
                    side: 0,
                    offset: 0,
                })
                .collect(),
            subsectors: vec![level::SubSector {
                num_segs,
                first_seg: 0,
                sector: 0,
            }],
            nodes: Vec::new(),
            things: Vec::new(),
            active_doors: Vec::new(),
            active_floors: Vec::new(),
        }
    }

    #[test]
    fn triangulates_ordered_square_into_two_triangles() {
        let polygon = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(64.0, 0.0),
            DVec2::new(64.0, 64.0),
            DVec2::new(0.0, 64.0),
        ];

        let (triangles, skipped) = triangulate_convex_fan_with_stats(&polygon);

        assert_eq!(triangles.len(), 2);
        assert_eq!(skipped, 0);
        assert!(triangles.iter().all(|tri| !is_degenerate_triangle(*tri)));
    }

    #[test]
    fn rejects_degenerate_triangle() {
        let polygon = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(32.0, 0.0),
            DVec2::new(64.0, 0.0),
        ];

        let (triangles, skipped) = triangulate_convex_fan_with_stats(&polygon);

        assert!(triangles.is_empty());
        assert_eq!(skipped, 1);
    }

    #[test]
    fn bsp_clipping_generates_polygon_without_nodes() {
        let level = test_square_level_with_edges(vec![(0, 1)]);

        let polygons = build_bsp_subsector_polygons(&level);

        assert_eq!(polygons.polygons.len(), 1);
        assert!(polygons.polygons[0].is_some());
        assert_eq!(polygons.degenerate_polygons_skipped, 0);
    }

    #[test]
    fn bsp_clipping_assigns_leaf_polygons_for_subsectors_with_few_segs() {
        let mut level = test_square_level_with_edges(vec![(0, 1), (1, 2), (2, 3)]);
        level.subsectors = vec![
            level::SubSector {
                num_segs: 1,
                first_seg: 0,
                sector: 0,
            },
            level::SubSector {
                num_segs: 2,
                first_seg: 1,
                sector: 0,
            },
        ];
        level.nodes = vec![level::Node {
            x: 32,
            y: 0,
            dx: 0,
            dy: 64,
            bbox: [[0, 0, 0, 0]; 2],
            children: [0x8000, 0x8001],
        }];

        let polygons = build_bsp_subsector_polygons(&level);

        assert!(polygons.polygons[0].is_some());
        assert!(polygons.polygons[1].is_some());
        assert_eq!(polygons.degenerate_polygons_skipped, 0);
        assert!(polygon_signed_area(polygons.polygons[0].as_ref().unwrap()).abs() > 1.0);
        assert!(polygon_signed_area(polygons.polygons[1].as_ref().unwrap()).abs() > 1.0);
    }

    #[test]
    fn bsp_side_clipping_splits_convex_polygon() {
        let node = level::Node {
            x: 32,
            y: 0,
            dx: 0,
            dy: 64,
            bbox: [[0, 0, 0, 0]; 2],
            children: [0x8000, 0x8001],
        };
        let polygon = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(64.0, 0.0),
            DVec2::new(64.0, 64.0),
            DVec2::new(0.0, 64.0),
        ];

        let front = clip_polygon_to_bsp_side(&polygon, &node, 0);
        let back = clip_polygon_to_bsp_side(&polygon, &node, 1);

        assert_eq!(front.len(), 4);
        assert_eq!(back.len(), 4);
        assert!(front.iter().all(|p| p.x >= 32.0 - SUBSECTOR_VERTEX_EPSILON));
        assert!(back.iter().all(|p| p.x <= 32.0 + SUBSECTOR_VERTEX_EPSILON));
    }

    #[test]
    fn flat_uvs_use_world_space_tiling_without_normalization() {
        let uv = flat_world_uv(DVec2::new(128.0, 192.0));

        assert_eq!(uv, [2.0, -3.0]);
    }

    #[test]
    fn wall_uvs_use_wall_distance_and_texture_dimensions() {
        let mut texture_sizes = HashMap::new();
        texture_sizes.insert("WALL".to_string(), (32.0, 16.0));

        let (uv_min, uv_max) = compute_wall_uvs(
            WallUvParams {
                texture_name: "WALL",
                u_start_world: 16.0,
                u_end_world: 112.0,
                bottom_z: 8.0,
                top_z: 72.0,
                row_offset_world: 4.0,
            },
            &texture_sizes,
        );

        assert_eq!(uv_min, [0.5, 0.25]);
        assert_eq!(uv_max, [3.5, 4.25]);
    }

    #[test]
    fn wall_uvs_use_simplified_texture_top_for_all_wall_parts() {
        let mut texture_sizes = HashMap::new();
        texture_sizes.insert("WALL".to_string(), (64.0, 64.0));

        let (uv_min, uv_max) = compute_wall_uvs(
            WallUvParams {
                texture_name: "WALL",
                u_start_world: 0.0,
                u_end_world: 64.0,
                bottom_z: 0.0,
                top_z: 128.0,
                row_offset_world: 16.0,
            },
            &texture_sizes,
        );

        assert_eq!(uv_min, [0.0, 0.25]);
        assert_eq!(uv_max, [1.0, 2.25]);
    }

    #[test]
    fn wall_sections_use_linedef_relative_u_coordinates() {
        let mut level = test_level_with_vertices_and_edges(
            vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(128.0, 0.0),
                DVec2::new(32.0, 0.0),
                DVec2::new(96.0, 0.0),
            ],
            vec![(2, 3)],
        );
        level.sidedefs.push(level::SideDef {
            texture_offset: 8.0,
            row_offset: 0.0,
            top_texture: "-".to_string(),
            bottom_texture: "-".to_string(),
            mid_texture: "WALL".to_string(),
            sector: 0,
        });
        level.linedefs.push(level::LineDef {
            v1: 0,
            v2: 1,
            flags: 0,
            special: 0,
            tag: 0,
            sidedef: [Some(0), None],
            sectors: [Some(0), None],
        });
        level.segs[0].linedef = Some(0);
        level.sidedefs[0].mid_texture = "WALL".to_string();

        let mut texture_sizes = HashMap::new();
        texture_sizes.insert("WALL".to_string(), (32.0, 64.0));
        let mut walls = Vec::new();
        build_wall_sections_for_seg(
            &level,
            &level.linedefs[0],
            &level.segs[0],
            level.vertices[2].p,
            level.vertices[3].p,
            &texture_sizes,
            &mut walls,
        );

        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].uv_min[0], 1.25);
        assert_eq!(walls[0].uv_max[0], 3.25);
    }

    #[test]
    fn reversed_wall_sections_keep_projected_u_direction() {
        let mut level = test_level_with_vertices_and_edges(
            vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(128.0, 0.0),
                DVec2::new(96.0, 0.0),
                DVec2::new(32.0, 0.0),
            ],
            vec![(2, 3)],
        );
        level.sidedefs.push(level::SideDef {
            texture_offset: 8.0,
            row_offset: 0.0,
            top_texture: "-".to_string(),
            bottom_texture: "-".to_string(),
            mid_texture: "WALL".to_string(),
            sector: 0,
        });
        level.linedefs.push(level::LineDef {
            v1: 0,
            v2: 1,
            flags: 0,
            special: 0,
            tag: 0,
            sidedef: [Some(0), None],
            sectors: [Some(0), None],
        });
        level.segs[0].linedef = Some(0);

        let mut texture_sizes = HashMap::new();
        texture_sizes.insert("WALL".to_string(), (32.0, 64.0));
        let mut walls = Vec::new();
        build_wall_sections_for_seg(
            &level,
            &level.linedefs[0],
            &level.segs[0],
            level.vertices[2].p,
            level.vertices[3].p,
            &texture_sizes,
            &mut walls,
        );

        assert_eq!(walls.len(), 1);
        assert_eq!(walls[0].uv_min[0], 3.25);
        assert_eq!(walls[0].uv_max[0], 1.25);
    }

    #[test]
    fn e1m1_scene_builds_floor_and_ceiling_triangles_when_wad_is_present() -> Result<()> {
        let wad_path = std::path::Path::new("doom.wad");
        if !wad_path.exists() {
            return Ok(());
        }

        let archive = Archive::load_wad(wad_path.to_string_lossy().as_ref())?;
        let level = level::load_level(&archive, "E1M1")?;
        let player_start = level
            .things
            .iter()
            .find(|thing| thing.type_id == 1)
            .map(|thing| DVec2::new(thing.x as f64, thing.y as f64))
            .unwrap_or(DVec2::ZERO);
        let player_angle = level
            .things
            .iter()
            .find(|thing| thing.type_id == 1)
            .map(|thing| (thing.angle as f64).to_radians())
            .unwrap_or(0.0);
        let mut available_flats: HashSet<String> = archive
            .load_flats()?
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        available_flats.insert("__missing".to_string());
        available_flats.insert(SKY_FALLBACK_TEXTURE.to_string());
        let texture_sizes: HashMap<String, (f32, f32)> = available_flats
            .iter()
            .map(|name| {
                let size = if name == "__missing" {
                    (16.0, 16.0)
                } else {
                    (64.0, 64.0)
                };
                (name.clone(), size)
            })
            .collect();

        let (_, stats) = build_render_scene_with_stats(
            &level,
            player_start,
            player_angle,
            41.0,
            &[],
            &HashSet::new(),
            &available_flats,
            &texture_sizes,
            RenderDebugMode::Lit,
        );

        assert!(stats.floor_triangles > 0);
        assert!(stats.ceiling_triangles > 0);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct AngleInterval {
    start: f64,
    end: f64,
}

#[derive(Debug)]
struct AngleClipper {
    viewer_angle: f64,
    half_fov: f64,
    intervals: Vec<AngleInterval>,
}

impl AngleClipper {
    fn new(viewer_angle: f64, half_fov: f64) -> Self {
        Self {
            viewer_angle,
            half_fov,
            intervals: Vec::new(),
        }
    }

    fn intersects_fov(&self, interval: AngleInterval) -> bool {
        interval.end >= -self.half_fov && interval.start <= self.half_fov
    }

    fn is_fully_occluded(&self, interval: AngleInterval) -> bool {
        if !self.intersects_fov(interval) {
            return true;
        }
        angle_intervals_cover(&self.intervals, interval)
    }

    fn occlude(&mut self, interval: AngleInterval) {
        if !self.intersects_fov(interval) {
            return;
        }
        self.intervals.push(AngleInterval {
            start: interval.start.clamp(-self.half_fov, self.half_fov),
            end: interval.end.clamp(-self.half_fov, self.half_fov),
        });
        self.intervals.sort_by(|a, b| {
            a.start
                .partial_cmp(&b.start)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut merged: Vec<AngleInterval> = Vec::with_capacity(self.intervals.len());
        for interval in &self.intervals {
            if let Some(last) = merged.last_mut() {
                if interval.start <= last.end + 0.0001 {
                    last.end = last.end.max(interval.end);
                    continue;
                }
            }
            merged.push(*interval);
        }
        self.intervals = merged;
    }
}

fn angle_intervals_cover(intervals: &[AngleInterval], target: AngleInterval) -> bool {
    let mut covered_until = target.start;
    for interval in intervals {
        if interval.end <= covered_until {
            continue;
        }
        if interval.start > covered_until + 0.0001 {
            return false;
        }
        covered_until = interval.end.max(covered_until);
        if covered_until >= target.end - 0.0001 {
            return true;
        }
    }
    false
}

fn segment_angle_interval(
    viewer_pos: DVec2,
    start: DVec2,
    end: DVec2,
    viewer_angle: f64,
) -> Option<AngleInterval> {
    let start_rel = start - viewer_pos;
    let end_rel = end - viewer_pos;
    if start_rel.length_squared() < 1.0 && end_rel.length_squared() < 1.0 {
        return None;
    }
    let mut a0 = angle_delta(start_rel.y.atan2(start_rel.x), viewer_angle);
    let mut a1 = angle_delta(end_rel.y.atan2(end_rel.x), viewer_angle);
    if (a1 - a0).abs() > std::f64::consts::PI {
        if a0 < a1 {
            a0 += std::f64::consts::TAU;
        } else {
            a1 += std::f64::consts::TAU;
        }
    }
    let start = a0.min(a1);
    let end = a0.max(a1);
    Some(AngleInterval { start, end })
}

fn seg_occludes_at_height(level: &Level, seg: &level::Seg, eye_height: f64) -> bool {
    let Some(opening) = level.opening_for_seg(seg) else {
        return false;
    };
    opening.solid || !level.opening_contains_height(&opening, eye_height)
}

fn angle_delta(a: f64, b: f64) -> f64 {
    let mut delta = a - b;
    while delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    }
    while delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    delta
}

fn actor_potentially_visible(
    actor_pos: DVec2,
    radius: f64,
    viewer_pos: DVec2,
    viewer_angle: f64,
    solid_intervals: &[AngleInterval],
    _actor_center_z: f64,
) -> bool {
    let rel = actor_pos - viewer_pos;
    let dist_sq = rel.length_squared();
    if dist_sq > 4096.0 * 4096.0 || dist_sq < 1.0 {
        return false;
    }
    let center = angle_delta(rel.y.atan2(rel.x), viewer_angle);
    let half_span = (radius / dist_sq.sqrt()).asin().clamp(0.01, 0.25);
    let interval = AngleInterval {
        start: center - half_span,
        end: center + half_span,
    };
    if interval.end < -std::f64::consts::FRAC_PI_2 * 0.7
        || interval.start > std::f64::consts::FRAC_PI_2 * 0.7
    {
        return false;
    }
    !angle_intervals_cover(solid_intervals, interval)
}

fn light_color(light_level: i16) -> [f32; 4] {
    let intensity = (light_level as f32 / 255.0).clamp(0.25, 1.0);
    [intensity, intensity, intensity, 1.0]
}

fn missing_texture_name(texture_name: &str) -> String {
    if texture_name.is_empty() || texture_name == "-" {
        "__missing".to_string()
    } else {
        texture_name.to_string()
    }
}

fn actor_sprite_name(type_id: i16, available_sprites: &HashSet<String>) -> String {
    let candidates = match type_id {
        3004 => &["POSSA1", "CPOSA1", "TROOA1"][..],
        9 => &["SPOSA1", "POSSA1", "TROOA1"][..],
        3001 => &["TROOA1", "CPOSA1", "POSSA1"][..],
        3002 | 58 => &["SARGA1", "TROOA1", "POSSA1"][..],
        3003 => &["BOS2A1C1", "BOSFA1C1", "TROOA1"][..],
        3005 => &["HEADA1", "SKULA1", "TROOA1"][..],
        3006 => &["SKULA1", "HEADA1", "TROOA1"][..],
        7 => &["SPIDA1D1", "CYBRA1", "TROOA1"][..],
        16 => &["CYBRA1", "SPIDA1D1", "TROOA1"][..],
        2011 => &["STIMA0", "BON1A0", "__missing"][..],
        2012 => &["MEDIA0", "STIMA0", "__missing"][..],
        2014 => &["BON1A0", "STIMA0", "__missing"][..],
        2015 => &["BON2A0", "ARM1A0", "__missing"][..],
        2018 => &["ARM1A0", "BON2A0", "__missing"][..],
        2019 => &["ARM2A0", "ARM1A0", "__missing"][..],
        2001 => &["SHOTA0", "CLIPA0", "__missing"][..],
        2002 => &["MGUNA0", "CLIPA0", "__missing"][..],
        2007 => &["CLIPA0", "AMMOA0", "__missing"][..],
        2008 => &["SHELA0", "SBOXA0", "__missing"][..],
        5 => &["BKEYA0", "BSKUA0", "__missing"][..],
        6 => &["YKEYA0", "YSKUA0", "__missing"][..],
        13 => &["RKEYA0", "RSKUA0", "__missing"][..],
        38 => &["RSKUA0", "RKEYA0", "__missing"][..],
        39 => &["YSKUA0", "YKEYA0", "__missing"][..],
        40 => &["BSKUA0", "BKEYA0", "__missing"][..],
        _ => &["TROOA1", "POSSA1", "__missing"][..],
    };

    for candidate in candidates {
        if available_sprites.contains(*candidate) || *candidate == "__missing" {
            return (*candidate).to_string();
        }
    }

    available_sprites
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(|| "__missing".to_string())
}
