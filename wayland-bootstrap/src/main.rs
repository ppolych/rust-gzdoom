use anyhow::Result;
use ash::{vk, Entry};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::ffi::CString;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::wayland::WindowBuilderExtWayland,
    window::WindowBuilder,
};

fn main() -> Result<()> {
    // 1. Wayland Connection & Window
    let event_loop = EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("rust-gzdoom Wayland Bootstrap")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .with_name("rust-gzdoom", "") // Wayland app_id
        .build(&event_loop)?;

    // 2. Vulkan Initialization
    let entry = unsafe { Entry::load()? };

    // Extensions required for Wayland surface
    let extension_names = vec![
        ash::extensions::khr::Surface::name().as_ptr(),
        ash::extensions::khr::WaylandSurface::name().as_ptr(),
    ];

    let app_name = CString::new("rust-gzdoom")?;
    let app_info = vk::ApplicationInfo::builder()
        .application_name(&app_name)
        .api_version(vk::API_VERSION_1_3);

    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names);

    let instance = unsafe { entry.create_instance(&create_info, None)? };

    // 3. Create Vulkan Surface for Wayland
    let _surface = unsafe {
        ash_window::create_surface(
            &entry,
            &instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
            None,
        )?
    };

    // 4. Physical Device & Logical Device (Minimal)
    let pdevices = unsafe { instance.enumerate_physical_devices()? };
    let pdevice = pdevices.into_iter().next().expect("No GPU found");

    let queue_info = [vk::DeviceQueueCreateInfo::builder()
        .queue_family_index(0)
        .queue_priorities(&[1.0])
        .build()];

    let device_create_info = vk::DeviceCreateInfo::builder().queue_create_infos(&queue_info);

    let device = unsafe { instance.create_device(pdevice, &device_create_info, None)? };

    println!("Wayland/Vulkan Bootstrap Successful");

    // 5. Basic Event Loop
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("Closing window...");
                elwt.exit();
            }
            _ => {}
        }
    })?;

    // Cleanup
    unsafe {
        device.destroy_device(None);
        instance.destroy_instance(None);
    }

    Ok(())
}
