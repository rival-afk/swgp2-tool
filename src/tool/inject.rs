//! Win32 process tools: find the game, inject a DLL, read/write/scan memory,
//! list loaded modules. All of this is safe-by-construction for the caller:
//! injection never happens unless the user asked for it in the UI/MCP.
//!
//! Used by the Inject section of the GUI, the `swgp2-mcp` server and the
//! in-game mod loader flow.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

type Handle = *mut c_void;

const PROCESS_CREATE_THREAD: u32 = 0x0002;
const PROCESS_VM_OPERATION: u32 = 0x0008;
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_VM_WRITE: u32 = 0x0020;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const MEM_RELEASE: u32 = 0x8000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_READONLY: u32 = 0x02;
const PAGE_EXECUTE_READ: u32 = 0x20;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const PAGE_GUARD: u32 = 0x100;
const PAGE_NOACCESS: u32 = 0x01;

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn VirtualAllocEx(h: Handle, addr: *const c_void, size: usize, typ: u32, protect: u32) -> *mut c_void;
    fn VirtualFreeEx(h: Handle, addr: *mut c_void, size: usize, typ: u32) -> i32;
    fn VirtualProtectEx(h: Handle, addr: *const c_void, size: usize, protect: u32, old: *mut u32) -> i32;
    fn VirtualQueryEx(
        h: Handle,
        addr: *const c_void,
        mbi: *mut MemoryBasicInformation,
        length: usize,
    ) -> usize;
    fn ReadProcessMemory(h: Handle, addr: *const c_void, buf: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn WriteProcessMemory(h: Handle, addr: *const c_void, buf: *const c_void, size: usize, written: *mut usize) -> i32;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn CreateRemoteThread(
        h: Handle,
        sa: *const c_void,
        stack_size: usize,
        start: *const c_void,
        param: *mut c_void,
        flags: u32,
        thread_id: *mut u32,
    ) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn Module32FirstW(snapshot: Handle, entry: *mut ModuleEntry32W) -> i32;
    fn Module32NextW(snapshot: Handle, entry: *mut ModuleEntry32W) -> i32;
    fn WaitForSingleObject(handle: Handle, ms: u32) -> u32;
    fn GetExitCodeThread(handle: Handle, code: *mut u32) -> i32;
}

const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const TH32CS_SNAPMODULE: u32 = 0x0000_0008;
const TH32CS_SNAPMODULE32: u32 = 0x0000_0010;

#[repr(C)]
#[allow(non_snake_case)]
struct ProcessEntry32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}

#[repr(C)]
#[allow(non_snake_case)]
struct ModuleEntry32W {
    dwSize: u32,
    th32ModuleID: u32,
    th32ProcessID: u32,
    GlblcntUsage: u32,
    ProccntUsage: u32,
    modBaseAddr: usize,
    modBaseSize: u32,
    hModule: usize,
    szModule: [u16; 256],
    szExePath: [u16; 260],
}

#[repr(C)]
#[allow(non_snake_case)]
struct MemoryBasicInformation {
    BaseAddress: usize,
    AllocationBase: usize,
    AllocationProtect: u32,
    RegionSize: usize,
    State: u32,
    Protect: u32,
    Type: u32,
}

const MEM_COMMIT_STATE: u32 = 0x1000;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Find processes whose exe name contains `needle` (case-insensitive).
pub fn find_processes(needle: &str) -> Result<Vec<u32>, String> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap.is_null() || snap == std::ptr::null_mut() || snap as isize == -1 {
        return Err(format!("CreateToolhelp32Snapshot failed ({})", unsafe { GetLastError() }));
    }
    let mut out = Vec::new();
    let mut entry = ProcessEntry32W {
        dwSize: std::mem::size_of::<ProcessEntry32W>() as u32,
        cntUsage: 0,
        th32ProcessID: 0,
        th32DefaultHeapID: 0,
        th32ModuleID: 0,
        cntThreads: 0,
        th32ParentProcessID: 0,
        pcPriClassBase: 0,
        dwFlags: 0,
        szExeFile: [0; 260],
    };
    let mut ok = unsafe { Process32FirstW(snap, &mut entry) };
    let needle = needle.to_lowercase();
    while ok != 0 {
        let name: String = entry.szExeFile.iter().take_while(|&&c| c != 0).map(|&c| c as u8 as char).collect();
        if name.to_lowercase().contains(&needle) {
            out.push(entry.th32ProcessID);
        }
        ok = unsafe { Process32NextW(snap, &mut entry) };
    }
    unsafe { CloseHandle(snap) };
    Ok(out)
}

/// The first PID of the game (e.g. "Super Woden GP 2.exe").
pub fn find_game_pid() -> Option<u32> {
    find_processes("Super Woden GP 2").ok().and_then(|v| v.into_iter().next())
}

fn open_process(pid: u32, extra: u32) -> Result<Handle, String> {
    let h = unsafe { OpenProcess(PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION | PROCESS_CREATE_THREAD | extra, 0, pid) };
    if h.is_null() {
        return Err(format!("OpenProcess({pid}) failed ({})", unsafe { GetLastError() }));
    }
    Ok(h)
}

