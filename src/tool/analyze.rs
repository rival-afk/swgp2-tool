//! File analyzer — the "inspect anything" half of the tool.
//!
//! Self-contained, no external crates beyond the stdlib: magic-byte sniffing,
//! entropy, ASCII/UTF-16 strings, wildcard scan, and a hand-written PE parser
//! (machine, sections, exports, imports, subsystem). Unity/GLB support lives
//! in `unity.rs` / `glb.rs` when those land; NRBF probing uses the existing
//! parser in `nrbf.rs`.

use std::fmt::Write as _;

// ===========================================================================
// magic sniffing / entropy / strings
// ===========================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileKind {
    Pe,
    Nrbf,
    Png,
    Tga,
    Jpeg,
    GZip,
    Zip,
    SevenZip,
    OleCompound,
    Ogg,
    Wave,
    Flac,
    Mp3,
    Gif,
    TextJson,
    BinaryFormatter,
    UnitySerializedFile,
    UnityAssetBundle,
    Unknown,
}

impl FileKind {
    pub fn label(&self) -> &'static str {
        match self {
            FileKind::Pe => "PE (Windows executable/DLL)",
            FileKind::Nrbf => "NRBF (.NET BinaryFormatter)",
            FileKind::Png => "PNG image",
            FileKind::Tga => "TGA image",
            FileKind::Jpeg => "JPEG image",
            FileKind::GZip => "gzip",
            FileKind::Zip => "ZIP",
            FileKind::SevenZip => "7-Zip",
            FileKind::OleCompound => "OLE compound document",
            FileKind::Ogg => "Ogg audio",
            FileKind::Wave => "WAV audio",
            FileKind::Flac => "FLAC audio",
            FileKind::Mp3 => "MP3 audio",
            FileKind::Gif => "GIF image",
            FileKind::TextJson => "JSON (text)",
            FileKind::BinaryFormatter => "NRBF (.NET BinaryFormatter)",
            FileKind::UnitySerializedFile => "Unity SerializedFile",
            FileKind::UnityAssetBundle => "Unity AssetBundle",
            FileKind::Unknown => "unknown",
        }
    }
}

pub fn detect_kind(head: &[u8]) -> FileKind {
    if head.starts_with(b"MZ") {
        return FileKind::Pe;
    }
    if head.len() >= 16 && head[..4] == [0x00, 0x01, 0x00, 0x00] {
        // MS-NRBF: SerializedStreamHeader = 00 01 00 00 00
        return FileKind::Nrbf;
    }
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return FileKind::Png;
    }
    if head.len() >= 2 && head[0] == 0 && head[1] == 0 && head[2] == 2 {
        return FileKind::Tga;
    }
    if head.starts_with(b"\xff\xd8\xff") {
        return FileKind::Jpeg;
    }
    if head.starts_with(b"\x1f\x8b") {
        return FileKind::GZip;
    }
    if head.starts_with(b"PK\x03\x04") {
        return FileKind::Zip;
    }
    if head.starts_with(b"7z\xbc\xaf\x27\x1c") {
        return FileKind::SevenZip;
    }
    if head.starts_with(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1") {
        return FileKind::OleCompound;
    }
    if head.starts_with(b"OggS") {
        return FileKind::Ogg;
    }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE") {
        return FileKind::Wave;
    }
    if head.starts_with(b"fLaC") {
        return FileKind::Flac;
    }
    if head.starts_with(b"ID3") {
        return FileKind::Mp3;
    }
    if head.starts_with(b"GIF8") {
        return FileKind::Gif;
    }
    // Unity serialized files start with: metadataSize(int64) + fileVersion(int32 0x16=22)
    if head.len() >= 24 {
        let meta = u64::from_be_bytes(head[..8].try_into().unwrap());
        let version = u32::from_be_bytes(head[8..12].try_into().unwrap());
        let data_off = u32::from_be_bytes(head[20..24].try_into().unwrap_or([0; 4]));
        if meta == 0 && (version == 0x16 || version == 0x0c || version == 0x11) && data_off > 0 {
            return FileKind::UnitySerializedFile;
        }
    }
    if head.starts_with(b"UnityFS") || head.starts_with(b"UnityWeb") || head.starts_with(b"UnityRaw") {
        return FileKind::UnityAssetBundle;
    }
    if head.starts_with(b"{\"") || head.starts_with(b"[{") || head.starts_with(b"\xef\xbb\xbf{") {
        return FileKind::TextJson;
    }
    FileKind::Unknown
}

pub fn entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut freq = [0u64; 256];
    for &b in bytes {
        freq[b as usize] += 1;
    }
    let len = bytes.len() as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// ASCII + UTF-16LE strings with their offsets.
