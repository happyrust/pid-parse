//! End-to-end tests for the `pid_backup_store` binary (plan
//! `docs/plans/2026-10-09-a-plant-backup-becomes-one-backup-store.md`
//! S3, Q17): the compiled binary is driven through `Command`, so
//! argument parsing, the store build, the temporary file and the
//! summary are exercised the way an operator runs them. Tests that
//! read a sample skip when it is absent.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pid_parse::backup::store::{BackupInput, DIRECTORY_INPUT_NOTE};
use rusqlite::Connection;

const TEST02_ZIP: &str = "test-file/backup-test/TEST02_p.zip";
const SQPLANT_ENV: &str = "PID_PARSE_SQPLANT_BACKUP";
const SQPLANT_DEFAULT: &str = r"D:\work\cad\pid-test-data";

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_pid_backup_store")
}

/// A fresh directory under the temp dir for one test's files.
fn scratch_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "pid-parse-backup-store-cli-{}-{label}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn test02() -> Option<&'static Path> {
    let path = Path::new(TEST02_ZIP);
    if path.exists() {
        Some(path)
    } else {
        eprintln!("skip: {TEST02_ZIP} is absent");
        None
    }
}

fn sqplant() -> Option<PathBuf> {
    let path = std::env::var_os(SQPLANT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(SQPLANT_DEFAULT));
    if path.join("Manifest.txt").is_file() {
        Some(path)
    } else {
        eprintln!(
            "skip: SQPlant backup is absent ({SQPLANT_ENV} unset, {} has no Manifest.txt)",
            path.display()
        );
        None
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(binary_path())
        .args(args)
        .output()
        .expect("run pid_backup_store")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn store_info(conn: &Connection) -> BTreeMap<String, String> {
    let mut stmt = conn.prepare("SELECT key, value FROM store_info").unwrap();
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0))
        .unwrap_or_else(|err| panic!("{sql}: {err}"))
}

#[test]
fn help_prints_the_usage_and_exits_0() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(Some(0), output.status.code(), "{flag}");
        let text = stdout(&output);
        assert!(
            text.starts_with("Usage: pid_backup_store <Plant Backup> -o <store.sqlite>"),
            "{flag}: {text}"
        );
        assert!(text.contains("--force") && text.contains("--keep-secrets"));
        assert!(text.contains("--embed-files"), "{text}");
    }
}

