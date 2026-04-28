use anyhow::{anyhow, Result};
use ash::vk;
use glam::DVec2;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use render_api::{Renderer, MAX_POINT_LIGHTS};
use std::ffi::CStr;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RenderVertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub world_pos: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct LightUniform {
    config: [f32; 4],
    light_position_radius: [[f32; 4]; MAX_POINT_LIGHTS],
    light_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
}

pub struct VulkanRenderer {
    _entry: ash::Entry,
    instance: ash::Instance,
    device: ash::Device,
    _physical_device: vk::PhysicalDevice,
    queue: vk::Queue,
    _queue_family_index: u32,
    surface: vk::SurfaceKHR,
    surface_loader: ash::extensions::khr::Surface,
    swapchain_loader: ash::extensions::khr::Swapchain,
    swapchain: vk::SwapchainKHR,
    _swapchain_images: Vec<vk::Image>,
    swapchain_format: vk::Format,
    swapchain_image_views: Vec<vk::ImageView>,
    depth_format: vk::Format,
    depth_image: vk::Image,
    depth_image_memory: vk::DeviceMemory,
    depth_image_view: vk::ImageView,
    render_pass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    image_available_semaphore: vk::Semaphore,
    render_finished_semaphore: vk::Semaphore,
    fence: vk::Fence,
    current_image: usize,
    width: u32,
    height: u32,
    pipeline_layout: vk::PipelineLayout,
    opaque_pipeline: vk::Pipeline,
    alpha_pipeline: vk::Pipeline,
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    vertex_buffer_capacity: usize,
    light_uniform_buffer: vk::Buffer,
    light_uniform_buffer_memory: vk::DeviceMemory,
    vertex_count: usize,
    draw_calls: Vec<DrawCall>,
    textures: std::collections::HashMap<String, VulkanTexture>,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    sampler: vk::Sampler,
}

struct DrawCall {
    texture_name: String,
    vertex_offset: u32,
    vertex_count: u32,
    alpha_blend: bool,
}

struct VulkanTexture {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    descriptor_set: vk::DescriptorSet,
    has_alpha: bool,
}

fn render_world_pos(p: [f32; 3]) -> [f32; 3] {
    [p[0], p[2], p[1]]
}

impl VulkanRenderer {
    const TEX_VERT_SPV: &'static [u8] = include_bytes!("../shaders/tex.vert.spv");
    const TEX_FRAG_SPV: &'static [u8] = include_bytes!("../shaders/tex.frag.spv");
    const FORCE_DEBUG_TRIANGLE: bool = false;
    const TRACE_RENDERER: bool = false;
    const DEBUG_GRAY_CLEAR: bool = false;

    #[allow(dead_code)]
    fn debug_vertex_shader_glsl() -> &'static str {
        r#"#version 450
layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 3) in vec3 in_world_pos;
layout(location = 4) in vec3 in_normal;

layout(location = 0) out vec2 out_uv;
layout(location = 1) out vec4 out_color;
layout(location = 2) out vec3 out_world_pos;
layout(location = 3) out vec3 out_normal;

void main() {
    gl_Position = vec4(in_pos, 1.0);
    out_uv = in_uv;
    out_color = in_color;
    out_world_pos = in_world_pos;
    out_normal = in_normal;
}
"#
    }

    #[allow(dead_code)]
    fn textured_fragment_shader_glsl() -> &'static str {
        r#"#version 450
layout(location = 0) in vec2 in_uv;
layout(location = 1) in vec4 in_color;
layout(location = 2) in vec3 in_world_pos;
layout(location = 3) in vec3 in_normal;

layout(binding = 0) uniform sampler2D tex;

#define MAX_LIGHTS 16

layout(binding = 1) uniform LightUniform {
    vec4 config; // x=light_count, y=ambient, z=dynamic_enabled
    vec4 light_position_radius[MAX_LIGHTS];
    vec4 light_color_intensity[MAX_LIGHTS];
} lights;

layout(location = 0) out vec4 out_color;

