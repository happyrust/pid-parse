//! Human-readable renderer for [`crate::package::PackageDiff`].
//!
//! Produces a multi-section plain-text report covering:
//! - Root CLSID status (match / mismatch)
//! - Stream paths present in only one side
//! - Per-stream byte diff (first mismatch offset + 16-byte hex context)
//!
//! The output is deliberately stable and line-oriented so it diffs well
//! in version control or CI logs. It stays under a page for passthrough
//! round-trips ("no diffs") and scales with the number of modified
//! streams otherwise.
use crate::package::PackageDiff;
use std::fmt::Write;

/// Render a [`PackageDiff`] as a human-readable report. Always ends with
/// a trailing newline.
pub fn render(diff: &PackageDiff) -> String {
    let mut out = String::new();
    writeln!(out, "=== Package Diff ===").unwrap();

    if diff.is_empty() {
        writeln!(out, "(no differences)").unwrap();
        writeln!(out, "  streams match:  yes").unwrap();
        writeln!(out, "  root CLSID:     match").unwrap();
        return out;
    }

    let clsid_status = if diff.root_clsid_match { "match" } else { "DIFFER" };
    writeln!(
        out,
        "root CLSID:  {}  (a={}, b={})",
        clsid_status,
        render_clsid(diff.root_clsid_a),
        render_clsid(diff.root_clsid_b),
    )
    .unwrap();

    writeln!(
        out,
        "summary:     {} diff(s) — {} only-in-a / {} only-in-b / {} modified",
        diff.diff_count(),
        diff.only_in_a.len(),
        diff.only_in_b.len(),
        diff.modified.len(),
    )
    .unwrap();

    if !diff.only_in_a.is_empty() {
        writeln!(out, "\n--- Only in A ---").unwrap();
        for p in &diff.only_in_a {
            writeln!(out, "  {}", p).unwrap();
        }
    }

    if !diff.only_in_b.is_empty() {
        writeln!(out, "\n--- Only in B ---").unwrap();
        for p in &diff.only_in_b {
            writeln!(out, "  {}", p).unwrap();
        }
    }

    if !diff.modified.is_empty() {
        writeln!(out, "\n--- Modified Streams ---").unwrap();
        for m in &diff.modified {
            writeln!(
                out,
                "  {}  len={} vs {}  first_diff@0x{:X}",
                m.path, m.len_a, m.len_b, m.first_mismatch_offset,
            )
            .unwrap();
            writeln!(out, "    a: {}", m.context_before).unwrap();
            writeln!(out, "    b: {}", m.context_after).unwrap();
        }
    }

    if !diff.storage_clsid_diffs.is_empty() {
        writeln!(out, "\n--- Non-root Storage CLSID Diffs ---").unwrap();
        for s in &diff.storage_clsid_diffs {
            writeln!(
                out,
                "  {}  a={}  b={}",
                s.path,
                render_clsid(s.a),
                render_clsid(s.b),
            )
            .unwrap();
        }
    }

    out
}

fn render_clsid(c: Option<uuid::Uuid>) -> String {
    match c {
        Some(uuid) => format!("{{{}}}", uuid),
        None => "(none)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{diff_packages, PidPackage};
    use crate::PidDocument;
    use std::collections::BTreeMap;

    fn sample_doc() -> PidDocument {
        PidDocument::default()
    }

    #[test]
    fn render_empty_diff_shows_no_differences() {
        let pkg = PidPackage::new(None, BTreeMap::new(), sample_doc());
        let d = diff_packages(&pkg, &pkg);
        let out = render(&d);
        assert!(out.contains("(no differences)"));
        assert!(out.contains("streams match:  yes"));
    }

    #[test]
    fn render_includes_only_in_a_and_only_in_b_sections() {
        let mut a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        a.replace_stream("/foo", vec![1]);
        let mut b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        b.replace_stream("/bar", vec![2]);
        let out = render(&diff_packages(&a, &b));
        assert!(out.contains("--- Only in A ---"));
        assert!(out.contains("  /foo"));
        assert!(out.contains("--- Only in B ---"));
        assert!(out.contains("  /bar"));
    }

    #[test]
    fn render_modified_stream_has_hex_context() {
        let mut a = PidPackage::new(None, BTreeMap::new(), sample_doc());
        a.replace_stream("/s", vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let mut b = PidPackage::new(None, BTreeMap::new(), sample_doc());
        b.replace_stream("/s", vec![1, 2, 3, 99, 5, 6, 7, 8]);
        let out = render(&diff_packages(&a, &b));
        assert!(out.contains("first_diff@0x3"));
        assert!(out.contains("04 05 06 07 08"), "A context:\n{out}");
        assert!(out.contains("63 05 06 07 08"), "B context:\n{out}");
    }
}
