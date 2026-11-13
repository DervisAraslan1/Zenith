//! System volume and screen brightness changes, plus the live output peak.
//!
//! [`LevelWatcher`] runs one worker thread that polls the default playback
//! endpoint's master volume every 60 ms (a cheap in-process call) and the
//! built-in display's brightness through WMI every 500 ms (a desktop
//! without a WMI brightness source simply never reports one). Changes go
//! to the UI through a channel; the first reading is the baseline and is
//! not reported, so nothing pops up at start.
//!
//! [`AudioMeter`] reads the playback peak for the pill's waveform; it is
//! cheap enough to call every frame on the UI thread.

use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use windows::Win32::Media::Audio::Endpoints::{IAudioEndpointVolume, IAudioMeterInformation};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, EOAC_NONE,
    RPC_C_AUTHN_LEVEL_CALL, RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
    CoCreateInstance, CoInitializeEx, CoInitializeSecurity, CoSetProxyBlanket,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4, VT_UI1, VariantClear};
use windows::Win32::System::Wmi::{
    IWbemContext, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
    WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE, WbemLocator,
};
use windows::core::{BSTR, Interface, w};

const VOLUME_POLL: Duration = Duration::from_millis(60);
const BRIGHTNESS_POLL: Duration = Duration::from_millis(500);
/// The default device can change; the endpoint is fetched again this often.
const ENDPOINT_REFRESH: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LevelChange {
    /// Master volume 0..1 and mute.
    Volume { level: f32, muted: bool },
    /// Built-in display brightness 0..1.
    Brightness(f32),
}

/// Latest known levels: (volume, muted) and brightness, for sliders.
static CURRENT: Mutex<(Option<(f32, bool)>, Option<f32>)> = Mutex::new((None, None));

/// Latest volume (0..1, muted) and brightness (0..1); `None` until read,
/// brightness stays `None` without a WMI source.
pub fn current() -> (Option<(f32, bool)>, Option<f32>) {
    CURRENT.lock().map(|c| *c).unwrap_or((None, None))
}

fn store_volume(v: (f32, bool)) {
    if let Ok(mut c) = CURRENT.lock() {
        c.0 = Some(v);
    }
}

fn store_brightness(b: f32) {
    if let Ok(mut c) = CURRENT.lock() {
        c.1 = Some(b);
    }
}

pub struct LevelWatcher {
    rx: Receiver<LevelChange>,
}

