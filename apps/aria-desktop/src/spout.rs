//! Windows Spout 2 sender: D3D11On12 GPU copies, no per-frame CPU readback.
//! Wire layout/names follow Lynn Jarvis's Spout2 SDK (BSD-2-Clause).
//! See docs/licenses/spout.txt and docs/obs-output.md for protocol references.
use anyhow::{Context, Result, bail, ensure};
use eframe::egui_wgpu::RenderState;
use std::{collections::BTreeSet, ffi::CString, ptr::NonNull};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct3D11::*,
            Direct3D11on12::*,
            Direct3D12::*,
            Dxgi::{Common::*, IDXGIResource},
        },
        System::{Memory::*, Registry::*, Threading::*},
    },
    core::{Interface, PCSTR, s},
};

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct NamedMutex(OwnedHandle);
struct Lock<'a>(&'a NamedMutex);
impl NamedMutex {
    fn new(name: &str) -> Result<Self> {
        let name = CString::new(name)?;
        // Null security uses the current user's session; no global namespace.
        Ok(Self(OwnedHandle(unsafe {
            CreateMutexA(None, false, PCSTR(name.as_ptr().cast()))?
        })))
    }
    fn lock(&self, timeout: u32) -> Result<Option<Lock<'_>>> {
        match unsafe { WaitForSingleObject(self.0.0, timeout) } {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Some(Lock(self))),
            WAIT_TIMEOUT => Ok(None),
            _ => Err(windows::core::Error::from_win32().into()),
        }
    }
}
impl Drop for Lock<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0.0.0);
        }
    }
}
struct Map {
    _handle: OwnedHandle,
    ptr: NonNull<u8>,
    len: usize,
    mutex: NamedMutex,
    existed: bool,
}
impl Map {
    fn new(name: &str, requested: usize) -> Result<Self> {
        let cname = CString::new(name)?;
        let (handle, existed) = unsafe {
            let handle = CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                requested as u32,
                PCSTR(cname.as_ptr().cast()),
            )?;
            (OwnedHandle(handle), GetLastError() == ERROR_ALREADY_EXISTS)
        };
        let mutex = NamedMutex::new(&format!("{name}_mutex"))?;
        let view = unsafe { MapViewOfFile(handle.0, FILE_MAP_ALL_ACCESS, 0, 0, 0) };
        let ptr = NonNull::new(view.Value.cast::<u8>()).context("Cannot map Spout metadata")?;
        // Existing older senders can create a smaller list. Never trust the
        // requested size for an existing mapping; bound access to its region.
        let mut info = MEMORY_BASIC_INFORMATION::default();
        let length = unsafe {
            VirtualQuery(
                Some(ptr.as_ptr().cast()),
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if length == 0 {
            unsafe {
                let _ = UnmapViewOfFile(view);
            }
            bail!("Cannot determine Spout map size");
        }
        Ok(Self {
            _handle: handle,
            ptr,
            len: requested.min(info.RegionSize),
            mutex,
            existed,
        })
    }
    fn edit<T>(&self, f: impl FnOnce(&mut [u8]) -> Result<T>) -> Result<T> {
        let _guard = self
            .mutex
            .lock(67)?
            .context("Spout metadata is busy; retry output")?;
        // The mapping is live, checked to len, and accessed exclusively under
        // the exact named mutex used by the official Spout implementation.
        f(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) })
    }
}
impl Drop for Map {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.ptr.as_ptr().cast(),
            });
        }
    }
}
fn read_names(bytes: &[u8]) -> BTreeSet<Vec<u8>> {
    bytes
        .as_chunks::<256>()
        .0
        .iter()
        .take_while(|s| s[0] != 0)
        .map(|s| s[..s.iter().position(|&b| b == 0).unwrap_or(255)].to_vec())
        .collect()
}
fn write_names(bytes: &mut [u8], names: &BTreeSet<Vec<u8>>) {
    for (i, slot) in bytes.as_chunks_mut::<256>().0.iter_mut().enumerate() {
        slot.fill(0);
        if let Some(name) = names.iter().nth(i) {
            slot[..name.len().min(255)].copy_from_slice(&name[..name.len().min(255)]);
        }
    }
}
fn max_senders() -> usize {
    let mut value = 64_u32;
    let mut len = 4;
    // Read only, following Spout's MaxSenders convention; never change it.
    unsafe {
        let _ = RegGetValueA(
            HKEY_CURRENT_USER,
            s!("Software\\Leading Edge\\Spout"),
            s!("MaxSenders"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut len),
        );
    }
    (value as usize).clamp(4, 4096)
}

