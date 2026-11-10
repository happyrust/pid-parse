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
//! The same holds for the corpus the pages are laid out from. `run_gate`
//! skips the count comparison, and still passes, when this run's symbol or
//! template sources are not the ones the baseline names. That is the right
//! answer for a corpus each machine holds its own copy of, and the wrong one
//! here, where a clean checkout always yields the same sources. A different
//! list means a template added under `test-file/Ref/Template Files`, an
//! archive gone from `test-file/backup-test`, or a stray `Ref` directory or
//! `RefData~*.zip` beside `test-file/`, where the template search also looks.
//! So this gate requires the comparison to have happened
//! (`GateReport::counts_compared`), not only the report to pass.
//!
//! # What is asserted
//!
//! 1. every page assembles, and its five layers of each domain balance
//!    (`unattributed == 0`);
//! 2. `cross_check` finds nothing — the ledger agrees with the page model
//!    (requirements 8.1–8.4 and 8.7) and with the geometry tables (8.5);
//! 3. laying the same page out twice, and serializing twice, gives the same
//!    bytes: the account is deterministic;
//! 4. no serialized account, and not the corpus descriptor stored above them,
//!    carries a machine path — the repository root, `CARGO_MANIFEST_DIR` or
//!    the user's home directory, a path on any drive, or a UNC prefix, each
//!    read both as the path itself and as JSON escapes it. A baseline may
//!    hold counts and stream names, never a machine path;
//! 5. the corpus descriptor is the one
//!    `docs/baselines/export-ledger-fixtures.json` states, and the counts
//!    were compared with that file's, key by key, and match. With
//!    `UPDATE_EXPORT_LEDGER=1` the file is rewritten instead and nothing is
//!    compared.
//!
//! # What is not asserted here
//!
//! Two checks hold the ledger against a page that was actually written, and
//! this gate writes none:
//!
//! - requirement 5.6, the writer's verdicts against what it wrote
//!   (`WriteLedger::mismatches`). Only `write_plot_audited` observes its own
//!   writes; `write_ledger` returns the list empty by construction;
//! - requirement 8.6, the two font marks against the writer's `PlotAudit`.
//!   `cross_check` is handed no audit here and leaves 8.6 out.
//!
//! Both are asserted where pages are written with their faces:
//! `export_plant_backup` over the 53 backup pages, `export_every_fixture` over
//! these seven fixtures. Those gates need a font directory, which a bare
//! runner lacks.

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
/// input: the archives exist in every checkout. A `backup-test` that cannot
/// be read, or that yields no archive, is a broken checkout — and laying the
/// pages out without the archives would hand the gate a corpus its baseline
/// was never taken from — so this fails instead of returning nothing.
fn archives(corpus: &Path) -> Vec<PathBuf> {
    let backup = corpus.join("backup-test");
    let entries = std::fs::read_dir(&backup).unwrap_or_else(|error| {
        panic!(
            "{} holds the committed backup packages and cannot be read: {error}",
            backup.display()
        )
    });
    let mut packages: Vec<PathBuf> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("{}: {error}", backup.display()))
                .path()
        })
        .collect();
    packages.sort();
    let mut found: Vec<PathBuf> = Vec::new();
    for package in packages {
        if !package.is_dir() {
            continue;
        }
        found.extend(pid_parse::refdata_archive::refdata_archives(&package));
    }
    found.sort();
    found.dedup();
    assert!(
        !found.is_empty(),
        "{} holds the committed backup packages, and none of them yielded a RefData~*.zip",
        backup.display()
    );
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

/// The canonical serialization of what a baseline stores — one page's
/// account, or the corpus descriptor above the pages.
fn render(value: &impl serde::Serialize) -> String {
    let mut json = serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| panic!("a baseline value is serializable: {error}"));
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

/// `fragment` as JSON text spells it inside a string: every backslash
/// doubled, quotes and control characters escaped.
///
/// Taken from the serializer the accounts are rendered with rather than
/// rebuilt by hand, so the spelling searched for is the spelling written.
fn json_spelling(fragment: &str) -> String {
    let quoted = serde_json::to_string(fragment)
        .unwrap_or_else(|error| panic!("a string is serializable: {error}"));
    quoted
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(&quoted)
        .to_string()
}

