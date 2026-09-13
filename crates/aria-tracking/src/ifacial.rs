//! iFacialMocap's documented live UDP protocol (legacy and v2 delimiters).
//! https://www.ifacialmocap.com/for-developer/
use anyhow::{Context, Result, ensure};
use aria_core::{TrackingFrame, Vec3};
use std::collections::BTreeMap;

pub const PORT: u16 = 49983;
pub const START: &[u8] = b"iFacialMocap_sahuasouryya9218sauhuiayeta91555dy3719|sendDataVersion=v2";

fn numbers<const N: usize>(text: &str) -> Result<[f32; N]> {
    let mut values = [0.0_f32; N];
    let mut fields = text.split(',');
    for value in &mut values {
        *value = fields
            .next()
            .context("Missing pose component")?
            .trim()
            .parse()?;
        ensure!(value.is_finite(), "Pose values must be finite");
    }
    ensure!(fields.next().is_none(), "Too many pose components");
    Ok(values)
}

fn name(text: &str) -> Result<String> {
    ensure!(
        !text.is_empty()
            && text.len() <= 64
            && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "Invalid iFacialMocap blendshape name"
    );
    let mut result = text.to_ascii_lowercase();
    if result.ends_with("_l") {
        result.truncate(result.len() - 2);
        result.push_str("left");
    } else if result.ends_with("_r") {
        result.truncate(result.len() - 2);
        result.push_str("right");
    }
    Ok(result)
}

pub(super) fn decode(bytes: &[u8]) -> Result<TrackingFrame> {
    let text = std::str::from_utf8(bytes).context("iFacialMocap expects UTF-8 text")?;
    let mut head = None;
    let mut left = None;
    let mut right = None;
    let mut blend_shapes = BTreeMap::new();
    let mut fields = 0;
    for field in text.trim().trim_end_matches('\0').split('|') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        fields += 1;
        ensure!(fields <= 131, "Too many iFacialMocap fields");
        if let Some(values) = field.strip_prefix("=head#") {
            ensure!(head.is_none(), "Duplicate head pose");
            head = Some(numbers::<6>(values)?);
        } else if let Some(values) = field.strip_prefix("leftEye#") {
            ensure!(left.is_none(), "Duplicate left eye pose");
            left = Some(numbers::<3>(values)?);
        } else if let Some(values) = field.strip_prefix("rightEye#") {
            ensure!(right.is_none(), "Duplicate right eye pose");
            right = Some(numbers::<3>(values)?);
        } else {
            let (channel, value) = field
                .split_once('&')
                .or_else(|| field.split_once('-'))
                .context("Expected a blendshape value or head/eye pose")?;
            let value: f32 = value.trim().parse()?;
            ensure!(value.is_finite(), "Blendshape values must be finite");
            ensure!(blend_shapes.len() < 128, "Too many blendshapes");
            ensure!(
                blend_shapes
                    .insert(name(channel.trim())?, value / 100.0)
                    .is_none(),
                "Duplicate blendshape"
            );
        }
    }
    let [pitch, yaw, roll, x, y, z] =
        head.context("iFacialMocap packet is missing its head pose")?;
    ensure!(
        !blend_shapes.is_empty(),
        "iFacialMocap packet has no blendshapes"
    );
    let vec = |v: [f32; 3]| Vec3 {
        x: v[0],
        y: v[1],
        z: v[2],
    };
    Ok(TrackingFrame {
        // This protocol has no sender timestamp or face-confidence flag. A valid
        // packet is available tracking; Receiver's local stale timeout handles loss.
        timestamp: 0,
        face_found: true,
        // Keep the axes separate and align imported headRotX/Y/Z with the
        // original iFacialMocap coordinates before VBridger equations run.
        rotation: Vec3 {
            x: -pitch,
            y: -yaw,
            z: roll,
        },
        position: Vec3 { x, y, z },
        eye_left: vec(left.unwrap_or_default()),
        eye_right: vec(right.unwrap_or_default()),
        blend_shapes,
        hotkey: -1,
        parameters: BTreeMap::new(),
    })
}

