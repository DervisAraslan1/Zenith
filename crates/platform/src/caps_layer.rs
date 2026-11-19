//! CapsLock as a shortcut key: hold CapsLock and press a letter (or digit)
//! to fire that shortcut, like `caps+KeyQ`. A low-level keyboard hook on its
//! own thread swallows CapsLock while it is held; tapped alone it still
//! toggles caps lock (the hook sends a fresh CapsLock press on release).
//!
//! Why not Ctrl+Alt: Windows treats AltGr as Ctrl+Alt, so on layouts like
//! Turkish Q those shortcuts eat characters (AltGr+Q is `@`). Why not Fn: the
//! keyboard firmware handles it and Windows never sees it.
//!
//! Only keys that are bound are swallowed; CapsLock with any other key lets
//! that key through and just skips the caps toggle.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_CAPITAL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
    SetWindowsHookExW, TranslateMessage, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

/// Lowest key code a binding can use ('0'); bit `vk - FIRST` in [`KEYS`].
const FIRST: u32 = 0x30;
/// Highest: 'Z'.
const LAST: u32 = 0x5A;
/// Marks the CapsLock presses the hook sends itself, so it lets them pass.
const MARK: usize = 0x5A45_4E49;

/// Bound keys as a bit set over `FIRST..=LAST`.
static KEYS: AtomicU64 = AtomicU64::new(0);
/// CapsLock is down.
static HELD: AtomicBool = AtomicBool::new(false);
/// Another key was pressed while CapsLock was down, so no toggle on release.
static USED: AtomicBool = AtomicBool::new(false);
/// Bound key currently down, so auto-repeat fires only once.
static DOWN: AtomicU32 = AtomicU32::new(0);
static CALLBACK: OnceLock<Box<dyn Fn(u32) + Send + Sync>> = OnceLock::new();

/// Parses `caps+KeyQ`, `caps+Q` or `caps+Digit1` into a key code. Returns
/// None for anything else (an ordinary `ctrl+...` accelerator).
pub fn parse(accel: &str) -> Option<u32> {
    let (head, key) = accel.split_once('+')?;
    if !head.trim().eq_ignore_ascii_case("caps") {
        return None;
    }
    let key = key.trim();
    let key = key
        .strip_prefix("Key")
        .or_else(|| key.strip_prefix("Digit"))
        .unwrap_or(key);
    let mut chars = key.chars();
    let c = chars.next()?.to_ascii_uppercase();
    if chars.next().is_some() || !c.is_ascii_alphanumeric() {
        return None;
    }
    Some(c as u32)
}

/// Installs the hook once, on its own thread. `on_key` runs on that thread
/// with the key code of each bound key pressed while CapsLock is held.
pub fn install(on_key: impl Fn(u32) + Send + Sync + 'static) {
    if CALLBACK.set(Box::new(on_key)).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("zenith-caps-layer".into())
        .spawn(|| {
            // SAFETY: the hook belongs to this thread; the message loop keeps
            // it alive for the life of the process.
            unsafe {
                let module = GetModuleHandleW(None).unwrap_or_default();
                if let Err(e) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), Some(module.into()), 0) {
                    tracing::warn!("caps layer: hook: {e}");
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
        tracing::warn!("caps layer thread: {e}");
    }
}

/// Sets the bound keys. With none bound CapsLock behaves normally.
pub fn set_keys(keys: impl IntoIterator<Item = u32>) {
    let bits = keys
        .into_iter()
        .filter(|vk| (FIRST..=LAST).contains(vk))
        .fold(0u64, |acc, vk| acc | 1 << (vk - FIRST));
    KEYS.store(bits, Ordering::Relaxed);
}

fn bound(vk: u32) -> bool {
    (FIRST..=LAST).contains(&vk) && KEYS.load(Ordering::Relaxed) & (1 << (vk - FIRST)) != 0
}

/// Sends one CapsLock tap so a lone press still toggles caps lock.
fn tap_caps() {
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(VK_CAPITAL.0),
                wScan: 0x3A,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARK,
            },
        },
    };
    let inputs = [key(Default::default()), key(KEYEVENTF_KEYUP)];
    // SAFETY: two well-formed keyboard inputs.
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && KEYS.load(Ordering::Relaxed) != 0 {
        // SAFETY: for WH_KEYBOARD_LL, lparam points at a KBDLLHOOKSTRUCT.
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if kb.dwExtraInfo != MARK {
            let vk = kb.vkCode;
            let down = wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN;
            if vk == u32::from(VK_CAPITAL.0) {
                if down {
                    // Auto-repeat keeps sending downs; only the first counts.
                    if !HELD.swap(true, Ordering::Relaxed) {
                        USED.store(false, Ordering::Relaxed);
                    }
                } else {
                    HELD.store(false, Ordering::Relaxed);
                    if !USED.load(Ordering::Relaxed) {
                        tap_caps();
                    }
                }
                return LRESULT(1);
            }
            if HELD.load(Ordering::Relaxed) {
                if down {
                    USED.store(true, Ordering::Relaxed);
                }
                if bound(vk) {
                    if down {
                        if DOWN.swap(vk, Ordering::Relaxed) != vk
                            && let Some(cb) = CALLBACK.get()
                        {
                            cb(vk);
                        }
                    } else {
                        DOWN.store(0, Ordering::Relaxed);
                    }
                    return LRESULT(1);
                }
            } else if !down && DOWN.load(Ordering::Relaxed) == vk {
                // CapsLock let go before the key: swallow its release too.
                DOWN.store(0, Ordering::Relaxed);
                return LRESULT(1);
            }
        }
    }
    // SAFETY: passes everything else on unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_caps_accelerators() {
        assert_eq!(parse("caps+KeyQ"), Some('Q' as u32));
        assert_eq!(parse("Caps+n"), Some('N' as u32));
        assert_eq!(parse("caps+Digit1"), Some('1' as u32));
        assert_eq!(parse("ctrl+alt+KeyQ"), None);
        assert_eq!(parse("caps+F1"), None);
        assert_eq!(parse("caps+"), None);
    }
}