/// Inject a native DLL into the process (classic LoadLibraryW via CreateRemoteThread).
/// Returns Ok on success, Err with a reason otherwise. The game must be started
/// for this to work.
pub fn inject_dll(pid: u32, dll_path: &Path) -> Result<(), String> {
    if !dll_path.is_file() {
        return Err(format!("DLL not found: {}", dll_path.display()));
    }
    let h = open_process(pid, 0)?;
    let path_w = wide_path(dll_path);
    let path_bytes = path_w.len() * 2;
    let mem = unsafe {
        VirtualAllocEx(h, std::ptr::null(), path_bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE)
    };
    if mem.is_null() {
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(h) };
        return Err(format!("VirtualAllocEx failed ({err})"));
    }
    let mut written = 0usize;
    let ok = unsafe {
        WriteProcessMemory(h, mem, path_w.as_ptr() as *const c_void, path_bytes, &mut written)
    };
    if ok == 0 || written != path_bytes {
        unsafe {
            VirtualFreeEx(h, mem, 0, MEM_RELEASE);
            CloseHandle(h);
        }
        return Err("WriteProcessMemory failed".to_string());
    }
    let kernel = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
    if kernel.is_null() {
        unsafe {
            VirtualFreeEx(h, mem, 0, MEM_RELEASE);
            CloseHandle(h);
        }
        return Err("GetModuleHandleW(kernel32) failed".to_string());
    }
    let load_library = unsafe { GetProcAddress(kernel, b"LoadLibraryW\0".as_ptr()) };
    if load_library.is_null() {
        unsafe {
            VirtualFreeEx(h, mem, 0, MEM_RELEASE);
            CloseHandle(h);
        }
        return Err("GetProcAddress(LoadLibraryW) failed".to_string());
    }
    let thread = unsafe {
        CreateRemoteThread(
            h,
            std::ptr::null(),
            0,
            load_library as *const c_void,
            mem,
            0,
            std::ptr::null_mut(),
        )
    };
    if thread.is_null() {
        let err = unsafe { GetLastError() };
        unsafe {
            VirtualFreeEx(h, mem, 0, MEM_RELEASE);
            CloseHandle(h);
        }
        return Err(format!("CreateRemoteThread failed ({err})"));
    }
    unsafe {
        WaitForSingleObject(thread, 10_000);
        CloseHandle(thread);
        CloseHandle(h);
        // note: we deliberately do not VirtualFreeEx(mem) - the remote thread may still be reading it
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// memory read / write / search
// ---------------------------------------------------------------------------

/// One committed, readable memory region in the target process.
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub base: u64,
    pub size: u64,
    pub protection: u32,
    pub state: u32,
}

/// Committed readable regions (page committed, not noaccess/guard, size > 0).
pub fn memory_regions(pid: u32) -> Result<Vec<MemoryRegion>, String> {
    let h = open_process(pid, 0)?;
    let mut out = Vec::new();
    let mut addr: usize = 0;
    loop {
        let mut mbi = MemoryBasicInformation {
            BaseAddress: 0,
            AllocationBase: 0,
            AllocationProtect: 0,
            RegionSize: 0,
            State: 0,
            Protect: 0,
            Type: 0,
        };
        let ret = unsafe {
            VirtualQueryEx(
                h,
                addr as *const c_void,
                &mut mbi,
                std::mem::size_of::<MemoryBasicInformation>(),
            )
        };
        if ret == 0 {
            break;
        }
        let is_committed = mbi.State == MEM_COMMIT_STATE;
        let is_readable = (mbi.Protect & PAGE_NOACCESS) == 0 && (mbi.Protect & PAGE_GUARD) == 0;
        if is_committed && is_readable && mbi.RegionSize > 0 {
            out.push(MemoryRegion {
                base: mbi.BaseAddress as u64,
                size: mbi.RegionSize as u64,
                protection: mbi.Protect,
                state: mbi.State,
            });
        }
        let next = mbi.BaseAddress + mbi.RegionSize;
        if next <= mbi.BaseAddress || next >= 0x7fff_ffff_ffff {
            break;
        }
        addr = next;
    }
    unsafe { CloseHandle(h) };
    Ok(out)
}

/// Read `len` bytes at `addr` from the process; error if any part fails.
pub fn memory_read(pid: u32, addr: u64, len: usize) -> Result<Vec<u8>, String> {
    let h = open_process(pid, 0)?;
    let mut buf = vec![0u8; len];
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(h, addr as *const c_void, buf.as_mut_ptr() as *mut c_void, len, &mut read)
    };
    unsafe { CloseHandle(h) };
    if ok == 0 || read != len {
        return Err(format!("ReadProcessMemory({addr:#x}, {len}) failed - read {read} bytes"));
    }
    Ok(buf)
}

/// Write bytes at `addr`; returns number of bytes written.
pub fn memory_write(pid: u32, addr: u64, bytes: &[u8]) -> Result<usize, String> {
    let h = open_process(pid, 0)?;
    let mut written = 0usize;
    let ok = unsafe {
        WriteProcessMemory(h, addr as *const c_void, bytes.as_ptr() as *const c_void, bytes.len(), &mut written)
    };
    unsafe { CloseHandle(h) };
    if ok == 0 {
        return Err(format!("WriteProcessMemory({addr:#x}) failed ({})", unsafe { GetLastError() }));
    }
    Ok(written)
}

