//! A backup package supplies its own reference data: the `.igr` border
//! templates and the `.sym` symbol bodies inside its `RefData~*.zip`.
//!
//! A backup keeps reference data as archives beside `Export.dmp` rather than as
//! an exploded `Ref/` tree, so a drawing laid out against the package directory
//! has neither a template directory nor a symbol root to read — unless the
//! archives are read. This gate covers that path end to end, on the backup the
//! repository itself carries (`test-file/backup-test/DWG-0202GP06-01_p`), whose
//! package is one drawing's own:
//!
//! - `RefData~4~682.zip` holds eleven `.igr`, among them `CPECCHBA2.igr` — the
//!   template `test-file/DWG-0202GP06-01.pid` names in its link record, and one
//!   that is on no disk in this repository;
//! - `RefData~4~681.zip` holds 616 `.sym`, laid out as the same
//!   `Design/…`, `Piping/…`, `Equipment/…` tree a symbol root holds.
//!
//! Four readings, in the order a regression would show up:
//!
//! - the drawing wears the template it names, and the file it came from is a
//!   member of the archive rather than a file on disk;
//! - a second drawing of the same plant wears a template out of the same
//!   archive, while the one it *names* is refused — see below;
//! - with the archive out of reach, none of them wears anything: that is what
//!   makes the furniture above the archive's doing and not some disk copy;
//! - a symbol body resolves with **no** directory root at all, so the archive
//!   is the source of the strokes and not a fallback beside one.
//!
//! # The template that is named and still refused
//!
//! `test-file/DWG-0201GP06-01.pid` links `XIONGANA2.igr`, and that member is in
//! the archive. The page still wears `A2-W - New.igr`, because `XIONGANA2.igr`
//! carries no frame to match it against: the file reads as 6 entities — 2
//! `Other`, 1 `Symbol`, 3 `Text` (核准 / 校对 / 储运工艺, the sign-off column) —
//! with a 653-byte `Sheet6` that declares a sheet and holds no geometry. Its
//! own summary says what it is: `SmartSketch Application`, `TransAcad.igr` for
//! a template, `ACAD_GLOBALS.FileName = C:\Users\sp3d\Desktop\xionganA2.dwg`,
//! `File Version: SmartSketch(R) 6.*/RAD 4.2.` — a `.dwg` carried through
//! SmartSketch rather than a SmartPlant P&ID border. So the extent rule has
//! nothing to measure, and the page falls back to the unnamed A2 template whose
//! ink is 0.31 mm from its stated 594.31 × 420.31. The pairing the checklist
//! recorded ("`XIONGANA2.igr` is what `DWG-0201GP06-01` needs") is therefore
//! answered — the file is here — and still unsatisfied, for a reason that is
//! in the file rather than missing from the corpus.
//!
//! Soft-skips when the fixture backup is absent, which is the case on CI.

use std::path::{Path, PathBuf};

use pid_plot::model::{PaperModel, lay_out_drawing, lay_out_with_symbol_roots};

/// The repository's own fixtures.
fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../test-file")
}

/// The backup package this gate reads, if the fixture is present.
fn backup_dir() -> Option<PathBuf> {
    let dir = fixture_dir().join("backup-test/DWG-0202GP06-01_p");
    dir.is_dir().then_some(dir)
}

fn layout(drawing: &Path, corpus: &Path) -> PaperModel {
    lay_out_drawing(drawing, corpus)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()))
}

/// The template's file name, or a panic naming what was drawn instead.
fn template_file(model: &PaperModel, drawing: &str) -> String {
    let template = model
        .template
        .as_ref()
        .unwrap_or_else(|| panic!("{drawing}: no furniture was drawn"));
    template
        .path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_string()
}

/// Whether a reported path is an archive member rather than a file on disk.
fn from_an_archive(label: &str) -> bool {
    Path::new(label)
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|above| {
            above
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".zip")
        })
}

