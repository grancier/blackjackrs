use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn configures_session_and_quits_cleanly() {
    let executable = env!("CARGO_BIN_EXE_blackjackrs");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the blackjackrs binary must be executable");

    let _write_result = child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(b"100\n\n\n\nquit\n");
    let output = child.wait_with_output().expect("process completes");

    assert!(output.status.success(), "process failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("Blackjack"));
    assert!(stdout.contains("Bankroll: 100 chips"));
    assert!(stdout.contains("Goodbye."));
}