void main() {
    vec4 tex_color = texture(tex, in_uv);
    if (tex_color.a < 0.1) discard;

    vec3 lighting = vec3(1.0);
    if (lights.config.z > 0.5) {
        vec3 normal = normalize(in_normal);
        lighting = vec3(max(lights.config.y, 0.0));
        int light_count = min(int(lights.config.x + 0.5), MAX_LIGHTS);
        for (int i = 0; i < light_count; i++) {
            vec3 light_pos = lights.light_position_radius[i].xyz;
            float radius = max(lights.light_position_radius[i].w, 0.001);
            vec3 to_light = light_pos - in_world_pos;
            float dist = length(to_light);
            if (dist < radius) {
                vec3 light_dir = to_light / max(dist, 0.001);
                float ndotl = max(dot(normal, light_dir), 0.0);
                float attenuation = max(1.0 - (dist / radius), 0.0);
                vec3 light_color = lights.light_color_intensity[i].rgb;
                float intensity = lights.light_color_intensity[i].a;
                lighting += light_color * intensity * attenuation * ndotl;
            }
        }
    }

    vec4 lit = vec4(clamp(tex_color.rgb * in_color.rgb * lighting, 0.0, 1.0), tex_color.a * in_color.a);
    out_color = lit;
    if (out_color.a < 0.1) discard;
}
"#
    }

    fn append_debug_geometry(vertices: &mut Vec<RenderVertex>, draw_calls: &mut Vec<DrawCall>) {
        let start = vertices.len() as u32;
        let texture_name = "__missing".to_string();

        let triangle = [
            RenderVertex {
                pos: [-0.6, -0.4, 0.0],
                uv: [0.0, 1.0],
                color: [1.0, 0.0, 0.0, 1.0],
                world_pos: [-0.6, 0.0, -0.4],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [0.0, 0.6, 0.0],
                uv: [0.5, 0.0],
                color: [0.0, 1.0, 0.0, 1.0],
                world_pos: [0.0, 0.0, 0.6],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [0.6, -0.4, 0.0],
                uv: [1.0, 1.0],
                color: [0.0, 0.0, 1.0, 1.0],
                world_pos: [0.6, 0.0, -0.4],
                normal: [0.0, 1.0, 0.0],
            },
        ];
        vertices.extend_from_slice(&triangle);
        draw_calls.push(DrawCall {
            texture_name: texture_name.clone(),
            vertex_offset: start,
            vertex_count: triangle.len() as u32,
            alpha_blend: false,
        });

        let quad_start = vertices.len() as u32;
        let quad = [
            RenderVertex {
                pos: [-0.9, -0.9, 0.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.9, 0.0, -0.9],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [-0.9, -0.2, 0.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.9, 0.0, -0.2],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [-0.2, -0.2, 0.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.2, 0.0, -0.2],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [-0.9, -0.9, 0.0],
                uv: [0.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.9, 0.0, -0.9],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [-0.2, -0.2, 0.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.2, 0.0, -0.2],
                normal: [0.0, 1.0, 0.0],
            },
            RenderVertex {
                pos: [-0.2, -0.9, 0.0],
                uv: [1.0, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
                world_pos: [-0.2, 0.0, -0.9],
                normal: [0.0, 1.0, 0.0],
            },
        ];
        vertices.extend_from_slice(&quad);
        draw_calls.push(DrawCall {
            texture_name,
            vertex_offset: quad_start,
            vertex_count: quad.len() as u32,
            alpha_blend: false,
        });
    }

    fn create_framebuffers(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        image_views: &[vk::ImageView],
        depth_view: vk::ImageView,
        width: u32,
        height: u32,
    ) -> Result<Vec<vk::Framebuffer>> {
        let mut framebuffers = Vec::new();
        for &image_view in image_views {
            let attachments = [image_view, depth_view];
            let create_info = vk::FramebufferCreateInfo::builder()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(width)
                .height(height)
                .layers(1);

            let framebuffer = unsafe { device.create_framebuffer(&create_info, None)? };
            framebuffers.push(framebuffer);
        }
        Ok(framebuffers)
    }

    pub fn new(window: &Window) -> Result<Self> {
        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        let entry = unsafe { ash::Entry::load()? };
        let instance = Self::create_instance(&entry)?;
        let surface = Self::create_surface(
            &entry,
            &instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
        )?;

        let surface_loader = ash::extensions::khr::Surface::new(&entry, &instance);
        let (physical_device, queue_family_index) =
            Self::pick_physical_device(&instance, &surface_loader, surface)?;
        let (device, queue) = Self::create_device(&instance, physical_device, queue_family_index)?;

        let swapchain_loader = ash::extensions::khr::Swapchain::new(&instance, &device);
        let (swapchain, swapchain_images, swapchain_image_views, swapchain_format) =
            Self::create_swapchain(
                &device,
                &swapchain_loader,
                surface,
                physical_device,
                &surface_loader,
                queue_family_index,
                width,
                height,
            )?;

        let depth_format = Self::find_depth_format(&instance, physical_device)?;
        let render_pass = Self::create_render_pass(&device, swapchain_format, depth_format)?;
        let (depth_image, depth_image_memory, depth_image_view) = Self::create_depth_resources(
            &instance,
            &device,
            physical_device,
            width,
            height,
            depth_format,
            render_pass,
            queue,
            queue_family_index,
        )?;
        let framebuffers = Self::create_framebuffers(
            &device,
            render_pass,
            &swapchain_image_views,
            depth_image_view,
            width,
            height,
        )?;

        let sampler = Self::create_sampler(&device)?;
        let descriptor_set_layout = Self::create_descriptor_set_layout(&device)?;
        let (pipeline_layout, opaque_pipeline, alpha_pipeline) =
            Self::create_graphics_pipelines(&device, render_pass, descriptor_set_layout)?;
        let descriptor_pool = Self::create_descriptor_pool(&device)?;

        let vertex_buffer_capacity = 200_000;
        let (vertex_buffer, vertex_buffer_memory) = Self::create_buffer(
            &instance,
            physical_device,
            &device,
            (std::mem::size_of::<RenderVertex>() * vertex_buffer_capacity) as vk::DeviceSize,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let (light_uniform_buffer, light_uniform_buffer_memory) = Self::create_buffer(
            &instance,
            physical_device,
            &device,
            std::mem::size_of::<LightUniform>() as vk::DeviceSize,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        let command_pool = Self::create_command_pool(&device, queue_family_index)?;
        let command_buffers =
            Self::create_command_buffers(&device, command_pool, framebuffers.len())?;
        let (image_available_semaphore, render_finished_semaphore, fence) =
            Self::create_sync_objects(&device)?;

        Ok(Self {
            _entry: entry,
            instance,
            device,
            _physical_device: physical_device,
            queue,
            _queue_family_index: queue_family_index,
            surface,
            surface_loader,
            swapchain_loader,
            swapchain,
            _swapchain_images: swapchain_images,
            swapchain_format,
            swapchain_image_views,
            depth_format,
            depth_image,
            depth_image_memory,
            depth_image_view,
            render_pass,
            framebuffers,
            command_pool,
            command_buffers,
            image_available_semaphore,
            render_finished_semaphore,
            fence,
            current_image: 0,
            width,
            height,
            pipeline_layout,
            opaque_pipeline,
            alpha_pipeline,
            vertex_buffer,
            vertex_buffer_memory,
            vertex_buffer_capacity,
            light_uniform_buffer,
            light_uniform_buffer_memory,
            vertex_count: 0,
            draw_calls: Vec::new(),
            textures: std::collections::HashMap::new(),
            descriptor_set_layout,
            descriptor_pool,
            sampler,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        unsafe {
            self.device.device_wait_idle()?;
        }
        self.recreate_swapchain_resources(width, height)
    }

    fn create_instance(entry: &ash::Entry) -> Result<ash::Instance> {
        let app_info = vk::ApplicationInfo::builder()
            .application_name(CStr::from_bytes_with_nul(b"Rust GZDoom\0")?)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(CStr::from_bytes_with_nul(b"No Engine\0")?)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        let extension_names = [
            ash::extensions::khr::Surface::name().as_ptr(),
            ash::extensions::khr::WaylandSurface::name().as_ptr(),
        ];

        let create_info = vk::InstanceCreateInfo::builder()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        unsafe { Ok(entry.create_instance(&create_info, None)?) }
    }

    fn create_surface(
        entry: &ash::Entry,
        instance: &ash::Instance,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> Result<vk::SurfaceKHR> {
        unsafe {
            Ok(ash_window::create_surface(
                entry,
                instance,
                display_handle,
                window_handle,
                None,
            )?)
        }
    }

    fn create_graphics_pipelines(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> Result<(vk::PipelineLayout, vk::Pipeline, vk::Pipeline)> {
        debug_assert_eq!(std::mem::size_of::<RenderVertex>(), 60);
        debug_assert_eq!(std::mem::offset_of!(RenderVertex, pos), 0);
        debug_assert_eq!(std::mem::offset_of!(RenderVertex, uv), 12);
        debug_assert_eq!(std::mem::offset_of!(RenderVertex, color), 20);
        debug_assert_eq!(std::mem::offset_of!(RenderVertex, world_pos), 36);
        debug_assert_eq!(std::mem::offset_of!(RenderVertex, normal), 48);
        if Self::TRACE_RENDERER {
            eprintln!(
                "RenderVertex layout: size={} pos={} uv={} color={} world_pos={} normal={} stride={}",
                std::mem::size_of::<RenderVertex>(),
                std::mem::offset_of!(RenderVertex, pos),
                std::mem::offset_of!(RenderVertex, uv),
                std::mem::offset_of!(RenderVertex, color),
                std::mem::offset_of!(RenderVertex, world_pos),
                std::mem::offset_of!(RenderVertex, normal),
                std::mem::size_of::<RenderVertex>(),
            );
            eprintln!("Vertex attrs: loc0=R32G32B32@0 loc1=R32G32@12 loc2=R32G32B32A32@20 loc3=R32G32B32@36 loc4=R32G32B32@48");
        }

        let (vert_module, frag_module) = (
            Self::create_shader_module_from_spirv_bytes(device, Self::TEX_VERT_SPV)?,
            Self::create_shader_module_from_spirv_bytes(device, Self::TEX_FRAG_SPV)?,
        );

        let entry_point = CStr::from_bytes_with_nul(b"main\0")?;
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(entry_point)
                .build(),
            vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(entry_point)
                .build(),
        ];

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                binding: 0,
                stride: std::mem::size_of::<RenderVertex>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            }])
            .vertex_attribute_descriptions(&[
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 0,
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 1,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 12,
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 2,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: 20,
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 3,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 36,
                },
                vk::VertexInputAttributeDescription {
                    binding: 0,
                    location: 4,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 48,
                },
            ]);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let opaque_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);

        let alpha_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .alpha_blend_op(vk::BlendOp::ADD);

        let opaque_color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&opaque_blend_attachment));

        let alpha_color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&alpha_blend_attachment));

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_states);

        let debug_depth = !Self::FORCE_DEBUG_TRIANGLE;
        let opaque_depth_stencil = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(debug_depth)
            .depth_write_enable(debug_depth)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let alpha_depth_stencil = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(debug_depth)
            .depth_write_enable(false)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout));
        let pipeline_layout =
            unsafe { device.create_pipeline_layout(&pipeline_layout_info, None)? };

        let opaque_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&opaque_color_blending)
            .depth_stencil_state(&opaque_depth_stencil)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let alpha_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&alpha_color_blending)
            .depth_stencil_state(&alpha_depth_stencil)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipelines = unsafe {
            device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[opaque_pipeline_info.build(), alpha_pipeline_info.build()],
                    None,
                )
                .map_err(|e| anyhow!("Pipeline creation failed: {:?}", e))?
        };

        unsafe {
            device.destroy_shader_module(vert_module, None);
            device.destroy_shader_module(frag_module, None);
        }

        Ok((pipeline_layout, pipelines[0], pipelines[1]))
    }

    fn create_shader_module(device: &ash::Device, bytes: &[u32]) -> Result<vk::ShaderModule> {
        let code = bytes;
        let create_info = vk::ShaderModuleCreateInfo::builder().code(code);
        unsafe { Ok(device.create_shader_module(&create_info, None)?) }
    }

    fn create_shader_module_from_spirv_bytes(
        device: &ash::Device,
        bytes: &[u8],
    ) -> Result<vk::ShaderModule> {
        if bytes.len() % 4 != 0 {
            return Err(anyhow!("SPIR-V bytecode length must be 4-byte aligned"));
        }
        let words = bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<_>>();
        Self::create_shader_module(device, &words)
    }

    fn pick_physical_device(
        instance: &ash::Instance,
        surface_loader: &ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
    ) -> Result<(vk::PhysicalDevice, u32)> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        for device in devices {
            if Self::is_device_suitable(instance, surface_loader, surface, device) {
                if let Some(queue_family_index) =
                    Self::find_queue_family(instance, surface_loader, surface, device)
                {
                    return Ok((device, queue_family_index));
                }
            }
        }
        Err(anyhow!("No suitable Vulkan device found"))
    }

    fn is_device_suitable(
        _instance: &ash::Instance,
        _surface_loader: &ash::extensions::khr::Surface,
        _surface: vk::SurfaceKHR,
        _device: vk::PhysicalDevice,
    ) -> bool {
        true
    }

    fn find_queue_family(
        instance: &ash::Instance,
        surface_loader: &ash::extensions::khr::Surface,
        surface: vk::SurfaceKHR,
        device: vk::PhysicalDevice,
    ) -> Option<u32> {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };
        for (i, queue_family) in queue_families.iter().enumerate() {
            if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                if unsafe {
                    surface_loader
                        .get_physical_device_surface_support(device, i as u32, surface)
                        .unwrap_or(false)
                } {
                    return Some(i as u32);
                }
            }
        }
        None
    }

    fn create_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Result<(ash::Device, vk::Queue)> {
        let queue_create_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&[1.0]);

        let extension_names = [ash::extensions::khr::Swapchain::name().as_ptr()];

        let create_info = vk::DeviceCreateInfo::builder()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&extension_names);

        let device = unsafe { instance.create_device(physical_device, &create_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };
        Ok((device, queue))
    }

    fn create_swapchain(
        device: &ash::Device,
        swapchain_loader: &ash::extensions::khr::Swapchain,
        surface: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice,
        surface_loader: &ash::extensions::khr::Surface,
        queue_family_index: u32,
        width: u32,
        height: u32,
    ) -> Result<(
        vk::SwapchainKHR,
        Vec<vk::Image>,
        Vec<vk::ImageView>,
        vk::Format,
    )> {
        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?
        };
        let surface_formats = unsafe {
            surface_loader.get_physical_device_surface_formats(physical_device, surface)?
        };
        let surface_format = surface_formats[0];

        let extent = vk::Extent2D { width, height };
        eprintln!(
            "create_swapchain: extent={}x{} format={:?}",
            extent.width, extent.height, surface_format.format
        );
        let image_count = surface_capabilities.min_image_count + 1;
        let queue_family_indices = [queue_family_index];

        let create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);

        let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None)? };
        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
        let image_views = Self::create_image_views(device, &images, surface_format.format)?;

        Ok((swapchain, images, image_views, surface_format.format))
    }

    fn create_image_views(
        device: &ash::Device,
        images: &[vk::Image],
        format: vk::Format,
    ) -> Result<Vec<vk::ImageView>> {
        let mut image_views = Vec::new();
        for &image in images {
            let create_info = vk::ImageViewCreateInfo::builder()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .components(vk::ComponentMapping::default())
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(1)
                        .build(),
                );

            let image_view = unsafe { device.create_image_view(&create_info, None)? };
            image_views.push(image_view);
        }
        Ok(image_views)
    }

    fn create_render_pass(
        device: &ash::Device,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> Result<vk::RenderPass> {
        eprintln!(
            "create_render_pass: color_format={:?} depth_format={:?} color_attachment=swapchain load=CLEAR store=STORE",
            color_format,
            depth_format
        );
        let color_attachment = vk::AttachmentDescription::builder()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let depth_attachment = vk::AttachmentDescription::builder()
            .format(depth_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::DONT_CARE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let color_attachment_ref = vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let depth_attachment_ref = vk::AttachmentReference::builder()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref))
            .depth_stencil_attachment(&depth_attachment_ref);

        let dependency = vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            );

        let attachments = [color_attachment.build(), depth_attachment.build()];
        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        unsafe { Ok(device.create_render_pass(&create_info, None)?) }
    }

    fn find_supported_format(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        candidates: &[vk::Format],
        tiling: vk::ImageTiling,
        features: vk::FormatFeatureFlags,
    ) -> Result<vk::Format> {
        for &format in candidates {
            let props =
                unsafe { instance.get_physical_device_format_properties(physical_device, format) };
            let supported = match tiling {
                vk::ImageTiling::LINEAR => props.linear_tiling_features.contains(features),
                vk::ImageTiling::OPTIMAL => props.optimal_tiling_features.contains(features),
                _ => false,
            };
            if supported {
                return Ok(format);
            }
        }
        Err(anyhow!("No supported format found"))
    }

    fn find_depth_format(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<vk::Format> {
        Self::find_supported_format(
            instance,
            physical_device,
            &[
                vk::Format::D32_SFLOAT,
                vk::Format::D32_SFLOAT_S8_UINT,
                vk::Format::D24_UNORM_S8_UINT,
            ],
            vk::ImageTiling::OPTIMAL,
            vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    fn find_memory_type(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        type_filter: u32,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        let mem_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        for i in 0..mem_properties.memory_type_count {
            if (type_filter & (1 << i)) != 0
                && (mem_properties.memory_types[i as usize].property_flags & properties)
                    == properties
            {
                return Ok(i);
            }
        }
        Err(anyhow!("Failed to find suitable memory type"))
    }

    fn create_buffer(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<(vk::Buffer, vk::DeviceMemory)> {
        let buffer_info = vk::BufferCreateInfo::builder()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = unsafe { device.create_buffer(&buffer_info, None)? };
        let mem_requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let memory_type = Self::find_memory_type(
            instance,
            physical_device,
            mem_requirements.memory_type_bits,
            properties,
        )?;

        let alloc_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type);

        let memory = unsafe { device.allocate_memory(&alloc_info, None)? };
        unsafe { device.bind_buffer_memory(buffer, memory, 0)? };

        Ok((buffer, memory))
    }

    fn create_command_pool(
        device: &ash::Device,
        queue_family_index: u32,
    ) -> Result<vk::CommandPool> {
        let create_info =
            vk::CommandPoolCreateInfo::builder().queue_family_index(queue_family_index);

        unsafe { Ok(device.create_command_pool(&create_info, None)?) }
    }

    fn create_command_buffers(
        device: &ash::Device,
        command_pool: vk::CommandPool,
        count: usize,
    ) -> Result<Vec<vk::CommandBuffer>> {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count as u32);

        unsafe { Ok(device.allocate_command_buffers(&allocate_info)?) }
    }

    fn create_sync_objects(
        device: &ash::Device,
    ) -> Result<(vk::Semaphore, vk::Semaphore, vk::Fence)> {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

        unsafe {
            let image_available = device.create_semaphore(&semaphore_info, None)?;
            let render_finished = device.create_semaphore(&semaphore_info, None)?;
            let fence = device.create_fence(&fence_info, None)?;
            Ok((image_available, render_finished, fence))
        }
    }

    fn create_sampler(device: &ash::Device) -> Result<vk::Sampler> {
        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(false)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST);

        unsafe { Ok(device.create_sampler(&sampler_info, None)?) }
    }

    fn create_descriptor_set_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout> {
        let sampler_layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        let light_layout_binding = vk::DescriptorSetLayoutBinding::builder()
            .binding(1)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        let bindings = [sampler_layout_binding.build(), light_layout_binding.build()];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings);

        unsafe { Ok(device.create_descriptor_set_layout(&layout_info, None)?) }
    }

    fn create_descriptor_pool(device: &ash::Device) -> Result<vk::DescriptorPool> {
        let pool_sizes = [
            vk::DescriptorPoolSize::builder()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(4096)
                .build(),
            vk::DescriptorPoolSize::builder()
                .ty(vk::DescriptorType::UNIFORM_BUFFER)
                .descriptor_count(4096)
                .build(),
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(4096);

        unsafe { Ok(device.create_descriptor_pool(&pool_info, None)?) }
    }

    fn create_image(
        &self,
        width: u32,
        height: u32,
        format: vk::Format,
        tiling: vk::ImageTiling,
        usage: vk::ImageUsageFlags,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<(vk::Image, vk::DeviceMemory)> {
        let image_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1);

        let image = unsafe { self.device.create_image(&image_info, None)? };
        let mem_requirements = unsafe { self.device.get_image_memory_requirements(image) };
        let memory_type = Self::find_memory_type(
            &self.instance,
            self._physical_device,
            mem_requirements.memory_type_bits,
            properties,
        )?;

        let alloc_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type);

        let memory = unsafe { self.device.allocate_memory(&alloc_info, None)? };
        unsafe { self.device.bind_image_memory(image, memory, 0)? };

        Ok((image, memory))
    }

    fn create_depth_resources(
        instance: &ash::Instance,
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        width: u32,
        height: u32,
        depth_format: vk::Format,
        _render_pass: vk::RenderPass,
        _queue: vk::Queue,
        _queue_family_index: u32,
    ) -> Result<(vk::Image, vk::DeviceMemory, vk::ImageView)> {
        let image_info = vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(depth_format)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1);

        let image = unsafe { device.create_image(&image_info, None)? };
        let mem_requirements = unsafe { device.get_image_memory_requirements(image) };
        let memory_type = Self::find_memory_type(
            instance,
            physical_device,
            mem_requirements.memory_type_bits,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        let alloc_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(mem_requirements.size)
            .memory_type_index(memory_type);

        let memory = unsafe { device.allocate_memory(&alloc_info, None)? };
        unsafe { device.bind_image_memory(image, memory, 0)? };

        let view_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(depth_format)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            );
        let view = unsafe { device.create_image_view(&view_info, None)? };

        Ok((image, memory, view))
    }

    fn destroy_swapchain_resources(&mut self) {
        unsafe {
            for &framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.framebuffers.clear();
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_pipeline(self.opaque_pipeline, None);
            self.device.destroy_pipeline(self.alpha_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }
            self.swapchain_image_views.clear();
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
        }
    }

    fn recreate_swapchain_resources(&mut self, width: u32, height: u32) -> Result<()> {
        unsafe {
            if !self.command_buffers.is_empty() {
                self.device
                    .free_command_buffers(self.command_pool, &self.command_buffers);
            }
        }
        self.destroy_swapchain_resources();
        let (swapchain, images, image_views, swapchain_format) = Self::create_swapchain(
            &self.device,
            &self.swapchain_loader,
            self.surface,
            self._physical_device,
            &self.surface_loader,
            self._queue_family_index,
            width,
            height,
        )?;
        self.swapchain = swapchain;
        self._swapchain_images = images;
        self.swapchain_format = swapchain_format;
        self.swapchain_image_views = image_views;
        self.width = width;
        self.height = height;
        self.render_pass =
            Self::create_render_pass(&self.device, self.swapchain_format, self.depth_format)?;
        let (depth_image, depth_memory, depth_view) = Self::create_depth_resources(
            &self.instance,
            &self.device,
            self._physical_device,
            width,
            height,
            self.depth_format,
            self.render_pass,
            self.queue,
            self._queue_family_index,
        )?;
        self.depth_image = depth_image;
        self.depth_image_memory = depth_memory;
        self.depth_image_view = depth_view;
        self.framebuffers = Self::create_framebuffers(
            &self.device,
            self.render_pass,
            &self.swapchain_image_views,
            self.depth_image_view,
            width,
            height,
        )?;
        let (pipeline_layout, opaque_pipeline, alpha_pipeline) = Self::create_graphics_pipelines(
            &self.device,
            self.render_pass,
            self.descriptor_set_layout,
        )?;
        self.pipeline_layout = pipeline_layout;
        self.opaque_pipeline = opaque_pipeline;
        self.alpha_pipeline = alpha_pipeline;
        self.command_buffers =
            Self::create_command_buffers(&self.device, self.command_pool, self.framebuffers.len())?;
        Ok(())
    }

    fn transition_image_layout(
        &self,
        image: vk::Image,
        _format: vk::Format,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) -> Result<()> {
        let command_buffer = self.begin_single_time_commands()?;

        let mut barrier = vk::ImageMemoryBarrier::builder()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            );

        let (src_stage, dst_stage) = match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => {
                barrier.src_access_mask = vk::AccessFlags::empty();
                barrier.dst_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                (
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                )
            }
            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => {
                barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
                barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;
                (
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                )
            }
            _ => return Err(anyhow!("Unsupported layout transition")),
        };

        unsafe {
            self.device.cmd_pipeline_barrier(
                command_buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&barrier),
            );
        }

        self.end_single_time_commands(command_buffer)?;
        Ok(())
    }

    fn copy_buffer_to_image(
        &self,
        buffer: vk::Buffer,
        image: vk::Image,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let command_buffer = self.begin_single_time_commands()?;

        let region = vk::BufferImageCopy::builder()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(
                vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .mip_level(0)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            )
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });

        unsafe {
            self.device.cmd_copy_buffer_to_image(
                command_buffer,
                buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&region),
            );
        }

        self.end_single_time_commands(command_buffer)?;
        Ok(())
    }

    fn begin_single_time_commands(&self) -> Result<vk::CommandBuffer> {
        let alloc_info = vk::CommandBufferAllocateInfo::builder()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(self.command_pool)
            .command_buffer_count(1);

        let command_buffer = unsafe { self.device.allocate_command_buffers(&alloc_info)?[0] };
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .begin_command_buffer(command_buffer, &begin_info)?
        };
        Ok(command_buffer)
    }

    fn end_single_time_commands(&self, command_buffer: vk::CommandBuffer) -> Result<()> {
        unsafe {
            self.device.end_command_buffer(command_buffer)?;
            let submit_info =
                vk::SubmitInfo::builder().command_buffers(std::slice::from_ref(&command_buffer));
            self.device.queue_submit(
                self.queue,
                std::slice::from_ref(&submit_info),
                vk::Fence::null(),
            )?;
            self.device.queue_wait_idle(self.queue)?;
            self.device
                .free_command_buffers(self.command_pool, std::slice::from_ref(&command_buffer));
        }
        Ok(())
    }

    fn create_image_view_with_format(
        &self,
        image: vk::Image,
        format: vk::Format,
    ) -> Result<vk::ImageView> {
        let view_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    .build(),
            );

        unsafe { Ok(self.device.create_image_view(&view_info, None)?) }
    }

    fn update_light_uniform(&self, scene: &render_api::RenderScene) -> Result<()> {
        let mut uniform = LightUniform {
            config: [
                scene.point_lights.len().min(MAX_POINT_LIGHTS) as f32,
                scene.ambient_strength.max(0.0),
                if scene.dynamic_lighting_enabled {
                    1.0
                } else {
                    0.0
                },
                scene.debug_mode.shader_value(),
            ],
            light_position_radius: [[0.0; 4]; MAX_POINT_LIGHTS],
            light_color_intensity: [[0.0; 4]; MAX_POINT_LIGHTS],
        };

        for (i, light) in scene.point_lights.iter().take(MAX_POINT_LIGHTS).enumerate() {
            uniform.light_position_radius[i] = [
                light.position[0],
                light.position[1],
                light.position[2],
                light.radius,
            ];
            uniform.light_color_intensity[i] = [
                light.color[0],
                light.color[1],
                light.color[2],
                light.intensity,
            ];
        }

        unsafe {
            let data_ptr = self.device.map_memory(
                self.light_uniform_buffer_memory,
                0,
                std::mem::size_of::<LightUniform>() as vk::DeviceSize,
                vk::MemoryMapFlags::empty(),
            )?;
            std::ptr::copy_nonoverlapping(&uniform, data_ptr as *mut LightUniform, 1);
            self.device.unmap_memory(self.light_uniform_buffer_memory);
        }
        Ok(())
    }

    fn create_descriptor_set(&self, view: vk::ImageView) -> Result<vk::DescriptorSet> {
        let alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(self.descriptor_pool)
            .set_layouts(std::slice::from_ref(&self.descriptor_set_layout));

        let descriptor_set = unsafe { self.device.allocate_descriptor_sets(&alloc_info)?[0] };

        let image_info = vk::DescriptorImageInfo::builder()
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .image_view(view)
            .sampler(self.sampler);
        let light_buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(self.light_uniform_buffer)
            .offset(0)
            .range(std::mem::size_of::<LightUniform>() as vk::DeviceSize);

        let writes = [
            vk::WriteDescriptorSet::builder()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info))
                .build(),
            vk::WriteDescriptorSet::builder()
                .dst_set(descriptor_set)
                .dst_binding(1)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&light_buffer_info))
                .build(),
        ];

        unsafe { self.device.update_descriptor_sets(&writes, &[]) };

        Ok(descriptor_set)
    }
}