/// Parse a pattern like "F3 0F ?? ?? 8B C1" into (bytes, mask) with mask=true for wildcards skipped.
/// Wildcards are `??` or `*` tokens.
pub fn parse_pattern(pattern: &str) -> Result<(Vec<u8>, Vec<bool>), String> {
    let mut bytes = Vec::new();
    let mut mask = Vec::new();
    for token in pattern.split_whitespace() {
        if token == "??" || token == "*" || token == "?" {
            bytes.push(0);
            mask.push(false); // 0 = wildcard (skip compare)
        } else {
            let b = u8::from_str_radix(token, 16).map_err(|_| format!("bad pattern byte '{token}'"))?;
            bytes.push(b);
            mask.push(true);
        }
    }
    if bytes.is_empty() {
        return Err("empty pattern".into());
    }
    Ok((bytes, mask))
}

/// Scan committed memory for `pattern` ("F3 0F ?? ?? ..."). Returns absolute addresses.
/// `max_results` caps output (0 = unlimited).
pub fn memory_search(pid: u32, pattern: &str, max_results: usize) -> Result<Vec<u64>, String> {
    let (pat, mask) = parse_pattern(pattern)?;
    let regions = memory_regions(pid)?;
    let h = open_process(pid, 0)?;
    let mut out = Vec::new();
    for region in regions {
        let mut buf = vec![0u8; region.size as usize];
        let mut read = 0usize;
        let ok = unsafe {
            ReadProcessMemory(
                h,
                region.base as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                &mut read,
            )
        };
        if ok == 0 || read < pat.len() {
            continue;
        }
        buf.truncate(read);
        for i in 0..buf.len().saturating_sub(pat.len() - 1) {
            let mut match_ = true;
            for j in 0..pat.len() {
                if mask[j] && buf[i + j] != pat[j] {
                    match_ = false;
                    break;
                }
            }
            if match_ {
                out.push(region.base + i as u64);
                if max_results != 0 && out.len() >= max_results {
                    unsafe { CloseHandle(h) };
                    return Ok(out);
                }
            }
        }
    }
    unsafe { CloseHandle(h) };
    Ok(out)
}

// ---------------------------------------------------------------------------
// modules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub base: u64,
    pub size: u64,
    pub path: String,
}

/// Loaded modules of the process (name, base, size, path).
pub fn list_modules(pid: u32) -> Result<Vec<ModuleInfo>, String> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
    if snap.is_null() || snap as isize == -1 {
        return Err(format!("CreateToolhelp32Snapshot failed ({})", unsafe { GetLastError() }));
    }
    let mut out = Vec::new();
    let mut entry = ModuleEntry32W {
        dwSize: std::mem::size_of::<ModuleEntry32W>() as u32,
        th32ModuleID: 0,
        th32ProcessID: 0,
        GlblcntUsage: 0,
        ProccntUsage: 0,
        modBaseAddr: 0,
        modBaseSize: 0,
        hModule: 0,
        szModule: [0; 256],
        szExePath: [0; 260],
    };
    let mut ok = unsafe { Module32FirstW(snap, &mut entry) };
    while ok != 0 {
        let name: String = entry.szModule.iter().take_while(|&&c| c != 0).map(|&c| c as u8 as char).collect();
        let path: String = entry.szExePath.iter().take_while(|&&c| c != 0).map(|&c| c as u8 as char).collect();
        out.push(ModuleInfo {
            name,
            base: entry.modBaseAddr as u64,
            size: entry.modBaseSize as u64,
            path,
        });
        ok = unsafe { Module32NextW(snap, &mut entry) };
    }
    unsafe { CloseHandle(snap) };
    Ok(out)
}

// ---------------------------------------------------------------------------
// thread for injection DLL (used by the loader)
// ---------------------------------------------------------------------------

/// Resolve a DLL we control and inject it into the game if it is running.
/// Looks for `swgp2-mod.dll` next to the current exe, then under
/// `%APPDATA%\swgp2-tool\mod\`. Returns Err if the DLL is missing or the game
/// is not running.
pub fn inject_mod_dll() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().unwrap_or(Path::new("."));
    let candidates = [
        dir.join("swgp2-mod.dll"),
        std::env::var_os("APPDATA").map(|a| Path::new(&a).join("swgp2-tool").join("mod").join("swgp2-mod.dll")).unwrap_or_default(),
    ];
    let dll = candidates.iter().find(|p| p.is_file()).cloned();
    let Some(dll) = dll else {
        return Err("swgp2-mod.dll not found (next to the exe or in %APPDATA%\\swgp2-tool\\mod)".to_string());
    };
    let pid = find_game_pid().ok_or_else(|| "game is not running".to_string())?;
    inject_dll(pid, &dll)?;
    Ok(format!("injected {} into pid {pid}", dll.display()))
}
