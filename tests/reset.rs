//! Reset-all-saves logic: archive -> delete -> restore, all in a temp dir.

use swgp2_tool::tool::reset;
use std::fs;

fn fake_saves(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..3 {
        fs::write(dir.join(format!("Supersave{i}.sav")), format!("fake-save-{i}").as_bytes()).unwrap();
    }
}

#[test]
fn reset_and_restore_roundtrip() {
    let base = std::env::temp_dir().join(format!("swgp2-reset-test-{}", std::process::id()));
    let dir = base.join("Save");
    let _ = fs::remove_dir_all(&base);
    fake_saves(&dir);

    // archive + delete
    let (archive, n) = reset::reset_all(&dir).expect("reset should succeed");
    assert_eq!(n, 3);
    assert_eq!(reset::saves_in(&dir).len(), 0, "no saves left after reset");
    assert_eq!(archive.files.len(), 3);
    assert!(archive.dir.is_dir());

    // archives are listed
    let archives = reset::list_archives(&dir);
    assert_eq!(archives.len(), 1);
    assert_eq!(archives[0].files.len(), 3);

    // restore brings everything back byte-identical
    let restored = reset::restore_archive(&dir, &archives[0]).expect("restore should succeed");
    assert_eq!(restored, 3);
    for i in 0..3 {
        let content = fs::read(dir.join(format!("Supersave{i}.sav"))).unwrap();
        assert_eq!(content, format!("fake-save-{i}").as_bytes());
    }

    // restoring again moves current saves aside first (pre-restore archive)
    let restored = reset::restore_archive(&dir, &archives[0]).expect("re-restore should succeed");
    assert_eq!(restored, 3);
    let names: Vec<String> = fs::read_dir(dir.join("Backups"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().any(|n| n.starts_with("FullReset-pre-restore-")), "pre-restore archive missing: {names:?}");

    // delete archive
    reset::delete_archive(&reset::list_archives(&dir)[0]).expect("delete archive should succeed");
    assert_eq!(reset::list_archives(&dir).len(), 1); // one remains from pre-restore... it starts with FullReset-pre-restore-, which still matches strip_prefix("FullReset-")

    let _ = fs::remove_dir_all(&base);
}
