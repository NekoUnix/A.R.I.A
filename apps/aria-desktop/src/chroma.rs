//! Compact avatar color occupancy for choosing a separated chroma-key background.
use anyhow::{Result, ensure};

#[derive(Clone)]
pub struct Palette {
    // 5 bits/channel retains small accents without storing decoded atlas copies.
    bins: Vec<bool>,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            bins: vec![false; 32 * 32 * 32],
        }
    }
}
impl Palette {
    pub fn add_rgba(&mut self, rgba: &[u8]) {
        for p in rgba.as_chunks::<4>().0 {
            if p[3] >= 16 {
                self.add_rgb([p[0], p[1], p[2]]);
            }
        }
    }
    pub fn add_rgb(&mut self, [r, g, b]: [u8; 3]) {
        self.bins[((r as usize >> 3) << 10) | ((g as usize >> 3) << 5) | (b as usize >> 3)] = true;
    }
    pub fn merge(&mut self, other: &Self) {
        for (a, b) in self.bins.iter_mut().zip(&other.bins) {
            *a |= b;
        }
    }
    pub fn suggest(&self) -> Result<Suggestion> {
        let colors: Vec<_> = self
            .bins
            .iter()
            .enumerate()
            .filter(|(_, used)| **used)
            .map(|(i, _)| {
                chroma([
                    (((i >> 10) & 31) * 8 + 4) as u8,
                    (((i >> 5) & 31) * 8 + 4) as u8,
                    ((i & 31) * 8 + 4) as u8,
                ])
            })
            .collect();
        ensure!(
            !colors.is_empty(),
            "No visible avatar colors were available to analyze"
        );
        // Saturated RGB-cube edges avoid gray keys and cover more than green/blue.
        let mut best = Suggestion {
            rgb: [0, 255, 0],
            distance: -1.0,
            colors: colors.len(),
        };
        for n in (0..=255).step_by(5) {
            for rgb in [
                [0, 255, n],
                [0, n, 255],
                [n, 0, 255],
                [255, 0, n],
                [255, n, 0],
                [n, 255, 0],
            ] {
                let c = chroma(rgb);
                let distance = colors
                    .iter()
                    .map(|p| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2))
                    .fold(f32::INFINITY, f32::min)
                    .sqrt();
                if distance > best.distance {
                    best.rgb = rgb;
                    best.distance = distance;
                }
            }
        }
        // Allow for the quantization cell radius. This is guidance, not an OBS guarantee.
        best.distance = (best.distance - 0.02).max(0.0);
        Ok(best)
    }
}
pub struct Suggestion {
    pub rgb: [u8; 3],
    pub distance: f32,
    pub colors: usize,
}
// BT.709 Cb/Cr chroma separation, consistent with OBS's standard chroma-key metric.
// Constant offsets cancel when measuring distance; input channels are nonlinear RGB.
fn chroma(rgb: [u8; 3]) -> [f32; 2] {
    let [r, g, b] = rgb.map(|v| f32::from(v) / 255.0);
    [
        -0.100644 * r - 0.338572 * g + 0.439216 * b,
        0.439216 * r - 0.398942 * g - 0.040274 * b,
    ]
}
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
pub fn parse_hex(text: &str) -> Result<[u8; 3]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    ensure!(
        text.len() == 6 && text.bytes().all(|c| c.is_ascii_hexdigit()),
        "Enter six hex digits, such as #00FF00"
    );
    Ok([
        u8::from_str_radix(&text[0..2], 16)?,
        u8::from_str_radix(&text[2..4], 16)?,
        u8::from_str_radix(&text[4..6], 16)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hex_validation_never_accepts_partial_or_alpha_values() {
        assert_eq!(parse_hex(" #aBcD01 ").unwrap(), [171, 205, 1]);
        assert_eq!(hex(parse_hex("00ff00").unwrap()), "#00FF00");
        for bad in ["", "#abc", "#000000ff", "#GG00FF", "🦊00"] {
            assert!(parse_hex(bad).is_err());
        }
    }
    #[test]
    fn ignores_transparent_padding_and_avoids_artwork_colors() {
        let mut p = Palette::default();
        p.add_rgba(&[255, 0, 255, 0]);
        assert!(p.suggest().is_err());
        p.add_rgba(&[
            0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
        ]);
        let result = p.suggest().unwrap();
        assert!(result.distance > 0.3);
        assert_ne!(result.rgb, [0, 255, 0]);
        let mut other = Palette::default();
        other.add_rgb(result.rgb);
        p.merge(&other);
        assert_ne!(p.suggest().unwrap().rgb, result.rgb);
    }
    #[test]
    fn detects_when_no_well_separated_key_exists() {
        let p = Palette {
            bins: vec![true; 32 * 32 * 32],
        };
        assert!(p.suggest().unwrap().distance < 0.02);
    }
}