#[test]
fn usage_errors_exit_2_without_touching_the_output() {
    let dir = scratch_dir("usage");
    let out = dir.join("store.sqlite");
    let out_str = out.to_str().unwrap();

    // No -o.
    let output = run(&[TEST02_ZIP]);
    assert_eq!(Some(2), output.status.code());
    let text = stderr(&output);
    assert!(
        text.starts_with("argument error: -o <store.sqlite> is required"),
        "{text}"
    );
    assert!(text.contains("Usage: pid_backup_store"), "{text}");

    // No input.
    let output = run(&["-o", out_str]);
    assert_eq!(Some(2), output.status.code());
    assert!(
        stderr(&output).starts_with("argument error: missing the Plant Backup to read"),
        "{}",
        stderr(&output)
    );

    // An unknown flag.
    let output = run(&[TEST02_ZIP, "-o", out_str, "--fast"]);
    assert_eq!(Some(2), output.status.code());
    assert!(
        stderr(&output).starts_with("argument error: unknown flag: --fast"),
        "{}",
        stderr(&output)
    );

    // Nothing at all.
    let output = run(&[]);
    assert_eq!(Some(2), output.status.code());

    assert!(!out.exists(), "a usage error writes nothing");
    assert!(
        stdout(&run(&[TEST02_ZIP])).is_empty(),
        "usage errors go to stderr"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_input_that_cannot_be_read_exits_1() {
    let dir = scratch_dir("missing-input");
    let out = dir.join("store.sqlite");
    let missing = dir.join("no-such-backup_p.zip");
    let output = run(&[missing.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(Some(1), output.status.code());
    let text = stderr(&output);
    assert!(text.starts_with("error: "), "{text}");
    assert!(text.contains("no-such-backup_p.zip"), "{text}");
    assert!(!out.exists());
    assert!(
        !dir.join("store.sqlite.tmp").exists(),
        "no temporary file is left behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test02_store_is_built_and_summarised() {
    let Some(zip) = test02() else {
        return;
    };
    let dir = scratch_dir("test02");
    let out = dir.join("TEST02.sqlite");
    let output = run(&[zip.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(Some(0), output.status.code(), "stderr: {}", stderr(&output));
    assert_eq!(
        format!(
            "{}: Backup Store written\n  files           1554 (0 embedded)\n  manifest lines  319, redactions 14\n  dump tables     154, rows 37470, ghost rows 5, LOBs 4\n  warnings        0\n",
            out.display()
        ),
        stdout(&output)
    );
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    assert!(out.is_file());
    assert!(!dir.join("TEST02.sqlite.tmp").exists());

    let conn = Connection::open(&out).unwrap();
    let info = store_info(&conn);
    assert_eq!(
        Some("pid_backup_store"),
        info.get("tool_name").map(String::as_str)
    );
    assert_eq!(Some("zip"), info.get("input_kind").map(String::as_str));
    assert_eq!(Some("1"), info.get("redacted").map(String::as_str));
    assert_eq!(Some("0"), info.get("files_embedded").map(String::as_str));
    assert_eq!(
        Some("sql-server-mtf"),
        info.get("dump_kind").map(String::as_str)
    );
    assert_eq!(1_554, count(&conn, "SELECT count(*) FROM backup_file"));
    assert_eq!(0, count(&conn, "SELECT count(*) FROM backup_file_content"));
    assert_eq!(319, count(&conn, "SELECT count(*) FROM manifest_line"));
    assert_eq!(14, count(&conn, "SELECT count(*) FROM store_redaction"));
    assert_eq!(154, count(&conn, "SELECT count(*) FROM dump_table"));
    assert_eq!(37_470, count(&conn, "SELECT sum(rows) FROM dump_table"));
    assert_eq!(5, count(&conn, "SELECT count(*) FROM dump_ghost_row"));
    assert_eq!(4, count(&conn, "SELECT count(*) FROM dump_lob"));
    assert_eq!(1, count(&conn, "SELECT count(*) FROM pid__T_Drawing"));
    drop(conn);

    // The same output again is refused without --force, and the file
    // is left as it was.
    let before = std::fs::metadata(&out).unwrap().len();
    let output = run(&[zip.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(Some(1), output.status.code());
    assert_eq!(
        format!(
            "error: {} already exists; pass --force to replace it\n",
            out.display()
        ),
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert_eq!(before, std::fs::metadata(&out).unwrap().len());
    assert!(!dir.join("TEST02.sqlite.tmp").exists());

    // --force replaces it, keeping secrets and embedding files this time.
    let output = run(&[
        zip.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--force",
        "--keep-secrets",
        "--embed-files",
    ]);
    assert_eq!(Some(0), output.status.code(), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains("  manifest lines  319, redactions 0\n"),
        "{}",
        stdout(&output)
    );
    let conn = Connection::open(&out).unwrap();
    let info = store_info(&conn);
    assert_eq!(Some("0"), info.get("redacted").map(String::as_str));
    assert_eq!(Some("1"), info.get("files_embedded").map(String::as_str));
    assert_eq!(0, count(&conn, "SELECT count(*) FROM store_redaction"));
    // Every file has its bytes stored: 1,348 of the 1,554 entries, the
    // other 206 being directory entries of the Option Archives.
    let files = count(&conn, "SELECT count(*) FROM backup_file WHERE is_dir = 0");
    assert_eq!(1_348, files);
    assert_eq!(
        files,
        count(&conn, "SELECT count(*) FROM backup_file_content")
    );
    assert!(
        stdout(&output).contains(&format!("  files           1554 ({files} embedded)\n")),
        "{}",
        stdout(&output)
    );
    // The embedded bytes are the zip's: PlantConfig.xml and Manifest.txt
    // as BackupInput reads them from the original.
    let mut original = BackupInput::open(zip).unwrap();
    for name in ["PlantConfig.xml", "Manifest.txt"] {
        let index = original
            .files()
            .iter()
            .find(|file| file.name == name)
            .unwrap()
            .entry_index;
        let expected = original.read(index).unwrap();
        let stored: Vec<u8> = conn
            .query_row(
                "SELECT c.bytes FROM backup_file_content c JOIN backup_file f ON f.id = c.file_id \
                 WHERE f.container_id IS NULL AND f.path = ?1",
                [name],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(expected, stored, "{name}");
    }
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sqplant_directory_is_built_with_its_warnings_on_stderr() {
    let Some(backup) = sqplant() else {
        return;
    };
    let dir = scratch_dir("sqplant");
    let out = dir.join("SQPlant.sqlite");
    let output = run(&[backup.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert_eq!(Some(0), output.status.code(), "stderr: {}", stderr(&output));
    assert_eq!(
        format!(
            "{}: Backup Store written\n  files           865 (0 embedded)\n  manifest lines  322, redactions 15\n  dump tables     154, rows 0, ghost rows 0, LOBs 0\n  warnings        2\n",
            out.display()
        ),
        stdout(&output)
    );
    assert_eq!(
        format!(
            "warning: Export.dmp: an Oracle `exp` dump; its 154 tables are registered from the \
             DDL and left empty, their rows are not decoded in this version\nwarning: {DIRECTORY_INPUT_NOTE}\n"
        ),
        stderr(&output)
    );
    let conn = Connection::open(&out).unwrap();
    let info = store_info(&conn);
    assert_eq!(
        Some("directory"),
        info.get("input_kind").map(String::as_str)
    );
    assert_eq!(
        Some("oracle-exp"),
        info.get("dump_kind").map(String::as_str)
    );
    assert_eq!(
        0,
        count(&conn, "SELECT count(*) FROM dump_table WHERE decoded <> 0")
    );
    assert_eq!(2_024, count(&conn, "SELECT count(*) FROM dump_column"));
    drop(conn);
    let _ = std::fs::remove_dir_all(&dir);
}
