#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod avatar;
mod broadcast;
mod chroma;
mod cubism_render;
mod expressions_panel;
mod help;
mod hotkeys;
mod input_monitor;
mod items;
mod items_panel;
mod live2d;
mod metrics;
mod object_models;
mod output;
mod performance;
mod physics_panel;
#[cfg(feature = "screenshots")]
mod screenshot;
#[cfg(windows)]
mod spout;
mod theme;

fn smoke_mode() -> bool {
    cfg!(feature = "screenshots") && std::env::var_os("ARIA_SCREENSHOT_TO").is_some()
}

fn main() -> eframe::Result {
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if cfg!(target_os = "windows")
        && let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup
    {
        setup.instance_descriptor.backends =
            wgpu::Backends::from_env().unwrap_or(wgpu::Backends::DX12);
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("A.R.I.A. — Avatar Studio")
            .with_app_id("com.nekounix.aria")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        persist_window: !smoke_mode(),
        ..Default::default()
    };
    let result = eframe::run_native(
        "A.R.I.A.",
        options,
        Box::new(|cc| Ok(Box::new(app::AriaApp::new(cc)))),
    );
    if let Err(error) = &result {
        if smoke_mode() {
            return result;
        }
        rfd::MessageDialog::new().set_title("A.R.I.A. could not start")
            .set_description(format!("{error}\n\nUpdate your graphics driver. See docs/windows.md for WGPU_BACKEND=vulkan fallback and build instructions."))
            .set_level(rfd::MessageLevel::Error).show();
    }
    result
}
