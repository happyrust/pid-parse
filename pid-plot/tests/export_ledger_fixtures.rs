//! The export ledger's own gate: the seven committed fixtures, laid out from
//! nothing but repository files, must each account for everything they were
//! given.
//!
//! This is the gate that actually asserts on CI. `export_every_fixture` cannot:
//! it writes a PDF, so it needs a font directory, and a clean checkout has none.
//! The ledger needs no typeface at all — `write_ledger` reaches every verdict
//! `write_plot_audited` reaches without loading a face — so this gate runs the
//! same way on a developer's machine and on a bare runner.
//!
//! # No soft skip
//!
//! Every input is committed:
//!
//! | input | source |
//! |---|---|
//! | the seven drawings | `test-file/**` (the `Fixture_Set` of the spec) |
//! | symbol bodies | `test-file/symbols`, `test-file/Ref/Symbols` |
//! | templates | `test-file/Ref/Template Files` |
//! | backup archives | `test-file/backup-test/*_p/RefData~*.zip` |
//! | typefaces | **none** — `write_ledger` reads no face |
//!
//! A missing fixture is therefore a repository defect, and the test fails
//! rather than returning: a gate that can pass by doing nothing is not a gate.
//!
//! # What is asserted
//!
//! 1. every page assembles, and its five layers of each domain balance
//!    (`unattributed == 0`);
//! 2. `cross_check` finds nothing — the ledger agrees with the page model, the
//!    geometry tables and its own segment arithmetic;
//! 3. laying the same page out twice, and serializing twice, gives the same
//!    bytes: the account is deterministic;
//! 4. no serialized account carries the repository root, `CARGO_MANIFEST_DIR`,
//!    the user's home directory or a drive path — a baseline may hold counts
//!    and stream names, never a machine path;
//! 5. the account matches `docs/baselines/export-ledger-fixtures.json`, or
//!    rewrites it when `UPDATE_EXPORT_LEDGER=1`.

use std::path::{Path, PathBuf};

use pid_parse::{PidParser, build_normalized_geometry};
use pid_plot::ledger::assemble::ExportLedger;
use pid_plot::ledger::baseline::{GateInput, LedgerCorpus, descriptor_for, run_gate};
use pid_plot::model::lay_out_with_symbol_roots;
use pid_plot::write::write_ledger;

/// The seven committed fixtures, relative to `test-file/`.
///
/// Taken from the specification's `Fixture_Set` by name rather than by walking
/// the tree: a directory walk would silently admit a new drawing to the gate's
/// corpus, and a baseline is only meaningful over a stated set.
const FIXTURES: [&str; 7] = [
    "D06.pid",
    "DWG-0201GP06-01.pid",
    "DWG-0202GP06-01.pid",
    "export-test/publish-data/A01/A01.pid",
    "export-test/publish-data/DWG-0202GP06-01/DWG-0202GP06-01.pid",
    "工艺管道及仪表流程-1.pid",
    "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pid-plot is a directory in the repository")
        .to_path_buf()
}

/// The symbol roots this gate states: the committed sample library and the
/// project's own `Xa …` files.
fn symbol_roots(corpus: &Path) -> Vec<PathBuf> {
    vec![corpus.join("symbols"), corpus.join("Ref").join("Symbols")]
}

/// Every `RefData~*.zip` the committed backup packages carry, path-sorted.
///
/// The packages are part of the repository, so this is not a conditional
/// input: the archives exist in every checkout.
fn archives(corpus: &Path) -> Vec<PathBuf> {
    let backup = corpus.join("backup-test");
    let mut found: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&backup) else {
        return found;
    };
    let mut packages: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    packages.sort();
    for package in packages {
        if !package.is_dir() {
            continue;
        }
        found.extend(pid_parse::refdata_archive::refdata_archives(&package));
    }
    found.sort();
    found.dedup();
    found
}

