//! The page `pid_plot` writes for a drawing, for a gate that holds a page it
//! wrote itself to that one.
//!
//! A gate that reads a page, or keeps one, writes it with the library: lay
//! the drawing out, prepare its faces, write. `pid_plot` does the same for
//! one drawing, and the pages that are handed over are that program's.
//! Whether a gate's page is that page turns on how its faces were prepared.
//! Prepared for several sheets together a face is a subset cut for all of
//! them, with other glyph codes, and the page letters and strokes the same
//! and is other bytes. So it is held, not said: the program is run on the
//! drawing and the two pages are compared byte for byte.
//!
//! `reference_plots_acceptance` holds the eleven pages it delivers this way,
//! with a function of its own. This is the same check for the gates that say
//! they read those pages (`reference_plots_ink`) or that keep pages under a
//! corpus's `plot` (`write_five_drawings`).
//!
//! Included by path: `#[path = "support/exported_page.rs"] mod exported_page;`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The page `pid_plot` writes for `drawing` laid out against `corpus`, read
/// back from `out`: the program run as a process, the way a sheet is
/// exported.
///
/// The program looks for its faces in `PID_PLOT_FONTS` and then in the
/// corpus. It is handed `dirs` — the directories the caller prepared its own
/// faces from, in the caller's order — as that list, so it reads the files
/// the caller read. `PID_PLOT_CJK_FONT`, which has it embed one face more, is
/// taken out of its environment: the page is the one the program writes with
/// no such face named.
pub fn pid_plot_page(drawing: &Path, corpus: &Path, dirs: &[PathBuf], out: &Path) -> Vec<u8> {
    let fonts = std::env::join_paths(dirs).expect("the font directories as one list");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_pid_plot"))
        .arg(drawing)
        .arg(out)
        .arg(corpus)
        .env("PID_PLOT_FONTS", fonts)
        .env_remove("PID_PLOT_CJK_FONT")
        .output()
        .unwrap_or_else(|error| panic!("{}: pid_plot did not start: {error}", drawing.display()));
    assert!(
        output.status.success(),
        "{}: pid_plot exited {:?}:\n{}",
        drawing.display(),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read(out).unwrap_or_else(|error| panic!("{}: {error}", out.display()))
}

/// Hold `ours`, the page a gate wrote for the drawing `name`, to `exported`,
/// the page [`pid_plot_page`] read for it: the same bytes, or a panic that
/// says what the gate does with the page (`reads`, `keeps under plot`), how
/// long each page is and at which byte they first part.
pub fn assert_is_the_page_pid_plot_writes(name: &str, does: &str, ours: &[u8], exported: &[u8]) {
    assert!(
        ours == exported,
        "{name}: the page this gate {does} is not the bytes pid_plot writes for the drawing: {} \
         bytes here, {} from pid_plot, first apart at byte {}",
        ours.len(),
        exported.len(),
        ours.iter()
            .zip(exported)
            .position(|(here, there)| here != there)
            .unwrap_or_else(|| ours.len().min(exported.len()))
    );
}
