//! Opt-in native wgpu screenshot smoke hook. Disabled in normal release packages.
use eframe::egui;
use std::time::{Duration, Instant};

pub fn delay() -> Duration {
    Duration::from_secs(
        std::env::var("ARIA_SMOKE_DELAY_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2)
            .clamp(1, 30),
    )
}

pub fn capture(ctx: &egui::Context, started: Instant, output: bool) {
    let Some(path) = std::env::var_os("ARIA_SCREENSHOT_TO") else {
        return;
    };
    let want_output = std::env::var("ARIA_SMOKE_SCENARIO").is_ok_and(|s| s.starts_with("output"));
    let output_index = std::env::var("ARIA_SMOKE_OUTPUT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(2);
    let target = if std::env::var("ARIA_SMOKE_SCENARIO")
        .is_ok_and(|s| s.starts_with("chat") && s != "chat-controls")
    {
        crate::chat::viewport_id()
    } else if std::env::var("ARIA_SMOKE_SCENARIO").is_ok_and(|s| s.starts_with("help-")) {
        crate::help::viewport_id()
    } else if want_output {
        crate::output::viewport_id(output_index)
    } else {
        egui::ViewportId::ROOT
    };
    let requested = egui::Id::new("aria-smoke-requested");
    let delay = delay();
    if output == want_output
        && ctx.viewport_id() == target
        && started.elapsed() > delay
        && !ctx.data(|d| d.get_temp::<bool>(requested).unwrap_or(false))
    {
        eprintln!(
            "Screenshot request; repaint causes: {:?}",
            ctx.repaint_causes()
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        ctx.data_mut(|d| d.insert_temp(requested, true));
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("aria-smoke-request-time"), Instant::now()));
    }
    let image = ctx.input(|input| {
        input.events.iter().find_map(|event| {
            if let egui::Event::Screenshot {
                image, viewport_id, ..
            } = event
            {
                (*viewport_id == target).then(|| image.clone())
            } else {
                None
            }
        })
    });
    if let Some(image) = image {
        let rgba: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_array())
            .collect();
        if let Err(e) = image::save_buffer(
            &path,
            &rgba,
            image.width() as u32,
            image.height() as u32,
            image::ColorType::Rgba8,
        ) {
            eprintln!("Screenshot failed: {e}");
        }
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }
    // Large avatars can finish loading long after the requested delay. Give the
    // GPU ten seconds from the actual screenshot request, not from app startup.
    if !output
        && ctx
            .data(|d| d.get_temp::<Instant>(egui::Id::new("aria-smoke-request-time")))
            .is_some_and(|at| at.elapsed() > Duration::from_secs(10))
    {
        eprintln!("Screenshot timed out");
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    }
}
#[derive(Debug)]
pub struct SmokeDroppedFile(pub std::path::PathBuf);
impl eframe::egui::DroppedFile for SmokeDroppedFile {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|error| error.to_string())
    }
}
