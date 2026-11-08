//! The page model a committed fixture lays out, pinned byte for byte.
//!
//! Every other plot gate in this crate reads a corpus that lives outside the
//! repository (a licensed symbol library, a font directory, an extracted plant
//! backup), so on CI they all soft-skip and nothing about the layout is
//! measured. This gate has no such input: the seven `.pid` fixtures under
//! `test-file/` are committed, the templates under `test-file/Ref/Template
//! Files/` are committed, the four project symbols under
//! `test-file/Ref/Symbols/` are committed, and the backup package's
//! `RefData~4~681.zip` (616 `.sym`) and `RefData~4~682.zip` (11 `.igr`) are
//! committed. **There is no skip path** — a missing fixture is a broken
//! checkout and fails here.
//!
//! # What it measures
//!
//! [`lay_out_drawing`] turns a `.pid` into a [`PaperModel`]: the page size, the
//! border template and its furniture, and every primitive in paper
//! millimetres. None of that needs a font — the writer is what reads faces —
//! so the whole layout is exercised without one. That is why this gate runs
//! where the others cannot.
//!
//! # Why the symbol sources are stated, not discovered
//!
//! `lay_out_drawing` searches for a symbol library itself, and one of the
//! places it looks is `test-file/symbols-full` — 618 `.sym` files that are
//! gitignored. A golden blessed on a developer machine would carry strokes a
//! clean checkout cannot reproduce, so this gate calls
//! [`lay_out_with_symbol_roots`] with the committed sources only. The reading
//! is then the same here and on the runner.
//!
//! # Why the paths are normalized
//!
//! Three fields carry a filesystem path: `library`, `template.path` and
//! `template.symbols[].key`. The first is already the relative label stated
//! above; the other two are absolute. Every string is therefore separator-
//! unified (`\` → `/`) and has the repository root replaced with `<repo>`, so
//! the golden is portable. A path a `.pid` itself stores — an authoring
//! machine's `C:\Users\…\Temp\x.sym` — is left alone: it is source bytes, it is
//! the same on every platform, and rewriting it would hide a real difference.
//!
//! # Workflow
//!
//! * Normal run — read the committed golden, and on a mismatch report the first
//!   differing JSON path with both sides, so a failure names the sheet and the
//!   field rather than dumping four megabytes.
//! * `PID_PLOT_BLESS=1 cargo test -p pid-plot --test paper_model_golden` —
//!   rewrite every golden from current behaviour. Only for an **intentional**
//!   layout change; call it out in the PR.
//!
//! The seven goldens total about 4.25 MiB, which is why they are committed as
//! one file per fixture rather than one blob.

use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_with_symbol_roots};
use serde_json::Value;

/// One fixture and the golden filename stem it takes.
///
/// The stems are explicit rather than derived from the path so a non-ASCII
/// fixture name maps onto a portable golden filename, exactly as
/// `tests/golden/geometry/*.json` does in the parser crate.
struct GoldenCase {
    /// Fixture path relative to `test-file/`.
    path: &'static str,
    /// Golden filename stem under `tests/golden/paper/`.
    slug: &'static str,
}

const GOLDEN_CASES: &[GoldenCase] = &[
    GoldenCase {
        path: "D06.pid",
        slug: "d06",
    },
    GoldenCase {
        path: "DWG-0201GP06-01.pid",
        slug: "dwg-0201gp06-01",
    },
    GoldenCase {
        path: "DWG-0202GP06-01.pid",
        slug: "dwg-0202gp06-01",
    },
    GoldenCase {
        path: "工艺管道及仪表流程-1.pid",
        slug: "gongyi-guandao-1",
    },
    GoldenCase {
        path: "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
        slug: "jizhuzhan-flash-b",
    },
    GoldenCase {
        path: "export-test/publish-data/A01/A01.pid",
        slug: "publish-a01",
    },
    GoldenCase {
        path: "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
        slug: "publish-dwg-0202gp06-01",
    },
];

/// The repository root: `pid-plot/`'s parent.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot is a directory in the repository")
        .to_path_buf()
}

/// The committed fixtures root.
fn corpus_dir() -> PathBuf {
    repo_root().join("test-file")
}

/// The symbol sources this gate states: the four committed project `.sym`
/// files, then the backup package's committed library archive.
///
/// Stating them is what keeps the reading identical in a clean checkout — see
/// the module docs.
fn symbol_sources(corpus: &Path) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<String>) {
    let roots = vec![corpus.join("Ref").join("Symbols")];
    let archives = vec![
        corpus
            .join("backup-test")
            .join("DWG-0202GP06-01_p")
            .join("RefData~4~681.zip"),
    ];
    let label = vec![
        "test-file/Ref/Symbols".to_string(),
        "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip".to_string(),
    ];
    (roots, archives, label)
}

fn golden_path(case: &GoldenCase) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("paper")
        .join(format!("{}.json", case.slug))
}

fn bless_enabled() -> bool {
    std::env::var_os("PID_PLOT_BLESS").is_some_and(|value| value != "0" && !value.is_empty())
}

/// Canonical serialization: pretty JSON with a trailing newline, every path
/// normalized onto `<repo>`.
fn render(model: &PaperModel, repo: &Path) -> String {
    let mut value = serde_json::to_value(model)
        .unwrap_or_else(|error| panic!("a PaperModel is serializable: {error}"));
    let prefix = repo.display().to_string().replace('\\', "/");
    normalize(&mut value, &prefix);
    let mut json = serde_json::to_string_pretty(&value)
        .unwrap_or_else(|error| panic!("a JSON value is serializable: {error}"));
    json.push('\n');
    json
}

