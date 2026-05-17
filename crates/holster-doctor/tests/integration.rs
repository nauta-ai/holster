use std::fs;

use holster_doctor::scanner::{scan_local_path, ScanArgs};

#[test]
fn scan_fixture_repo_finds_redacted_secret_shape() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        dir.path().join("main.py"),
        "OPENAI_API_KEY='sk-FAKE000000000000000000000000000000'\n",
    )
    .expect("write fixture");

    let report = scan_local_path(ScanArgs {
        path: dir.path().display().to_string(),
        follow_symlinks: false,
        respect_gitignore: false,
        max_file_size_bytes: 0,
        max_depth: 0,
    })
    .expect("scan succeeds");

    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.detections.len(), 1);
    assert_eq!(report.detections[0].file_path.as_deref(), Some("main.py"));
    assert!(!report.detections[0]
        .redacted_preview
        .contains("FAKE000000000000000000000000000000"));
}
