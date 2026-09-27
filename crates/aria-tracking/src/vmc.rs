//! Bounded OSC/VMC facial input. Never handles remote load, keyboard or file commands.
//! Wire reference: https://protocol.vmc.info/english.html
use anyhow::{Result, ensure};
use aria_core::{TrackingFrame, Vec3};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Decoder {
    frame: TrackingFrame,
    pending: BTreeMap<String, f32>,
}
impl Default for Decoder {
    fn default() -> Self {
        Self {
            frame: TrackingFrame {
                hotkey: -1,
                ..Default::default()
            },
            pending: Default::default(),
        }
    }
}
enum Value<'a> {
    Text(&'a str),
    Float(f32),
    Int(i32),
    Other,
}
fn string<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a str> {
    let rest = bytes
        .get(*offset..)
        .ok_or_else(|| anyhow::anyhow!("Truncated OSC string"))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| anyhow::anyhow!("Unterminated OSC string"))?;
    ensure!(end <= 1024, "OSC string is too long");
    let value = std::str::from_utf8(&rest[..end])?;
    let size = (end + 4) & !3;
    ensure!(
        rest.len() >= size && rest[end..size].iter().all(|&b| b == 0),
        "Invalid OSC padding"
    );
    *offset += size;
    Ok(value)
}
impl Decoder {
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Option<TrackingFrame>> {
        ensure!(
            bytes.len() <= super::protocol::MAX_PACKET_SIZE,
            "VMC packet exceeds 16 KiB"
        );
        // A malformed later bundle member cannot partially mutate the current pose.
        let mut next = self.clone();
        let mut count = 0;
        let changed = next.packet(bytes, 0, &mut count)?;
        ensure!(next.frame.is_finite(), "Non-finite VMC pose");
        next.frame.normalize();
        *self = next;
        Ok(changed.then(|| self.frame.clone()))
    }
    fn packet(&mut self, bytes: &[u8], depth: usize, count: &mut usize) -> Result<bool> {
        ensure!(
            depth <= 4 && *count < 256,
            "OSC bundle nesting or message limit exceeded"
        );
        *count += 1;
        if bytes.starts_with(b"#bundle\0") {
            ensure!(bytes.len() >= 16, "Truncated OSC bundle");
            let mut offset = 16;
            let mut changed = false;
            while offset < bytes.len() {
                let n = bytes
                    .get(offset..offset + 4)
                    .ok_or_else(|| anyhow::anyhow!("Truncated OSC bundle size"))?;
                let length = u32::from_be_bytes(n.try_into().unwrap()) as usize;
                offset += 4;
                ensure!(
                    length > 0 && length <= bytes.len() - offset,
                    "Invalid OSC bundle member length"
                );
                changed |= self.packet(&bytes[offset..offset + length], depth + 1, count)?;
                offset += length;
            }
            return Ok(changed);
        }
        let mut offset = 0;
        let address = string(bytes, &mut offset)?;
        if !matches!(
            address,
            "/VMC/Ext/OK" | "/VMC/Ext/Bone/Pos" | "/VMC/Ext/Blend/Val" | "/VMC/Ext/Blend/Apply"
        ) {
            return Ok(false);
        }
        let types = string(bytes, &mut offset)?;
        ensure!(
            types.starts_with(',') && types.len() <= 33,
            "Invalid OSC type tags"
        );
        let mut args = Vec::new();
        for kind in types.chars().skip(1) {
            args.push(match kind {
                's' => Value::Text(string(bytes, &mut offset)?),
                'f' | 'i' => {
                    let raw = bytes
                        .get(offset..offset + 4)
                        .ok_or_else(|| anyhow::anyhow!("Truncated OSC argument"))?;
                    offset += 4;
                    let raw: [u8; 4] = raw.try_into().unwrap();
                    if kind == 'f' {
                        let value = f32::from_be_bytes(raw);
                        ensure!(value.is_finite(), "Non-finite OSC value");
                        Value::Float(value)
                    } else {
                        Value::Int(i32::from_be_bytes(raw))
                    }
                }
                'T' | 'F' | 'N' | 'I' => Value::Other,
                _ => return Ok(false),
            });
        }
        ensure!(offset == bytes.len(), "Unexpected OSC trailing bytes");
        match address {
            "/VMC/Ext/OK" => {
                if let Some(Value::Int(loaded)) = args.first() {
                    self.frame.face_found = *loaded == 1;
                    if let Some(Value::Int(tracking)) = args.get(3) {
                        self.frame.face_found &= *tracking == 1;
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            "/VMC/Ext/Blend/Val" => {
                if let [Value::Text(name), Value::Float(value), ..] = args.as_slice() {
                    ensure!(
                        !name.is_empty() && name.len() <= 64,
                        "Invalid VMC blendshape name"
                    );
                    ensure!(
                        self.pending.len() < 128 || self.pending.contains_key(*name),
                        "Too many VMC blendshapes"
                    );
                    self.pending.insert((*name).into(), value.clamp(0., 1.));
                }
                Ok(false)
            }
            "/VMC/Ext/Blend/Apply" => {
                for (name, value) in &self.pending {
                    self.frame.blend_shapes.insert(name.to_lowercase(), *value);
                }
                ensure!(
                    self.frame.blend_shapes.len() <= 128,
                    "Too many VMC blendshapes"
                );
                self.pending.clear();
                Ok(true)
            }
            "/VMC/Ext/Bone/Pos" => {
                if let [
                    Value::Text(name),
                    Value::Float(px),
                    Value::Float(py),
                    Value::Float(pz),
                    Value::Float(x),
                    Value::Float(y),
                    Value::Float(z),
                    Value::Float(w),
                    ..,
                ] = args.as_slice()
                {
                    if !name.eq_ignore_ascii_case("Head") {
                        return Ok(false);
                    }
                    ensure!(
                        [px, py, pz].iter().all(|v| v.abs() < 1e4),
                        "Invalid VMC head position"
                    );
                    self.frame.rotation = angles([*x, *y, *z, *w])?;
                    self.frame.position = Vec3 {
                        x: *px,
                        y: *py,
                        z: -*pz,
                    };
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }
}
pub fn encode(frame: &TrackingFrame) -> Result<Vec<u8>> {
    ensure!(
        frame.is_finite() && frame.blend_shapes.len() <= 128,
        "Invalid VMC frame"
    );
    fn text(value: &str) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }
    let mut messages = Vec::new();
    let mut ok = text("/VMC/Ext/OK");
    ok.extend(text(",i"));
    ok.extend(i32::from(frame.face_found).to_be_bytes());
    messages.push(ok);
    let (sx, cx) = (frame.rotation.x.to_radians() / 2.).sin_cos();
    let (sy, cy) = (frame.rotation.y.to_radians() / 2.).sin_cos();
    let (sz, cz) = (frame.rotation.z.to_radians() / 2.).sin_cos();
    let q = [
        -(sx * cy * cz - cx * sy * sz),
        -(cx * sy * cz + sx * cy * sz),
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    ];
    let mut head = text("/VMC/Ext/Bone/Pos");
    head.extend(text(",sfffffff"));
    head.extend(text("Head"));
    for value in [
        frame.position.x,
        frame.position.y,
        -frame.position.z,
        q[0],
        q[1],
        q[2],
        q[3],
    ] {
        head.extend(value.to_be_bytes());
    }
    messages.push(head);
    for (name, value) in &frame.blend_shapes {
        ensure!(
            !name.is_empty() && name.len() <= 64 && !name.contains('\0'),
            "Invalid VMC blendshape name"
        );
        let mut message = text("/VMC/Ext/Blend/Val");
        message.extend(text(",sf"));
        message.extend(text(name));
        message.extend(value.to_be_bytes());
        messages.push(message);
    }
    let mut apply = text("/VMC/Ext/Blend/Apply");
    apply.extend(text(","));
    messages.push(apply);
    let mut packet = b"#bundle\0".to_vec();
    packet.extend(1_u64.to_be_bytes());
    for message in messages {
        packet.extend((message.len() as u32).to_be_bytes());
        packet.extend(message);
    }
    ensure!(
        packet.len() <= super::protocol::MAX_PACKET_SIZE,
        "VMC packet exceeds 16 KiB"
    );
    Ok(packet)
}
fn angles(q: [f32; 4]) -> Result<Vec3> {
    let length = q.iter().map(|v| v * v).sum::<f32>().sqrt();
    ensure!(
        length.is_finite() && length > 1e-6,
        "Invalid VMC quaternion"
    );
    // Unity left-handed quaternion -> right-handed glTF convention (reflect Z).
    let [x, y, z, w] = [-q[0] / length, -q[1] / length, q[2] / length, q[3] / length];
    Ok(Vec3 {
        x: (2. * (w * x + y * z))
            .atan2(1. - 2. * (x * x + y * y))
            .to_degrees(),
        y: (2. * (w * y - z * x)).clamp(-1., 1.).asin().to_degrees(),
        z: (2. * (w * z + x * y))
            .atan2(1. - 2. * (y * y + z * z))
            .to_degrees(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes
    }
    pub(crate) fn blend_packet() -> Vec<u8> {
        let mut value = text("/VMC/Ext/Blend/Val");
        value.extend(text(",sf"));
        value.extend(text("JawOpen"));
        value.extend(0.75_f32.to_be_bytes());
        let mut apply = text("/VMC/Ext/Blend/Apply");
        apply.extend(text(","));
        let mut ok = text("/VMC/Ext/OK");
        ok.extend(text(",i"));
        ok.extend(1_i32.to_be_bytes());
        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend(1_u64.to_be_bytes());
        for message in [ok, value, apply] {
            bundle.extend((message.len() as u32).to_be_bytes());
            bundle.extend(message);
        }
        bundle
    }
    #[test]
    fn bundled_blends_apply_atomically_and_malformed_packets_leave_pose_intact() {
        let mut decoder = Decoder::default();
        let bytes = blend_packet();
        let frame = decoder.decode(&bytes).unwrap().unwrap();
        assert!(frame.face_found);
        assert_eq!(frame.blend("jawopen"), 0.75);
        for n in 1..bytes.len() {
            let _ = decoder.decode(&bytes[..n]);
        }
        assert_eq!(decoder.frame.blend("jawopen"), 0.75);
        let mut bad = bytes;
        bad.extend([0, 0, 0, 255]);
        assert!(decoder.decode(&bad).is_err());
        assert_eq!(decoder.frame.blend("jawopen"), 0.75);
    }
    #[test]
    fn quaternion_conversion_is_finite_and_rejects_zero() {
        assert!(angles([0.; 4]).is_err());
        let angle = angles([0., 0., 0., 1.]).unwrap();
        assert_eq!(angle, Vec3::default());
        assert!((angles([0., 0., 0.38268343, 0.9238795]).unwrap().z - 45.).abs() < 0.001);
    }
}
