//! Import ceilings, not reserved memory. GPU textures still respect device limits.
pub const MIB: u64 = 1024 * 1024;
pub const IMAGE_FILE: u64 = 512 * MIB;
pub const IMAGE_SIDE: u32 = 40_960;
pub const IMAGE_DECODED: u64 = 4096 * MIB;
pub const GIF_FRAMES: usize = 4096;
pub const GIF_SOURCE_TOTAL: u64 = 256 * 1024 * MIB;
pub const GIF_PLAYBACK_MIB: u32 = 256;
pub const IMAGE_COLLECTION: u64 = 2560 * MIB;
pub const MOC_FILE: usize = 1280 * 1024 * 1024;
pub const MODEL_JSON: usize = 20 * 1024 * 1024;
pub const EXPRESSION_JSON: usize = 10 * 1024 * 1024;
pub const CORE_ALLOCATION: u64 = 5120 * MIB;
pub const ATLAS_FILE: u64 = 1280 * MIB;
pub const ATLAS_SIDE: u32 = 81_920;
pub const ATLAS_DECODED: u64 = 5120 * MIB;
pub const ATLAS_COLLECTION: u64 = 10 * 1024 * MIB;

/// Preserve aspect ratio while fitting an upload to a GPU's actual texture limit.
pub fn texture_size(width: u32, height: u32, maximum: u32) -> [u32; 2] {
    let maximum = maximum.max(1);
    let largest = width.max(height).max(1);
    if largest <= maximum {
        return [width.max(1), height.max(1)];
    }
    [
        ((u64::from(width) * u64::from(maximum)) / u64::from(largest)).max(1) as u32,
        ((u64::from(height) * u64::from(maximum)) / u64::from(largest)).max(1) as u32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_fit_preserves_small_images_and_bounds_very_large_dimensions() {
        assert_eq!(texture_size(300, 800, 8192), [300, 800]);
        assert_eq!(texture_size(40_960, 20_480, 8192), [8192, 4096]);
        assert_eq!(texture_size(256, 81_920, 8192), [25, 8192]);
        assert_eq!(texture_size(u32::MAX, 1, 8192), [8192, 1]);
    }
}
