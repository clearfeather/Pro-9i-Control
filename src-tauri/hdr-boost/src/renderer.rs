use std::{
    cell::Cell,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use windows::{
    core::{s, w, Error, Interface, Result, HRESULT, PCSTR},
    Foundation::{TimeSpan, TypedEventHandler},
    Graphics::{
        Capture::{
            Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem,
            GraphicsCaptureSession,
        },
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{
            CloseHandle, COLORREF, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT,
            WAIT_FAILED, WAIT_OBJECT_0, WPARAM,
        },
        Graphics::{
            Direct3D::{
                Fxc::D3DCompile, ID3DBlob, D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0,
                D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            },
            Direct3D11::*,
            DirectComposition::{DCompositionCreateDevice, IDCompositionDevice},
            Dxgi::{
                Common::{
                    DXGI_ALPHA_MODE_IGNORE, DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709,
                    DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020, DXGI_FORMAT_R16G16B16A16_FLOAT,
                    DXGI_SAMPLE_DESC,
                },
                CreateDXGIFactory1, IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGIOutput6,
                IDXGISwapChain3, DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
                DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
                DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
            Gdi::{BeginPaint, EndPaint, GetStockObject, HBRUSH, PAINTSTRUCT, WHITE_BRUSH},
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            Threading::{CreateEventW, SetEvent},
            WinRT::{
                Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
                Graphics::Capture::IGraphicsCaptureItemInterop,
                RoInitialize, RO_INIT_MULTITHREADED,
            },
        },
        UI::{
            Input::KeyboardAndMouse::{
                RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
            },
            WindowsAndMessaging::*,
        },
    },
};

use crate::{display::DisplayInfo, power::Budget, Scenario, Signals};
use windows::Win32::System::Power::{
    RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification, HPOWERNOTIFY,
};
use windows::Win32::{
    System::RemoteDesktop::{
        WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
    },
    UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
};

const STOP_KEY: i32 = 0x4c42;
#[derive(Default)]
struct CpuTiming {
    count: u64,
    total: Duration,
    max: Duration,
}
impl CpuTiming {
    fn record(&mut self, start: Instant) {
        let elapsed = start.elapsed();
        self.count += 1;
        self.total += elapsed;
        self.max = self.max.max(elapsed);
    }
    fn report(&self, name: &str) {
        println!(
            "HDR CPU {name}: mean {:.3}ms, max {:.3}ms, {} samples",
            self.total.as_secs_f64() * 1000.0 / self.count.max(1) as f64,
            self.max.as_secs_f64() * 1000.0,
            self.count
        );
    }
}
#[derive(Clone, Copy)]
struct Controls {
    gain: f32,
    split: bool,
}

thread_local! { static STOP_REQUESTED: Cell<bool> = const { Cell::new(false) }; }

struct DpiGuard(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
impl Drop for DpiGuard {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
struct SessionNotifications(HWND);
impl Drop for SessionNotifications {
    fn drop(&mut self) {
        unsafe {
            let _ = WTSUnRegisterSessionNotification(self.0);
        }
    }
}
struct PowerNotification(HPOWERNOTIFY);
impl Drop for PowerNotification {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterSuspendResumeNotification(self.0);
        }
    }
}
struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        let _ = unsafe { DestroyWindow(self.0) };
    }
}

