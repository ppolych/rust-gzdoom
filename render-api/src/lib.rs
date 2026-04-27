use anyhow::Result;
use glam::DVec2;

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

#[derive(Clone, Debug, Default)]
pub struct RenderScene {
    pub flats: Vec<FlatTriangle>,
    pub walls: Vec<WallQuad>,
    pub sprites: Vec<Sprite>,
}

pub trait Renderer {
    fn begin_frame(&mut self) -> Result<()>;
    fn end_frame(&mut self) -> Result<()>;
    fn render_scene(&mut self, scene: &RenderScene, view: &ViewState) -> Result<()>;
    fn load_texture(&mut self, name: &str, image: &TextureImage) -> Result<()>;
}
