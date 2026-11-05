#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
pub use rynthv::{i18n,lformat};
mod ui;

use rynthv::{midi, model, storage, synthesis};

fn main() -> eframe::Result {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    let mut gpu = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    #[cfg(windows)]
    {
        gpu.instance_descriptor.backends = wgpu::Backends::DX12;
        gpu.instance_descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    }
    #[cfg(target_os = "linux")]
    {
        gpu.instance_descriptor.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!(
                    "../assets/branding/app-icon.png"
                ))
                .expect("embedded app icon must be a valid PNG"),
            )
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([820.0, 560.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(gpu),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "RynthV",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, path)))),
    )
}
