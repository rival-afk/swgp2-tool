//! Match CarThumbnails PNGs to CAR_SKINS by hue histogram of the body region.
//! Run: cargo run --release --example match -- "<thumbnails dir>" "<skins root>"
use std::path::{Path, PathBuf};
use image::GenericImageView;

fn avg_hsv(img: &image::DynamicImage) -> (f32, f32, f32) {
    let mut h = 0f32;
    let mut s = 0f32;
    let mut v = 0f32;
    let mut n = 0f32;
    for (_, _, px) in img.pixels() {
        let r = px[0] as f32 / 255.0;
        let g = px[1] as f32 / 255.0;
        let b = px[2] as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let s_c = if max == 0.0 { 0.0 } else { d / max };
        let h_c = if d == 0.0 { 0.0 } else if max == r { 60.0 * (((g - b) / d) % 6.0) } else if max == g { 60.0 * (((b - r) / d) + 2.0) } else { 60.0 * (((r - g) / d) + 4.0) };
        // skip near-grey pixels (background, grey paint reads as grey anyway)
        if s_c < 0.12 { continue; }
        h += h_c;
        s += s_c;
        v += max;
        n += 1.0;
    }
    if n == 0.0 { return (0.0, 0.0, 0.0); }
    (h / n, s / n, v / n)
}

fn main() {
    let thumbs = PathBuf::from(std::env::args().nth(1).unwrap());
    let skins = PathBuf::from(std::env::args().nth(2).unwrap());

    // index skins: (folder name, base image)
    let mut car_entries: Vec<(String, PathBuf)> = Vec::new();
    for brand in std::fs::read_dir(&skins).into_iter().flatten().flatten() {
        if !brand.path().is_dir() { continue; }
        for car in std::fs::read_dir(brand.path()).into_iter().flatten().flatten() {
            if !car.path().is_dir() { continue; }
            let mut base: Option<PathBuf> = None;
            for f in std::fs::read_dir(car.path()).into_iter().flatten().flatten() {
                let p = f.path();
                if p.extension().map(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("tga")).unwrap_or(false) {
                    let name = p.file_name().unwrap().to_string_lossy().to_lowercase();
                    if name.contains("base") { base = Some(p); break; }
                    if base.is_none() { base = Some(p); }
                }
            }
            let folder = car.path().file_name().unwrap().to_string_lossy().into_owned();
            if let Some(b) = base { car_entries.push((folder, b)); }
        }
    }
    eprintln!("indexed {} car folders with skins", car_entries.len());

    let mut thumbs_list: Vec<PathBuf> = std::fs::read_dir(&thumbs).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("sw")).unwrap_or(false)).collect();
    thumbs_list.sort();
    eprintln!("thumbnails: {}", thumbs_list.len());

    // build thumbnail stats: crop body region (lower 60%), avg_hsv
    for t in &thumbs_list {
        let img = match image::load_from_memory_with_format(&std::fs::read(t).unwrap_or_default(), image::ImageFormat::Png) { Ok(i) => i, Err(e) => { eprintln!("skip {}: {e}", t.display()); continue; } };
        let (tw, th) = img.dimensions();
        let body = img.crop_imm((tw as f32 * 0.15) as u32, (th as f32 * 0.45) as u32, (tw as f32 * 0.7) as u32, (th as f32 * 0.5) as u32);
        let (h, s, v) = avg_hsv(&body);
        // best match
        let mut best: Option<(f32, &Path, &str)> = None;
        for (folder, img_path) in &car_entries {
            let skin = match image::open(img_path) { Ok(i) => i, Err(_) => continue };
            let (sh, ss, sv) = avg_hsv(&skin);
            let dh = (h - sh).abs().min(360.0 - (h - sh).abs());
            let ds = (s - ss).abs() * 60.0;
            let dv = (v - sv).abs() * 60.0;
            let score = dh + ds + dv;
            match &best {
                Some((bs, _, _)) if score >= *bs => {}
                _ => best = Some((score, img_path, folder.as_str())),
            }
        }
        if let Some((score, _img, folder)) = best {
            println!("{:?} -> {folder}  (score {score:.1})", t.file_name().unwrap().to_string_lossy());
        }
    }
}
