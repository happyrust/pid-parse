//! Paper-space plots of a parsed `.pid` drawing.
//!
//! `pid-parse` stays a reader. This crate is the only workspace member that
//! links pdfcraft.

#![deny(
    missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links
)]

pub mod compare;
pub mod fonts;
pub mod ledger;
pub mod model;
pub mod read;
pub mod witness;
pub mod write;

#[cfg(test)]
mod scratch;

/// Confirms the pinned PDF toolchain and the parser are linked into this crate.
#[must_use]
pub fn pdf_toolchain_is_linked() -> bool {
    std::mem::size_of::<pdfcraft_cos::Object>() > 0
        && std::mem::size_of::<pdfcraft_content::Op>() > 0
        && std::mem::size_of::<pdfcraft_render::OpenError>() > 0
        && std::mem::size_of::<hayro::RenderSettings>() > 0
        && std::mem::size_of::<pid_parse::PidGraphicEntity>() > 0
        && pdfcraft_fonts::helvetica_width("M", 1.0) > 0.0
}

#[cfg(test)]
mod tests {
    use super::pdf_toolchain_is_linked;
    use super::scratch::ScratchDir;

    #[test]
    fn the_pinned_pdf_toolchain_and_the_parser_are_linked() {
        assert!(pdf_toolchain_is_linked());
    }

    /// A scratch directory is the caller's alone — a second one of the same
    /// name is another directory — and is gone, with what was put in it, once
    /// its guard is dropped.
    #[test]
    fn a_scratch_directory_is_its_tests_own_and_goes_with_its_guard() {
        let first = ScratchDir::new("scratch-own");
        let second = ScratchDir::new("scratch-own");
        assert_ne!(first.to_path_buf(), second.to_path_buf());
        assert!(first.is_dir() && second.is_dir());
        std::fs::create_dir_all(first.join("Ref").join("Template Files"))
            .expect("a nested directory");
        std::fs::write(first.join("Ref").join("kept.txt"), b"kept").expect("a file");

        let (first_path, second_path) = (first.to_path_buf(), second.to_path_buf());
        drop(first);
        assert!(
            !first_path.exists(),
            "{} outlived its guard",
            first_path.display()
        );
        assert!(
            second_path.is_dir(),
            "dropping one guard removed another's directory"
        );
        drop(second);
        assert!(
            !second_path.exists(),
            "{} outlived its guard",
            second_path.display()
        );
    }

    /// A test that fails unwinds through its guard, so a failed run leaves no
    /// more behind than a passing one.
    #[test]
    fn a_scratch_directory_goes_when_the_test_holding_it_fails() {
        let mut made = None;
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dir = ScratchDir::new("scratch-unwinds");
            std::fs::write(dir.join("kept.txt"), b"kept").expect("a file");
            made = Some(dir.to_path_buf());
            // What a failed assertion does, without the panic hook's message
            // in the output of a run that passes.
            std::panic::resume_unwind(Box::new("the assertion that fails"));
        }));
        assert!(outcome.is_err(), "the closure was to unwind");
        let path = made.expect("the directory was made before the unwind");
        assert!(!path.exists(), "{} outlived the unwind", path.display());
    }
}