/// Unify separators and replace the repository root in every string.
fn normalize(value: &mut Value, prefix: &str) {
    match value {
        Value::String(text) => {
            *text = text.replace('\\', "/").replace(prefix, "<repo>");
        }
        Value::Array(items) => {
            for item in items {
                normalize(item, prefix);
            }
        }
        Value::Object(map) => {
            for (_, item) in map.iter_mut() {
                normalize(item, prefix);
            }
        }
        _ => {}
    }
}

/// The first place two JSON documents disagree, as a readable path with both
/// sides.
///
/// A four-megabyte string comparison would print the whole model; this walks
/// the two trees and stops at the first difference, so a moved label reports
/// `primitives[1234].shape.origin_mm[1]: expected 214.9, actual 216.4`.
fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(left), Value::Object(right)) => {
            for (key, left_value) in left {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                match right.get(key) {
                    Some(right_value) => {
                        if let Some(found) = first_difference(left_value, right_value, &child) {
                            return Some(found);
                        }
                    }
                    None => return Some(format!("{child}: expected {left_value}, absent")),
                }
            }
            for (key, right_value) in right {
                if !left.contains_key(key) {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    return Some(format!("{child}: absent, actual {right_value}"));
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            for (index, left_value) in left.iter().enumerate() {
                let child = format!("{path}[{index}]");
                match right.get(index) {
                    Some(right_value) => {
                        if let Some(found) = first_difference(left_value, right_value, &child) {
                            return Some(found);
                        }
                    }
                    None => {
                        return Some(format!(
                            "{child}: expected {left_value}, but the list ends after {} entries",
                            right.len()
                        ));
                    }
                }
            }
            if right.len() > left.len() {
                return Some(format!(
                    "{path}: expected {} entries, actual {} (first extra is {})",
                    left.len(),
                    right.len(),
                    right[left.len()]
                ));
            }
            None
        }
        _ if expected != actual => Some(format!("{path}: expected {expected}, actual {actual}")),
        _ => None,
    }
}

#[test]
fn every_committed_fixture_lays_out_byte_for_byte_as_its_golden_says() {
    let repo = repo_root();
    let corpus = corpus_dir();
    let (roots, archives, label) = symbol_sources(&corpus);
    let bless = bless_enabled();
    let mut total = 0usize;

    for case in GOLDEN_CASES {
        let fixture = corpus.join(case.path);
        assert!(
            fixture.is_file(),
            "fixture `{}` is not in this checkout at {}; the seven fixtures this \
             gate reads are committed, so this is a broken checkout rather than \
             a corpus that is merely absent",
            case.path,
            fixture.display()
        );

        let model =
            lay_out_with_symbol_roots(&fixture, &corpus, roots.clone(), archives.clone(), &label)
                .unwrap_or_else(|error| panic!("{}: {error}", case.path));
        let actual_text = render(&model, &repo);
        total += actual_text.len();
        let golden = golden_path(case);

        if bless {
            if let Some(parent) = golden.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!("failed to create {}: {error}", parent.display())
                });
            }
            std::fs::write(&golden, actual_text.as_bytes())
                .unwrap_or_else(|error| panic!("failed to write {}: {error}", golden.display()));
            eprintln!(
                "blessed {}: {} bytes, {} primitives",
                golden.display(),
                actual_text.len(),
                model.primitives.len()
            );
            continue;
        }

        let expected_text = std::fs::read_to_string(&golden).unwrap_or_else(|error| {
            panic!(
                "golden `{}` for fixture `{}` is missing ({}). The fixture is \
                 committed, so this is a new baseline rather than an absent \
                 corpus: run `PID_PLOT_BLESS=1 cargo test -p pid-plot --test \
                 paper_model_golden` once to establish it, then commit the file.",
                case.slug, case.path, error
            )
        });
        let expected: Value = serde_json::from_str(&expected_text)
            .unwrap_or_else(|error| panic!("golden `{}` is not JSON: {error}", case.slug));
        let actual: Value = serde_json::from_str(&actual_text)
            .unwrap_or_else(|error| panic!("the model of `{}` is not JSON: {error}", case.path));

        if expected == actual {
            eprintln!(
                "{}: {} bytes, {} primitives, matches its golden",
                case.slug,
                actual_text.len(),
                model.primitives.len()
            );
            continue;
        }

        let where_it_differs = first_difference(&expected, &actual, "").unwrap_or_else(|| {
            "<the documents compare equal but serialize differently>".to_string()
        });
        panic!(
            "the page model of `{}` (golden `{}`) changed at {}.\n\
             If this is an intentional layout change, re-bless with \
             `PID_PLOT_BLESS=1 cargo test -p pid-plot --test paper_model_golden` \
             and call it out in the PR; otherwise this is a layout regression.",
            case.path, case.slug, where_it_differs
        );
    }

    eprintln!(
        "paper model golden: {} fixtures, {total} bytes ({:.2} MiB){}",
        GOLDEN_CASES.len(),
        total as f64 / (1024.0 * 1024.0),
        if bless { ", blessed" } else { "" }
    );
}