struct FrameLatencyHandle(HANDLE);
impl Drop for FrameLatencyHandle {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct Capture {
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    item: GraphicsCaptureItem,
    arrived_token: i64,
    closed_token: i64,
    wake: Arc<CaptureWake>,
}

// A borrowed pool texture is valid only while its capture frame remains open.
// Retain the newest frame for idle redraws and release it on replacement/teardown.
struct CapturedFrame(Direct3D11CaptureFrame);
impl std::ops::Deref for CapturedFrame {
    type Target = Direct3D11CaptureFrame;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for CapturedFrame {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.pool.RemoveFrameArrived(self.arrived_token);
        let _ = self.item.RemoveClosed(self.closed_token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

// Kernel events support cross-thread signaling. Each callback owns an Arc, so
// the handle stays open even if a callback is still completing during teardown.
struct CaptureWake {
    event: HANDLE,
    arrivals: AtomicU64,
    closed: AtomicBool,
}
unsafe impl Send for CaptureWake {}
unsafe impl Sync for CaptureWake {}
impl Drop for CaptureWake {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.event) };
    }
}

fn power_budget(self_test: bool, scenario: Scenario, elapsed: Duration) -> Budget {
    match (self_test, scenario) {
        (true, Scenario::Battery) => Budget::Battery,
        (true, Scenario::BatterySaver) => Budget::Saver,
        (true, Scenario::PowerTransition) => match elapsed.as_secs() {
            2..4 => Budget::Battery,
            4..6 => Budget::Saver,
            _ => Budget::Ac,
        },
        _ => Budget::current(),
    }
}

fn set_capture_budget(session: &GraphicsCaptureSession, budget: Budget) -> Result<()> {
    match session.SetMinUpdateInterval(TimeSpan {
        Duration: (budget.interval().as_nanos() / 100) as i64,
    }) {
        Ok(()) => Ok(()),
        // The renderer also enforces the budget on older Windows versions.
        Err(error) if error.code().0 == 0x80004002u32 as i32 => Ok(()),
        Err(error) => Err(error),
    }
}

struct Hotkeys(HWND);
impl Drop for Hotkeys {
    fn drop(&mut self) {
        let _ = unsafe { UnregisterHotKey(Some(self.0), STOP_KEY) };
    }
}

struct Pipeline {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    chain: IDXGISwapChain3,
    target: ID3D11RenderTargetView,
    output: ID3D11Texture2D,
    constants: ID3D11Buffer,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    source: Option<ID3D11Texture2D>,
    source_view: Option<ID3D11ShaderResourceView>,
    borrowed_source: bool,
    width: u32,
    height: u32,
}

impl Pipeline {
    /// True means the texture is borrowed: keep its capture frame open until
    /// this source is replaced. False means the private copy owns the pixels.
    fn accept(&mut self, texture: &ID3D11Texture2D) -> Result<bool> {
        unsafe {
            let mut descriptor = D3D11_TEXTURE2D_DESC::default();
            texture.GetDesc(&mut descriptor);
            if descriptor.Format != DXGI_FORMAT_R16G16B16A16_FLOAT
                || descriptor.Width != self.width
                || descriptor.Height != self.height
            {
                return Err(failure(
                    "The display format or resolution changed. Restart LumaLift.",
                ));
            }
            self.context.PSSetShaderResources(0, Some(&[None]));
            if descriptor.BindFlags & D3D11_BIND_SHADER_RESOURCE.0 as u32 != 0 {
                let mut view = None;
                if self
                    .device
                    .CreateShaderResourceView(texture, None, Some(&mut view))
                    .is_ok()
                {
                    self.source = Some(texture.clone());
                    self.source_view = view;
                    self.borrowed_source = true;
                    return Ok(true);
                }
            }
            // Drivers without shader-readable capture textures retain the copy
            // path. Never close a borrowed capture frame while reading its pixels.
            if self.source.is_none() || self.borrowed_source {
                descriptor.Usage = D3D11_USAGE_DEFAULT;
                descriptor.BindFlags = D3D11_BIND_SHADER_RESOURCE.0 as u32;
                descriptor.CPUAccessFlags = 0;
                descriptor.MiscFlags = 0;
                let mut source = None;
                self.device
                    .CreateTexture2D(&descriptor, None, Some(&mut source))?;
                let source = source.unwrap();
                let mut view = None;
                self.device
                    .CreateShaderResourceView(&source, None, Some(&mut view))?;
                self.source = Some(source);
                self.source_view = view;
            }
            self.borrowed_source = false;
            self.context
                .CopyResource(self.source.as_ref().unwrap(), texture);
        }
        Ok(false)
    }