/// What a finding starts with, for the failure message: `rest` up to the end
/// of its JSON string, and no more than 60 characters of it.
fn excerpt(rest: &str) -> String {
    rest.chars()
        .take_while(|character| *character != '"' && !character.is_control())
        .take(60)
        .collect()
}

/// Every machine path `text` carries, one line per distinct finding; empty
/// when it carries none (requirement 12.4).
///
/// `text` is JSON as `render` writes it, and a path can be spelled two ways
/// there: as the path itself, or as JSON writes it, with every backslash
/// doubled. A search for the first spelling alone finds no Windows directory
/// in JSON text, so each rule reads both:
///
/// - each of `fragments` — the directories this machine would leak — as given
///   and as `json_spelling` writes it;
/// - a path on a drive: one ASCII letter of either case, a colon, then `\` or
///   `/`. JSON's `\\` begins with that same `\`, so the one pattern covers both
///   spellings;
/// - a UNC prefix: two backslashes, then the first character of a server name
///   or the `?` or `.` of a verbatim or device path. JSON text writes the two
///   as four, and four in a row are a prefix wherever they stand. Two in a row
///   are also how JSON writes a single separator, so they are read as a prefix
///   only where a path can open: at the start of the text, or after a quote or
///   whitespace. A string that opens with a lone backslash is spelled the same
///   way there and is reported too; an account holds neither.
///
/// Nothing an account legitimately holds matches: stream paths (`/Sheet6`) and
/// page keys put no colon before their `/`, and type codes (`0x00FA`), archive
/// names (`RefData~4~682.zip`) and template names (`A0-W - New.igr`) carry
/// neither a colon nor a backslash.
fn machine_paths(text: &str, fragments: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut report = |finding: String| {
        if !found.contains(&finding) {
            found.push(finding);
        }
    };
    for fragment in fragments {
        if text.contains(fragment.as_str()) || text.contains(json_spelling(fragment).as_str()) {
            report(format!("this machine's directory {fragment}"));
        }
    }
    let mut previous: Option<char> = None;
    for (at, current) in text.char_indices() {
        let rest = &text[at..];
        let mut after = rest.chars().skip(1);
        if current.is_ascii_alphabetic()
            && after.next() == Some(':')
            && matches!(after.next(), Some('\\' | '/'))
        {
            report(format!("a path on a drive: {}", excerpt(rest)));
        }
        if current == '\\' && previous != Some('\\') {
            let run = rest
                .chars()
                .take_while(|character| *character == '\\')
                .count();
            let names_a_server = rest.chars().nth(run).is_some_and(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '?' | '.')
            });
            let opens_a_path =
                previous.is_none_or(|before| before == '"' || before.is_whitespace());
            if names_a_server && (run >= 4 || (run >= 2 && opens_a_path)) {
                report(format!("a UNC prefix: {}", excerpt(rest)));
            }
        }
        previous = Some(current);
    }
    found
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
        // `write.mismatches` is not asserted: here it is empty by construction.
        // A mismatch is the writer finding that it did not act on its own
        // verdict (requirement 5.6), and only `write_plot_audited` writes
        // anything to hold a verdict against; `write_ledger` reaches the
        // verdicts and stops. The check is made where a page is written:
        // `export_plant_backup` asserts it for the 53 backup pages and
        // `export_every_fixture` for these seven fixtures.
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
        // No audit is handed over, so requirement 8.6 — the two font marks
        // against the writer's `PlotAudit` — is not run here: an audit exists
        // only for a page written with its faces, and this gate loads none.
        // `export_plant_backup` and `export_every_fixture` run 8.6 on the
        // pages they write.
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
        let leaked = machine_paths(&first_json, &forbidden);
        assert!(
            leaked.is_empty(),
            "{key}: the account carries a machine path:\n  {}",
            leaked.join("\n  ")
        );

        ledgers.push(first.ledger.clone());
        pages.push((key, ledger));
    }

    pages.sort_by(|left, right| left.0.cmp(&right.0));
    let refs: Vec<&pid_plot::ledger::LayoutLedger> = ledgers.iter().collect();
    let descriptor = descriptor_for(&refs, Vec::new());
    // The descriptor is the baseline's header, so it answers to the same rule
    // as the accounts stored under it.
    let leaked = machine_paths(&render(&descriptor), &forbidden);
    assert!(
        leaked.is_empty(),
        "the corpus descriptor carries a machine path:\n  {}",
        leaked.join("\n  ")
    );

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
    // A clean checkout always yields the same sources, so the descriptor this
    // run built has to be the baseline's own. `run_gate` passes a run whose
    // descriptor differs without comparing a count; that pass is refused here.
    // A rewrite compares nothing either, and is what it was asked to do.
    assert!(
        update || report.counts_compared(),
        "the fixture gate passed without comparing a single count: the symbol or template \
         sources this run laid out are not the ones the baseline names (both lists are below), \
         and a count taken over one corpus says nothing about another. A clean checkout always \
         yields the baseline's sources, so one was added, removed or renamed under test-file/ — \
         or sits beside it: the template search also reads Ref/ and RefData~*.zip in the \
         repository root. If the change is not intended, undo it. If the corpus was changed on \
         purpose, delete docs/baselines/export-ledger-fixtures.json and rebuild it with \
         UPDATE_EXPORT_LEDGER=1 — the variable alone is not enough, because the gate refuses to \
         rewrite a baseline taken over another corpus.\n{rendered}"
    );
}

