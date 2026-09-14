//! Reproducible CPU-only benchmark; no camera, model files or window required.
use super::*;

fn sample() -> (Settings, ModelPreferences) {
    let mut settings = Settings {
        source: Source::Vts,
        sender_ip: "192.0.2.10".into(),
        fps: 90,
        always_on_top: true,
        outputs: Some(OutputSettings::default()),
        ..Default::default()
    };
    for index in 0..3 {
        let canvas = settings.outputs.as_mut().unwrap().canvas_mut(index);
        for id in 1..=4 {
            canvas.avatars.insert(
                id,
                crate::output::Transform {
                    position: [id as f32 * 0.1, -0.2],
                    zoom: 0.8,
                },
            );
        }
    }
    let mut other = ModelPreferences::capture(&settings);
    other.source = Source::Webcam;
    other.sender_ip = "192.0.2.20".into();
    other.zoom = 1.2;
    other.camera.device = 3;
    other.mapping.invert_roll = true;
    (settings, other)
}

#[test]
fn live_preferences_exchange_preserves_identity_and_workspace_without_allocating_strings() {
    let (mut settings, mut other) = sample();
    let outputs = serde_json::to_value(&settings.outputs).unwrap();
    let old = serde_json::to_value(ModelPreferences::capture(&settings)).unwrap();
    let first_ip = settings.sender_ip.as_ptr();
    let second_ip = other.sender_ip.as_ptr();
    other.exchange_live(&mut settings);
    assert_eq!(settings.sender_ip, "192.0.2.20");
    assert!(settings.source == Source::Webcam);
    assert!(settings.mapping.invert_roll);
    assert_eq!(settings.camera.device, 3);
    assert_eq!(settings.sender_ip.as_ptr(), second_ip);
    assert_eq!(other.sender_ip.as_ptr(), first_ip);
    assert_eq!(settings.fps, 90);
    assert!(settings.always_on_top);
    assert_eq!(serde_json::to_value(&settings.outputs).unwrap(), outputs);
    other.exchange_live(&mut settings);
    assert_eq!(
        serde_json::to_value(ModelPreferences::capture(&settings)).unwrap(),
        old
    );
    assert_eq!(settings.sender_ip.as_ptr(), first_ip);
}

#[test]
#[ignore = "CPU microbenchmark; run optimized with --ignored --nocapture"]
fn profile_settings_handoff_benchmark() {
    use std::hint::black_box;
    const ITERATIONS: usize = 20_000;
    // Keep the previous release's exact preferences path as the baseline. This
    // measures only the work being replaced, not frame rate or GPU rendering.
    fn previous(preferences: &mut ModelPreferences, settings: &mut Settings) {
        let captured = ModelPreferences::capture(settings);
        let outputs = settings.outputs.clone();
        let fps = settings.fps;
        let on_top = settings.always_on_top;
        preferences.restore(settings);
        settings.outputs = outputs;
        settings.fps = fps;
        settings.always_on_top = on_top;
        *preferences = captured;
    }
    let mut old = Vec::new();
    let mut new = Vec::new();
    for round in 0..7 {
        for optimized in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let (mut settings, mut preferences) = sample();
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                if optimized {
                    black_box(&mut preferences).exchange_live(black_box(&mut settings));
                } else {
                    previous(black_box(&mut preferences), black_box(&mut settings));
                }
            }
            let ns = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
            if optimized {
                new.push(ns);
            } else {
                old.push(ns);
            }
            black_box(settings);
            black_box(preferences);
        }
    }
    old.sort_by(f64::total_cmp);
    new.sort_by(f64::total_cmp);
    println!(
        "Four-avatar settings handoff, 3 canvases, median of 7 x {ITERATIONS}: previous {:.1} ns, current {:.1} ns ({:.1}% less CPU time in this operation; not an overall FPS result)",
        old[3],
        new[3],
        100. * (1. - new[3] / old[3])
    );
}
