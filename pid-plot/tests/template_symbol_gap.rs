//! A template symbol whose path names no reference share is accounted for by
//! file name, and that account keeps two different gaps apart.
//!
//! `BLFH.sym` — the `比例` cell's wave — is the case: the placement stores
//! `C:\Users\…\AppData\Local\Temp\BLFH.sym`, so no library root can be
//! addressed with it, and the page draws the template's cached body. What the
//! page says about that gap is what tells a later residual "the corpus lacks
//! this file" from "the corpus carries this name twice and the library refuses
//! to pick one" — the second is not a missing source.
//!
//! Soft-skips when the corpus or the drawing is absent, which is what CI sees.

use std::path::{Path, PathBuf};

use pid_parse::symbol_library::NameMiss;
use pid_plot::model::{SymbolBodySource, lay_out_with_symbol_roots};

const A1_DRAWING: &str = "集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图";

fn corpus_dir() -> PathBuf {
    std::env::var_os("PID_PARSE_SELF_CHECK_DIR").map_or_else(
        || PathBuf::from(r"D:\work\cad\PID自研软件验证"),
        PathBuf::from,
    )
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-file")
}

fn scratch_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let path = std::env::temp_dir().join(format!(
        "pid-plot-template-gap-{tag}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("scratch root");
    path
}

/// The one `BLFH.sym` placement the A1 template carries.
fn blfh(model: &pid_plot::model::PaperModel) -> &pid_plot::model::TemplateSymbol {
    let template = model
        .template
        .as_ref()
        .expect("the A1 sheet has a template");
    template
        .symbols
        .iter()
        .find(|symbol| {
            symbol
                .key
                .rsplit(['\\', '/'])
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("BLFH.sym"))
        })
        .unwrap_or_else(|| panic!("no BLFH.sym placement: {:?}", template.symbols))
}

#[test]
fn the_gap_tells_a_missing_file_from_a_name_the_library_carries_twice() {
    let corpus = corpus_dir();
    let drawing = corpus.join(format!("{A1_DRAWING}.pid"));
    if !drawing.is_file() {
        eprintln!(
            "skipping template symbol gap; {} is absent",
            drawing.display()
        );
        return;
    }
    let circle = fixture_dir().join("symbols/Design/Annotation/Graphics/Circle.sym");
    if !circle.is_file() {
        eprintln!(
            "skipping template symbol gap; {} is absent",
            circle.display()
        );
        return;
    }

    let lay_out = |root: &Path| {
        let label = vec![root.display().to_string()];
        lay_out_with_symbol_roots(&drawing, &corpus, vec![root.to_path_buf()], &label)
            .unwrap_or_else(|error| panic!("layout: {error}"))
    };

    // A root that holds nothing: the name is simply not in the library.
    let root = scratch_root("absent");
    let model = lay_out(&root);
    let symbol = blfh(&model);
    assert_eq!(symbol.body, SymbolBodySource::Cached, "the cache drew it");
    assert!(!symbol.library_available);
    assert_eq!(symbol.name_gap, Some(NameMiss::NoSuchFile));

    // Two folders of one root carry the name: refused, not guessed at.
    for folder in ["First", "Second"] {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).expect("folder");
        std::fs::copy(&circle, dir.join("BLFH.sym")).expect("copied body");
    }
    let model = lay_out(&root);
    let symbol = blfh(&model);
    assert_eq!(symbol.body, SymbolBodySource::Cached, "the cache drew it");
    assert!(
        !symbol.library_available,
        "two candidates are not one answer"
    );
    assert_eq!(symbol.name_gap, Some(NameMiss::NotUnique));

    // One of them left: the name resolves, and the gap is no longer a gap.
    // The page still draws the cache — the cached definition is tried first —
    // so this is the library's account, not a change of ink.
    std::fs::remove_file(root.join("Second/BLFH.sym")).expect("removed the duplicate");
    let model = lay_out(&root);
    let symbol = blfh(&model);
    assert_eq!(symbol.body, SymbolBodySource::Cached);
    assert!(symbol.library_available, "the one remaining copy answers");
    assert_eq!(symbol.name_gap, None);

    std::fs::remove_dir_all(&root).ok();
}
