//! One-shot recovery: restore a backup over the live save, then re-apply the Economy edit.
//! Run: cargo run --release --example dorestore -- <save> <backup> [economy]
use swgp2_tool::app;
use std::path::PathBuf;

fn main() {
    let save = std::env::args().nth(1).expect("usage: dorestore <save> <backup> [economy]");
    let backup = std::env::args().nth(2).expect("usage: dorestore <save> <backup> [economy]");
    let save_path = PathBuf::from(&save);

    let pre = app::restore_backup(&save_path, std::path::Path::new(&backup))
        .expect("restore failed");
    println!("restored {backup} -> {save}");
    println!("previous (broken) save backed up to {}", pre.display());

    if let Some(econ) = std::env::args().nth(3) {
        let econ: i64 = econ.parse().expect("economy must be an integer");
        let b = app::patch_save(&save_path, &[("Economy".to_string(), serde_json::json!(econ))])
            .expect("patch failed");
        println!("Economy set to {econ} (backup: {})", b.display());
    }
}