impl LevelWatcher {
    pub fn spawn(on_change: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("zenith-levels".into())
            .spawn(move || {
                // SAFETY: COM for this thread only; the objects never leave it.
                unsafe {
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                let mut volume: Option<IAudioEndpointVolume> = None;
                let mut refresh_at = Instant::now();
                let mut last_volume: Option<(f32, bool)> = None;
                let mut wmi: Option<IWbemServices> = None;
                let mut wmi_failed = false;
                let mut last_brightness: Option<u8> = None;
                let mut brightness_at = Instant::now();
                loop {
                    let now = Instant::now();
                    if now >= refresh_at {
                        // A new endpoint may sit at another level: re-baseline.
                        volume = endpoint::<IAudioEndpointVolume>()
                            .inspect_err(|e| tracing::warn!("levels: volume endpoint: {e}"))
                            .ok();
                        last_volume = None;
                        refresh_at = now + ENDPOINT_REFRESH;
                    }
                    if let Some(v) = &volume {
                        // SAFETY: plain getters on a live COM object.
                        let read = unsafe { (v.GetMasterVolumeLevelScalar(), v.GetMute()) };
                        match read {
                            (Ok(level), Ok(muted)) => {
                                let cur = (level, muted.as_bool());
                                store_volume(cur);
                                let changed = last_volume.is_some_and(|(l, m)| {
                                    (l - cur.0).abs() > 0.004 || m != cur.1
                                });
                                last_volume = Some(cur);
                                if changed {
                                    tracing::debug!("levels: volume {cur:?}");
                                    let msg = LevelChange::Volume {
                                        level: cur.0,
                                        muted: cur.1,
                                    };
                                    if tx.send(msg).is_err() {
                                        return;
                                    }
                                    on_change();
                                }
                            }
                            _ => volume = None,
                        }
                    }
                    if !wmi_failed && now >= brightness_at {
                        brightness_at = now + BRIGHTNESS_POLL;
                        if wmi.is_none() {
                            wmi = wmi_services()
                                .inspect_err(|e| tracing::warn!("levels: WMI connect: {e}"))
                                .ok();
                        }
                        match wmi.as_ref().and_then(brightness) {
                            Some(b) => {
                                store_brightness(b as f32 / 100.0);
                                if last_brightness.is_some_and(|p| p != b) {
                                    if tx.send(LevelChange::Brightness(b as f32 / 100.0)).is_err()
                                    {
                                        return;
                                    }
                                    on_change();
                                }
                                last_brightness = Some(b);
                            }
                            // No brightness source (external monitor): stop asking.
                            None if last_brightness.is_none() => {
                                tracing::info!("levels: no WMI brightness source");
                                wmi_failed = true;
                            }
                            None => {}
                        }
                    }
                    std::thread::sleep(VOLUME_POLL);
                }
            })?;
        Ok(Self { rx })
    }

    /// Changes since the last call, oldest first.
    pub fn poll(&self) -> Vec<LevelChange> {
        self.rx.try_iter().collect()
    }
}

/// An interface on the default playback endpoint.
fn endpoint<T: Interface>() -> windows::core::Result<T> {
    // SAFETY: standard MMDevice activation; COM is initialized by the caller.
    unsafe {
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let dev = en.GetDefaultAudioEndpoint(eRender, eConsole)?;
        dev.Activate::<T>(CLSCTX_ALL, None)
    }
}

fn wmi_services() -> windows::core::Result<IWbemServices> {
    // SAFETY: in-process WMI locator; COM is initialized on this thread.
    unsafe {
        // WMI refuses anonymous callers: ask for impersonation. Process-wide
        // and only once; a later call (or one made by someone else first)
        // fails harmlessly with RPC_E_TOO_LATE.
        let _ = CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        );
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let services = locator.ConnectServer(
            &BSTR::from("ROOT\\WMI"),
            &BSTR::new(),
            &BSTR::new(),
            &BSTR::new(),
            0,
            &BSTR::new(),
            None::<&IWbemContext>,
        )?;
        // RPC_C_AUTHN_WINNT (10), RPC_C_AUTHZ_NONE (0).
        CoSetProxyBlanket(
            &services,
            10,
            0,
            windows::core::PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        Ok(services)
    }
}

/// Current brightness percent of the first WMI monitor, if any.
fn brightness(services: &IWbemServices) -> Option<u8> {
    // SAFETY: one forward-only query; the VARIANT is cleared after reading.
    unsafe {
        let rows = services
            .ExecQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT CurrentBrightness FROM WmiMonitorBrightness"),
                WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0),
                None::<&IWbemContext>,
            )
            .inspect_err(|e| tracing::warn!("levels: WMI query: {e}"))
            .ok()?;
        let mut objs = [None];
        let mut n = 0u32;
        let hr = rows.Next(WBEM_INFINITE, &mut objs, &mut n);
        if objs[0].is_none() {
            tracing::warn!("levels: WMI brightness: no rows ({hr:?})");
        }
        let obj = objs[0].take()?;
        let mut v = VARIANT::default();
        obj.Get(w!("CurrentBrightness"), 0, &mut v, None, None).ok()?;
        let b = v.Anonymous.Anonymous.Anonymous.bVal;
        let _ = VariantClear(&mut v);
        Some(b.min(100))
    }
}

/// Peak level of what is playing right now, 0..1.
pub struct AudioMeter {
    meter: Option<IAudioMeterInformation>,
    refresh_at: Instant,
}

impl Default for AudioMeter {
    fn default() -> Self {
        Self {
            meter: None,
            refresh_at: Instant::now(),
        }
    }
}

impl AudioMeter {
    pub fn peak(&mut self) -> f32 {
        let now = Instant::now();
        if now >= self.refresh_at {
            // SAFETY: harmless if COM is already initialized on this thread.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            self.meter = endpoint::<IAudioMeterInformation>()
                .inspect_err(|e| tracing::warn!("levels: meter endpoint: {e}"))
                .ok();
            self.refresh_at = now + ENDPOINT_REFRESH * 2;
        }
        match &self.meter {
            // SAFETY: plain getter on a live COM object.
            Some(m) => unsafe { m.GetPeakValue() }.unwrap_or(0.0).clamp(0.0, 1.0),
            None => 0.0,
        }
    }
}

/// A level to set (from a slider).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SetLevel {
    Volume(f32),
    Mute(bool),
    Brightness(f32),
}