/// A page's key in the baseline: its path relative to `test-file/`, `/`
/// separated.
fn page_key(corpus: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(corpus).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The canonical serialization of one page's account.
fn render(ledger: &ExportLedger) -> String {
    let mut json = serde_json::to_string_pretty(ledger)
        .unwrap_or_else(|error| panic!("an export ledger is serializable: {error}"));
    json.push('\n');
    json
}

/// The machine paths that must never reach a baseline (requirement 12.4).
fn forbidden_fragments() -> Vec<String> {
    let mut fragments = vec![
        repo_root().display().to_string(),
        env!("CARGO_MANIFEST_DIR").to_string(),
    ];
    for name in ["USERPROFILE", "HOME"] {
        if let Some(home) = std::env::var_os(name) {
            let home = home.to_string_lossy().into_owned();
            if !home.is_empty() {
                fragments.push(home);
            }
        }
    }
    fragments
}

#[test]
fn the_seven_fixtures_balance_their_ledger() {
    let corpus = repo_root().join("test-file");
    let roots = symbol_roots(&corpus);
    let archives = archives(&corpus);
    let label = vec![
        "test-file/symbols".to_string(),
        "test-file/Ref/Symbols".to_string(),
    ];

    let mut pages: Vec<(String, ExportLedger)> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut breaches: Vec<String> = Vec::new();
    let mut ledgers = Vec::new();
    let mut forbidden = forbidden_fragments();
    forbidden.sort();
    forbidden.dedup();

    for fixture in FIXTURES {
        let path = corpus.join(fixture);
        assert!(
            path.is_file(),
            "{} is a committed fixture and is missing from this checkout",
            path.display()
        );
        let key = page_key(&corpus, &path);

        // Layout once, and again from scratch: the account has to be a
        // function of the bytes, not of what the first pass left behind.
        let first =
            lay_out_with_symbol_roots(&path, &corpus, roots.clone(), archives.clone(), &label)
                .unwrap_or_else(|error| panic!("{key}: {error}"));
        let second =
            lay_out_with_symbol_roots(&path, &corpus, roots.clone(), archives.clone(), &label)
                .unwrap_or_else(|error| panic!("{key}: {error}"));

        let write = write_ledger(&first).unwrap_or_else(|error| panic!("{key}: {error}"));
        assert!(
            write.mismatches.is_empty(),
            "{key}: the writer's verdicts disagreed with its observations: {:?}",
            write.mismatches
        );
        let ledger = ExportLedger::assemble(&first, &write);
        assert!(
            ledger.conservation_breaches().is_empty(),
            "{key}: {:?}",
            ledger.conservation_breaches()
        );

        // Requirement 8.5: the parse-layer counts are checked against the
        // geometry tables, so the drawing is parsed a second time for them.
        let doc = PidParser::new()
            .parse_file(&path)
            .unwrap_or_else(|error| panic!("{key}: {error}"));
        let geometry = build_normalized_geometry(&doc);
        let mismatches = ledger.cross_check(&first, None, Some(&geometry));
        assert!(
            mismatches.is_empty(),
            "{key}: the ledger disagrees with the counts the code already keeps: {mismatches:#?}"
        );

        // Determinism: the same page, laid out twice, serializes to the same
        // bytes (requirement 10.1).
        let second_write = write_ledger(&second).unwrap_or_else(|error| panic!("{key}: {error}"));
        let second_ledger = ExportLedger::assemble(&second, &second_write);
        let first_json = render(&ledger);
        let second_json = render(&second_ledger);
        assert_eq!(
            first_json, second_json,
            "{key}: two layouts of the same page produced different accounts"
        );

        // Requirement 12.4: no machine path may reach the baseline.
        for fragment in &forbidden {
            assert!(
                !first_json.contains(fragment.as_str()),
                "{key}: the account carries the machine path {fragment:?}"
            );
        }
        for drive in ["D:/", "C:/", "D:\\", "C:\\"] {
            assert!(
                !first_json.contains(drive),
                "{key}: the account carries a drive path ({drive})"
            );
        }

        ledgers.push(first.ledger.clone());
        pages.push((key, ledger));
    }

    pages.sort_by(|left, right| left.0.cmp(&right.0));
    let refs: Vec<&pid_plot::ledger::LayoutLedger> = ledgers.iter().collect();
    let descriptor = descriptor_for(&refs, Vec::new());

    let baseline_path = repo_root()
        .join("docs")
        .join("baselines")
        .join("export-ledger-fixtures.json");
    let update = std::env::var("UPDATE_EXPORT_LEDGER").is_ok_and(|value| value == "1");
    let report = run_gate(GateInput {
        corpus: LedgerCorpus::Fixtures,
        baseline_path: &baseline_path,
        descriptor,
        pages,
        failures: std::mem::take(&mut failures),
        breaches: std::mem::take(&mut breaches),
        update,
        hint: "夹具基线取法：cargo test -p pid-plot --test export_ledger_fixtures（详见 docs/baselines/README.md）",
    });
    let rendered = report.render();
    eprintln!("{rendered}");
    assert!(report.passed(), "{rendered}");
}
