//! Opt-in native wgpu screenshot smoke hook. Disabled in normal release packages.
use eframe::egui;
use std::time::{Duration, Instant};

/// Feed real egui pointer events through the normal stage interaction in an
/// isolated screenshot run. Normal packages do not compile this module.
pub fn layer_input(ctx: &egui::Context, input: &mut egui::RawInput) {
    if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() != Ok("layer-selection") {
        return;
    }
    let Some(rect) = ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new("layer-smoke-stage")))
    else {
        return;
    };
    input.events.retain(|e| {
        !matches!(
            e,
            egui::Event::PointerMoved(_)
                | egui::Event::PointerButton { .. }
                | egui::Event::PointerGone
        )
    });
    let key = egui::Id::new("layer-smoke-phase");
    let phase = ctx.data(|d| d.get_temp::<u8>(key).unwrap_or(0));
    let start = rect.min + rect.size() * egui::vec2(0.20, 0.13);
    let end = rect.min + rect.size() * egui::vec2(0.72, 0.69);
    if phase == 1 {
        input.events.push(egui::Event::PointerMoved(start));
        input.events.push(egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
    } else if phase >= 2 {
        input.events.push(egui::Event::PointerMoved(end));
    }
    ctx.data_mut(|d| d.insert_temp(key, phase.saturating_add(1)));
}

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
    // Exercise the same native OpenUrl command as hyperlinks, social icons and
    // OAuth. The local test server must actually receive a browser request;
    // merely observing the egui command would miss a disabled `links` feature.
    if !output
        && ctx.viewport_id() == egui::ViewportId::ROOT
        && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("browser-link")
        && started.elapsed() > Duration::from_secs(1)
    {
        let id = egui::Id::new("aria-browser-smoke-opened");
        if !ctx.data(|d| d.get_temp::<bool>(id).unwrap_or(false)) {
            let url = std::env::var("ARIA_SMOKE_BROWSER_URL").expect("Local test URL required");
            let parsed = reqwest::Url::parse(&url).expect("Valid local test URL required");
            assert!(parsed.scheme() == "http" && parsed.host_str() == Some("127.0.0.1"));
            ctx.open_url(egui::OpenUrl::new_tab(url));
            ctx.data_mut(|d| d.insert_temp(id, true));
        }
    }
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
        if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("layer-selection") {
            let count = ctx
                .data(|d| d.get_temp::<usize>(egui::Id::new("layer-smoke-selected")))
                .unwrap_or(0);
            assert!(
                count > 1,
                "Native pointer drag must select more than one layer before capture"
            );
            eprintln!("Native pointer drag selected {count} layers");
        }
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