/// Sets a level on a worker thread. Calls made while it is busy (a slider
/// being dragged) collapse into the latest value of each kind.
pub fn set_level(level: SetLevel) {
    static TX: OnceLock<Option<Sender<SetLevel>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<SetLevel>();
        std::thread::Builder::new()
            .name("zenith-set-level".into())
            .spawn(move || control_loop(rx))
            .inspect_err(|e| tracing::warn!("set level thread: {e}"))
            .ok()
            .map(|_| tx)
    });
    // Show the new value at once; the watcher confirms it.
    if let Ok(mut c) = CURRENT.lock() {
        match level {
            SetLevel::Volume(v) => c.0 = Some((v, false)),
            SetLevel::Mute(m) => c.0 = c.0.map(|(v, _)| (v, m)),
            SetLevel::Brightness(b) => {
                if c.1.is_some() {
                    c.1 = Some(b);
                }
            }
        }
    }
    if let Some(tx) = tx {
        let _ = tx.send(level);
    }
}

fn control_loop(rx: Receiver<SetLevel>) {
    // SAFETY: COM for this thread only; the objects never leave it.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let mut volume: Option<IAudioEndpointVolume> = None;
    let mut wmi: Option<IWbemServices> = None;
    while let Ok(first) = rx.recv() {
        // Keep only the newest value of each kind.
        let (mut vol, mut mute, mut bright) = (None, None, None);
        for l in std::iter::once(first).chain(rx.try_iter()) {
            match l {
                SetLevel::Volume(v) => vol = Some(v),
                SetLevel::Mute(m) => mute = Some(m),
                SetLevel::Brightness(b) => bright = Some(b),
            }
        }
        if vol.is_some() || mute.is_some() {
            if volume.is_none() {
                volume = endpoint::<IAudioEndpointVolume>().ok();
            }
            if let Some(ep) = &volume {
                // SAFETY: plain setters on a live COM object.
                let r = unsafe {
                    let a = match vol {
                        Some(v) => ep
                            .SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null())
                            .and_then(|()| ep.SetMute(false, std::ptr::null())),
                        None => Ok(()),
                    };
                    a.and_then(|()| match mute {
                        Some(m) => ep.SetMute(m, std::ptr::null()),
                        None => Ok(()),
                    })
                };
                if let Err(e) = r {
                    tracing::warn!("set volume: {e}");
                    volume = None;
                }
            }
        }
        if let Some(b) = bright {
            if wmi.is_none() {
                wmi = wmi_services().ok();
            }
            if let Some(s) = &wmi
                && let Err(e) = set_brightness(s, (b.clamp(0.0, 1.0) * 100.0).round() as u8)
            {
                tracing::warn!("set brightness: {e}");
                wmi = None;
            }
        }
    }
}

/// `WmiMonitorBrightnessMethods.WmiSetBrightness(Timeout, Brightness)`.
fn set_brightness(services: &IWbemServices, percent: u8) -> windows::core::Result<()> {
    use windows::Win32::Foundation::E_FAIL;
    let fail = || windows::core::Error::from(E_FAIL);
    // SAFETY: WMI calls on this thread's services; VARIANTs are cleared.
    unsafe {
        let rows = services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from("SELECT * FROM WmiMonitorBrightnessMethods"),
            WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0),
            None::<&IWbemContext>,
        )?;
        let mut objs = [None];
        let mut n = 0u32;
        let _ = rows.Next(WBEM_INFINITE, &mut objs, &mut n);
        let instance = objs[0].take().ok_or_else(fail)?;
        let mut path = VARIANT::default();
        instance.Get(w!("__PATH"), 0, &mut path, None, None)?;
        let path_str: BSTR = (*path.Anonymous.Anonymous.Anonymous.bstrVal).clone();
        let _ = VariantClear(&mut path);

        let mut class = None;
        services.GetObject(
            &BSTR::from("WmiMonitorBrightnessMethods"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None::<&IWbemContext>,
            Some(&mut class),
            None,
        )?;
        let class = class.ok_or_else(fail)?;
        let (mut input, mut output) = (None, None);
        class.GetMethod(w!("WmiSetBrightness"), 0, &mut input, &mut output)?;
        let params = input.ok_or_else(fail)?.SpawnInstance(0)?;

        let mut timeout = VARIANT::default();
        (*timeout.Anonymous.Anonymous).vt = VT_I4;
        (*timeout.Anonymous.Anonymous).Anonymous.lVal = 1;
        params.Put(w!("Timeout"), 0, &timeout, 0)?;
        let mut level = VARIANT::default();
        (*level.Anonymous.Anonymous).vt = VT_UI1;
        (*level.Anonymous.Anonymous).Anonymous.bVal = percent.min(100);
        params.Put(w!("Brightness"), 0, &level, 0)?;

        services.ExecMethod(
            &path_str,
            &BSTR::from("WmiSetBrightness"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None::<&IWbemContext>,
            &params,
            None,
            None,
        )
    }
}