/// Encode a synthetic live frame for diagnostics; this is not recorded-animation playback.
pub(super) fn encode(frame: &TrackingFrame) -> Result<Vec<u8>> {
    ensure!(
        frame.is_finite() && frame.face_found,
        "iFacialMocap live packets require a valid tracked face"
    );
    ensure!(
        !frame.blend_shapes.is_empty() && frame.blend_shapes.len() <= 128,
        "Expected 1–128 blendshapes"
    );
    let mut fields = Vec::new();
    for (key, value) in &frame.blend_shapes {
        let key = name(key)?;
        let key = if let Some(prefix) = key.strip_suffix("left") {
            format!("{prefix}_L")
        } else if let Some(prefix) = key.strip_suffix("right") {
            format!("{prefix}_R")
        } else {
            key
        };
        fields.push(format!("{key}&{}", value.clamp(0.0, 1.0) * 100.0));
    }
    fields.push(format!(
        "=head#{},{},{},{},{},{}",
        -frame.rotation.x,
        -frame.rotation.y,
        frame.rotation.z,
        frame.position.x,
        frame.position.y,
        frame.position.z
    ));
    for (name, eye) in [("rightEye", frame.eye_right), ("leftEye", frame.eye_left)] {
        fields.push(format!("{name}#{},{},{}", eye.x, eye.y, eye.z));
    }
    Ok((fields.join("|") + "|").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Protocol, protocol};
    fn parse(text: &str) -> Result<TrackingFrame> {
        protocol::decode(text.as_bytes(), Protocol::IFacialMocap)
    }
    #[test]
    fn documented_legacy_and_v2_fields_normalize_without_crossing_axes() {
        for separator in ["-", "&"] {
            let f = parse(&format!("jawOpen{separator}72|eyeBlink_L{separator}25|eyeBlink_R{separator}0|=head#-12,23,7,0.1,-0.2,0.3|rightEye#1,2,3|leftEye#4,5,6|")).unwrap();
            assert_eq!(
                f.rotation,
                Vec3 {
                    x: 12.0,
                    y: -23.0,
                    z: 7.0
                }
            );
            assert_eq!(f.blend("jawopen"), 0.72);
            assert_eq!(f.blend("eyeblinkleft"), 0.25);
            assert_eq!(f.position.y, -0.2);
            assert_eq!(f.eye_left.x, 4.0);
            assert_eq!(f.eye_right.y, 2.0);
            assert!(f.face_found && f.timestamp == 0);
        }
        let f = parse("jawOpen&-10|eyeBlink_L&130|=head#0,0,0,0,0,0|").unwrap();
        assert_eq!(f.blend("jawopen"), 0.0);
        assert_eq!(f.blend("eyeblinkleft"), 1.0);
    }
    #[test]
    fn rejects_incomplete_nonfinite_duplicate_and_oversized_packets() {
        for text in [
            "",
            "jawOpen-30|",
            "=head#0,0,0,0,0,0|",
            "jawOpen-NaN|=head#0,0,0,0,0,0|",
            "jawOpen-30|=head#inf,0,0,0,0,0|",
            "jawOpen-30|=head#1,2,3|",
            "jawOpen-30|=head#0,0,0,0,0,0|=head#0,0,0,0,0,0|",
            "eyeBlink_L-1|eyeBlinkLeft-2|=head#0,0,0,0,0,0|",
            "jawOpen&2&3|=head#0,0,0,0,0,0|",
        ] {
            assert!(parse(text).is_err(), "accepted {text}");
        }
        assert!(protocol::decode(&[0xff], Protocol::IFacialMocap).is_err());
        assert!(parse(&"x".repeat(protocol::MAX_PACKET_SIZE + 1)).is_err());
        let many = (0..129)
            .map(|i| format!("channel{i}-1|"))
            .collect::<String>()
            + "=head#0,0,0,0,0,0|";
        assert!(parse(&many).is_err());
    }
    #[test]
    fn imported_equations_receive_original_head_coordinates_and_face_channels() {
        let f = parse("jawOpen-65|eyeBlink_L-20|=head#-10,25,8,0,0,0|").unwrap();
        let c=aria_core::vbridger::import(br#"{"store":[{"output":"FaceAngle","equation":"-headRotY","equationY":"-headRotX","equationZ":"headRotZ","vectorMode":true,"min":-50,"max":50,"defaultValue":0},{"output":"MouthOpen","equation":"jawOpen","min":0,"max":1,"defaultValue":0}]}"#, "axes").unwrap();
        let mut inputs = aria_core::rig::Inputs::new();
        c.apply(
            Some(&f),
            Vec3::default(),
            &mut inputs,
            &mut aria_core::vbridger::Runtime::default(),
            0.016,
        );
        assert_eq!(inputs["FaceAngleX"], -25.0);
        assert_eq!(inputs["FaceAngleY"], 10.0);
        assert_eq!(inputs["FaceAngleZ"], 8.0);
        assert_eq!(inputs["MouthOpen"], 0.65);
        let decoded = protocol::decode(&encode(&f).unwrap(), Protocol::IFacialMocap).unwrap();
        assert_eq!(decoded.rotation, f.rotation);
        assert_eq!(decoded.blend_shapes, f.blend_shapes);
    }
}
