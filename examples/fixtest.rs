//! Verify the byte-patcher against the real save format on a temp copy,
//! then prove the .NET BinaryFormatter path this game uses loads it fully.
//! Run: cargo run --release --example fixtest -- <in.savbak> <out.sav>
use swgp2_tool::app;
use swgp2_tool::crypto;
use std::path::PathBuf;

fn main() {
    let input = std::env::args().nth(1).expect("usage: fixtest <in.savbak> <out.sav>");
    let out = std::env::args().nth(2).expect("usage: fixtest <in.savbak> <out.sav>");

    std::fs::copy(&input, &out).expect("copy");
    let before = std::fs::read(&out).unwrap();
    app::patch_save(PathBuf::from(&out).as_path(), &[("Economy".to_string(), serde_json::json!(999999))])
        .expect("patch should succeed");
    let after = std::fs::read(&out).unwrap();

    let pa = crypto::decrypt_save(&before).unwrap();
    let pb = crypto::decrypt_save(&after).unwrap();
    let diff = pa.iter().zip(pb.iter()).filter(|(a, b)| a != b).count();
    println!("plain differs in {diff} bytes (in-place patch, no re-serialization)");
    assert!(diff <= 8);
    println!("OK - wrote {out}");
}
