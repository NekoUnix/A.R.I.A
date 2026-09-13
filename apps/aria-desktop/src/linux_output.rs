//! Bounded asynchronous GPU readback into the native Linux OBS canvas protocol.
use anyhow::{Context, Result, ensure};
use eframe::egui_wgpu::RenderState;
use std::{
    ffi::{CString, c_char, c_void},
    ptr::NonNull,
    sync::mpsc,
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn aria_canvas_create(name: *const c_char, w: u32, h: u32, format: u32) -> *mut c_void;
    #[cfg(test)]
    fn aria_canvas_path(canvas: *mut c_void) -> *const c_char;
    fn aria_canvas_write(canvas: *mut c_void, pixels: *const u8, stride: usize) -> i32;
    fn aria_canvas_destroy(canvas: *mut c_void);
}
pub struct Bridge {
    device: wgpu::Device,
    queue: wgpu::Queue,
}
impl Bridge {
    pub fn new(state: &RenderState) -> Result<Self> {
        Ok(Self {
            device: state.device.clone(),
            queue: state.queue.clone(),
        })
    }
    pub fn sender(&self, name: &str, texture: &wgpu::Texture) -> Result<Sender> {
        let format = match texture.format() {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => 1,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 2,
            _ => anyhow::bail!("Linux OBS output requires an 8-bit canvas"),
        };
        let name = format!("{name} ({})", std::process::id());
        let cname = CString::new(name.as_str())?;
        let raw = unsafe {
            aria_canvas_create(cname.as_ptr(), texture.width(), texture.height(), format)
        };
        let handle = NonNull::new(raw).context("Cannot reserve Linux shared canvas. Check free /dev/shm space (up to 64 MiB per 4096×4096 output).")?;
        let stride = (texture.width() * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA asynchronous OBS readback"),
            size: u64::from(stride) * u64::from(texture.height()),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Sender {
            handle,
            name,
            texture: texture.clone(),
            buffer,
            stride,
            pending: None,
            dirty: true,
            last_capture: None,
        })
    }
    pub fn send(&self, sender: &mut Sender, changed: bool) -> Result<bool> {
        sender.dirty |= changed;
        // Poll only; neither OBS contention nor the GPU can block the UI thread.
        self.device.poll(wgpu::PollType::Poll)?;
        if let Some(rx) = &sender.pending {
            match rx.try_recv() {
                Ok(result) => {
                    sender.pending = None;
                    result?;
                    let pixels = sender.buffer.slice(..).get_mapped_range()?;
                    let result = unsafe {
                        aria_canvas_write(
                            sender.handle.as_ptr(),
                            pixels.as_ptr(),
                            sender.stride as usize,
                        )
                    };
                    drop(pixels);
                    sender.buffer.unmap();
                    ensure!(result >= 0, "Linux canvas publication failed");
                    if result == 0 {
                        sender.dirty = true;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(false),
                Err(mpsc::TryRecvError::Disconnected) => {
                    anyhow::bail!("GPU readback callback disconnected")
                }
            }
        }
        if sender.dirty
            && sender
                .last_capture
                .is_none_or(|t| t.elapsed() >= Duration::from_micros(16_667))
        {
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                sender.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &sender.buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(sender.stride),
                        rows_per_image: Some(sender.texture.height()),
                    },
                },
                sender.texture.size(),
            );
            self.queue.submit([encoder.finish()]);
            let (tx, rx) = mpsc::sync_channel(1);
            sender
                .buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(result);
                });
            sender.pending = Some(rx);
            sender.dirty = false;
            sender.last_capture = Some(Instant::now());
        }
        Ok(true)
    }
}
pub struct Sender {
    handle: NonNull<c_void>,
    name: String,
    texture: wgpu::Texture,
    buffer: wgpu::Buffer,
    stride: u32,
    pending: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    dirty: bool,
    last_capture: Option<Instant>,
}
impl Sender {
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        // destroy cancels a pending map and also accepts an already-unmapped
        // buffer. Unconditional unmap is a validation error on wgpu 30.
        self.buffer.destroy();
        unsafe {
            aria_canvas_destroy(self.handle.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_canvas_permissions_bounded_dimensions_and_cleanup() {
        use std::os::unix::fs::PermissionsExt;
        let name = CString::new("ARIA Test").unwrap();
        unsafe {
            assert!(aria_canvas_create(name.as_ptr(), 4097, 64, 1).is_null());
            assert!(aria_canvas_create(name.as_ptr(), 64, 64, 9).is_null());
            let c = aria_canvas_create(name.as_ptr(), 64, 64, 1);
            assert!(!c.is_null());
            let path = std::ffi::CStr::from_ptr(aria_canvas_path(c))
                .to_str()
                .unwrap()
                .to_owned();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let pixels = vec![128u8; 64 * 64 * 4];
            assert_eq!(aria_canvas_write(c, pixels.as_ptr(), 256), 1);
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(&bytes[..8], b"ARIACV01");
            assert_eq!(&bytes[256..], pixels);
            aria_canvas_destroy(c);
            assert!(!std::path::Path::new(&path).exists());
        }
    }

    #[test]
    #[ignore = "requires a Vulkan device (software Vulkan works); run in Linux native CI"]
    fn gpu_readback_preserves_canvas_pixels_without_blocking_send() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = eframe::egui_wgpu::Renderer::new(
            &device,
            wgpu::TextureFormat::Bgra8Unorm,
            Default::default(),
        );
        let state = RenderState {
            instance,
            surface_config: eframe::egui_wgpu::SurfaceConfig::LOW_LATENCY,
            adapter,
            available_adapters: vec![],
            device,
            queue,
            target_format: wgpu::TextureFormat::Bgra8Unorm,
            renderer: std::sync::Arc::new(eframe::egui::mutex::RwLock::new(renderer)),
        };
        let bridge = Bridge::new(&state).unwrap();
        for (width, height, format) in [
            (65, 128, wgpu::TextureFormat::Bgra8Unorm),
            (128, 65, wgpu::TextureFormat::Rgba8Unorm),
        ] {
            let texture = state.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Linux native output test"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let pixels: Vec<u8> = (0..width * height)
                .flat_map(|i| [(i % 128) as u8, 32, 64, 128])
                .collect();
            state.queue.write_texture(
                texture.as_image_copy(),
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture.size(),
            );
            let mut sender = bridge.sender("ARIA CI", &texture).unwrap();
            let path =
                unsafe { std::ffi::CStr::from_ptr(aria_canvas_path(sender.handle.as_ptr())) }
                    .to_str()
                    .unwrap()
                    .to_owned();
            bridge.send(&mut sender, true).unwrap();
            // Only the test waits; production send() always uses Poll.
            state
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(5)),
                })
                .unwrap();
            bridge.send(&mut sender, false).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(
                &bytes[256..],
                pixels,
                "row padding/channel order/alpha changed"
            );
            assert_eq!(u64::from_le_bytes(bytes[24..32].try_into().unwrap()), 1);
            bridge.send(&mut sender, false).unwrap();
            assert!(
                sender.pending.is_none(),
                "unchanged canvas queued another readback"
            );
            drop(sender);
            assert!(!std::path::Path::new(&path).exists());
            // Close immediately after submitting another frame, before poll.
            let mut in_flight = bridge.sender("ARIA CI pending close", &texture).unwrap();
            let path =
                unsafe { std::ffi::CStr::from_ptr(aria_canvas_path(in_flight.handle.as_ptr())) }
                    .to_str()
                    .unwrap()
                    .to_owned();
            bridge.send(&mut in_flight, true).unwrap();
            drop(in_flight);
            assert!(!std::path::Path::new(&path).exists());
            state
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(5)),
                })
                .unwrap();
        }
    }
}
