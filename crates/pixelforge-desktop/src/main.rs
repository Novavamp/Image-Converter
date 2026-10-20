#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("pixelforge=info".parse().unwrap()),
        )
        .init();
    let mut options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([860.0, 600.0])
            .with_title("PixelForge — Image Converter")
            .with_icon(app::app_icon()),
        ..Default::default()
    };
    #[cfg(target_os = "windows")]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        // Avoid Vulkan drivers that crash during adapter initialization on some Windows GPUs;
        // Direct3D 12 is preferred, with OpenGL retained as a compatibility fallback.
        setup.instance_descriptor.backends =
            eframe::egui_wgpu::wgpu::Backends::DX12 | eframe::egui_wgpu::wgpu::Backends::GL;
    }
    eframe::run_native(
        "PixelForge",
        options,
        Box::new(|cc| Ok(Box::new(app::PixelForgeApp::new(cc)))),
    )
}
