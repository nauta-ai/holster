use std::path::Path;
use std::process::Command;

const FAKE_CHILD_WRAPPER_VALUE: &str = "sk-test-fake-child-wrapper-2026";
const FAKE_ALIZA_VALUE: &str = "sk-test-fake-aliza-2026-05-05";
const FAKE_SIDECAR_VALUE: &str = "sk-test-fake-sidecar-rollback-2026";
const FAKE_LEGACY_VALUE: &str = "sk-test-fake-legacy-env-rollback-2026";

#[test]
fn fake_child_env_wrapper_does_not_print_fake_secret() {
    let output = run_example("fake_child_env_wrapper");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "example failed: {stderr}");
    assert!(
        !stdout.contains(FAKE_CHILD_WRAPPER_VALUE),
        "stdout leaked fake secret: {stdout}"
    );
    assert!(
        !stderr.contains(FAKE_CHILD_WRAPPER_VALUE),
        "stderr leaked fake secret: {stderr}"
    );
    assert!(
        stdout.contains("child_stdout=child_env_present"),
        "expected child presence marker, got: {stdout}"
    );
}

#[test]
fn fake_aliza_adapter_does_not_print_fake_secret() {
    let output = run_example("fake_aliza_adapter");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "example failed: {stderr}");
    assert!(
        !stdout.contains(FAKE_ALIZA_VALUE),
        "stdout leaked fake Aliza secret: {stdout}"
    );
    assert!(
        !stderr.contains(FAKE_ALIZA_VALUE),
        "stderr leaked fake Aliza secret: {stderr}"
    );
    assert!(
        stdout.contains("child_stdout=child_env_present"),
        "expected child presence marker, got: {stdout}"
    );
}

#[test]
fn fake_sidecar_rollback_does_not_print_fake_secret() {
    let output = run_example("fake_sidecar_rollback");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "example failed: {stderr}");
    assert!(
        !stdout.contains(FAKE_SIDECAR_VALUE) && !stdout.contains(FAKE_LEGACY_VALUE),
        "stdout leaked fake secret: {stdout}"
    );
    assert!(
        !stderr.contains(FAKE_SIDECAR_VALUE) && !stderr.contains(FAKE_LEGACY_VALUE),
        "stderr leaked fake secret: {stderr}"
    );
    assert!(
        stdout.contains("rollback=restored"),
        "expected rollback marker, got: {stdout}"
    );
    assert!(
        stdout.contains("child_stdout=child_env_present"),
        "expected child presence marker, got: {stdout}"
    );
}

fn run_example(name: &str) -> std::process::Output {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");

    Command::new(env!("CARGO"))
        .args(["run", "-p", "holster-vault", "--example", name])
        .current_dir(workspace_root)
        .output()
        .expect("run example")
}
