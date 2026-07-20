use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_atom-db")
}

fn temp_path(name: &str, extension: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "atom-db-process-{name}-{}-{nonce}.{extension}",
        std::process::id()
    ))
}

fn spawn_holder(mode: &str, store: &Path, ready: &Path) -> Child {
    Command::new(binary())
        .arg("lease-hold")
        .arg(mode)
        .arg(store)
        .arg(ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

fn wait_until_ready(child: &mut Child, ready: &Path) {
    for _ in 0..100 {
        if ready.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("lease holder exited before ready marker: {status}");
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("lease holder did not become ready within two seconds");
}

fn probe(mode: &str, store: &Path) -> std::process::Output {
    Command::new(binary())
        .arg("lease-probe")
        .arg(mode)
        .arg(store)
        .output()
        .unwrap()
}

#[test]
fn operating_system_releases_writer_lease_after_process_crash() {
    let store = temp_path("store", "atoms");
    let writer_ready = temp_path("writer", "ready");
    let reader_ready = temp_path("reader", "ready");
    let initialized = Command::new(binary())
        .arg("init")
        .arg(&store)
        .status()
        .unwrap();
    assert!(initialized.success());

    let mut writer = spawn_holder("writer", &store, &writer_ready);
    wait_until_ready(&mut writer, &writer_ready);
    let denied_writer = probe("writer", &store);
    assert!(!denied_writer.status.success());
    assert!(
        String::from_utf8_lossy(&denied_writer.stderr).contains("active writer"),
        "unexpected contention error: {}",
        String::from_utf8_lossy(&denied_writer.stderr)
    );

    let mut reader = spawn_holder("reader", &store, &reader_ready);
    wait_until_ready(&mut reader, &reader_ready);
    let second_reader = probe("reader", &store);
    assert!(second_reader.status.success());

    writer.kill().unwrap();
    writer.wait().unwrap();
    let replacement_writer = probe("writer", &store);
    assert!(
        replacement_writer.status.success(),
        "writer lease survived process crash: {}",
        String::from_utf8_lossy(&replacement_writer.stderr)
    );

    drop(reader.stdin.take());
    assert!(reader.wait().unwrap().success());
    fs::remove_file(writer_ready).unwrap();
    fs::remove_file(reader_ready).unwrap();
    fs::remove_file(store).unwrap();
}
