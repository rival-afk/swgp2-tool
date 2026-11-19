//! swgp2-mod - native loader for the SWGP2 in-game mod.
//!
//! Injected into `Super Woden GP 2.exe` by the tool. It waits for Mono to be up,
//! then loads the managed `%APPDATA%\swgp2-tool\mod\Swgp2Mod.dll` into the game's
//! Mono domain and calls `Swgp2Mod.Entry.Main()`, which installs the in-game
//! menu (a Unity IMGUI window).
//!
//! Zero dependencies on purpose: everything below is hand-written FFI against
//! kernel32 and the game's mono-2.0-bdwgc.dll.

use std::ffi::c_void;
use std::io::Write;
use std::path::PathBuf;

const DLL_PROCESS_ATTACH: u32 = 1;

type Hmodule = *mut c_void;
type Hthread = *mut c_void;
type Handle = *mut c_void;

#[link(name = "kernel32")]
extern "system" {
    fn CreateThread(
        lp_thread_attributes: *const c_void,
        dw_stack_size: usize,
        lp_start_address: unsafe extern "system" fn(*mut c_void) -> u32,
        lp_parameter: *mut c_void,
        dw_creation_flags: u32,
        lp_thread_id: *mut u32,
    ) -> Hthread;
    fn GetModuleHandleW(lp_module_name: *const u16) -> Hmodule;
    fn GetProcAddress(h_module: Hmodule, lp_proc_name: *const u8) -> *const c_void;
    fn GetEnvironmentVariableW(lp_name: *const u16, lp_buffer: *mut u16, n_size: u32) -> u32;
    fn Sleep(dw_milliseconds: u32);
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentThreadId() -> u32;
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn mod_dir() -> PathBuf {
    let mut buf = vec![0u16; 260];
    let len = unsafe { GetEnvironmentVariableW(wide("APPDATA").as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    let appdata = String::from_utf16_lossy(&buf[..len as usize]);
    PathBuf::from(appdata).join("swgp2-tool").join("mod")
}

fn log(msg: &str) {
    let line = format!("{} swgp2-mod: {msg}\n", chrono_stamp());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(mod_dir().join("mod.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

fn chrono_stamp() -> String {
    // seconds precision, no chrono crate: use SystemTime + manual civil-from-days
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y + if m <= 2 { 1 } else { 0 },
        m,
        d,
        h,
        mi,
        s
    )
}

// ---------------------------------------------------------------------------
// mono interop
// ---------------------------------------------------------------------------

type MonoDomain = *mut c_void;
type MonoAssembly = *mut c_void;
type MonoImage = *mut c_void;
type MonoClass = *mut c_void;
type MonoMethod = *mut c_void;
type MonoObject = *mut c_void;
type MonoString = *mut c_void;

struct Mono {
    get_root_domain: unsafe extern "system" fn() -> MonoDomain,
    thread_attach: unsafe extern "system" fn(MonoDomain) -> *mut c_void,
    domain_assembly_open: unsafe extern "system" fn(MonoDomain, *const u8) -> MonoAssembly,
    assembly_get_image: unsafe extern "system" fn(MonoAssembly) -> MonoImage,
    image_get_name: unsafe extern "system" fn(MonoImage) -> *const u8,
    class_from_name: unsafe extern "system" fn(MonoImage, *const u8, *const u8) -> MonoClass,
    class_get_method_from_name: unsafe extern "system" fn(MonoClass, *const u8, i32) -> MonoMethod,
    runtime_invoke: unsafe extern "system" fn(MonoMethod, *mut c_void, *mut *mut c_void, *mut *mut MonoObject) -> MonoObject,
}

unsafe fn proc_addr(mono: Hmodule, name: &[u8]) -> *const c_void {
    GetProcAddress(mono, name.as_ptr())
}

unsafe fn bind_mono(mono: Hmodule) -> Option<Mono> {
    macro_rules! sym {
        ($field:ident, $name:expr) => {{
            let raw = proc_addr(mono, concat!($name, "\0").as_bytes());
            if raw.is_null() {
                log(&format!("mono export missing: {name}", name = $name));
                return None;
            }
            std::mem::transmute(raw)
        }};
    }
    Some(Mono {
        get_root_domain: sym!(get_root_domain, "mono_get_root_domain"),
        thread_attach: sym!(thread_attach, "mono_thread_attach"),
        domain_assembly_open: sym!(domain_assembly_open, "mono_domain_assembly_open"),
        assembly_get_image: sym!(assembly_get_image, "mono_assembly_get_image"),
        image_get_name: sym!(image_get_name, "mono_image_get_name"),
        class_from_name: sym!(class_from_name, "mono_class_from_name"),
        class_get_method_from_name: sym!(class_get_method_from_name, "mono_class_get_method_from_name"),
        runtime_invoke: sym!(runtime_invoke, "mono_runtime_invoke"),
    })
}

unsafe extern "system" fn mod_thread(_: *mut c_void) -> u32 {
    log("thread start, waiting for mono");
    let mono_name = wide("mono-2.0-bdwgc.dll");
    let mut mono = GetModuleHandleW(mono_name.as_ptr());
    let mut tries = 0;
    while mono.is_null() && tries < 240 {
        Sleep(250);
        mono = GetModuleHandleW(mono_name.as_ptr());
        tries += 1;
    }
    if mono.is_null() {
        log("mono-2.0-bdwgc.dll not found - aborting");
        return 1;
    }
    log("mono found");

    let mono = match bind_mono(mono) {
        Some(m) => m,
        None => return 1,
    };

    let domain = (mono.get_root_domain)();
    log(&format!("root domain: {domain:p}"));
    (mono.thread_attach)(domain);

    let mod_path = mod_dir().join("Swgp2Mod.dll");
    if !mod_path.is_file() {
        log(&format!("managed mod not found: {}", mod_path.display()));
        return 2;
    }
    let path_c = match std::ffi::CString::new(mod_path.to_str().unwrap_or("")) {
        Ok(p) => p,
        Err(_) => {
            log("mod path is not valid C string");
            return 3;
        }
    };
    log(&format!("loading {}", mod_path.display()));
    let asm = (mono.domain_assembly_open)(domain, path_c.as_ptr() as *const u8);
    if asm.is_null() {
        log("mono_domain_assembly_open returned null");
        return 4;
    }
    let image = (mono.assembly_get_image)(asm);
    let name = (mono.image_get_name)(image);
    let name = if name.is_null() {
        "<null>".to_string()
    } else {
        std::ffi::CStr::from_ptr(name as *const i8).to_string_lossy().into_owned()
    };
    log(&format!("image loaded: {name}"));

    let ns = b"Swgp2Mod\0";
    let cls = b"Entry\0";
    let klass = (mono.class_from_name)(image, ns.as_ptr(), cls.as_ptr());
    if klass.is_null() {
        log("class Swgp2Mod.Entry not found");
        return 5;
    }
    let meth = (mono.class_get_method_from_name)(klass, b"Main\0".as_ptr(), 0);
    if meth.is_null() {
        log("method Swgp2Mod.Entry.Main not found");
        return 6;
    }
    let mut exc: *mut MonoObject = std::ptr::null_mut();
    let result = (mono.runtime_invoke)(meth, std::ptr::null_mut(), std::ptr::null_mut(), &mut exc);
    if exc.is_null() {
        log(&format!("Main() invoked, returned {result:p}"));
        0
    } else {
        log("Main() threw an exception");
        7
    }
}

// ---------------------------------------------------------------------------
// DllMain
// ---------------------------------------------------------------------------

#[no_mangle]
unsafe extern "system" fn DllMain(
    _hinst: Hmodule,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        let t = CreateThread(std::ptr::null(), 0, mod_thread, std::ptr::null_mut(), 0, std::ptr::null_mut());
        if t.is_null() {
            log("CreateThread failed");
        }
    }
    1
}
