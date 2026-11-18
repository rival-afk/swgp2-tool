//! Round-trip test against the local save, if one exists.
//! Always operates on a temp copy - never touches the real save.

use swgp2_save_editor::app;
use std::path::PathBuf;

fn local_save() -> Option<PathBuf> {
    swgp2_save_editor::save::default_save_path(0).filter(|p| p.exists())
}

#[test]
fn load_dump_store_roundtrip() {
    let Some(src) = local_save() else {
        eprintln!("no local save found, skipping");
        return;
    };
    let tmp = std::env::temp_dir().join("swgp2-roundtrip-test.sav");
    std::fs::copy(&src, &tmp).unwrap();

    let dump1 = app::dump_json(&tmp).expect("dump should succeed");

    // set Economy to its current value -> forces a full read/write/verify cycle
    let g = app::load(&tmp).unwrap();
    let econ = swgp2_save_editor::save::get_path(&g, "Economy")
        .map(|v| v.as_i64().unwrap_or(0))
        .unwrap_or(0);
    app::save_with_backup(&tmp, &[("Economy".to_string(), serde_json::json!(econ))])
        .expect("save_with_backup should verify");

    let dump2 = app::dump_json(&tmp).expect("dump after store should succeed");
    assert_eq!(dump1, dump2, "roundtrip through store changed the data");

    let _ = std::fs::remove_file(&tmp);
    if let Ok(rd) = std::fs::read_dir(std::env::temp_dir().join("Backups")) {
        for e in rd.flatten() {
            let p = e.path();
            if p.file_name()
                .map(|n| n.to_string_lossy().starts_with("swgp2-roundtrip-test."))
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}