pub fn strings(bytes: &[u8], min_len: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // ASCII run
        if bytes[i] >= 0x20 && bytes[i] < 0x7f {
            let mut j = i;
            while j < bytes.len() && bytes[j] >= 0x20 && bytes[j] < 0x7f {
                j += 1;
            }
            if j - i >= min_len {
                out.push((i, String::from_utf8_lossy(&bytes[i..j]).into_owned()));
            }
            i = j;
            continue;
        }
        // UTF-16LE run (byte pairs of printable ASCII + NUL)
        if i + 1 < bytes.len() && bytes[i] >= 0x20 && bytes[i] < 0x7f && bytes[i + 1] == 0 {
            let start = i;
            let mut j = i;
            while j + 1 < bytes.len() && bytes[j] >= 0x20 && bytes[j] < 0x7f && bytes[j + 1] == 0 {
                j += 2;
            }
            let n = (j - start) / 2;
            if n >= min_len {
                let s: String = bytes[start..j].chunks(2).filter(|c| c.len() == 2).map(|c| c[0] as char).collect();
                out.push((start, s));
            }
            i = j.max(start + 2);
            continue;
        }
        i += 1;
    }
    out
}

/// Wildcard pattern search (`??` = any byte).
pub fn wildcard_search(bytes: &[u8], pattern: &[u8], mask: &[bool], max_results: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if pattern.is_empty() || pattern.len() > bytes.len() {
        return out;
    }
    for i in 0..=bytes.len() - pattern.len() {
        let mut ok = true;
        for j in 0..pattern.len() {
            if mask[j] && bytes[i + j] != pattern[j] {
                ok = false;
                break;
            }
        }
        if ok {
            out.push(i);
            if out.len() >= max_results {
                return out;
            }
        }
    }
    out
}

