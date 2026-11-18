//! Render a PNG as ASCII art (for eyeballing image content in a terminal).
//! Run: cargo run --release --example peek -- <file> [w] [h]
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let w: u32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    let h: u32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(24);
    let img = if std::path::Path::new(&path).extension().map(|e| e.eq_ignore_ascii_case("sw")).unwrap_or(false) {
        image::load_from_memory_with_format(&std::fs::read(&path).unwrap(), image::ImageFormat::Png).expect("decode png")
    } else {
        image::open(&path).expect("decode")
    };
    let small = img.resize_exact(w, h, image::imageops::FilterType::Triangle).to_luma8();
    let ramp = b" .:-=+*#%@";
    for y in 0..h {
        let mut line = String::with_capacity(w as usize);
        for x in 0..w {
            let l = small.get_pixel(x, y).0[0] as usize;
            line.push(ramp[(l * (ramp.len() - 1) / 255).min(ramp.len() - 1)] as char);
        }
        println!("{line}");
    }
}
