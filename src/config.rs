use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::OnceLock;
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::WindowsProgramming::{
    GetPrivateProfileIntW, WritePrivateProfileStringW,
};

static CONFIG: OnceLock<Config> = OnceLock::new();

const DEFAULT_LOCK_MODS: i32 = 7;   // MOD_ALT|MOD_CONTROL|MOD_SHIFT = 1|2|4
const DEFAULT_LOCK_VK: i32 = 76;    // 'L'
const DEFAULT_UNLOCK_MODS: i32 = 7;
const DEFAULT_UNLOCK_VK: i32 = 85;  // 'U'
const DEFAULT_PANIC_VK: i32 = 27;   // VK_ESCAPE
const DEFAULT_LOCK_ON_START: i32 = 0;

// Hotkey/panic-key fields are AtomicU32 so a settings dialog can change them at
// runtime with no hook reinstall — keyboard_proc already re-reads Config::get()
// fresh on every keydown, so a plain store here is immediately visible there.
// lock_on_start is only ever read once at startup (main.rs) — nothing in the
// hook path touches it — but it's still an AtomicBool rather than a plain
// bool so the settings dialog can update it (the change just has no effect
// until the next restart, unlike the other 5 fields).
pub struct Config {
    pub lock_mods: AtomicU32,
    pub lock_vk: AtomicU32,
    pub unlock_mods: AtomicU32,
    pub unlock_vk: AtomicU32,
    pub panic_vk: AtomicU32,
    pub lock_on_start: AtomicBool,
}

impl Config {
    pub fn load() -> Self {
        let ini = exe_relative("wraith.ini");
        let sec = crate::to_wide("Wraith");

        macro_rules! get_int {
            ($key:expr, $default:expr) => {{
                let k = crate::to_wide($key);
                unsafe {
                    GetPrivateProfileIntW(sec.as_ptr(), k.as_ptr(), $default, ini.as_ptr()) as u32
                }
            }};
        }

        Config {
            lock_mods:     AtomicU32::new(get_int!("LockModifiers",  DEFAULT_LOCK_MODS)),
            lock_vk:       AtomicU32::new(get_int!("LockKey",         DEFAULT_LOCK_VK)),
            unlock_mods:   AtomicU32::new(get_int!("UnlockModifiers", DEFAULT_UNLOCK_MODS)),
            unlock_vk:     AtomicU32::new(get_int!("UnlockKey",       DEFAULT_UNLOCK_VK)),
            panic_vk:      AtomicU32::new(get_int!("PanicKey",        DEFAULT_PANIC_VK)),
            lock_on_start: AtomicBool::new(get_int!("LockOnStart", DEFAULT_LOCK_ON_START) != 0),
        }
    }

    pub fn get() -> &'static Self {
        CONFIG.get_or_init(Self::load)
    }

    /// Write all 6 keys back to wraith.ini from current in-memory values.
    /// WritePrivateProfileStringW only touches the given key inside the given
    /// section — everything else in the file (comments, other keys/sections)
    /// is left untouched.
    pub fn write_back(&self) {
        let ini = exe_relative("wraith.ini");
        let sec = crate::to_wide("Wraith");

        macro_rules! set_int {
            ($key:expr, $val:expr) => {{
                let k = crate::to_wide($key);
                let v = crate::to_wide(&$val.to_string());
                unsafe {
                    WritePrivateProfileStringW(sec.as_ptr(), k.as_ptr(), v.as_ptr(), ini.as_ptr());
                }
            }};
        }

        set_int!("LockModifiers",  self.lock_mods.load(Relaxed));
        set_int!("LockKey",        self.lock_vk.load(Relaxed));
        set_int!("UnlockModifiers", self.unlock_mods.load(Relaxed));
        set_int!("UnlockKey",      self.unlock_vk.load(Relaxed));
        set_int!("PanicKey",       self.panic_vk.load(Relaxed));
        set_int!("LockOnStart",    self.lock_on_start.load(Relaxed) as u32);
    }
}