#[test]
fn a_backup_archive_draws_the_furniture_its_drawings_name() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the archive corpus; no fixture backup is present");
        return;
    };

    // The A2 drawing says `Ref\Template Files\CPECCHBA2.igr` in its link record
    // and states 593.75 x 419.63. The file is in the archive, whose own ink is
    // 594.00 x 420.00 — a 0.00 mm hit on the width, 0.37 on the height.
    let drawing = fixture_dir().join("DWG-0202GP06-01.pid");
    let model = layout(&drawing, &backup);
    let template = model.template.as_ref().expect("furniture");
    assert_eq!(template_file(&model, "DWG-0202GP06-01"), "CPECCHBA2.igr");
    assert!(
        from_an_archive(&template.path),
        "the template came from {} rather than an archive member",
        template.path
    );
    assert!(
        template.path.contains("RefData~4~682.zip"),
        "the A2 template is a member of {}",
        template.path
    );
    assert!(
        (template.extent_mm[0] - 594.00).abs() < 0.01,
        "{:?}",
        template.extent_mm
    );
    assert!(
        (template.extent_mm[1] - 420.00).abs() < 0.01,
        "{:?}",
        template.extent_mm
    );
    assert_eq!(template.primitives, 101);
    // Its title block places two symbols, both from the file's own cache: the
    // stored paths are temp names on the machine that authored the template.
    assert_eq!(template.symbols.len(), 2, "{:?}", template.symbols);
    for symbol in &template.symbols {
        assert!(symbol.name_gap.is_some(), "{symbol:?}");
        assert!(!symbol.library_available, "{symbol:?}");
    }
    assert!(
        model
            .library
            .iter()
            .any(|root| root.contains("RefData~4~681.zip")),
        "{:?}",
        model.library
    );
}

#[test]
fn a_template_the_sheet_names_but_the_file_cannot_supply_is_refused() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the archive corpus; no fixture backup is present");
        return;
    };
    // This drawing links `XIONGANA2.igr`, which the archive holds and which
    // reads as lettering and no frame, so the page takes the unnamed A2
    // template 0.31 mm from the stated 594.31 x 420.31 instead.
    let drawing = fixture_dir().join("DWG-0201GP06-01.pid");
    let model = layout(&drawing, &backup);
    let template = model.template.as_ref().expect("furniture");
    assert_eq!(
        template_file(&model, "DWG-0201GP06-01"),
        "A2-W - New.igr",
        "the linked template cannot supply a frame; the page wears the one whose ink fits"
    );
    assert!(from_an_archive(&template.path), "{}", template.path);
    assert!(
        (template.extent_mm[0] - 594.00).abs() < 0.01,
        "{:?}",
        template.extent_mm
    );
    assert_eq!(template.primitives, 101);
}

#[test]
fn without_the_archive_the_same_drawings_draw_no_furniture() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the archive corpus; no fixture backup is present");
        return;
    };
    // `test-file` itself: `test-file/Ref/Template Files` carries the A0 and A1
    // templates only, and no `RefData~*.zip` is in reach of it or its parent,
    // so an A2 page matches nothing. This is the negative control that makes
    // the two readings above the archive's doing.
    let bare = fixture_dir();
    for name in ["DWG-0202GP06-01.pid", "DWG-0201GP06-01.pid", "D06.pid"] {
        let drawing = bare.join(name);
        if !drawing.is_file() {
            continue;
        }
        let model = layout(&drawing, &bare);
        assert!(
            model.template.is_none(),
            "{name} drew furniture from {:?} with the archive out of reach",
            model.template.as_ref().map(|template| &template.path)
        );
    }
    // The same drawing with the package directory as its corpus does wear one,
    // and out of the archive — so the control is about what the archive
    // supplies rather than about the drawing being a frameless page.
    let drawing = bare.join("D06.pid");
    if drawing.is_file() {
        let with_archive = layout(&drawing, &backup);
        let template = with_archive
            .template
            .as_ref()
            .expect("D06 wears the backup's A2 template");
        assert!(from_an_archive(&template.path), "{}", template.path);
        assert!(
            template.path.contains("RefData~4~682.zip"),
            "{}",
            template.path
        );
    }
}

#[test]
fn a_symbol_body_resolves_from_the_archive_alone() {
    let Some(backup) = backup_dir() else {
        eprintln!("skipping the archive corpus; no fixture backup is present");
        return;
    };
    let archive = backup.join("RefData~4~681.zip");
    if !archive.is_file() {
        eprintln!(
            "skipping the archive symbol case; {} is absent",
            archive.display()
        );
        return;
    }
    let drawing = fixture_dir().join("DWG-0202GP06-01.pid");
    if !drawing.is_file() {
        return;
    }
    // No directory root at all — not even the bundled library, since the roots
    // are stated rather than discovered. Every body has to come out of the
    // archive, so anything drawn is proof the members were read and decoded.
    let label = vec![archive.display().to_string()];
    let model = lay_out_with_symbol_roots(&drawing, &backup, Vec::new(), vec![archive], &label)
        .unwrap_or_else(|error| panic!("{}: {error}", drawing.display()));
    assert!(
        model.symbols_found > 0,
        "no symbol resolved from the archive: {:?}",
        model.missing_symbols
    );
    assert!(
        model.symbols_referenced >= model.symbols_found,
        "{} found of {} referenced",
        model.symbols_found,
        model.symbols_referenced
    );
    assert_eq!(
        model.missing_boxes.len(),
        model.symbols_referenced - model.symbols_found,
        "every unplaced symbol should be one the archive lacks, not a placeholder"
    );
}