struct Registration {
    name: String,
    names: Map,
    info: Map,
    access: NamedMutex,
    count: OwnedHandle,
}
impl Registration {
    fn new(base: &str, size: [u32; 2], share: HANDLE, format: DXGI_FORMAT) -> Result<Self> {
        let names = Map::new("SpoutSenderNames", max_senders() * 256)?;
        let (name, info, access, count) = names.edit(|bytes| {
            let mut all = read_names(bytes);
            let mut candidate = None;
            for suffix in 0..100 {
                let name = match suffix {
                    0 => base.to_string(),
                    1 => format!("{base} ({})", std::process::id()),
                    _ => format!("{base} ({}-{suffix})", std::process::id()),
                };
                let info = Map::new(&name, 280)?;
                if !info.existed {
                    candidate = Some((name, info));
                    break;
                }
            }
            let (name, info) = candidate.context("Spout sender name is already in use")?;
            ensure!(
                all.len() < bytes.len() / 256 || all.contains(name.as_bytes()),
                "Spout sender list is full"
            );
            info.edit(|data| {
                ensure!(data.len() >= 280, "Spout metadata map is too small");
                data.fill(0);
                for (i, v) in [share.0 as usize as u32, size[0], size[1], format.0 as u32]
                    .iter()
                    .enumerate()
                {
                    data[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
                let path = std::env::current_exe()?.to_string_lossy().into_owned();
                let path = path.as_bytes();
                let n = path.len().min(255);
                data[20..20 + n].copy_from_slice(&path[..n]);
                Ok(())
            })?;
            let access = NamedMutex::new(&format!("{name}_SpoutAccessMutex"))?;
            let count_name = CString::new(format!("{name}_Count_Semaphore"))?;
            let count = OwnedHandle(unsafe {
                CreateSemaphoreA(None, 1, i32::MAX, PCSTR(count_name.as_ptr().cast()))?
            });
            all.insert(name.as_bytes().to_vec());
            write_names(bytes, &all);
            Ok((name, info, access, count))
        })?;
        Ok(Self {
            name,
            names,
            info,
            access,
            count,
        })
    }
    fn new_frame(&self) {
        unsafe {
            if WaitForSingleObject(self.count.0, 0) == WAIT_OBJECT_0 {
                // Spout receivers read this monotonically increasing semaphore
                // count without consuming it. Restore the consumed count if
                // the semaphore has reached LONG_MAX.
                if ReleaseSemaphore(self.count.0, 2, None).is_err() {
                    let _ = ReleaseSemaphore(self.count.0, 1, None);
                }
            }
        }
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.names.edit(|bytes| {
            let mut names = read_names(bytes);
            names.remove(self.name.as_bytes());
            write_names(bytes, &names);
            Ok(())
        });
        let _ = self.info.edit(|bytes| {
            bytes.fill(0);
            Ok(())
        });
    }
}

pub struct Bridge {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    on12: ID3D11On12Device,
}
impl Bridge {
    pub fn new(state: &RenderState) -> Result<Self> {
        let hal = unsafe { state.device.as_hal::<wgpu::hal::api::Dx12>() }.context(
            "Spout needs the Windows DirectX 12 backend. Restart without WGPU_BACKEND=vulkan.",
        )?;
        let mut device = None;
        let mut context = None;
        // Reuse wgpu's queue so model render, canvas render and the GPU copy are
        // ordered. All bridge operations run on ARIA's render/event thread.
        unsafe {
            D3D11On12CreateDevice(
                hal.raw_device(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT.0,
                None,
                Some(&[Some(hal.raw_queue().cast()?)]),
                0,
                Some(&mut device),
                Some(&mut context),
                None,
            )?;
        }
        let device = device.context("D3D11On12 returned no device")?;
        let on12 = device.cast()?;
        Ok(Self {
            device,
            context: context.context("D3D11On12 returned no context")?,
            on12,
        })
    }
    pub fn sender(&self, name: &str, texture: &wgpu::Texture) -> Result<Sender> {
        let format = match texture.format() {
            wgpu::TextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
            wgpu::TextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
            other => bail!("Spout cannot share canvas format {other:?}"),
        };
        let hal = unsafe { texture.as_hal::<wgpu::hal::api::Dx12>() }
            .context("Missing DX12 canvas texture")?;
        let mut wrapped: Option<ID3D11Resource> = None;
        let mut shared = None;
        unsafe {
            // export() explicitly leaves the wgpu texture in COPY_SRC. Release
            // restores that same state, keeping wgpu's tracker accurate.
            self.on12.CreateWrappedResource(
                hal.raw_resource(),
                &D3D11_RESOURCE_FLAGS::default(),
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                &mut wrapped,
            )?;
            self.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: texture.width(),
                    Height: texture.height(),
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                    MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut shared),
            )?;
        }
        let shared: ID3D11Texture2D = shared.context("Missing Spout shared texture")?;
        let dxgi: IDXGIResource = shared.cast()?;
        let handle = unsafe { dxgi.GetSharedHandle()? };
        // Legacy DXGI shared handles are owned by the resource, not CloseHandle.
        let registration =
            Registration::new(name, [texture.width(), texture.height()], handle, format)?;
        Ok(Sender {
            registration,
            wrapped: wrapped.context("Missing wrapped canvas")?,
            shared,
            _texture: texture.clone(),
        })
    }
    pub fn send(&self, sender: &Sender) -> Result<bool> {
        // A busy OBS receiver must never stall tracking. Keep its previous frame.
        let Some(_guard) = sender.registration.access.lock(0)? else {
            return Ok(false);
        };
        unsafe {
            let resources = [Some(sender.wrapped.clone())];
            self.on12.AcquireWrappedResources(&resources);
            self.context.CopyResource(&sender.shared, &sender.wrapped);
            self.on12.ReleaseWrappedResources(&resources);
            // Official Spout SendTexture synchronization: submit the GPU copy
            // before publishing frame count and releasing the access mutex.
            self.context.Flush();
        }
        sender.registration.new_frame();
        Ok(true)
    }
}
pub struct Sender {
    registration: Registration,
    wrapped: ID3D11Resource,
    shared: ID3D11Texture2D,
    // Keep the underlying wgpu resource alive as long as its D3D11 wrapper.
    _texture: wgpu::Texture,
}
impl Sender {
    pub fn name(&self) -> &str {
        &self.registration.name
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[test]
    fn sender_list_preserves_foreign_names_and_terminates_slots() {
        let names = BTreeSet::from([
            b"ARIA".to_vec(),
            vec![0xc0, 0xff, 0xfe],
            b"Other app".to_vec(),
        ]);
        let mut bytes = vec![0; 1024];
        write_names(&mut bytes, &names);
        assert_eq!(read_names(&bytes), names);
        assert_eq!(bytes[768], 0);
        assert_eq!(bytes[255], 0);
    }
    pub fn gpu_state() -> RenderState {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = eframe::egui_wgpu::Renderer::new(
            &device,
            wgpu::TextureFormat::Bgra8Unorm,
            Default::default(),
        );
        RenderState {
            adapter,
            available_adapters: vec![],
            device,
            queue,
            target_format: wgpu::TextureFormat::Bgra8Unorm,
            renderer: std::sync::Arc::new(eframe::egui::mutex::RwLock::new(renderer)),
        }
    }
    #[test]
    #[ignore = "requires Windows DX12 GPU; creates temporary Spout senders"]
    fn dx12_texture_reaches_independent_dx11_receiver_with_alpha_and_cleanup() {
        use windows::Win32::Graphics::{Direct3D::D3D_DRIVER_TYPE_UNKNOWN, Dxgi::IDXGIDevice};
        let state = gpu_state();
        let bridge = Bridge::new(&state).unwrap();
        let base = format!("ARIA Test {}", std::process::id());
        for (width, height) in [(1920, 1080), (1080, 1920), (1536, 1024)] {
            let texture = state.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Spout test canvas"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let sender = bridge.sender(&base, &texture).unwrap();
            let second = bridge.sender(&base, &texture).unwrap();
            assert_ne!(sender.name(), second.name());
            let mut encoder = state.device.create_command_encoder(&Default::default());
            drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture.create_view(&Default::default()),
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.25,
                            b: 0.4,
                            a: 0.5,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            }));
            encoder.transition_resources(
                std::iter::empty(),
                std::iter::once(wgpu::TextureTransition {
                    texture: &texture,
                    selector: None,
                    state: wgpu::TextureUses::COPY_SRC,
                }),
            );
            state.queue.submit([encoder.finish()]);
            assert!(bridge.send(&sender).unwrap());
            // Parse the documented wire bytes as an unrelated receiver does.
            let info = Map::new(sender.name(), 280).unwrap();
            let words = info
                .edit(|b| {
                    Ok(b[..16]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|v| u32::from_le_bytes(*v))
                        .collect::<Vec<_>>())
                })
                .unwrap();
            assert_eq!(&words[1..], &[width, height, 87]);
            let dxgi: IDXGIDevice = bridge.device.cast().unwrap();
            let mut receiver = None;
            let mut context = None;
            unsafe {
                D3D11CreateDevice(
                    &dxgi.GetAdapter().unwrap(),
                    D3D_DRIVER_TYPE_UNKNOWN,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut receiver),
                    None,
                    Some(&mut context),
                )
                .unwrap();
                let receiver = receiver.unwrap();
                let context = context.unwrap();
                let mut received: Option<ID3D11Texture2D> = None;
                receiver
                    .OpenSharedResource(HANDLE(words[0] as usize as *mut _), &mut received)
                    .unwrap();
                let received = received.unwrap();
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                received.GetDesc(&mut desc);
                assert_eq!([desc.Width, desc.Height], [width, height]);
                desc.Usage = D3D11_USAGE_STAGING;
                desc.BindFlags = 0;
                desc.MiscFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                let mut staging = None;
                receiver
                    .CreateTexture2D(&desc, None, Some(&mut staging))
                    .unwrap();
                let staging = staging.unwrap();
                let _guard = sender.registration.access.lock(1000).unwrap().unwrap();
                context.CopyResource(&staging, &received);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                context
                    .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .unwrap();
                for (x, y) in [(0, 0), (width / 2, height / 2), (width - 1, height - 1)] {
                    let pixel = std::slice::from_raw_parts(
                        mapped
                            .pData
                            .cast::<u8>()
                            .add((y * mapped.RowPitch + x * 4) as usize),
                        4,
                    );
                    for (actual, expected) in pixel.iter().zip([102u8, 64, 26, 128]) {
                        assert!(actual.abs_diff(expected) <= 1, "{pixel:?}");
                    }
                }
                context.Unmap(&staging, 0);
            }
            let second_name = second.name().as_bytes().to_vec();
            drop(sender);
            assert!(info.edit(|b| Ok(b[..16].iter().all(|&v| v == 0))).unwrap());
            assert!(
                second
                    .registration
                    .names
                    .edit(|b| Ok(read_names(b).contains(&second_name)))
                    .unwrap()
            );
        }
    }
}