/// Parse a pattern string like "4D 5A ?? ?? 90" into (bytes, mask) with mask=true for concrete bytes.
pub fn parse_pattern(pattern: &str) -> Result<(Vec<u8>, Vec<bool>), String> {
    let mut bytes = Vec::new();
    let mut mask = Vec::new();
    for token in pattern.split_whitespace() {
        if token == "??" || token == "*" || token == "?" {
            bytes.push(0);
            mask.push(false);
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

// ===========================================================================
// PE (Windows executable/DLL) parser
// ===========================================================================

#[derive(Debug, Clone)]
pub struct PeSection {
    pub name: String,
    pub va: u64,
    pub vsize: u64,
    pub raw_size: u64,
    pub raw_offset: u64,
    pub characteristics: u32,
}

#[derive(Debug, Clone)]
pub struct PeInfo {
    pub machine: u16,
    pub characteristics: u16,
    pub is_dll: bool,
    pub is_executable: bool,
    pub timestamp: u32,
    pub num_sections: u16,
    pub entry_point: u64,
    pub image_base: u64,
    pub subsystem: u16,
    pub sections: Vec<PeSection>,
    pub exports: Vec<String>,
    pub imports: Vec<(String, usize)>,
    pub ep_in_data_dir: bool,
    pub data_directories: Vec<(u32, u64, u64)>,
}

fn read_u16(d: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(d[off..off + 2].try_into().unwrap_or([0, 0]))
}
fn read_u32(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(d[off..off + 4].try_into().unwrap_or([0, 0, 0, 0]))
}
fn read_u64(d: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(d[off..off + 8].try_into().unwrap_or([0; 8]))
}

pub fn parse_pe(d: &[u8]) -> Result<PeInfo, String> {
    if d.len() < 64 || &d[..2] != b"MZ" {
        return Err("not a PE (missing MZ)".into());
    }
    let pe_off = read_u32(d, 0x3c) as usize;
    if pe_off == 0 || pe_off + 24 > d.len() || &d[pe_off..pe_off + 4] != b"PE\0\0" {
        return Err("not a PE (missing PE\\0\\0)".into());
    }
    let file_header = pe_off + 4;
    let machine = read_u16(d, file_header);
    let num_sections = read_u16(d, file_header + 2);
    let timestamp = read_u32(d, file_header + 4);
    let opt_size = read_u16(d, file_header + 16);
    let characteristics = read_u16(d, file_header + 18);
    let opt = file_header + 20;
    if opt + opt_size as usize > d.len() {
        return Err("PE optional header truncated".into());
    }
    let magic = read_u16(d, opt);
    let (is_pe32_plus, entry_point, image_base) = match magic {
        0x10b => (false, read_u32(d, opt + 16) as u64, read_u32(d, opt + 28) as u64),
        0x20b => (true, read_u32(d, opt + 16) as u64, read_u64(d, opt + 24)),
        _ => return Err(format!("unknown PE magic {magic:#x}")),
    };
    let subsystem = read_u16(d, if is_pe32_plus { opt + 68 } else { opt + 68 });
    let (num_rva, dd_off) = if is_pe32_plus { (16, opt + 112) } else { (16, opt + 96) };
    let mut data_directories = Vec::with_capacity(num_rva);
    for i in 0..num_rva {
        let o = dd_off + i * 8;
        if o + 8 > d.len() {
            break;
        }
        let va = read_u32(d, o) as u64;
        let size = read_u32(d, o + 4) as u64;
        data_directories.push((i as u32, va, size));
    }

    // sections
    let section_table = opt + opt_size as usize;
    let mut sections = Vec::with_capacity(num_sections as usize);
    for i in 0..num_sections as usize {
        let s = section_table + i * 40;
        if s + 40 > d.len() {
            break;
        }
        let mut name = [0u8; 8];
        name.copy_from_slice(&d[s..s + 8]);
        let name = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or(&[])).into_owned();
        sections.push(PeSection {
            name,
            va: read_u32(d, s + 12) as u64,
            vsize: read_u32(d, s + 8) as u64,
            raw_size: read_u32(d, s + 16) as u64,
            raw_offset: read_u32(d, s + 20) as u64,
            characteristics: read_u32(d, s + 36),
        });
    }

    // helpers: RVA -> file offset
    let rva_to_off = |rva: u64| -> Option<usize> {
        for s in &sections {
            if rva >= s.va && rva < s.va + s.vsize.max(s.raw_size) {
                return Some((s.raw_offset + (rva - s.va)) as usize);
            }
        }
        None
    };

    // exports
    let mut exports = Vec::new();
    if let Some(&(_, va, _)) = data_directories.iter().find(|(i, _, _)| *i == 0).filter(|(_, va, size)| *va != 0 && *size != 0) {
        if let Some(off) = rva_to_off(va) {
            let names_rva = read_u32(d, off + 32) as usize;
            let base = read_u32(d, off + 16) as usize;
            let num = read_u32(d, off + 24) as usize;
            let name_base = off + 32 + 4 * 3; // skip ExportDir fields
            let entries_base = off + 40;
            if let Some(name_off) = rva_to_off(names_rva as u64) {
                for i in 0..num {
                    let o = name_off + i * 4;
                    if o + 4 > d.len() {
                        break;
                    }
                    let nrva = read_u32(d, o) as u64;
                    if let Some(no) = rva_to_off(nrva) {
                        let mut end = no;
                        while end < d.len() && d[end] != 0 {
                            end += 1;
                        }
                        if let Ok(s) = std::str::from_utf8(&d[no..end]) {
                            exports.push(s.to_string());
                        }
                    }
                    let _ = (base, name_base, entries_base);
                }
            }
        }
    }

    // imports (DLL name + count of functions)
    let mut imports = Vec::new();
    if let Some(&(_, va, _)) = data_directories.iter().find(|(i, _, _)| *i == 1).filter(|(_, va, size)| *va != 0 && *size != 0) {
        if let Some(off) = rva_to_off(va) {
            let mut cur = off;
            loop {
                let name_rva = read_u32(d, cur + 12) as u64;
                if name_rva == 0 {
                    break;
                }
                let thunk = read_u32(d, cur) as u64; // OriginalFirstThunk (lookup table)
                let mut count = 0usize;
                if let Some(thunk_off) = rva_to_off(thunk) {
                    let mut t = thunk_off;
                    loop {
                        let v = read_u32(d, t) as u64;
                        if v == 0 {
                            break;
                        }
                        count += 1;
                        t += 4;
                    }
                }
                if let Some(no) = rva_to_off(name_rva) {
                    let mut end = no;
                    while end < d.len() && d[end] != 0 {
                        end += 1;
                    }
                    if let Ok(s) = std::str::from_utf8(&d[no..end]) {
                        imports.push((s.to_string(), count));
                    }
                }
                cur += 20;
                if cur + 20 > d.len() {
                    break;
                }
            }
        }
    }

    Ok(PeInfo {
        machine,
        characteristics,
        is_dll: characteristics & 0x2000 != 0,
        is_executable: characteristics & 0x0002 != 0,
        timestamp,
        num_sections,
        entry_point: entry_point + image_base,
        image_base,
        subsystem,
        sections,
        exports,
        imports,
        ep_in_data_dir: false,
        data_directories,
    })
}

/// Convenience: whole file -> PeInfo (error for non-PE).
pub fn pe_info(bytes: &[u8]) -> Result<PeInfo, String> {
    parse_pe(bytes)
}

/// Render a compact human/json-friendly summary of a file.
pub fn summary(path: &str, bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(64)];
    let kind = detect_kind(head);
    let mut out = String::new();
    let _ = writeln!(out, "path: {path}");
    let _ = writeln!(out, "size: {} bytes", bytes.len());
    let _ = writeln!(out, "type: {}", kind.label());
    let _ = writeln!(out, "entropy: {:.2} bits/byte", entropy(bytes));
    if let Ok(pe) = parse_pe(bytes) {
        let _ = writeln!(out, "pe: machine={:#06x} dll={} sections={} imports={} exports={}",
            pe.machine, pe.is_dll, pe.sections.len(), pe.imports.len(), pe.exports.len());
        for imp in pe.imports.iter().take(10) {
            let _ = writeln!(out, "  imports {} ({})", imp.0, imp.1);
        }
        for exp in pe.exports.iter().take(20) {
            let _ = writeln!(out, "  exports {exp}");
        }
        for s in pe.sections.iter().take(10) {
            let _ = writeln!(out, "  section {:8} va={:#x} raw={}B", s.name, s.va, s.raw_size);
        }
    }
    out
}