impl Renderer for VulkanRenderer {
    fn load_texture(&mut self, name: &str, image: &render_api::TextureImage) -> Result<()> {
        let image_size = (image.width * image.height * 4) as vk::DeviceSize;
        let (staging_buffer, staging_memory) = Self::create_buffer(
            &self.instance,
            self._physical_device,
            &self.device,
            image_size,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            let data_ptr = self.device.map_memory(
                staging_memory,
                0,
                image_size,
                vk::MemoryMapFlags::empty(),
            )?;
            std::ptr::copy_nonoverlapping(
                image.data.as_ptr(),
                data_ptr as *mut u8,
                image.data.len(),
            );
            self.device.unmap_memory(staging_memory);
        }

        let (vk_image, image_memory) = self.create_image(
            image.width,
            image.height,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
        )?;

        self.transition_image_layout(
            vk_image,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        )?;
        self.copy_buffer_to_image(staging_buffer, vk_image, image.width, image.height)?;
        self.transition_image_layout(
            vk_image,
            vk::Format::R8G8B8A8_UNORM,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;

        unsafe {
            self.device.destroy_buffer(staging_buffer, None);
            self.device.free_memory(staging_memory, None);
        }

        let view = self.create_image_view_with_format(vk_image, vk::Format::R8G8B8A8_UNORM)?;
        let descriptor_set = self.create_descriptor_set(view)?;
        let has_alpha = image.data.chunks_exact(4).any(|px| px[3] < 250);

        self.textures.insert(
            name.to_string(),
            VulkanTexture {
                image: vk_image,
                memory: image_memory,
                view,
                descriptor_set,
                has_alpha,
            },
        );

        Ok(())
    }

    fn render_scene(
        &mut self,
        scene: &render_api::RenderScene,
        view: &render_api::ViewState,
    ) -> Result<()> {
        self.update_light_uniform(scene)?;
        let mut opaque_batches: Vec<(String, Vec<RenderVertex>)> = Vec::new();
        let mut alpha_batches: Vec<(f32, String, Vec<RenderVertex>)> = Vec::new();
        let near_plane = 0.1f32;
        // self.update_camera_uniform(view, near_plane, 8192.0)?; // Uniforms currently not used by stable SPIR-V

        let cos_a = view.angle.cos() as f32;
        let sin_a = view.angle.sin() as f32;
        let aspect = self.width as f32 / self.height as f32;
        // Adjust for Doom's vertical stretching (Doom assumes 320x200 pixels are stretched to 4:3)
        let fov_scale = (view.fov_y_radians as f32 * 0.5).tan().recip();
        let vertical_stretch = 1.2f32;

        #[derive(Clone, Copy)]
        struct ProjectVertex {
            p: [f32; 3],
            uv: [f32; 2],
            color: [f32; 4],
            world_pos: [f32; 3],
            normal: [f32; 3],
        }

        let cam_y_for = |p: [f32; 3]| -> f32 {
            let rel_x = p[0] - view.position.x as f32;
            let rel_y = p[1] - view.position.y as f32;
            rel_x * cos_a + rel_y * sin_a
        };

        let interpolate = |a: ProjectVertex, b: ProjectVertex, t: f32| -> ProjectVertex {
            let lerp = |x: f32, y: f32| x + (y - x) * t;
            ProjectVertex {
                p: [
                    lerp(a.p[0], b.p[0]),
                    lerp(a.p[1], b.p[1]),
                    lerp(a.p[2], b.p[2]),
                ],
                uv: [lerp(a.uv[0], b.uv[0]), lerp(a.uv[1], b.uv[1])],
                color: [
                    lerp(a.color[0], b.color[0]),
                    lerp(a.color[1], b.color[1]),
                    lerp(a.color[2], b.color[2]),
                    lerp(a.color[3], b.color[3]),
                ],
                world_pos: [
                    lerp(a.world_pos[0], b.world_pos[0]),
                    lerp(a.world_pos[1], b.world_pos[1]),
                    lerp(a.world_pos[2], b.world_pos[2]),
                ],
                normal: [
                    lerp(a.normal[0], b.normal[0]),
                    lerp(a.normal[1], b.normal[1]),
                    lerp(a.normal[2], b.normal[2]),
                ],
            }
        };

        let clip_near = |polygon: &[ProjectVertex]| -> Vec<ProjectVertex> {
            let mut clipped = Vec::new();
            let Some(mut prev) = polygon.last().copied() else {
                return clipped;
            };
            let mut prev_y = cam_y_for(prev.p);
            let mut prev_inside = prev_y >= near_plane;

            for &current in polygon {
                let current_y = cam_y_for(current.p);
                let current_inside = current_y >= near_plane;

                if current_inside != prev_inside {
                    let denom = current_y - prev_y;
                    if denom.abs() > f32::EPSILON {
                        let t = ((near_plane - prev_y) / denom).clamp(0.0, 1.0);
                        clipped.push(interpolate(prev, current, t));
                    }
                }
                if current_inside {
                    clipped.push(current);
                }

                prev = current;
                prev_y = current_y;
                prev_inside = current_inside;
            }

            clipped
        };

        let project = |p: [f32; 3]| -> Option<[f32; 3]> {
            let rel_x = p[0] - view.position.x as f32;
            let rel_y = p[1] - view.position.y as f32;

            // Doom: 0 is East (+X), 90 is North (+Y)
            let cam_y = rel_x * cos_a + rel_y * sin_a;
            if cam_y < near_plane {
                return None;
            }

            let cam_x = rel_x * sin_a - rel_y * cos_a;
            let x_ndc = (cam_x / cam_y) * fov_scale / aspect;
            let y_ndc = ((p[2] - view.eye_height) / cam_y) * fov_scale * vertical_stretch;
            let z_ndc = (cam_y / 8192.0).clamp(0.0, 1.0);
            Some([x_ndc, -y_ndc, z_ndc])
        };

        let append_projected_polygon =
            |out: &mut Vec<RenderVertex>, polygon: &[ProjectVertex]| -> bool {
                let clipped = clip_near(polygon);
                if clipped.len() < 3 {
                    return false;
                }

                let base = clipped[0];
                for i in 1..clipped.len() - 1 {
                    let tri = [base, clipped[i], clipped[i + 1]];
                    for vertex in tri {
                        let Some(pos) = project(vertex.p) else {
                            return false;
                        };
                        out.push(RenderVertex {
                            pos,
                            uv: vertex.uv,
                            color: vertex.color,
                            world_pos: vertex.world_pos,
                            normal: vertex.normal,
                        });
                    }
                }
                true
            };

        for flat in &scene.flats {
            let polygon = [
                ProjectVertex {
                    p: flat.positions[0],
                    uv: flat.uvs[0],
                    color: flat.color,
                    world_pos: render_world_pos(flat.positions[0]),
                    normal: flat.normal,
                },
                ProjectVertex {
                    p: flat.positions[1],
                    uv: flat.uvs[1],
                    color: flat.color,
                    world_pos: render_world_pos(flat.positions[1]),
                    normal: flat.normal,
                },
                ProjectVertex {
                    p: flat.positions[2],
                    uv: flat.uvs[2],
                    color: flat.color,
                    world_pos: render_world_pos(flat.positions[2]),
                    normal: flat.normal,
                },
            ];
            let mut tri_vertices = Vec::new();
            if !append_projected_polygon(&mut tri_vertices, &polygon) {
                continue;
            }
            opaque_batches.push((flat.texture_name.clone(), tri_vertices));
        }

        for wall in &scene.walls {
            let polygon = [
                ProjectVertex {
                    p: [wall.start.x as f32, wall.start.y as f32, wall.top_z],
                    uv: [wall.uv_min[0], wall.uv_min[1]],
                    color: wall.color,
                    world_pos: [wall.start.x as f32, wall.top_z, wall.start.y as f32],
                    normal: wall.normal,
                },
                ProjectVertex {
                    p: [wall.start.x as f32, wall.start.y as f32, wall.bottom_z],
                    uv: [wall.uv_min[0], wall.uv_max[1]],
                    color: wall.color,
                    world_pos: [wall.start.x as f32, wall.bottom_z, wall.start.y as f32],
                    normal: wall.normal,
                },
                ProjectVertex {
                    p: [wall.end.x as f32, wall.end.y as f32, wall.bottom_z],
                    uv: [wall.uv_max[0], wall.uv_max[1]],
                    color: wall.color,
                    world_pos: [wall.end.x as f32, wall.bottom_z, wall.end.y as f32],
                    normal: wall.normal,
                },
                ProjectVertex {
                    p: [wall.end.x as f32, wall.end.y as f32, wall.top_z],
                    uv: [wall.uv_max[0], wall.uv_min[1]],
                    color: wall.color,
                    world_pos: [wall.end.x as f32, wall.top_z, wall.end.y as f32],
                    normal: wall.normal,
                },
            ];
            let mut quad_vertices = Vec::new();
            if !append_projected_polygon(&mut quad_vertices, &polygon) {
                continue;
            }
            if wall.masked {
                let dist_sq = (wall.start - view.position).length_squared() as f32;
                alpha_batches.push((dist_sq, wall.texture_name.clone(), quad_vertices));
            } else {
                opaque_batches.push((wall.texture_name.clone(), quad_vertices));
            }
        }

        for sprite in &scene.sprites {
            let right_dir = DVec2::new(-view.angle.sin(), view.angle.cos());
            let half_width = sprite.width as f64 * 0.5;
            let p_start = sprite.position - right_dir * half_width;
            let p_end = sprite.position + right_dir * half_width;

            let polygon = [
                ProjectVertex {
                    p: [
                        p_start.x as f32,
                        p_start.y as f32,
                        sprite.bottom_z + sprite.height,
                    ],
                    uv: [0.0, 0.0],
                    color: sprite.color,
                    world_pos: [
                        p_start.x as f32,
                        sprite.bottom_z + sprite.height,
                        p_start.y as f32,
                    ],
                    normal: [0.0, 0.0, 1.0],
                },
                ProjectVertex {
                    p: [p_start.x as f32, p_start.y as f32, sprite.bottom_z],
                    uv: [0.0, 1.0],
                    color: sprite.color,
                    world_pos: [p_start.x as f32, sprite.bottom_z, p_start.y as f32],
                    normal: [0.0, 0.0, 1.0],
                },
                ProjectVertex {
                    p: [p_end.x as f32, p_end.y as f32, sprite.bottom_z],
                    uv: [1.0, 1.0],
                    color: sprite.color,
                    world_pos: [p_end.x as f32, sprite.bottom_z, p_end.y as f32],
                    normal: [0.0, 0.0, 1.0],
                },
                ProjectVertex {
                    p: [
                        p_end.x as f32,
                        p_end.y as f32,
                        sprite.bottom_z + sprite.height,
                    ],
                    uv: [1.0, 0.0],
                    color: sprite.color,
                    world_pos: [
                        p_end.x as f32,
                        sprite.bottom_z + sprite.height,
                        p_end.y as f32,
                    ],
                    normal: [0.0, 0.0, 1.0],
                },
            ];
            let mut sprite_vertices = Vec::new();
            if !append_projected_polygon(&mut sprite_vertices, &polygon) {
                continue;
            }
            let dist_sq = (sprite.position - view.position).length_squared() as f32;
            alpha_batches.push((dist_sq, sprite.texture_name.clone(), sprite_vertices));
        }

        alpha_batches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut vertices = Vec::new();
        let mut draw_calls = Vec::new();

        // Draw opaque batches BACK-TO-FRONT (Reverse of BSP front-to-back order)
        for (texture_name, batch_vertices) in opaque_batches.into_iter().rev() {
            if batch_vertices.is_empty() {
                continue;
            }
            draw_calls.push(DrawCall {
                texture_name,
                vertex_offset: vertices.len() as u32,
                vertex_count: batch_vertices.len() as u32,
                alpha_blend: false,
            });
            vertices.extend(batch_vertices);
        }

        // Alpha batches are already sorted back-to-front
        for (_, texture_name, batch_vertices) in alpha_batches {
            if batch_vertices.is_empty() {
                continue;
            }
            draw_calls.push(DrawCall {
                texture_name,
                vertex_offset: vertices.len() as u32,
                vertex_count: batch_vertices.len() as u32,
                alpha_blend: true,
            });
            vertices.extend(batch_vertices);
        }

        self.vertex_count = vertices.len();
        self.draw_calls = draw_calls;

        if self.vertex_count == 0 {
            Self::append_debug_geometry(&mut vertices, &mut self.draw_calls);
            self.vertex_count = vertices.len();
        }
        if Self::TRACE_RENDERER {
            eprintln!(
                "render_scene: force_debug={} flats={} walls={} sprites={} vertex_count={} draw_calls={}",
                Self::FORCE_DEBUG_TRIANGLE,
                scene.flats.len(),
                scene.walls.len(),
                scene.sprites.len(),
                self.vertex_count,
                self.draw_calls.len(),
            );
        }

        if self.vertex_count > 0 && self.vertex_count <= self.vertex_buffer_capacity {
            unsafe {
                let data_ptr = self.device.map_memory(
                    self.vertex_buffer_memory,
                    0,
                    (std::mem::size_of::<RenderVertex>() * self.vertex_count) as vk::DeviceSize,
                    vk::MemoryMapFlags::empty(),
                )?;
                std::ptr::copy_nonoverlapping(
                    vertices.as_ptr(),
                    data_ptr as *mut RenderVertex,
                    self.vertex_count,
                );
                self.device.unmap_memory(self.vertex_buffer_memory);
            }
        }

        Ok(())
    }

    fn begin_frame(&mut self) -> Result<()> {
        unsafe {
            self.device.wait_for_fences(&[self.fence], true, u64::MAX)?;
            self.device.reset_fences(&[self.fence])?;
            let acquire_result = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available_semaphore,
                vk::Fence::null(),
            );
            let (image_index, _) = match acquire_result {
                Ok(ok) => ok,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.recreate_swapchain_resources(self.width, self.height)?;
                    self.swapchain_loader
                        .acquire_next_image(
                            self.swapchain,
                            u64::MAX,
                            self.image_available_semaphore,
                            vk::Fence::null(),
                        )
                        .map_err(|e| {
                            anyhow!("Failed to acquire swapchain image after resize: {:?}", e)
                        })?
                }
                Err(e) => return Err(anyhow!("Failed to acquire swapchain image: {:?}", e)),
            };
            self.current_image = image_index as usize;

            let command_buffer = self.command_buffers[self.current_image];
            self.device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())?;