// pub(crate): tests (here and in settings.rs) need the real ini path to
// verify write-back landed on disk, not just in memory.
pub(crate) fn exe_relative(filename: &str) -> Vec<u16> {
    let mut buf = [0u16; 520];
    let len = unsafe { GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    let dir_end = buf[..len]
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut path = buf[..dir_end].to_vec();
    path.extend(crate::to_wide(filename));
    path
}

// Every test below touches the process-wide Config::get() singleton and/or
// the real wraith.ini next to the test binary. cargo test runs #[test] fns
// in parallel threads by default, so without serialization these would race
// each other. This lock is test-only — it has nothing to do with the
// lock-free atomics the hook path reads (ADR 0003 still holds for those).
#[cfg(test)]
pub(crate) static CONFIG_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        CONFIG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    // Behavior 1: the AtomicU32 conversion didn't change Config's shape in any
    // way that breaks construction or the OnceLock accessor.
    #[test]
    fn config_get_returns_same_singleton() {
        let _g = lock();
        let a = Config::get() as *const Config;
        let b = Config::get() as *const Config;
        assert_eq!(a, b);
    }

    // Behavior 2: storing a new value into panic_vk is immediately visible to
    // any other call site that fetches Config::get() fresh — this is exactly
    // what makes "no hook reinstall" true, since keyboard_proc re-reads
    // Config::get() on every keydown rather than caching a snapshot.
    #[test]
    fn panic_vk_store_is_immediately_visible() {
        let _g = lock();
        let original = Config::get().panic_vk.load(Relaxed);
        Config::get().panic_vk.store(123, Relaxed);
        assert_eq!(Config::get().panic_vk.load(Relaxed), 123);
        Config::get().panic_vk.store(original, Relaxed); // restore
    }

    // Behavior 3: write_back() only touches the 6 keys it owns — a sentinel
    // key/section written directly into the same real ini file must survive.
    #[test]
    fn write_back_preserves_unrelated_ini_content() {
        let _g = lock();
        let ini_wide = exe_relative("wraith.ini");
        let ini_path = wide_to_path(&ini_wide);

        std::fs::write(&ini_path, "[Wraith]\r\nSentinelKey=42\r\n[Other]\r\nFoo=bar\r\n")
            .expect("write seed ini");

        let cfg = Config {
            lock_mods: AtomicU32::new(7),
            lock_vk: AtomicU32::new(76),
            unlock_mods: AtomicU32::new(7),
            unlock_vk: AtomicU32::new(85),
            panic_vk: AtomicU32::new(200),
            lock_on_start: AtomicBool::new(false),
        };
        cfg.write_back();

        let wraith = crate::to_wide("Wraith");
        let other = crate::to_wide("Other");
        let sentinel_key = crate::to_wide("SentinelKey");
        let foo_key = crate::to_wide("Foo");
        let panic_key = crate::to_wide("PanicKey");

        unsafe {
            assert_eq!(
                GetPrivateProfileIntW(wraith.as_ptr(), sentinel_key.as_ptr(), -1, ini_wide.as_ptr()),
                42,
                "write_back must not clobber an unrelated key in its own section"
            );
            assert_eq!(
                GetPrivateProfileIntW(other.as_ptr(), foo_key.as_ptr(), -1, ini_wide.as_ptr()),
                0, // Foo=bar is not an int; GetPrivateProfileIntW returns the default on parse failure
                "write_back must not touch [Other] at all"
            );
            assert_eq!(
                GetPrivateProfileIntW(wraith.as_ptr(), panic_key.as_ptr(), -1, ini_wide.as_ptr()),
                200,
                "write_back must persist the value it was given"
            );
        }

        let _ = std::fs::remove_file(&ini_path);
    }

    // Behavior 4: wraith.ini is not a hard dependency. Config::load() must
    // fall back to defaults when the file is entirely absent (not just when
    // a key is missing), and write_back() must be able to create the file
    // from scratch -- e.g. the settings dialog is the very first thing that
    // ever touches config on a fresh install with no shipped ini.
    #[test]
    fn missing_ini_falls_back_to_defaults_and_write_back_creates_it() {
        let _g = lock();
        let ini_wide = exe_relative("wraith.ini");
        let ini_path = wide_to_path(&ini_wide);
        let _ = std::fs::remove_file(&ini_path);
        assert!(!std::path::Path::new(&ini_path).exists(), "test setup: ini must be absent");

        let fresh = Config::load();
        assert_eq!(fresh.lock_mods.load(Relaxed), DEFAULT_LOCK_MODS as u32);
        assert_eq!(fresh.lock_vk.load(Relaxed), DEFAULT_LOCK_VK as u32);
        assert_eq!(fresh.panic_vk.load(Relaxed), DEFAULT_PANIC_VK as u32);
        assert!(!fresh.lock_on_start.load(Relaxed));

        fresh.panic_vk.store(88, Relaxed);
        fresh.write_back();

        assert!(std::path::Path::new(&ini_path).exists(), "write_back must create a missing ini file");
        let panic_key = crate::to_wide("PanicKey");
        let wraith = crate::to_wide("Wraith");
        unsafe {
            assert_eq!(
                GetPrivateProfileIntW(wraith.as_ptr(), panic_key.as_ptr(), -1, ini_wide.as_ptr()),
                88,
                "value must be readable back from the freshly created file"
            );
        }

        let _ = std::fs::remove_file(&ini_path);
    }

    fn wide_to_path(wide: &[u16]) -> String {
        let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
        String::from_utf16(&wide[..end]).expect("valid utf-16 path")
    }
}
