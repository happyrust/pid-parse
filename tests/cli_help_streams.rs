//! One contract, all six operator-facing binaries: `-h` / `--help`
//! writes the usage body to **stdout** and exits 0, while a usage error
//! writes the same body to **stderr** next to its diagnostic and leaves
//! stdout clean.
//!
//! The per-binary suites pin their own flag sets; this file pins the
//! stream routing, which no fixture is needed to observe. The routing
//! matters because usage text on stderr is invisible to
//! `--help > help.txt` — a redirect that once produced an empty file
//! and left the operator staring at it.

use std::process::Command;

/// `(binary, usage needle)` for every CLI that prints a usage body.
const BINARIES: &[(&str, &str)] = &[
    (
        env!("CARGO_BIN_EXE_pid_backup_extract"),
        "Usage: pid_backup_extract",
    ),
    (
        env!("CARGO_BIN_EXE_pid_backup_probe"),
        "Usage: pid_backup_probe",
    ),
    (env!("CARGO_BIN_EXE_pid_inspect"), "Usage: pid_inspect"),
    (
        env!("CARGO_BIN_EXE_pid_msda_probe"),
        "Usage: pid_msda_probe",
    ),
    (
        env!("CARGO_BIN_EXE_pid_publish_xml"),
        "Usage: pid_publish_xml",
    ),
    (
        env!("CARGO_BIN_EXE_pid_writer_validate"),
        "Usage: pid_writer_validate",
    ),
];

#[test]
fn help_flag_writes_usage_to_stdout_and_exits_zero() {
    for (bin, needle) in BINARIES {
        for flag in ["-h", "--help"] {
            let out = Command::new(bin)
                .arg(flag)
                .output()
                .unwrap_or_else(|e| panic!("spawn {bin} {flag}: {e}"));
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert_eq!(
                out.status.code(),
                Some(0),
                "{bin} {flag} should exit 0; stdout:\n{stdout}\nstderr:\n{stderr}"
            );
            assert!(
                stdout.contains(needle),
                "{bin} {flag} should print `{needle}` to stdout; stdout:\n{stdout}"
            );
            assert!(
                stderr.is_empty(),
                "{bin} {flag} is not a diagnostic; stderr:\n{stderr}"
            );
        }
    }
}

#[test]
fn missing_input_writes_usage_to_stderr_and_keeps_stdout_clean() {
    for (bin, needle) in BINARIES {
        let out = Command::new(bin)
            .output()
            .unwrap_or_else(|e| panic!("spawn {bin} with no args: {e}"));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{bin} with no input should fail; stdout:\n{stdout}"
        );
        assert!(
            stderr.contains(needle),
            "{bin} should put `{needle}` on stderr when the input is missing; stderr:\n{stderr}"
        );
        assert!(
            stdout.is_empty(),
            "{bin} failed and must leave stdout untouched; stdout:\n{stdout}"
        );
    }
}