            let begin_info = vk::CommandBufferBeginInfo::builder();
            self.device
                .begin_command_buffer(command_buffer, &begin_info)?;

            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: if Self::DEBUG_GRAY_CLEAR {
                            [0.18, 0.18, 0.18, 1.0]
                        } else {
                            [0.0, 0.0, 0.0, 1.0]
                        },
                    },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];

            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.render_pass)
                .framebuffer(self.framebuffers[self.current_image])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    },
                })
                .clear_values(&clear_values);

            self.device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: self.width as f32,
                height: self.height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            self.device.cmd_set_viewport(command_buffer, 0, &viewports);
            if Self::TRACE_RENDERER {
                eprintln!(
                    "begin_frame: viewport x={} y={} width={} height={} min_depth={} max_depth={}",
                    viewports[0].x,
                    viewports[0].y,
                    viewports[0].width,
                    viewports[0].height,
                    viewports[0].min_depth,
                    viewports[0].max_depth
                );
            }

            let scissors = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: self.width,
                    height: self.height,
                },
            }];
            self.device.cmd_set_scissor(command_buffer, 0, &scissors);
            if Self::TRACE_RENDERER {
                eprintln!(
                    "begin_frame: scissor offset=({}, {}) extent={}x{} current_image={}",
                    scissors[0].offset.x,
                    scissors[0].offset.y,
                    scissors[0].extent.width,
                    scissors[0].extent.height,
                    self.current_image
                );
            }
        }
        Ok(())
    }

    fn end_frame(&mut self) -> Result<()> {
        unsafe {
            let command_buffer = self.command_buffers[self.current_image];
            if self.vertex_count > 0 {
                self.device
                    .cmd_bind_vertex_buffers(command_buffer, 0, &[self.vertex_buffer], &[0]);
                let mut current_pipeline = vk::Pipeline::null();
                for pass in [false, true] {
                    for draw in &self.draw_calls {
                        let texture = self
                            .textures
                            .get(&draw.texture_name)
                            .or_else(|| self.textures.get("__missing"));
                        let Some(texture) = texture else {
                            continue;
                        };
                        let use_alpha = draw.alpha_blend || texture.has_alpha;
                        if use_alpha != pass {
                            continue;
                        }
                        let target_pipeline = if use_alpha {
                            self.alpha_pipeline
                        } else {
                            self.opaque_pipeline
                        };
                        if current_pipeline != target_pipeline {
                            self.device.cmd_bind_pipeline(
                                command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                target_pipeline,
                            );
                            current_pipeline = target_pipeline;
                        }
                        self.device.cmd_bind_descriptor_sets(
                            command_buffer,
                            vk::PipelineBindPoint::GRAPHICS,
                            self.pipeline_layout,
                            0,
                            &[texture.descriptor_set],
                            &[],
                        );
                        if Self::TRACE_RENDERER {
                            eprintln!(
                                "cmd_draw: pass_alpha={} texture={} vertex_offset={} vertex_count={}",
                                pass, draw.texture_name, draw.vertex_offset, draw.vertex_count,
                            );
                        }
                        self.device.cmd_draw(
                            command_buffer,
                            draw.vertex_count,
                            1,
                            draw.vertex_offset,
                            0,
                        );
                    }
                }
            }
            self.device.cmd_end_render_pass(command_buffer);
            self.device.end_command_buffer(command_buffer)?;
            if Self::TRACE_RENDERER {
                eprintln!(
                    "end_frame: queue_submit image={} vertex_count={} draw_calls={}",
                    self.current_image,
                    self.vertex_count,
                    self.draw_calls.len()
                );
            }

            let wait_semaphores = [self.image_available_semaphore];
            let signal_semaphores = [self.render_finished_semaphore];
            let command_buffers = [command_buffer];
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores)
                .build();

            self.device
                .queue_submit(self.queue, &[submit_info], self.fence)?;
            if Self::TRACE_RENDERER {
                eprintln!("end_frame: queue_submit ok");
            }

            let swapchains = [self.swapchain];
            let image_indices = [self.current_image as u32];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            match self
                .swapchain_loader
                .queue_present(self.queue, &present_info)
            {
                Ok(_) => {
                    if Self::TRACE_RENDERER {
                        eprintln!("end_frame: queue_present ok image={}", self.current_image);
                    }
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
                    self.recreate_swapchain_resources(self.width, self.height)?;
                }
                Err(e) => return Err(anyhow!("Failed to present swapchain image: {:?}", e)),
            }
        }
        Ok(())
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_sampler(self.sampler, None);
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None);
            self.device
                .destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            for tex in self.textures.values() {
                self.device.destroy_image_view(tex.view, None);
                self.device.destroy_image(tex.image, None);
                self.device.free_memory(tex.memory, None);
            }
            self.device.destroy_buffer(self.vertex_buffer, None);
            self.device.free_memory(self.vertex_buffer_memory, None);
            self.device.destroy_buffer(self.light_uniform_buffer, None);
            self.device
                .free_memory(self.light_uniform_buffer_memory, None);
            self.device
                .free_command_buffers(self.command_pool, &self.command_buffers);
            self.device.destroy_image_view(self.depth_image_view, None);
            self.device.destroy_image(self.depth_image, None);
            self.device.free_memory(self.depth_image_memory, None);
            self.device.destroy_pipeline(self.opaque_pipeline, None);
            self.device.destroy_pipeline(self.alpha_pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_fence(self.fence, None);
            self.device
                .destroy_semaphore(self.render_finished_semaphore, None);
            self.device
                .destroy_semaphore(self.image_available_semaphore, None);
            self.device.destroy_command_pool(self.command_pool, None);
            for &framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.device.destroy_render_pass(self.render_pass, None);
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