/// `value` the way a serialized account carries it: a string of a JSON
/// document, escaped by the serializer the accounts are rendered with.
fn in_an_account(value: &str) -> String {
    render(&serde_json::json!({ "name": value }))
}

#[test]
fn a_drive_path_is_caught_in_either_spelling_and_on_any_letter() {
    // As the path itself.
    assert_eq!(
        machine_paths(r"D:\work\cad\D06.pid", &[]),
        [r"a path on a drive: D:\work\cad\D06.pid"]
    );
    // As JSON writes it. This is the spelling a serialized account really
    // holds, so it is the one the gate meets.
    let account = in_an_account(r"D:\work\cad\D06.pid");
    assert!(account.contains(r"D:\\work\\cad\\D06.pid"), "{account}");
    assert_eq!(
        machine_paths(&account, &[]),
        [r"a path on a drive: D:\\work\\cad\\D06.pid"]
    );

    // Either case, any letter, either separator, in both spellings.
    for path in [
        r"d:\work\cad",
        r"E:\plant",
        r"z:\plant",
        "D:/work/cad",
        "c:/Users/somebody",
        "Q:/plant",
    ] {
        assert_eq!(
            machine_paths(path, &[]),
            [format!("a path on a drive: {path}")]
        );
        let account = in_an_account(path);
        let escaped = json_spelling(path);
        assert_eq!(
            machine_paths(&account, &[]),
            [format!("a path on a drive: {escaped}")],
            "{account}"
        );
    }

    // Wherever it stands in a longer string: after a space, or straight after
    // the letter of a JSON escape.
    assert_eq!(
        machine_paths(&in_an_account("cannot read D:/work/cad/A1.igr"), &[]),
        ["a path on a drive: D:/work/cad/A1.igr"]
    );
    let account = in_an_account("no such template\nD:/work/cad/A1.igr");
    assert!(account.contains(r"template\nD:/work"), "{account}");
    assert_eq!(
        machine_paths(&account, &[]),
        ["a path on a drive: D:/work/cad/A1.igr"]
    );
}

#[test]
fn a_unc_prefix_is_caught_in_either_spelling() {
    // As the path itself: two backslashes.
    let share = r"\\fileserver\plant\A1.igr";
    assert_eq!(
        machine_paths(share, &[]),
        [format!("a UNC prefix: {share}")]
    );
    // As JSON writes it: four.
    let account = in_an_account(share);
    assert!(
        account.contains(r"\\\\fileserver\\plant\\A1.igr"),
        "{account}"
    );
    assert_eq!(
        machine_paths(&account, &[]),
        [r"a UNC prefix: \\\\fileserver\\plant\\A1.igr"]
    );

    // Wherever it stands in a longer string: after a space, or straight after
    // the letter of a JSON escape.
    for message in [
        r"cannot read \\fileserver\plant\A1.igr",
        "no such template\n\\\\fileserver\\plant\\A1.igr",
    ] {
        let account = in_an_account(message);
        assert_eq!(
            machine_paths(&account, &[]),
            [r"a UNC prefix: \\\\fileserver\\plant\\A1.igr"],
            "{account}"
        );
    }

    // One backslash is a separator, which JSON spells as two: not a prefix.
    let account = in_an_account(r"Ref\Symbols");
    assert!(account.contains(r"Ref\\Symbols"), "{account}");
    assert_eq!(
        machine_paths(&account, &[]),
        Vec::<String>::new(),
        "{account}"
    );

    // What `std::fs::canonicalize` returns on Windows is both: a verbatim
    // prefix, then a drive.
    assert_eq!(
        machine_paths(r"\\?\D:\work\cad", &[]),
        [
            r"a UNC prefix: \\?\D:\work\cad",
            r"a path on a drive: D:\work\cad"
        ]
    );
}

