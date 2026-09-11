//! Wire formats are implemented from DenchiSoft's published protocol (see docs/tracking.md).
use anyhow::{Result, bail, ensure};
use aria_core::{TrackingFrame, Vec3};
use serde::{Deserialize, Serialize};

pub const MAX_PACKET_SIZE: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Protocol {
    #[default]
    VTubeStudio,
    AriaJson,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
struct VtsPacket {
    timestamp: u64,
    face_found: bool,
    rotation: Vec3,
    position: Vec3,
    #[serde(default)]
    eye_left: Vec3,
    #[serde(default)]
    eye_right: Vec3,
    blend_shapes: Vec<BlendShape>,
    #[serde(default = "no_hotkey")]
    hotkey: i32,
}

fn no_hotkey() -> i32 {
    -1
}

#[derive(Deserialize, Serialize)]
struct BlendShape {
    k: String,
    v: f32,
}

#[derive(Deserialize, Serialize)]
struct AriaPacket {
    version: u32,
    #[serde(flatten)]
    frame: TrackingFrame,
}

pub fn decode(bytes: &[u8], protocol: Protocol) -> Result<TrackingFrame> {
    ensure!(
        bytes.len() <= MAX_PACKET_SIZE,
        "Packet exceeds 16 KiB limit"
    );
    let mut frame = match protocol {
        Protocol::VTubeStudio => {
            let p: VtsPacket = serde_json::from_slice(bytes)?;
            ensure!(p.blend_shapes.len() <= 128, "Too many blendshapes");
            TrackingFrame {
                timestamp: p.timestamp,
                face_found: p.face_found,
                // VTS sends horizontal/vertical/lean as X/Y/Z. ARIA's internal
                // contract is pitch/yaw/roll; convert only at this boundary.
                rotation: Vec3 {
                    x: p.rotation.y,
                    y: p.rotation.x,
                    z: p.rotation.z,
                },
                position: p.position,
                eye_left: p.eye_left,
                eye_right: p.eye_right,
                hotkey: p.hotkey,
                blend_shapes: p.blend_shapes.into_iter().map(|b| (b.k, b.v)).collect(),
            }
        }
        Protocol::AriaJson => {
            let p: AriaPacket = serde_json::from_slice(bytes)?;
            ensure!(
                p.version == 1,
                "Unsupported ARIA tracking version {}",
                p.version
            );
            p.frame
        }
    };
    ensure!(frame.blend_shapes.len() <= 128, "Too many blendshapes");
    ensure!(
        frame
            .blend_shapes
            .keys()
            .all(|k| !k.is_empty() && k.len() <= 64),
        "Invalid blendshape name"
    );
    if !frame.is_finite() {
        bail!("Tracking values must be finite");
    }
    frame.normalize();
    Ok(frame)
}

pub fn subscription(listen_port: u16) -> Vec<u8> {
    // Renew a five-second lease every second. This is not the desktop WebSocket API.
    serde_json::to_vec(&serde_json::json!({
        "messageType": "iOSTrackingDataRequest", "time": 5.0,
        "sentBy": "A.R.I.A.", "ports": [listen_port]
    }))
    .expect("fixed subscription serializes")
}

/// Simulator output uses the same documented wire schema, including PascalCase VTS keys.
pub fn encode(frame: &TrackingFrame, protocol: Protocol) -> Result<Vec<u8>> {
    match protocol {
        Protocol::AriaJson => Ok(serde_json::to_vec(&AriaPacket {
            version: 1,
            frame: frame.clone(),
        })?),
        Protocol::VTubeStudio => Ok(serde_json::to_vec(&VtsPacket {
            timestamp: frame.timestamp,
            face_found: frame.face_found,
            rotation: Vec3 {
                x: frame.rotation.y,
                y: frame.rotation.x,
                z: frame.rotation.z,
            },
            position: frame.position,
            eye_left: frame.eye_left,
            eye_right: frame.eye_right,
            hotkey: frame.hotkey,
            blend_shapes: frame
                .blend_shapes
                .iter()
                .map(|(k, &v)| BlendShape { k: k.clone(), v })
                .collect(),
        })?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_official_shape_and_future_fields() {
        let f = decode(
            include_bytes!("../tests/fixtures/vts-frame.json"),
            Protocol::VTubeStudio,
        )
        .unwrap();
        assert!(f.face_found);
        assert_eq!(f.timestamp, 1724000000123);
        assert_eq!(f.rotation.x, 358.5);
        assert_eq!(f.blend("jawopen"), 0.7);
        assert_eq!(f.blend("eyeblinkleft"), 0.25);
        assert_eq!(f.hotkey, 3);
        assert_eq!(f.eye_right.y, 5.0);
    }

    #[test]
    fn rejects_unrelated_and_malformed_datagrams() {
        for input in [b"{}".as_slice(), b"not json", br#"{"FaceFound":true}"#] {
            assert!(decode(input, Protocol::VTubeStudio).is_err());
        }
        assert!(decode(&vec![b' '; MAX_PACKET_SIZE + 1], Protocol::AriaJson).is_err());
        assert!(decode(br#"{"version":2,"face_found":true,"rotation":{"x":0,"y":0,"z":0},"blend_shapes":{}}"#, Protocol::AriaJson).is_err());
        assert!(decode(br#"{"version":1,"face_found":true,"rotation":{"x":1e99,"y":0,"z":0},"blend_shapes":{}}"#, Protocol::AriaJson).is_err());
    }

    #[test]
    fn phone_head_axes_reach_the_matching_model_axis() {
        use aria_core::{MappingSettings, ParameterPipeline};
        for (rotation, expected) in [
            ([12, 0, 0], [12.0, 0.0, 0.0]),
            ([0, 9, 0], [0.0, 9.0, 0.0]),
            ([0, 0, -7], [0.0, 0.0, -7.0]),
        ] {
            let bytes = serde_json::to_vec(&serde_json::json!({
                "Timestamp": 1, "FaceFound": true,
                "Rotation": { "x": rotation[0], "y": rotation[1], "z": rotation[2] },
                "Position": { "x": 0, "y": 0, "z": 0 }, "BlendShapes": []
            }))
            .unwrap();
            let frame = decode(&bytes, Protocol::VTubeStudio).unwrap();
            let mut pipeline = ParameterPipeline::default();
            let settings = MappingSettings {
                smoothing_ms: 0.0,
                ..Default::default()
            };
            assert_eq!(
                pipeline.update(Some(&frame), &settings, 0.016).0[..3],
                expected
            );
            assert!(pipeline.calibrate(&frame));
            assert_eq!(
                pipeline.update(Some(&frame), &settings, 0.016).0[..3],
                [0.0; 3]
            );
        }
    }

    #[test]
    fn aria_contract_and_clamping() {
        let f = decode(br#"{"version":1,"face_found":true,"rotation":{"x":0,"y":0,"z":0},"blend_shapes":{"JawOpen":2,"EyeBlinkLeft":-1}}"#, Protocol::AriaJson).unwrap();
        assert_eq!(f.blend("jawopen"), 1.0);
        assert_eq!(f.blend("eyeblinkleft"), 0.0);
    }
}
