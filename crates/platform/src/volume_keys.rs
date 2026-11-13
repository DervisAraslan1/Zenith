//! Takes the volume keys away from Windows so its own volume flyout never
//! opens: a low-level keyboard hook on its own thread swallows Volume Up,
//! Volume Down and Mute and sets the master volume itself. The change then
//! reaches the notch through `levels::LevelWatcher` like any other.
//!
//! Brightness keys are handled by the keyboard firmware or driver and never
//! reach a hook, so Windows still shows its brightness flyout.
//!
//! The hook runs on a dedicated thread with its own message loop, so a busy
//! UI thread can never delay key presses. Turning the setting off lets the
//! keys through again without removing the hook.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
    SetWindowsHookExW, TranslateMessage, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

/// Whether the hook swallows the keys (the user's setting).
static ENABLED: AtomicBool = AtomicBool::new(true);
/// Windows' own step: 2 percent.
const STEP: f32 = 0.02;

thread_local! {
    /// Fetched again after a failure (the default device may have changed).
    static ENDPOINT: RefCell<Option<IAudioEndpointVolume>> = const { RefCell::new(None) };
}

/// Installs the hook once, on its own thread.
pub fn install() {
    let spawned = std::thread::Builder::new()
        .name("zenith-volume-keys".into())
        .spawn(|| {
            // SAFETY: COM and the hook belong to this thread; the message
            // loop below keeps the hook alive for the life of the process.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let module = GetModuleHandleW(None).unwrap_or_default();
                if let Err(e) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), Some(module.into()), 0) {
                    tracing::warn!("volume keys: hook: {e}");
                    return;
                }
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("volume keys thread: {e}");
    }
}

/// Turns the swallowing on or off (Settings).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

fn endpoint() -> Option<IAudioEndpointVolume> {
    ENDPOINT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            // SAFETY: standard MMDevice activation on this COM thread.
            *slot = unsafe {
                CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .and_then(|en| en.GetDefaultAudioEndpoint(eRender, eConsole))
                    .and_then(|dev| dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
                    .ok()
            };
        }
        slot.clone()
    })
}

/// Applies one key. Returns false when the endpoint is not available, so
/// the key can go on to Windows instead of doing nothing.
fn apply(vk: u16) -> bool {
    let Some(ep) = endpoint() else {
        return false;
    };
    // SAFETY: plain getters and setters on a live COM object.
    let ok = unsafe {
        match vk {
            v if v == VK_VOLUME_MUTE.0 => ep
                .GetMute()
                .and_then(|m| ep.SetMute(!m.as_bool(), std::ptr::null())),
            v => ep.GetMasterVolumeLevelScalar().and_then(|cur| {
                let delta = if v == VK_VOLUME_UP.0 { STEP } else { -STEP };
                // Snap to the step grid, as Windows does.
                let next = ((cur + delta) / STEP).round() * STEP;
                if v == VK_VOLUME_UP.0 {
                    let _ = ep.SetMute(false, std::ptr::null());
                }
                ep.SetMasterVolumeLevelScalar(next.clamp(0.0, 1.0), std::ptr::null())
            }),
        }
    };
    if ok.is_err() {
        ENDPOINT.with(|cell| *cell.borrow_mut() = None);
    }
    ok.is_ok()
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && ENABLED.load(Ordering::Relaxed) {
        // SAFETY: for WH_KEYBOARD_LL, lparam points at a KBDLLHOOKSTRUCT.
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kb.vkCode as u16;
        let ours = vk == VK_VOLUME_UP.0 || vk == VK_VOLUME_DOWN.0 || vk == VK_VOLUME_MUTE.0;
        if ours {
            let down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
            // Key down applies; both down and up are swallowed, so Windows
            // never sees the key and never opens its flyout.
            if !down || apply(vk) {
                return LRESULT(1);
            }
        }
    }
    // SAFETY: passes everything else on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
