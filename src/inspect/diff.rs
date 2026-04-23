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

/// Render a [`PackageDiff`] as a human-readable report. Always ends with
/// a trailing newline.
///
/// Output is assembled via [`String::push_str`] (and `push_str(&format!(..))`
/// for interpolated lines). We deliberately avoid `write!` / `writeln!` into
/// `&mut String` here because those trait-based writes return `fmt::Result`
/// that can never fail for `String` — the extra `.unwrap()` noise obscures
/// the actual rendering logic without any value.
pub fn render(diff: &PackageDiff) -> String {
    let mut out = String::new();
    out.push_str("=== Package Diff ===\n");

    if diff.is_empty() {
        out.push_str("(no differences)\n");
        out.push_str("  streams match:  yes\n");
        out.push_str("  root CLSID:     match\n");
        return out;
    }

    let clsid_status = if diff.root_clsid_match {
        "match"
    } else {
        "DIFFER"
    };
    out.push_str(&format!(
        "root CLSID:  {}  (a={}, b={})\n",
        clsid_status,
        render_clsid(diff.root_clsid_a),
        render_clsid(diff.root_clsid_b),
    ));

    out.push_str(&format!(
        "summary:     {} diff(s) — {} only-in-a / {} only-in-b / {} modified\n",
        diff.diff_count(),
        diff.only_in_a.len(),
        diff.only_in_b.len(),
        diff.modified.len(),
    ));

    if !diff.only_in_a.is_empty() {
        out.push_str("\n--- Only in A ---\n");
        for p in &diff.only_in_a {
            out.push_str(&format!("  {}\n", p));
        }
    }

    if !diff.only_in_b.is_empty() {
        out.push_str("\n--- Only in B ---\n");
        for p in &diff.only_in_b {
            out.push_str(&format!("  {}\n", p));
        }
    }

    if !diff.modified.is_empty() {
        out.push_str("\n--- Modified Streams ---\n");
        for m in &diff.modified {
            out.push_str(&format!(
                "  {}  len={} vs {}  first_diff@0x{:X}\n",
                m.path, m.len_a, m.len_b, m.first_mismatch_offset,
            ));
            out.push_str(&format!("    a: {}\n", m.context_before));
            out.push_str(&format!("    b: {}\n", m.context_after));
        }
    }

    if !diff.storage_clsid_diffs.is_empty() {
        out.push_str("\n--- Non-root Storage CLSID Diffs ---\n");
        for s in &diff.storage_clsid_diffs {
            out.push_str(&format!(
                "  {}  a={}  b={}\n",
                s.path,
                render_clsid(s.a),
                render_clsid(s.b),
            ));
        }
    }

    if !diff.storage_timestamp_diffs.is_empty() {
        writeln!(out, "\n--- Storage Timestamp Diffs ---").unwrap();
        for t in &diff.storage_timestamp_diffs {
            let a_c = t.a.as_ref().and_then(|ts| ts.created);
            let a_m = t.a.as_ref().and_then(|ts| ts.modified);
            let b_c = t.b.as_ref().and_then(|ts| ts.created);
            let b_m = t.b.as_ref().and_then(|ts| ts.modified);
            writeln!(
                out,
                "  {}  created  a={}  b={}",
                t.path,
                render_time(a_c),
                render_time(b_c)
            )
            .unwrap();
            writeln!(
                out,
                "  {}  modified a={}  b={}",
                t.path,
                render_time(a_m),
                render_time(b_m)
            )
            .unwrap();
        }
    }

    if !diff.state_bits_diffs.is_empty() {
        writeln!(out, "\n--- State Bits Diffs ---").unwrap();
        for s in &diff.state_bits_diffs {
            writeln!(
                out,
                "  {}  a={}  b={}",
                s.path,
                render_state_bits(s.a),
                render_state_bits(s.b),
            )
            .unwrap();
        }
    }

    out
}

fn render_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        None => "(none)".to_string(),
        Some(st) => {
            // Render as seconds-since-UNIX_EPOCH so the output is both
            // stable and human-comparable without pulling a formatting
            // crate. Negative values (pre-1970) fall back to `Debug`.
            match st.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => format!("unix+{}s", d.as_secs()),
                Err(_) => format!("{:?}", st),
            }
        }
    }
}

fn render_state_bits(b: Option<u32>) -> String {
    match b {
        None => "(none)".to_string(),
        Some(v) => format!("0x{:08X}", v),
    }
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
