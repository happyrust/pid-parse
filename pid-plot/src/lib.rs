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
pub mod model;
pub mod read;
pub mod write;

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

    #[test]
    fn the_pinned_pdf_toolchain_and_the_parser_are_linked() {
        assert!(pdf_toolchain_is_linked());
    }
}