    fn render(&self, controls: Controls) {
        unsafe {
            let settings = [
                controls.gain,
                12.5_f32,
                f32::from(controls.split),
                self.width as f32,
            ];
            self.context.UpdateSubresource(
                &self.constants,
                0,
                None,
                settings.as_ptr().cast(),
                0,
                0,
            );
            self.context
                .OMSetRenderTargets(Some(&[Some(self.target.clone())]), None);
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: self.width as f32,
                Height: self.height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }]));
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex, None);
            self.context.PSSetShader(&self.pixel, None);
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            self.context
                .PSSetShaderResources(0, Some(std::slice::from_ref(&self.source_view)));
            self.context.Draw(3, 0);
            self.context.PSSetShaderResources(0, Some(&[None]));
        }
    }

    fn draw(&self, controls: Controls, verify: bool) -> Result<()> {
        self.render(controls);
        if verify {
            // Read BEFORE Present flips buffers: buffer zero can contain an
            // older image afterwards, especially when only strength changed.
            let input = self.pixel_at(self.source.as_ref().unwrap(), 100, self.height - 140)?;
            let boosted = self.pixel_at(&self.output, 100, self.height - 140)?;
            let expected = (input[0] * controls.gain).min(12.5);
            if !boosted[0].is_finite() || (boosted[0] - expected).abs() > expected.max(0.001) * 0.02
            {
                return Err(failure(
                    "The live HDR output did not match the captured frame gain",
                ));
            }
        }
        // The waitable one-frame queue paces composition. A zero sync interval
        // avoids holding the completed frame for an additional refresh interval.
        unsafe { self.chain.Present(0, DXGI_PRESENT(0)).ok() }
    }

    fn pixel_at(&self, texture: &ID3D11Texture2D, x: u32, y: u32) -> Result<[f32; 4]> {
        unsafe {
            self.context.OMSetRenderTargets(None, None);
            let descriptor = D3D11_TEXTURE2D_DESC {
                Width: 1,
                Height: 1,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..Default::default()
            };
            let mut staging = None;
            self.device
                .CreateTexture2D(&descriptor, None, Some(&mut staging))?;
            let staging = staging.unwrap();
            let region = D3D11_BOX {
                left: x,
                top: y,
                front: 0,
                right: x + 1,
                bottom: y + 1,
                back: 1,
            };
            self.context
                .CopySubresourceRegion(&staging, 0, 0, 0, 0, texture, 0, Some(&region));
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            let raw = std::slice::from_raw_parts(mapped.pData.cast::<u16>(), 4);
            let values = [
                half_to_float(raw[0]),
                half_to_float(raw[1]),
                half_to_float(raw[2]),
                half_to_float(raw[3]),
            ];
            self.context.Unmap(&staging, 0);
            Ok(values)
        }
    }

    fn verify_shader(&mut self) -> Result<()> {
        unsafe {
            let descriptor = D3D11_TEXTURE2D_DESC {
                Width: self.width,
                Height: self.height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
                ..Default::default()
            };
            let mut texture = None;
            self.device
                .CreateTexture2D(&descriptor, None, Some(&mut texture))?;
            let texture = texture.unwrap();
            let mut view = None;
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut view))?;
            let view = view.unwrap();
            for (input, gain, split, x, expected) in [
                (
                    [1.0, 0.5, 0.25, 1.0],
                    2.5,
                    false,
                    0,
                    [2.5, 1.25, 0.625, 1.0],
                ),
                ([0.0, 0.0, 0.0, 1.0], 2.5, false, 0, [0.0, 0.0, 0.0, 1.0]),
                (
                    [8.0, 2.0, 0.5, 1.0],
                    2.5,
                    false,
                    0,
                    [12.5, 3.125, 0.78125, 1.0],
                ),
                ([1.0, 0.5, 0.25, 1.0], 2.5, true, 0, [1.0, 0.5, 0.25, 1.0]),
                (
                    [1.0, 0.5, 0.25, 1.0],
                    2.5,
                    true,
                    self.width - 1,
                    [2.5, 1.25, 0.625, 1.0],
                ),
            ] {
                self.context.ClearRenderTargetView(&view, &input);
                self.accept(&texture)?;
                self.render(Controls { gain, split });
                let actual = self.pixel_at(&self.output, x, 0)?;
                if actual.iter().zip(expected).any(|(actual, expected)| {
                    !actual.is_finite() || (actual - expected).abs() > 0.005
                }) {
                    return Err(failure(&format!(
                        "GPU HDR shader verification failed: expected {expected:?}, received {actual:?}"
                    )));
                }
            }
            // Exercise a driver texture without shader-resource binding, too.
            // The fallback must copy into private shader-readable storage.
            let mut copy_descriptor = descriptor;
            copy_descriptor.BindFlags = D3D11_BIND_RENDER_TARGET.0 as u32;
            let mut copy_texture = None;
            self.device
                .CreateTexture2D(&copy_descriptor, None, Some(&mut copy_texture))?;
            let copy_texture = copy_texture.unwrap();
            let mut copy_target = None;
            self.device
                .CreateRenderTargetView(&copy_texture, None, Some(&mut copy_target))?;
            self.context
                .ClearRenderTargetView(&copy_target.unwrap(), &[1.0, 0.5, 0.25, 1.0]);
            if self.accept(&copy_texture)? {
                return Err(failure(
                    "Capture texture fallback did not use private storage",
                ));
            }
            self.render(Controls {
                gain: 2.5,
                split: false,
            });
            let copied = self.pixel_at(&self.output, 0, 0)?;
            if copied
                .iter()
                .zip([2.5, 1.25, 0.625, 1.0])
                .any(|(actual, expected)| !actual.is_finite() || (actual - expected).abs() > 0.005)
            {
                return Err(failure("Copied capture texture did not preserve HDR gain"));
            }
            self.source_view = None;
            self.source = None;
            self.borrowed_source = false;
            println!(
                "GPU readback passed: 2.5x gain, black preservation, HDR ceiling, hue ratios, split A/B, copied-texture fallback"
            );
            Ok(())
        }
    }
}

/// Captures only to GPU memory and presents a click-through HDR layer.
/// `self_test` closes the layer automatically after eight seconds.
pub fn run(
    display: &DisplayInfo,
    self_test: bool,
    scenario: Scenario,
    signals: &Signals,
    ready: impl FnOnce(),
    power_changed: impl Fn(&str),
) -> std::result::Result<(), String> {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED).map_err(|error| error.to_string())?;
    }
    // This dedicated worker owns the WinRT objects. The thread exits after every
    // session; do not uninitialize cached capture factories during teardown.
    if !GraphicsCaptureSession::IsSupported().map_err(|error| error.to_string())? {
        return Err("Windows Graphics Capture is not supported on this system".into());
    }
    STOP_REQUESTED.with(|stop| stop.set(false));
    unsafe { run_inner(display, self_test, scenario, signals, ready, power_changed) }
        .map_err(|error| error.to_string())
}

