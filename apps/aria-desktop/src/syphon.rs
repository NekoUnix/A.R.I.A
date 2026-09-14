use anyhow::{Context, Result, ensure};
use eframe::egui_wgpu::RenderState;
use std::{
    ffi::{CString, c_char, c_void},
    ptr::NonNull,
};

unsafe extern "C" {
    fn aria_syphon_create(
        path: *const c_char,
        name: *const c_char,
        device: *const c_void,
        queue: *const c_void,
    ) -> *mut c_void;
    fn aria_syphon_send(sender: *mut c_void, texture: *const c_void, changed: i32) -> i32;
    fn aria_syphon_destroy(sender: *mut c_void);
}
pub struct Bridge {
    device: wgpu::Device,
    queue: wgpu::Queue,
    framework: CString,
}
impl Bridge {
    pub fn new(state: &RenderState) -> Result<Self> {
        ensure!(
            unsafe { state.device.as_hal::<wgpu::hal::api::Metal>() }.is_some(),
            "Syphon requires Metal"
        );
        let exe = std::env::current_exe()?;
        let dir = exe.parent().context("Missing application folder")?;
        let path = std::env::var_os("ARIA_SYPHON_FRAMEWORK").map(std::path::PathBuf::from)
            .or_else(|| [dir.join("../Frameworks/Syphon.framework"), dir.join("Syphon.framework")].into_iter().find(|p| p.exists()))
            .context("Syphon.framework is missing. Use the complete macOS Alpha app bundle, or run scripts/build-syphon.sh and set ARIA_SYPHON_FRAMEWORK for a source build.")?;
        Ok(Self {
            device: state.device.clone(),
            queue: state.queue.clone(),
            framework: CString::new(path.canonicalize()?.to_string_lossy().as_bytes())?,
        })
    }
    pub fn sender(&self, name: &str, texture: &wgpu::Texture) -> Result<Sender> {
        let device = unsafe { self.device.as_hal::<wgpu::hal::api::Metal>() }
            .context("Missing Metal device")?;
        let queue = unsafe { self.queue.as_hal::<wgpu::hal::api::Metal>() }
            .context("Missing Metal queue")?;
        let name = format!("{name} ({})", std::process::id());
        let cname = CString::new(name.as_str())?;
        // Objective-C retains the device/queue. The source texture stays owned by Sender.
        let handle = unsafe {
            aria_syphon_create(
                self.framework.as_ptr(),
                cname.as_ptr(),
                std::ptr::from_ref(&**device.raw_device()).cast(),
                std::ptr::from_ref(queue.as_raw()).cast(),
            )
        };
        Ok(Sender {
            handle: NonNull::new(handle).context(
                "Cannot start Syphon server; check the bundled framework and macOS Console",
            )?,
            texture: texture.clone(),
            name,
        })
    }
    pub fn send(&self, sender: &Sender, changed: bool) -> Result<bool> {
        let texture = unsafe { sender.texture.as_hal::<wgpu::hal::api::Metal>() }
            .context("Missing Metal canvas")?;
        let result = unsafe {
            aria_syphon_send(
                sender.handle.as_ptr(),
                std::ptr::from_ref(texture.raw_handle()).cast(),
                i32::from(changed),
            )
        };
        ensure!(result >= 0, "Metal command buffer creation failed");
        Ok(result == 1)
    }
}
pub struct Sender {
    handle: NonNull<c_void>,
    texture: wgpu::Texture,
    name: String,
}
impl Sender {
    pub fn name(&self) -> &str {
        &self.name
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        unsafe {
            aria_syphon_destroy(self.handle.as_ptr());
        }
    }
}