#[test]
fn this_machines_directories_are_caught_in_either_spelling() {
    // A POSIX directory has no shape of its own to go by: it is caught because
    // it is this machine's, and only then.
    let checkout = "/home/somebody/pid-parse".to_string();
    let account = in_an_account("/home/somebody/pid-parse/test-file/D06.pid");
    assert_eq!(
        machine_paths(&account, std::slice::from_ref(&checkout)),
        ["this machine's directory /home/somebody/pid-parse"]
    );
    assert_eq!(
        machine_paths(&account, &[]),
        Vec::<String>::new(),
        "{account}"
    );

    // A Windows directory reaches a serialized account with its backslashes
    // doubled, so the directory as given is not in the text at all; it is
    // found by its JSON spelling.
    let home = r"C:\Users\somebody".to_string();
    let account = in_an_account(r"C:\Users\somebody\.cargo\A1.igr");
    assert!(!account.contains(home.as_str()), "{account}");
    assert_eq!(
        machine_paths(&account, std::slice::from_ref(&home)),
        [
            r"this machine's directory C:\Users\somebody",
            r"a path on a drive: C:\\Users\\somebody\\.cargo\\A1.igr"
        ]
    );

    // The directories this machine would really leak, in both spellings.
    let forbidden = forbidden_fragments();
    for fragment in &forbidden {
        let finding = format!("this machine's directory {fragment}");
        for text in [fragment.clone(), in_an_account(fragment)] {
            assert!(
                machine_paths(&text, &forbidden).contains(&finding),
                "{finding} was not found in {text}"
            );
        }
    }
}

#[test]
fn what_an_account_holds_is_not_a_machine_path() {
    let forbidden = forbidden_fragments();
    let nothing: Vec<String> = Vec::new();
    // Stream paths, type codes, family names, archive names, template names
    // and page keys, each as itself and as an account serializes it.
    for held in [
        "/Sheet6",
        "/JSite145",
        "0x00FA",
        "SubRecord0x0010",
        "RefData~4~682.zip",
        "A0-W - New.igr",
        "CPECCHB-SQ-A2.igr",
        "export-test/publish-data/A01/A01.pid",
        "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid",
    ] {
        for text in [held.to_string(), in_an_account(held)] {
            assert_eq!(machine_paths(&text, &forbidden), nothing, "{text}");
        }
    }

    // The shapes those values take inside an account, pretty-printed the way a
    // baseline stores them and compact, where only quotes stand between a
    // one-letter key, its colon and the `/` its value opens with.
    let account = serde_json::json!({
        "page": {
            "template": { "name": "A0-W - New.igr", "archive": "RefData~4~682.zip" }
        },
        "furniture": {
            "parse": {
                "items": [
                    { "reason": "record_no_decoder", "stream": "/Sheet6", "type_code": "0x00FA" },
                    { "reason": "record_symbol_body_cache", "stream": "/JSite16", "count": 31 }
                ]
            }
        },
        "a": "/Sheet6"
    });
    let compact = serde_json::to_string(&account)
        .unwrap_or_else(|error| panic!("a JSON value is serializable: {error}"));
    assert!(compact.contains(r#""a":"/Sheet6""#), "{compact}");
    for text in [render(&account), compact] {
        assert_eq!(machine_paths(&text, &forbidden), nothing, "{text}");
    }

    // JSON's own escapes carry one backslash, never a run of them: a quote and
    // a control character inside a name are not a path.
    let escapes = in_an_account("/\u{5}SummaryInformation of \"A1\"");
    assert!(
        escapes.contains(r#"/\u0005SummaryInformation of \"A1\""#),
        "{escapes}"
    );
    assert_eq!(machine_paths(&escapes, &forbidden), nothing, "{escapes}");
}
