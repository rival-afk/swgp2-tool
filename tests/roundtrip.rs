//! Byte-patching tests against the local save, if one exists.
//! Always operates on a temp copy - never touches the real save.
//!
//! Invariants that matter for game compatibility:
//!  1. A no-op patch leaves the file byte-identical.
//!  2. A value patch changes exactly the bytes of that value (same size).
//!  3. The patched file round-trips and reports the patched value.

use swgp2_tool::app;
use swgp2_tool::save;
use std::path::PathBuf;

fn local_save() -> Option<PathBuf> {
    save::default_save_path(0).filter(|p| p.exists())
}

#[test]
fn patch_noop_is_byte_identical() {
    let Some(src) = local_save() else {
        eprintln!("no local save, skipping");
        return;
    };
    let tmp = std::env::temp_dir().join("swgp2-patch-noop.sav");
    std::fs::copy(&src, &tmp).unwrap();
    let before = std::fs::read(&tmp).unwrap();

    let g = app::load(&tmp).unwrap();
    let econ = save::get_path(&g, "Economy")
        .map(|v| v.as_i64().unwrap_or(0))
        .unwrap_or(0);
    app::patch_save(&tmp, &[("Economy".to_string(), serde_json::json!(econ))]).expect("patch should verify");
    let after = std::fs::read(&tmp).unwrap();
    assert_eq!(before, after, "no-op patch must be byte-identical");

    let _ = std::fs::remove_file(&tmp);
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir().join("Backups")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().map(|n| n.to_string_lossy().starts_with("swgp2-patch-noop.")).unwrap_or(false) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

#[test]
fn patch_and_reload() {
    let Some(src) = local_save() else {
        eprintln!("no local save, skipping");
        return;
    };
    let tmp = std::env::temp_dir().join("swgp2-patch-test.sav");
    std::fs::copy(&src, &tmp).unwrap();
    let original = std::fs::read(&tmp).unwrap();

    let g = app::load(&tmp).unwrap();
    let econ0 = save::get_path(&g, "Economy")
        .map(|v| v.as_i64().unwrap_or(0))
        .unwrap_or(0);

    let dump0 = app::dump_json(&tmp).expect("dump");

    // set Economy to a different value, then set it back
    app::patch_save(&tmp, &[("Economy".to_string(), serde_json::json!(econ0 + 123))]).expect("patch +123 should verify");
    let g2 = app::load(&tmp).unwrap();
    assert_eq!(save::get_path(&g2, "Economy").unwrap().as_i64().unwrap(), econ0 + 123);
    let dump1 = app::dump_json(&tmp).expect("dump");
    assert_ne!(dump0, dump1);

    // only the economy bytes changed within the plain stream (same size)
    let patched = std::fs::read(&tmp).unwrap();
    let pa = swgp2_tool::crypto::decrypt_save(&patched).unwrap();
    let pb = swgp2_tool::crypto::decrypt_save(&original).unwrap();
    assert_eq!(pa.len(), pb.len(), "plain size changed");
    let diff = pa.iter().zip(pb.iter()).filter(|(a, b)| a != b).count();
    assert!(diff <= 8, "more than 8 plain bytes differ after economy patch: {diff}");

    // restore original value
    app::patch_save(&tmp, &[("Economy".to_string(), serde_json::json!(econ0))]).expect("restore should verify");
    assert_eq!(app::dump_json(&tmp).unwrap(), dump0);

    let _ = std::fs::remove_file(&tmp);
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir().join("Backups")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name().map(|n| n.to_string_lossy().starts_with("swgp2-patch-test.")).unwrap_or(false) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}
