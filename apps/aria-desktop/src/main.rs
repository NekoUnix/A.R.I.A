#![cfg_attr(all(not(debug_assertions), not(test)), windows_subsystem = "windows")]

mod app;
mod avatar;
mod avatar_import;
mod broadcast;
mod chat;
mod chroma;
mod controller;
mod cubism_render;
mod deformation;
mod effect_api;
mod effect_audio;
mod effect_editor;
mod effects;
mod effects_panel;
mod expressions_panel;
mod help;
mod hotkeys;
mod image_actions;
mod input_monitor;
mod items;
mod items_panel;
mod liquid_art;
mod live2d;
mod media;
mod mesh_asset;
mod metrics;
mod microphone;
mod object_models;
mod output;
mod performance;
mod physics_panel;
mod prop_render;
#[cfg(feature = "screenshots")]
mod screenshot;
#[cfg(windows)]
mod spout;
mod theme;
mod tracking_guide;
mod vrm;
mod webcam;

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
        // Windows known-folder APIs ignore APPDATA overrides. Give test and
        // portable sessions an explicit storage directory instead.
        persistence_path: std::env::var_os("ARIA_PROFILE_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                smoke_mode().then(|| {
                    std::env::temp_dir().join(format!("aria-smoke-{}", std::process::id()))
                })
            })
            // eframe 0.33's implementation expects the file, despite its field
            // documentation describing a folder.
            .map(|directory| directory.join("app.ron")),
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
