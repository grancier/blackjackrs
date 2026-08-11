use std::process::Command;

#[test]
fn prints_hello_world_to_standard_output() {
    let executable = env!("CARGO_BIN_EXE_blackjackrs");

    let output = Command::new(executable)
        .output()
        .expect("the blackjackrs binary must be executable");

    assert!(output.status.success(), "process failed: {output:?}");
    assert_eq!(output.stdout, b"Hello World!\n");
    assert!(output.stderr.is_empty(), "unexpected stderr: {output:?}");
}
