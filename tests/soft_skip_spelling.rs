//! A soft skip is spelled one way.
//!
//! A test of this crate that leaves a check unmade for want of data — a
//! corpus that is not in the repository, a fixture a checkout does not hold
//! — prints one line that starts `skipping: ` and passes. `cargo test`
//! shows nothing a passing test prints, so those lines are read off a run
//! made with `-- --nocapture`, and `tools/soft_skips.py` counts them. The
//! count is worth reading only while every such line is spelled the same.
//!
//! Until 2026-11-12 the tests spelled it three ways: `skipping: …`, which
//! most of them used, `skip: …` in 50 places and `skipping …` with no colon
//! in 8. The two others are refused here, in every string literal of the
//! crate's test code: the files under `tests/` and, of each file under
//! `src/`, what follows its `#[cfg(test)]` module line.
//!
//! What this cannot hold is a skip that prints nothing, or says so in other
//! words. Those were looked for once, by running the tests with every early
//! exit made to speak, with and without the corpora this machine keeps
//! outside the repository (`docs/analysis/2026-11-12-what-the-root-crates-tests-skip.md`).

use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`, sorted.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// The index of the line a `#[cfg(test)]` module opens on: the attribute,
/// with a `mod` behind it and nothing but attributes between.
fn test_module_starts(lines: &[&str]) -> Option<usize> {
    lines.iter().enumerate().find_map(|(index, line)| {
        if !line.contains("#[cfg(test)]") {
            return None;
        }
        let next = lines[index + 1..]
            .iter()
            .map(|line| line.trim())
            .find(|line| !line.starts_with("#["))?;
        let next = next.strip_prefix("pub ").unwrap_or(next);
        next.starts_with("mod ").then_some(index)
    })
}

#[test]
fn a_notice_of_a_skip_opens_with_skipping_and_a_colon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let this_file = Path::new(file!())
        .file_name()
        .expect("this file has a name");
    // A literal's opening quote and then the other spelling.
    let refused = [format!("{}skip: ", '"'), format!("{}skipping ", '"')];
    let agreed = format!("{}skipping: ", '"');

    let mut found: Vec<String> = Vec::new();
    let mut agreed_literals = 0usize;
    let mut files_read = 0usize;
    for (top, whole_file) in [("tests", true), ("src", false)] {
        let mut files = Vec::new();
        sources(&root.join(top), &mut files);
        for file in files {
            if file.file_name() == Some(this_file) {
                continue;
            }
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
            let lines: Vec<&str> = text.lines().collect();
            let from = if whole_file {
                0
            } else {
                match test_module_starts(&lines) {
                    Some(index) => index,
                    None => continue,
                }
            };
            files_read += 1;
            for (index, line) in lines.iter().enumerate().skip(from) {
                // A comment may quote either spelling; a literal may not use it.
                if line.trim_start().starts_with("//") {
                    continue;
                }
                agreed_literals += line.matches(agreed.as_str()).count();
                if refused
                    .iter()
                    .any(|spelling| line.contains(spelling.as_str()))
                {
                    let relative = file.strip_prefix(root).unwrap_or(&file);
                    found.push(format!(
                        "{}:{}: {}",
                        relative.display(),
                        index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "a soft skip says `skipping: `; these say it another way:\n{}",
        found.join("\n")
    );
    // The walk read the test code: it found the literals that do say it.
    eprintln!(
        "soft-skip spelling: {files_read} files of test code, \
         {agreed_literals} literals that open the agreed way"
    );
    assert!(
        files_read >= 100 && agreed_literals >= 150,
        "{files_read} files of test code read, {agreed_literals} notices spelled the agreed way"
    );
}
