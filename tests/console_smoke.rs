use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

fn run_with_input(input: &[u8]) -> Output {
    let executable = env!("CARGO_BIN_EXE_blackjackrs");
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the blackjackrs binary must be executable");

    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(input)
        .expect("write process input");
    child.wait_with_output().expect("process completes")
}

#[test]
fn configures_session_and_quits_cleanly() {
    let output = run_with_input(b"100\n\n\n\nquit\n");

    assert!(output.status.success(), "process failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("Blackjack"));
    assert!(stdout.contains("Bankroll: 100 chips"));
    assert!(stdout.contains("Goodbye."));
}

#[test]
fn plays_a_complete_round_and_reports_settlement() {
    let output = run_with_input(b"100\n\n\n\n10\n0\nstand\nquit\n");

    assert!(output.status.success(), "process failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
    assert!(stdout.contains("Dealer:"));
    assert!(stdout.contains("Round result:"));
    assert!(stdout.contains("Total credit:"));
    assert!(stdout.contains("Bankroll after round:"));
    assert!(stdout.contains("Goodbye."));
}
