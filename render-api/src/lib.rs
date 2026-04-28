use anyhow::Result;
use glam::DVec2;

pub const MAX_POINT_LIGHTS: usize = 16;

#[derive(Clone, Debug)]
pub struct ViewState {
    pub position: DVec2,
    pub angle: f64,
    pub eye_height: f32,
    pub fov_y_radians: f32,
}

#[derive(Clone, Debug)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct FlatTriangle {
    pub texture_name: String,
    pub positions: [[f32; 3]; 3],
    pub uvs: [[f32; 2]; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub enum WallSectionKind {
    Upper,
    Lower,
    MiddleSolid,
    MiddleMasked,
}

#[derive(Clone, Debug)]
pub struct WallQuad {
    pub texture_name: String,
    pub section_kind: WallSectionKind,
    pub masked: bool,
    pub start: DVec2,
    pub end: DVec2,
    pub bottom_z: f32,
    pub top_z: f32,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub position: DVec2,
    pub bottom_z: f32,
    pub texture_name: String,
    pub width: f32,
    pub height: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct PointLight {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderDebugMode {
    #[default]
    Lit,
    Solid,
    Normals,
    Uv,
    LightOnly,
    TextureOnly,
}

impl RenderDebugMode {
    pub fn shader_value(self) -> f32 {
        match self {
            Self::Lit => 0.0,
            Self::Solid => 1.0,
            Self::Normals => 2.0,
            Self::Uv => 3.0,
            Self::LightOnly => 4.0,
            Self::TextureOnly => 5.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RenderScene {
    pub flats: Vec<FlatTriangle>,
    pub walls: Vec<WallQuad>,
    pub sprites: Vec<Sprite>,
    pub point_lights: Vec<PointLight>,
    pub dynamic_lighting_enabled: bool,
    pub ambient_strength: f32,
    pub debug_mode: RenderDebugMode,
}

pub trait Renderer {
    fn begin_frame(&mut self) -> Result<()>;
    fn end_frame(&mut self) -> Result<()>;
    fn render_scene(&mut self, scene: &RenderScene, view: &ViewState) -> Result<()>;
    fn load_texture(&mut self, name: &str, image: &TextureImage) -> Result<()>;
}