unsafe fn run_inner(
    display: &DisplayInfo,
    self_test: bool,
    scenario: Scenario,
    signals: &Signals,
    ready: impl FnOnce(),
    power_changed: impl Fn(&str),
) -> Result<()> {
    unsafe {
        let previous_dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _dpi = DpiGuard(previous_dpi);
        let factory: IDXGIFactory2 = CreateDXGIFactory1()?;
        let mut selected = None;
        let mut adapter_index = 0;
        while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
            let mut output_index = 0;
            while let Ok(output) = adapter.EnumOutputs(output_index) {
                let descriptor = output.GetDesc()?;
                let name = String::from_utf16_lossy(&descriptor.DeviceName)
                    .trim_end_matches('\0')
                    .to_string();
                if name == display.gdi_device_name {
                    selected = Some((adapter.clone(), output.cast::<IDXGIOutput6>()?));
                    break;
                }
                output_index += 1;
            }
            if selected.is_some() {
                break;
            }
            adapter_index += 1;
        }
        let (adapter, output) =
            selected.ok_or_else(|| failure("The Lenovo output could not be found"))?;
        let descriptor = output.GetDesc1()?;
        if descriptor.ColorSpace != DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020 {
            return Err(failure("The Lenovo panel is not currently in HDR mode"));
        }
        let bounds = descriptor.DesktopCoordinates;
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        RegisterClassW(&WNDCLASSW {
            hInstance: instance,
            lpszClassName: w!("LumaLiftDesktopLayer"),
            lpfnWndProc: Some(overlay_proc),
            ..Default::default()
        });
        let overlay = Window(CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            w!("LumaLiftDesktopLayer"),
            w!("LumaLift HDR desktop layer"),
            WS_POPUP,
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            None,
            None,
            Some(instance),
            None,
        )?);
        SetLayeredWindowAttributes(overlay.0, COLORREF(0), 255, LWA_ALPHA)?;
        // Essential: never allow the rendered layer to feed back into its capture.
        SetWindowDisplayAffinity(overlay.0, WDA_EXCLUDEFROMCAPTURE)?;
        let _hotkeys = Hotkeys(overlay.0);
        RegisterHotKey(
            Some(overlay.0),
            STOP_KEY,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            0x42,
        )?;
        WTSRegisterSessionNotification(overlay.0, NOTIFY_FOR_THIS_SESSION)?;
        let _notifications = SessionNotifications(overlay.0);
        let _power = PowerNotification(RegisterSuspendResumeNotification(
            HANDLE(overlay.0 .0),
            DEVICE_NOTIFY_WINDOW_HANDLE,
        )?);
        let mut affinity = 0;
        GetWindowDisplayAffinity(overlay.0, &mut affinity)?;
        if affinity != WDA_EXCLUDEFROMCAPTURE.0 {
            return Err(failure("Capture exclusion could not be verified"));
        }
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let base_adapter: IDXGIAdapter = adapter.cast()?;
        D3D11CreateDevice(
            &base_adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let device = device.unwrap();
        let context = context.unwrap();
        let dxgi_device: IDXGIDevice = device.cast()?;
        let winrt_device: IDirect3DDevice =
            CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device)?.cast()?;
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = interop.CreateForMonitor(descriptor.Monitor)?;
        let size = item.Size()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::R16G16B16A16Float,
            // One texture may remain checked out for idle strength changes.
            // Leave two buffers available for capture, as in the copy path.
            3,
            size,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        session.SetIsCursorCaptureEnabled(false)?;
        let mut budget = power_budget(self_test, scenario, Duration::ZERO);
        let mut budget_history = vec![budget];
        set_capture_budget(&session, budget)?;
        println!(
            "HDR power budget: {budget:?}, minimum {:.2}ms",
            budget.interval().as_secs_f64() * 1000.0
        );
        let wake = Arc::new(CaptureWake {
            event: CreateEventW(None, false, false, None)?,
            arrivals: AtomicU64::new(0),
            closed: AtomicBool::new(false),
        });
        let arrived = wake.clone();
        let arrived_token = pool.FrameArrived(&TypedEventHandler::new(move |_, _| {
            arrived.arrivals.fetch_add(1, Ordering::Relaxed);
            SetEvent(arrived.event)
        }))?;
        let closed = wake.clone();
        let closed_token = item.Closed(&TypedEventHandler::new(move |_, _| {
            closed.closed.store(true, Ordering::Release);
            SetEvent(closed.event)
        }))?;
        let capture = Capture {
            pool,
            session,
            item,
            arrived_token,
            closed_token,
            wake,
        };

        let swap_descriptor = DXGI_SWAP_CHAIN_DESC1 {
            Width: size.Width as u32,
            Height: size.Height as u32,
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            // Every output pixel is opaque. Let DWM treat the full-screen HDR
            // surface as opaque instead of requesting per-pixel transparency.
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
            ..Default::default()
        };
        let chain: IDXGISwapChain3 = factory
            .CreateSwapChainForComposition(&device, &swap_descriptor, None)?
            .cast()?;
        chain.SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709)?;
        chain.SetMaximumFrameLatency(1)?;
        if chain.GetMaximumFrameLatency()? != 1 {
            return Err(failure(
                "The HDR presentation queue could not be limited to one frame",
            ));
        }
        let frame_latency = FrameLatencyHandle(chain.GetFrameLatencyWaitableObject());
        if frame_latency.0.is_invalid() {
            return Err(failure("The HDR frame-latency wait handle is unavailable"));
        }
        let composition: IDCompositionDevice = DCompositionCreateDevice(&dxgi_device)?;
        let target = composition.CreateTargetForHwnd(overlay.0, true)?;
        let visual = composition.CreateVisual()?;
        visual.SetContent(&chain)?;
        target.SetRoot(&visual)?;
        composition.Commit()?;

        let back_buffer: ID3D11Texture2D = chain.GetBuffer(0)?;
        let mut target_view = None;
        device.CreateRenderTargetView(&back_buffer, None, Some(&mut target_view))?;
        let mut constant_buffer = None;
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: 16,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            },
            None,
            Some(&mut constant_buffer),
        )?;
        let vs_code = compile(s!("vertexMain"), s!("vs_5_0"))?;
        let ps_code = compile(s!("pixelMain"), s!("ps_5_0"))?;
        let mut vertex = None;
        let mut pixel = None;
        device.CreateVertexShader(blob_bytes(&vs_code), None, Some(&mut vertex))?;
        device.CreatePixelShader(blob_bytes(&ps_code), None, Some(&mut pixel))?;
        let mut pipeline = Pipeline {
            device,
            context,
            chain,
            target: target_view.unwrap(),
            output: back_buffer,
            constants: constant_buffer.unwrap(),
            vertex: vertex.unwrap(),
            pixel: pixel.unwrap(),
            source: None,
            source_view: None,
            borrowed_source: false,
            width: size.Width as u32,
            height: size.Height as u32,
        };
        let marker = if self_test {
            pipeline.verify_shader()?;
            RegisterClassW(&WNDCLASSW {
                hInstance: instance,
                lpszClassName: w!("LumaLiftVerificationMarker"),
                lpfnWndProc: Some(marker_proc),
                hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
                ..Default::default()
            });
            let marker = Window(CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("LumaLiftVerificationMarker"),
                w!("LumaLift verification"),
                WS_POPUP,
                bounds.left + 64,
                bounds.bottom - 180,
                160,
                100,
                None,
                None,
                Some(instance),
                None,
            )?);
            let _ = ShowWindow(marker.0, SW_SHOWNOACTIVATE);
            SetWindowPos(
                marker.0,
                Some(overlay.0),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )?;
            Some(marker)
        } else {
            None
        };
        println!(
            "Live HDR pipeline initialized: {}x{}, FP16 capture → linear gain → FP16 scRGB composition",
            size.Width, size.Height
        );
        println!("Capture exclusion enabled; Ctrl+Alt+B stops and restores");
        crate::desktop::ensure_active().map_err(|e| failure(&e))?;
        capture.session.StartCapture()?;
        let started = Instant::now();
        let mut frames = 0_u64;
        let mut retained_frame = None;
        let mut borrowed_frames = 0_u64;
        let mut presented = 0_u64;
        let mut present_time = Duration::ZERO;
        let mut longest_present = Duration::ZERO;
        let mut frame_slot_ready = false;
        let mut shown = false;
        let mut ready = Some(ready);
        let mut last_frame = Instant::now();
        let mut last_present = Instant::now();
        let mut drawn_gain = None;
        let mut accepted_arrivals = 0;
        let mut idle_stage = 0;
        let mut quiet_submissions = None;
        let mut last_health = Instant::now();
        let mut health_time = CpuTiming::default();
        let mut wait_time = CpuTiming::default();
        let mut poll_time = CpuTiming::default();
        let mut accept_time = CpuTiming::default();
        let mut close_time = CpuTiming::default();
        let mut capture_ages = Vec::<f64>::new();
        let mut qpc_frequency = 0;
        if self_test {
            QueryPerformanceFrequency(&mut qpc_frequency)?;
        }
        let mut descriptor_logged = false;
        let mut injected = false;
        let mut last_feedback_sample = Instant::now();
        let mut feedback_samples = Vec::new();
        let mut message = MSG::default();
        let result = 'live: loop {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                if message.message == WM_QUIT {
                    break 'live Ok(());
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            let controls = Controls {
                gain: signals.gain(),
                split: false,
            };
            if signals.stopped() {
                break Ok(());
            }
            if capture.wake.closed.load(Ordering::Acquire) {
                break Err(failure("The captured Lenovo display closed"));
            }
            if self_test
                && scenario != Scenario::Live
                && !injected
                && started.elapsed() >= Duration::from_secs(2)
            {
                injected = true;
                match scenario {
                    Scenario::DisplayChange => {
                        SendMessageW(
                            overlay.0,
                            WM_DISPLAYCHANGE,
                            Some(WPARAM(0)),
                            Some(LPARAM(0)),
                        );
                    }
                    Scenario::Suspend => {
                        SendMessageW(
                            overlay.0,
                            WM_POWERBROADCAST,
                            Some(WPARAM(4)),
                            Some(LPARAM(0)),
                        );
                    }
                    Scenario::Emergency => {
                        SendMessageW(
                            overlay.0,
                            WM_HOTKEY,
                            Some(WPARAM(STOP_KEY as usize)),
                            Some(LPARAM(0)),
                        );
                    }
                    Scenario::Failure => break Err(failure("Injected rendering failure")),
                    Scenario::Panic => panic!("Injected renderer panic"),
                    Scenario::PresentationStall => {}
                    Scenario::CaptureStall => {
                        capture.wake.arrivals.fetch_add(1, Ordering::Relaxed);
                    }
                    Scenario::Idle => {
                        // Emulate a desktop producing no new content. The retained
                        // source must remain valid with capture stopped.
                        capture.session.Close()?;
                        println!("Idle test: capture paused after two seconds");
                    }
                    Scenario::Live
                    | Scenario::Battery
                    | Scenario::BatterySaver
                    | Scenario::PowerTransition => (),
                }
            }
            if last_health.elapsed() >= Duration::from_secs(1) {
                let health_started = Instant::now();
                crate::desktop::ensure_active().map_err(|e| failure(&e))?;
                pipeline.device.GetDeviceRemovedReason()?;
                let next_budget = power_budget(self_test, scenario, started.elapsed());
                if next_budget != budget {
                    set_capture_budget(&capture.session, next_budget)?;
                    budget = next_budget;
                    budget_history.push(budget);
                    power_changed(budget.message());
                    println!(
                        "HDR power budget: {budget:?}, minimum {:.2}ms",
                        budget.interval().as_secs_f64() * 1000.0
                    );
                }
                let now = crate::display::active_displays().map_err(|e| failure(&e))?;
                if !now.iter().any(|d| {
                    d.device_path == display.device_path
                        && d.gdi_device_name == display.gdi_device_name
                        && d.hdr_enabled
                        && d.refresh == display.refresh
                }) {
                    break Err(failure("The display configuration or HDR state changed"));
                }
                last_health = Instant::now();
                if self_test {
                    health_time.record(health_started);
                }
            }
            if STOP_REQUESTED.with(Cell::get)
                || (self_test && started.elapsed() >= Duration::from_secs(8))
            {
                break Ok(());
            }
            if self_test && scenario == Scenario::Idle {
                if started.elapsed() >= Duration::from_secs(3) && quiet_submissions.is_none() {
                    quiet_submissions = Some(presented);
                }
                let stage = started.elapsed().as_secs() / 2;
                if stage >= 2 && stage != idle_stage {
                    let gain = if stage == 2 { 1.5_f32 } else { 2.5_f32 };
                    signals.gain.store(gain.to_bits(), Ordering::SeqCst);
                    idle_stage = stage;
                    println!("Idle test: changing cached-frame strength to {gain}");
                    continue;
                }
            }
            // Pace against the compositor, with at most one queued frame. Pump
            // stop/power/display messages while waiting, then take the freshest
            // capture only after DXGI has room. No fixed 60-Hz sleep or busy spin.
            if (!frame_slot_ready || (injected && scenario == Scenario::PresentationStall))
                && last_present.elapsed() > Duration::from_secs(5)
            {
                break Err(failure("HDR presentation stalled; overlay stopped"));
            }
            if self_test && injected && scenario == Scenario::PresentationStall {
                MsgWaitForMultipleObjectsEx(None, 20, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
                continue;
            }
            if !frame_slot_ready {
                let wait_started = Instant::now();
                let wait = MsgWaitForMultipleObjectsEx(
                    Some(&[frame_latency.0]),
                    20,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
                if self_test {
                    wait_time.record(wait_started);
                }
                if wait == WAIT_FAILED {
                    break Err(Error::from_win32());
                }
                if wait != WAIT_OBJECT_0 {
                    continue;
                }
                frame_slot_ready = true;
            }
            if drawn_gain.is_some() && last_present.elapsed() < budget.interval() {
                // Bound processing as well as Windows capture, including on OS
                // versions without MinUpdateInterval. Keep message/stop latency bounded.
                let remaining = budget.interval().saturating_sub(last_present.elapsed());
                MsgWaitForMultipleObjectsEx(
                    None,
                    remaining.as_millis().clamp(1, 20) as u32,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
                continue;
            }
            let mut latest = None;
            let mut content_changed = false;
            let poll_started = Instant::now();
            let poll_limit = if self_test && injected && scenario == Scenario::CaptureStall {
                0
            } else {
                3
            };
            for _ in 0..poll_limit {
                match capture.pool.TryGetNextFrame() {
                    Ok(frame) => {
                        content_changed = true;
                        latest = Some(CapturedFrame(frame));
                    }
                    // WinRT's nullable TryGetNextFrame result is represented as an
                    // Error with S_OK by windows-rs when no frame is queued yet.
                    Err(error) if error.code().0 == 0 || error.code().0 == 0x80004003u32 as i32 => {
                        break;
                    }
                    Err(error) => {
                        eprintln!("Capture poll failed: {error}");
                        break 'live Err(error);
                    }
                }
            }
            if self_test {
                poll_time.record(poll_started);
            }
            if let Some(frame) = latest {
                let accept_started = Instant::now();
                let frame_result = (|| -> Result<bool> {
                    let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
                    let texture: ID3D11Texture2D = access.GetInterface()?;
                    if self_test {
                        let mut now = 0;
                        QueryPerformanceCounter(&mut now)?;
                        capture_ages.push(
                            (now as f64 / qpc_frequency as f64
                                - frame.SystemRelativeTime()?.Duration as f64 / 10_000_000.0)
                                * 1000.0,
                        );
                        if !descriptor_logged {
                            let mut desc = D3D11_TEXTURE2D_DESC::default();
                            texture.GetDesc(&mut desc);
                            println!(
                                "Capture texture: bind={:#x}, usage={:?}, misc={:#x}",
                                desc.BindFlags, desc.Usage, desc.MiscFlags
                            );
                            descriptor_logged = true;
                        }
                    }
                    let content_size = frame.ContentSize()?;
                    if content_size.Width != size.Width || content_size.Height != size.Height {
                        return Err(failure("The capture size changed"));
                    }
                    pipeline.accept(&texture)
                })();
                if self_test {
                    accept_time.record(accept_started);
                }
                let close_started = Instant::now();
                match frame_result {
                    Ok(true) => {
                        retained_frame = Some(frame);
                        borrowed_frames += 1;
                    }
                    Ok(false) => {
                        retained_frame = None;
                        drop(frame);
                    }
                    Err(error) => {
                        eprintln!("Capture frame processing failed: {error}");
                        break Err(error);
                    }
                }
                if self_test {
                    close_time.record(close_started);
                }
                frames += 1;
                last_frame = Instant::now();
                accepted_arrivals = capture.wake.arrivals.load(Ordering::Relaxed);
            }
            if frames == 0 && started.elapsed() > Duration::from_secs(5) {
                break Err(failure(
                    "Windows did not deliver a capture frame. No desktop overlay was shown.",
                ));
            }
            // A static desktop may legitimately produce no new frames. Detect
            // notified frames that cannot be consumed; closed capture and GPU
            // removal are checked separately even while the desktop is idle.
            if frames > 0
                && last_frame.elapsed() > Duration::from_secs(5)
                && capture.wake.arrivals.load(Ordering::Relaxed) != accepted_arrivals
            {
                break Err(failure("Desktop capture stalled; overlay stopped"));
            }
            let needs_draw = content_changed || drawn_gain != Some(controls.gain);
            if pipeline.source.is_some() && needs_draw {
                let submit_started = Instant::now();
                let verify = self_test
                    && (last_feedback_sample.elapsed() > Duration::from_secs(1)
                        || drawn_gain != Some(controls.gain));
                if let Err(error) = pipeline.draw(controls, verify) {
                    break Err(error);
                }
                let submit_time = submit_started.elapsed();
                present_time += submit_time;
                longest_present = longest_present.max(submit_time);
                presented += 1;
                last_present = Instant::now();
                frame_slot_ready = false;
                drawn_gain = Some(controls.gain);
                if !shown {
                    let _ = ShowWindow(overlay.0, SW_SHOWNOACTIVATE);
                    if let Some(ready) = ready.take() {
                        ready();
                    }
                    power_changed(budget.message());
                    shown = true;
                }
            }
            if self_test && shown && last_feedback_sample.elapsed() > Duration::from_secs(1) {
                let pixel = pipeline.pixel_at(
                    pipeline.source.as_ref().unwrap(),
                    100,
                    pipeline.height - 140,
                )?;
                feedback_samples.push(pixel[0]);
                println!("Unboosted capture marker: {:.4} scRGB", pixel[0]);
                let hit = WindowFromPoint(POINT {
                    x: bounds.left + 100,
                    y: bounds.bottom - 140,
                });
                if hit != marker.as_ref().unwrap().0 {
                    break Err(failure(
                        "The HDR layer did not pass mouse hit-testing to the underlying verification window",
                    ));
                }
                last_feedback_sample = Instant::now();
            }
            if let Some(marker) = marker
                .as_ref()
                .filter(|_| scenario != Scenario::Idle && needs_draw)
            {
                // Animate the bounded-test marker by one pixel so capture-rate
                // measurements include changing content. The sampled point stays white.
                SetWindowPos(
                    marker.0,
                    None,
                    bounds.left + 64 + (presented % 2) as i32,
                    bounds.bottom - 180,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
            }
            if frame_slot_ready {
                // No GPU work is pending. Sleep on capture arrival or window
                // messages rather than polling/spinning on an already-ready DXGI slot.
                let wait = MsgWaitForMultipleObjectsEx(
                    Some(&[capture.wake.event]),
                    20,
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
                if wait == WAIT_FAILED {
                    break Err(Error::from_win32());
                }
            }
        };
        let _ = ShowWindow(overlay.0, SW_HIDE);
        drop(retained_frame);
        if result.is_ok()
            && self_test
            && scenario == Scenario::PowerTransition
            && budget_history != [Budget::Ac, Budget::Battery, Budget::Saver, Budget::Ac]
        {
            return Err(failure("Power budget transitions were not all exercised"));
        }
        println!(
            "Live HDR session ended after {:.1}s; captured {frames} frame(s)",
            started.elapsed().as_secs_f64()
        );
        println!(
            "HDR capture textures: {borrowed_frames} borrowed, {} copied",
            frames - borrowed_frames
        );
        println!(
            "HDR pacing: {presented} submissions ({:.1}/s), {:.1} captures/s; Present CPU time mean {:.2}ms, max {:.2}ms; queue limit=1",
            presented as f64 / started.elapsed().as_secs_f64(),
            frames as f64 / started.elapsed().as_secs_f64(),
            present_time.as_secs_f64() * 1000.0 / presented.max(1) as f64,
            longest_present.as_secs_f64() * 1000.0,
        );
        if let Some(quiet) = quiet_submissions {
            println!(
                "Idle test: {} submissions during five quiet seconds (two strength changes)",
                presented - quiet
            );
            if result.is_ok() && presented - quiet != 2 {
                return Err(failure(
                    "Idle renderer did not sleep or update cached-frame strength",
                ));
            }
        }
        if self_test {
            for (name, timing) in [
                ("health", health_time),
                ("wait", wait_time),
                ("poll", poll_time),
                ("accept", accept_time),
                ("close", close_time),
            ] {
                timing.report(name);
            }
            capture_ages.sort_by(f64::total_cmp);
            if !capture_ages.is_empty() {
                println!(
                    "HDR capture age at acquisition: mean {:.2}ms, p95 {:.2}ms",
                    capture_ages.iter().sum::<f64>() / capture_ages.len() as f64,
                    capture_ages[(capture_ages.len() * 95 / 100).min(capture_ages.len() - 1)]
                );
            }
        }
        if let Err(error) = &result {
            eprintln!("Live HDR result: {error}");
        }
        if result.is_ok() && !signals.stopped() && frames == 0 {
            return Err(failure("No capture frames were received"));
        }
        if result.is_ok()
            && self_test
            && matches!(
                scenario,
                Scenario::Live
                    | Scenario::Idle
                    | Scenario::Battery
                    | Scenario::BatterySaver
                    | Scenario::PowerTransition
            )
        {
            let reference = display.sdr_white_nits.unwrap_or(80) as f32 / 80.0;
            if feedback_samples.len() < 3
                || feedback_samples
                    .iter()
                    .any(|value| !value.is_finite() || (value - reference).abs() > reference * 0.15)
            {
                return Err(failure(&format!(
                    "Capture-exclusion verification failed: reference={reference}, samples={feedback_samples:?}"
                )));
            }
            println!(
                "Capture exclusion and mouse passthrough passed with {} stable marker samples",
                feedback_samples.len()
            );
        }
        result
    }
}

unsafe extern "system" fn marker_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

fn compile(entry: PCSTR, target: PCSTR) -> Result<ID3DBlob> {
    let source = include_bytes!("boost.hlsl");
    let mut code = None;
    let mut errors = None;
    unsafe {
        if let Err(error) = D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("boost.hlsl"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        ) {
            let detail = errors
                .as_ref()
                .map(|blob| String::from_utf8_lossy(blob_bytes(blob)).into_owned())
                .unwrap_or_default();
            return Err(Error::new(
                error.code(),
                format!("HDR shader compilation failed: {detail}"),
            ));
        }
    }
    code.ok_or_else(|| failure("The shader compiler returned no code"))
}

unsafe fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}

fn failure(message: &str) -> Error {
    Error::new(HRESULT(0x80004005u32 as i32), message)
}

fn half_to_float(value: u16) -> f32 {
    let sign = if value & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = (value >> 10) & 31;
    let fraction = value & 1023;
    match exponent {
        0 => sign * (fraction as f32 / 1024.0) * 2.0_f32.powi(-14),
        31 if fraction == 0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + fraction as f32 / 1024.0) * 2.0_f32.powi(exponent as i32 - 15),
    }
}

unsafe extern "system" fn overlay_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_HOTKEY if wparam.0 as i32 == STOP_KEY => {
            STOP_REQUESTED.with(|stop| stop.set(true));
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE | WM_ENDSESSION => {
            STOP_REQUESTED.with(|stop| stop.set(true));
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_POWERBROADCAST if matches!(wparam.0, 4 | 7 | 18) => {
            STOP_REQUESTED.with(|stop| stop.set(true));
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
            LRESULT(1)
        }
        WM_WTSSESSION_CHANGE if matches!(wparam.0, 2 | 4 | 7) => {
            STOP_REQUESTED.with(|stop| stop.set(true));
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(window, &mut paint);
                let _ = EndPaint(window, &paint);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
