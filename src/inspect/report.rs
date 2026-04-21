use crate::model::PidDocument;
use crate::package::PidPackage;
use crate::parsers::magic;
use std::fmt::Write;

/// Phase 10f (v0.6.5+): render a byte count with a K/M/G suffix using
/// a 1024-binary progression, kept to one decimal place. Pure integer
/// math below 1 KB to keep the common "< 1 KB" case clean; anything
/// larger reports e.g. `"1.5 KB"` / `"3.2 MB"`.
fn format_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if n < KB {
        format!("{n} B")
    } else if n < MB {
        format!("{:.1} KB", n as f64 / KB as f64)
    } else if n < GB {
        format!("{:.1} MB", n as f64 / MB as f64)
    } else {
        format!("{:.1} GB", n as f64 / GB as f64)
    }
}

/// Package-aware report. Starts with [`generate_report`] and appends
/// container-level metadata (root CLSID + non-root storage CLSIDs) that
/// only [`PidPackage`] carries. Prefer this over [`generate_report`]
/// when you already have a [`PidPackage`] at hand.
pub fn generate_package_report(pkg: &PidPackage) -> String {
    let mut out = generate_report(&pkg.parsed);
    if pkg.root_clsid.is_some() || !pkg.storage_clsids.is_empty() {
        writeln!(out, "\n--- Container CLSIDs ---").ok();
        if let Some(c) = pkg.root_clsid {
            writeln!(out, "  root: {{{}}}", c).ok();
        } else {
            writeln!(out, "  root: (nil — source container had no CLSID)").ok();
        }
        if pkg.storage_clsids.is_empty() {
            writeln!(out, "  non-root storages: (none carry a CLSID)").ok();
        } else {
            writeln!(out, "  non-root storages ({}):", pkg.storage_clsids.len()).ok();
            for (path, clsid) in &pkg.storage_clsids {
                writeln!(out, "    {}  {{{}}}", path, clsid).ok();
            }
        }
    }
    out
}

pub fn generate_report(doc: &PidDocument) -> String {
    let mut out = String::new();

    writeln!(out, "=== PID Document Report ===\n").ok();

    writeln!(out, "Streams: {}", doc.streams.len()).ok();
    writeln!(out, "JSites:  {}", doc.jsites.len()).ok();
    writeln!(out, "Clusters: {}", doc.clusters.len()).ok();
    writeln!(out, "Sheet streams: {}", doc.sheet_streams.len()).ok();
    writeln!(out, "Unknown streams: {}", doc.unknown_streams.len()).ok();

    if let Some(ref si) = doc.summary {
        writeln!(out, "\n--- Summary ---").ok();
        if let Some(ref v) = si.creating_application {
            writeln!(out, "  Application: {}", v).ok();
        }
        if let Some(ref v) = si.title {
            writeln!(out, "  Title: {}", v).ok();
        }
        if let Some(ref v) = si.template {
            writeln!(out, "  Template: {}", v).ok();
        }
        if let Some(ref v) = si.created_time {
            writeln!(out, "  Created: {}", v).ok();
        }
        if let Some(ref v) = si.modified_time {
            writeln!(out, "  Modified: {}", v).ok();
        }
        for (k, v) in &si.raw {
            writeln!(out, "  {}: {}", k, v).ok();
        }
    }

    if let Some(ref dm) = doc.drawing_meta {
        writeln!(out, "\n--- Drawing Meta ---").ok();
        if let Some(ref v) = dm.drawing_number {
            writeln!(out, "  DrawingNumber: {}", v).ok();
        }
        if let Some(ref v) = dm.document_category {
            writeln!(out, "  DocumentCategory: {}", v).ok();
        }
        if let Some(ref v) = dm.template_name {
            writeln!(out, "  Template: {}", v).ok();
        }
        for (k, v) in &dm.tags {
            writeln!(out, "  {}: {}", k, v).ok();
        }
    }

    if let Some(ref gm) = doc.general_meta {
        writeln!(out, "\n--- General Meta ---").ok();
        if let Some(ref v) = gm.file_path {
            writeln!(out, "  FilePath: {}", v).ok();
        }
        if let Some(ref v) = gm.file_size {
            writeln!(out, "  FileSize: {}", v).ok();
        }
    }

    if !doc.jsites.is_empty() {
        writeln!(out, "\n--- JSites ---").ok();
        for js in &doc.jsites {
            write!(out, "  {} ", js.name).ok();
            if let Some(ref sym) = js.symbol_name {
                write!(out, "[sym: {}]", sym).ok();
            }
            if js.has_ole_stream {
                write!(out, " [OLE]").ok();
            }
            if !js.properties.guids.is_empty() {
                write!(out, " [GUIDs: {}]", js.properties.guids.len()).ok();
            }
            writeln!(out).ok();
        }

        let total_guids: usize = doc.jsites.iter().map(|j| j.properties.guids.len()).sum();
        if total_guids > 0 {
            writeln!(out, "  Total GUIDs across JSites: {}", total_guids).ok();
        }
    }

    if !doc.clusters.is_empty() {
        writeln!(out, "\n--- Clusters ---").ok();
        for c in &doc.clusters {
            write!(out, "  {} ({} bytes, {:?})", c.name, c.size, c.kind).ok();
            if let Some(ref hdr) = c.header {
                write!(
                    out,
                    " [hdr: type=0x{:04X}, records={}, body={}]",
                    hdr.stream_type, hdr.record_count, hdr.body_len
                )
                .ok();
            }
            writeln!(out).ok();

            if let Some(ref pi) = c.probe_info {
                writeln!(
                    out,
                    "    [PROBE] table_offset=0x{:04X}, method={}, entries={}, end=0x{:04X}",
                    pi.string_table_offset, pi.detection_method, pi.entries_parsed, pi.end_offset
                )
                .ok();
            }
            if let Some(ref table) = c.string_table {
                writeln!(out, "    String table ({} entries):", table.len()).ok();
                for entry in table.iter().take(10) {
                    writeln!(out, "      [{}] {}", entry.index, entry.value).ok();
                }
                if table.len() > 10 {
                    writeln!(out, "      ... ({} more)", table.len() - 10).ok();
                }
            }
        }
    }

    if let Some(ref da) = doc.dynamic_attributes {
        writeln!(out, "\n--- Dynamic Attributes ---").ok();
        writeln!(out, "  Size: {} bytes", da.size).ok();
        if let Some(ref hdr) = da.header {
            writeln!(
                out,
                "  Header: type=0x{:04X}, records={}, body={}",
                hdr.stream_type, hdr.record_count, hdr.body_len
            )
            .ok();
        }
        writeln!(out, "  Strings: {}", da.strings.len()).ok();
        writeln!(out, "  Relationships: {}", da.relationships.len()).ok();
        writeln!(out, "  Class names: {:?}", da.class_names).ok();
        if let Some(ref ps) = da.probe_summary {
            writeln!(
                out,
                "  [PROBE] body_start=0x{:04X}, markers={}, records={}, bytes_scanned={}",
                ps.body_start_offset, ps.marker_count, ps.records_extracted, ps.bytes_scanned
            )
            .ok();
        }
        if !da.attribute_records.is_empty() {
            writeln!(
                out,
                "  Attribute records: {} classes [EXPERIMENTAL/heuristic]",
                da.attribute_records.len()
            )
            .ok();
            for rec in &da.attribute_records {
                writeln!(
                    out,
                    "    {} ({} attrs) [{}]",
                    rec.class_name,
                    rec.attributes.len(),
                    rec.confidence
                )
                .ok();
                for attr in rec.attributes.iter().take(5) {
                    writeln!(out, "      {}: {:?}", attr.name, attr.value).ok();
                }
                if rec.attributes.len() > 5 {
                    writeln!(out, "      ... ({} more)", rec.attributes.len() - 5).ok();
                }
            }
        }
    }

    if !doc.sheet_streams.is_empty() {
        writeln!(out, "\n--- Sheets ---").ok();
        for sh in &doc.sheet_streams {
            write!(out, "  {} ({} bytes", sh.name, sh.size).ok();
            if let Some(m) = sh.magic_u32_le {
                if let Some(ref tag) = sh.magic_tag {
                    write!(out, ", magic=0x{:08X} '{}'", m, tag).ok();
                } else {
                    write!(out, ", magic=0x{:08X}", m).ok();
                }
            }
            writeln!(out, ")").ok();

            if let Some(ref hdr) = sh.header {
                writeln!(
                    out,
                    "    header: type=0x{:04X}, records={}, body={}",
                    hdr.stream_type, hdr.record_count, hdr.body_len
                )
                .ok();
            }
            if let Some(ref ps) = sh.probe_summary {
                writeln!(
                    out,
                    "    [PROBE] body_start=0x{:04X}, markers={}, records={}, bytes_scanned={}",
                    ps.body_start_offset, ps.marker_count, ps.records_extracted, ps.bytes_scanned
                )
                .ok();
            }
            if !sh.attribute_records.is_empty() {
                writeln!(
                    out,
                    "    Attribute records: {} [EXPERIMENTAL/heuristic]",
                    sh.attribute_records.len()
                )
                .ok();
                for rec in sh.attribute_records.iter().take(5) {
                    writeln!(
                        out,
                        "      {} ({} attrs)",
                        rec.class_name,
                        rec.attributes.len()
                    )
                    .ok();
                }
                if sh.attribute_records.len() > 5 {
                    writeln!(out, "      ... ({} more)", sh.attribute_records.len() - 5).ok();
                }
            }
        }
    }

    if let Some(ref r) = doc.psm_roots {
        writeln!(out, "\n--- PSMroots ({} bytes) ---", r.size).ok();
        for e in &r.entries {
            writeln!(out, "  [@+{:04X}] id=0x{:08X}  {}", e.offset, e.id, e.name).ok();
        }
        if r.trailing_bytes > 0 {
            writeln!(out, "  ({} trailing bytes)", r.trailing_bytes).ok();
        }
    }

    if let Some(ref t) = doc.psm_cluster_table {
        writeln!(
            out,
            "\n--- PSMclustertable ({} bytes, declared count={}) ---",
            t.size, t.count
        )
        .ok();
        for e in &t.entries {
            write!(
                out,
                "  [@+{:04X}] {} (rec_len={}, name@{:04X}",
                e.record_offset, e.name, e.record_len, e.name_offset
            )
            .ok();
            if !e.prefix_bytes.is_empty() {
                let hex: Vec<String> = e.prefix_bytes.iter().map(|b| format!("{:02X}", b)).collect();
                write!(out, ", prefix=[{}]", hex.join(" ")).ok();
            }
            writeln!(out, ")").ok();
        }
        if t.entries.len() as u32 != t.count {
            writeln!(
                out,
                "  [WARN] extracted {} names but declared count {}",
                t.entries.len(),
                t.count
            )
            .ok();
        }
        if t.trailing_bytes > 0 {
            writeln!(out, "  ({} trailing bytes)", t.trailing_bytes).ok();
        }
    }

    if let Some(ref t) = doc.psm_segment_table {
        writeln!(
            out,
            "\n--- PSMsegmenttable ({} bytes, count={}) ---",
            t.size, t.count
        )
        .ok();
        if !t.entries.is_empty() {
            for e in t.entries.iter().take(20) {
                writeln!(
                    out,
                    "  [{}] @+{:04X} flag=0x{:02X}",
                    e.index, e.offset, e.flag
                )
                .ok();
            }
            if t.entries.len() > 20 {
                writeln!(out, "  ... ({} more)", t.entries.len() - 20).ok();
            }
        } else {
            let flags_hex: Vec<String> =
                t.flags.iter().map(|b| format!("0x{:02X}", b)).collect();
            writeln!(out, "  flags: [{}]", flags_hex.join(", ")).ok();
        }
        if t.trailing_bytes > 0 {
            writeln!(out, "  ({} trailing bytes)", t.trailing_bytes).ok();
        }
    }

    if let Some(ref vh) = doc.version_history {
        writeln!(
            out,
            "\n--- Version History ({} bytes, {} records, record_size={}) ---",
            vh.size,
            vh.records.len(),
            vh.record_size,
        )
        .ok();
        for r in &vh.records {
            let raw_suffix = if r.is_recognized_operation() {
                String::new()
            } else {
                format!(" ({})", r.operation)
            };
            writeln!(
                out,
                "  [@+{:04X}] [{}{} {}] {} {}",
                r.offset,
                r.operation_label(),
                raw_suffix,
                r.timestamp,
                r.product,
                r.version,
            )
            .ok();
        }
        if vh.trailing_bytes > 0 {
            writeln!(out, "  ({} trailing bytes)", vh.trailing_bytes).ok();
        }
    }

    if let Some(ref dv2) = doc.doc_version2_decoded {
        writeln!(
            out,
            "\n--- DocVersion2 (decoded, magic=0x{:08X}, {} records) ---",
            dv2.magic_u32_le,
            dv2.records.len()
        )
        .ok();
        if !dv2.reserved_all_zero {
            writeln!(out, "  (!) reserved header bytes are not all zero").ok();
        }
        for r in &dv2.records {
            let label = crate::parsers::doc_version2::op_type_label(r.op_type);
            writeln!(
                out,
                "  [{}] version={} (0x{:X})",
                label, r.version, r.version
            )
            .ok();
        }
    }

    if let Some(ref reg) = doc.app_object_registry {
        writeln!(
            out,
            "\n--- App Object Registry ({} bytes, leading=0x{:08X}, {} entries) ---",
            reg.size,
            reg.leading_u32,
            reg.entries.len()
        )
        .ok();
        for e in &reg.entries {
            writeln!(out, "  {} -> {}", e.clsid, e.path).ok();
        }
        if reg.trailing_bytes > 0 {
            writeln!(out, "  ({} trailing bytes)", reg.trailing_bytes).ok();
        }
    }

    if let Some(ref t) = doc.tagged_storages {
        writeln!(out, "\n--- Tagged Text Storage List ({} bytes) ---", t.size).ok();
        writeln!(out, "  list: {}", t.list_name).ok();
        for e in &t.entries {
            writeln!(out, "    -> {}", e.storage_name).ok();
        }
    }

    if let Some(ref d2) = doc.doc_version2 {
        writeln!(
            out,
            "\n--- DocVersion2 ({} bytes, magic=0x{:08X}, raw) ---",
            d2.size, d2.magic_u32_le
        )
        .ok();
        writeln!(out, "  hex: {}", d2.hex_preview).ok();
    }

    if let Some(ref g) = doc.object_graph {
        writeln!(out, "\n--- Object Graph ---").ok();
        if let Some(ref p) = g.project_number {
            writeln!(out, "  Project: {}", p).ok();
        }
        if let Some(ref d) = g.drawing_no {
            writeln!(out, "  Drawing: {}", d).ok();
        }
        writeln!(
            out,
            "  Objects: {}  Relationships: {}",
            g.objects.len(),
            g.relationships.len()
        )
        .ok();
        writeln!(out, "  By type:").ok();
        for (ty, n) in &g.counts_by_type {
            writeln!(out, "    {}: {}", ty, n).ok();
        }
        writeln!(out, "  Sample objects:").ok();
        for obj in g.objects.iter().take(6) {
            let sub = obj
                .drawing_item_type
                .as_deref()
                .map(|s| format!(" [{}]", s))
                .unwrap_or_default();
            writeln!(out, "    {} {}{}", obj.item_type, obj.drawing_id, sub).ok();
        }
        if g.objects.len() > 6 {
            writeln!(out, "    ... ({} more)", g.objects.len() - 6).ok();
        }
        if !g.relationships.is_empty() {
            let fully = g
                .relationships
                .iter()
                .filter(|r| r.source_drawing_id.is_some() && r.target_drawing_id.is_some())
                .count();
            let partial = g
                .relationships
                .iter()
                .filter(|r| r.source_drawing_id.is_some() ^ r.target_drawing_id.is_some())
                .count();
            let unresolved = g.relationships.len() - fully - partial;
            writeln!(
                out,
                "  Endpoint resolution: {} fully / {} partial / {} unresolved",
                fully, partial, unresolved
            )
            .ok();
            writeln!(out, "  Sample relationships:").ok();
            for rel in g.relationships.iter().take(4) {
                let src = rel.source_drawing_id.as_deref().unwrap_or("?");
                let tgt = rel.target_drawing_id.as_deref().unwrap_or("?");
                let guid = if rel.guid.is_empty() {
                    "(template)".to_string()
                } else {
                    rel.guid.clone()
                };
                writeln!(out, "    {}  {} -> {}", guid, src, tgt).ok();
            }
            if g.relationships.len() > 4 {
                writeln!(out, "    ... ({} more)", g.relationships.len() - 4).ok();
            }
        }
    }

    // Phase 10a (v0.6.0): structured coverage inventory. Written BEFORE
    // the legacy "Top-level Unidentified Streams" section so readers see
    // the new categorical view first, and the two remain cross-linked
    // for backward compatibility.
    // Phase 10f (v0.6.5+): each entry shows its byte footprint so users
    // can prioritize "large + unknown" streams first.
    let coverage = crate::inspect::coverage::coverage_report(doc);
    if !coverage.entries.is_empty() {
        writeln!(out, "\n--- Coverage ---").ok();
        let [full, partial, ident, unk] = coverage.status_counts();
        let [full_b, partial_b, ident_b, unk_b] = coverage.total_bytes_by_status();
        writeln!(
            out,
            "  Fully decoded:     {full} ({})",
            format_bytes(full_b)
        )
        .ok();
        writeln!(
            out,
            "  Partially decoded: {partial} ({})",
            format_bytes(partial_b)
        )
        .ok();
        writeln!(
            out,
            "  Identified only:   {ident} ({})",
            format_bytes(ident_b)
        )
        .ok();
        writeln!(out, "  Unknown:           {unk} ({})", format_bytes(unk_b)).ok();
        for entry in &coverage.entries {
            let tag = match entry.status {
                crate::model::ParseCoverageStatus::FullyDecoded => "[FULL]",
                crate::model::ParseCoverageStatus::PartiallyDecoded => "[PART]",
                crate::model::ParseCoverageStatus::IdentifiedOnly => "[ID]  ",
                crate::model::ParseCoverageStatus::Unknown => "[UNK] ",
            };
            let field = entry
                .document_field
                .as_deref()
                .map(|f| format!(" -> {f}"))
                .unwrap_or_default();
            let size = entry
                .stream_size
                .map(|sz| format!(" ({})", format_bytes(sz)))
                .unwrap_or_default();
            let note = entry
                .note
                .as_deref()
                .map(|n| format!("  ({n})"))
                .unwrap_or_default();
            writeln!(out, "  {tag} {}{}{}{}", entry.name, field, size, note).ok();
        }
    }

    let top_level_unidentified = crate::inspect::unidentified_top_level_streams(doc);
    if !top_level_unidentified.is_empty() {
        writeln!(out, "\n--- Top-level Unidentified Streams ---").ok();
        for s in top_level_unidentified {
            write!(out, "  {} ({} bytes", s.path, s.size).ok();
            if let Some(m) = s.magic_u32_le {
                write!(out, ", magic=0x{:08X}", m).ok();
                if let Some(tag) = magic::magic_tag(m) {
                    write!(out, " '{}'", tag).ok();
                }
                let desc = magic::describe_magic(m);
                if !desc.is_empty() {
                    write!(out, " [{}]", desc).ok();
                }
            }
            writeln!(out, ")").ok();
        }
    }

    if let Some(ref inv) = doc.object_inventory {
        writeln!(out, "\n--- P&ID Object Inventory ---").ok();
        if let Some(ref proj) = inv.project {
            writeln!(out, "  Project: {}", proj).ok();
        }
        if let Some(ref did) = inv.drawing_id {
            writeln!(out, "  Drawing: {}", did).ok();
        }
        writeln!(out, "  Total items: {}", inv.items.len()).ok();
        for (item_type, count) in &inv.item_counts {
            writeln!(out, "    {}: {}", item_type, count).ok();
        }
    }

    if let Some(ref xr) = doc.cross_reference {
        writeln!(out, "\n--- Cross Reference ---").ok();

        let cov = &xr.cluster_coverage;
        writeln!(
            out,
            "  Clusters: declared={} found={} matched={}",
            cov.declared.len(),
            cov.found.len(),
            cov.matched.len()
        )
        .ok();
        if !cov.declared_missing.is_empty() {
            writeln!(
                out,
                "    [WARN] declared but missing: {}",
                cov.declared_missing.join(", ")
            )
            .ok();
        }
        if !cov.found_extra.is_empty() {
            writeln!(
                out,
                "    [INFO] found but not declared: {}",
                cov.found_extra.join(", ")
            )
            .ok();
        }

        if !xr.symbol_usage.is_empty() {
            writeln!(
                out,
                "  Symbols: {} unique ({} total JSite refs)",
                xr.symbol_usage.len(),
                xr.symbol_usage.iter().map(|u| u.usage_count).sum::<usize>()
            )
            .ok();
            for u in xr.symbol_usage.iter().take(5) {
                let basename = u
                    .symbol_name
                    .clone()
                    .unwrap_or_else(|| u.symbol_path.clone());
                writeln!(
                    out,
                    "    [{}x] {} ({} ...)",
                    u.usage_count,
                    basename,
                    u.jsite_names
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(",")
                )
                .ok();
            }
            if xr.symbol_usage.len() > 5 {
                writeln!(out, "    ... ({} more)", xr.symbol_usage.len() - 5).ok();
            }
        }

        if !xr.attribute_classes.is_empty() {
            writeln!(out, "  Attribute classes: {}", xr.attribute_classes.len()).ok();
            for c in &xr.attribute_classes {
                writeln!(
                    out,
                    "    {} (records={}, attr_names={}, drawings={}, models={})",
                    c.class_name,
                    c.record_count,
                    c.unique_attribute_names.len(),
                    c.drawing_ids.len(),
                    c.model_ids.len()
                )
                .ok();
            }
        }

        if !xr.root_presence.is_empty() {
            let resolved = xr
                .root_presence
                .iter()
                .filter(|r| r.found_as_storage || r.found_as_stream)
                .count();
            writeln!(
                out,
                "  PSMroots: {} entries, {} resolved in CFB tree",
                xr.root_presence.len(),
                resolved
            )
            .ok();
            for r in &xr.root_presence {
                let marker = if r.found_as_storage {
                    "STORAGE"
                } else if r.found_as_stream {
                    "STREAM "
                } else {
                    "MISSING"
                };
                writeln!(out, "    [{}] id=0x{:08X}  {}", marker, r.id, r.name).ok();
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PidDocument, PsmSegmentTable, StreamEntry, VersionHistory, VersionRecord};

    fn doc_with_paths(paths: &[&str]) -> PidDocument {
        PidDocument {
            streams: paths
                .iter()
                .map(|p| StreamEntry {
                    path: (*p).to_string(),
                    size: 42,
                    preview_ascii: vec![],
                    magic_u32_le: None,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn report_includes_coverage_section_with_bucket_counts_and_per_entry_tags() {
        // Phase 10a: `generate_report` must surface the coverage
        // inventory ahead of the legacy "Top-level Unidentified
        // Streams" section. Phase 10b: the dynamic classifier needs
        // the corresponding model fields populated for the static
        // FullyDecoded / PartiallyDecoded verdict to stand — otherwise
        // the entry gets downgraded to IdentifiedOnly (that's by
        // design; see `coverage_downgrades_docversion3_when_parser_did_not_populate`).
        let mut doc = doc_with_paths(&[
            "/DocVersion3",     // Fully decoded (requires version_history)
            "/PSMsegmenttable", // Partially decoded (requires psm_segment_table)
            "/Sheet1/Payload",  // Identified only (via Sheet storage prefix)
            "/GhostStream",     // Unknown
        ]);
        doc.version_history = Some(VersionHistory {
            size: 48,
            records: vec![VersionRecord {
                product: "TestProduct".into(),
                version: "0.0.1".into(),
                operation: "SA".into(),
                timestamp: "01/01/26 00:00".into(),
                offset: 0,
            }],
            record_size: 48,
            trailing_bytes: 0,
        });
        doc.psm_segment_table = Some(PsmSegmentTable {
            size: 0,
            count: 0,
            flags: vec![],
            entries: vec![],
            trailing_bytes: 0,
        });
        let report = generate_report(&doc);
        assert!(
            report.contains("--- Coverage ---"),
            "coverage section heading missing; full report:\n{report}"
        );
        // Phase 10f: bucket summary lines now carry a byte total in
        // parens; fixture uses StreamEntry.size = 42 per stream, so
        // every single-stream bucket reports "(42 B)". Sheet storage
        // sums one child of 42 B as well.
        assert!(report.contains("Fully decoded:     1 (42 B)"), "{report}");
        assert!(report.contains("Partially decoded: 1 (42 B)"), "{report}");
        assert!(report.contains("Identified only:   1 (42 B)"), "{report}");
        assert!(report.contains("Unknown:           1 (42 B)"), "{report}");
        assert!(report.contains("[FULL] DocVersion3"), "{report}");
        assert!(report.contains("[PART] PSMsegmenttable"), "{report}");
        assert!(report.contains("[ID]   Sheet1"), "{report}");
        assert!(report.contains("[UNK]  GhostStream"), "{report}");
        // Phase 10f: per-entry byte tag is present inline.
        assert!(
            report.contains("[UNK]  GhostStream (42 B)"),
            "entry-level byte tag missing; {report}"
        );
    }

    #[test]
    fn report_omits_coverage_section_when_document_has_no_streams() {
        let doc = PidDocument::default();
        let report = generate_report(&doc);
        assert!(
            !report.contains("--- Coverage ---"),
            "empty doc should not render coverage section; got:\n{report}"
        );
    }

    #[test]
    fn report_version_history_uses_operation_label_instead_of_raw_code() {
        // Phase 10d: the Version History section must render the human
        // label produced by `VersionRecord::operation_label`
        // ("SaveAs"/"Save"/"unknown") instead of the raw 2-char code
        // ("SA"/"SV"). Unknown codes keep the raw value in parens.
        let doc = PidDocument {
            version_history: Some(VersionHistory {
                size: 48 * 3,
                records: vec![
                    VersionRecord {
                        product: "SmartPlantPID.a".into(),
                        version: "090000.0144".into(),
                        operation: "SA".into(),
                        timestamp: "12/29/25 10:45".into(),
                        offset: 0,
                    },
                    VersionRecord {
                        product: "SmartPlantPID.a".into(),
                        version: "090000.0144".into(),
                        operation: "SV".into(),
                        timestamp: "12/30/25 09:12".into(),
                        offset: 48,
                    },
                    VersionRecord {
                        product: "SmartPlantPID.a".into(),
                        version: "090000.0144".into(),
                        operation: "XY".into(),
                        timestamp: "01/01/26 00:00".into(),
                        offset: 96,
                    },
                ],
                record_size: 48,
                trailing_bytes: 0,
            }),
            ..Default::default()
        };
        let report = generate_report(&doc);
        assert!(
            report.contains("[SaveAs 12/29/25 10:45]"),
            "SaveAs label missing; report:\n{report}"
        );
        assert!(
            report.contains("[Save 12/30/25 09:12]"),
            "Save label missing; report:\n{report}"
        );
        assert!(
            report.contains("[unknown (XY) 01/01/26 00:00]"),
            "unknown + raw-code suffix missing; report:\n{report}"
        );
        assert!(
            !report.contains("[SA 12/29/25"),
            "raw 'SA' form still appears; report:\n{report}"
        );
        assert!(
            report.contains("record_size=48"),
            "record_size missing from heading; report:\n{report}"
        );
        assert!(
            report.contains("[@+0000]"),
            "record offset missing; report:\n{report}"
        );
    }

    #[test]
    fn report_coverage_section_precedes_top_level_unidentified_when_both_present() {
        let doc = doc_with_paths(&["/GhostStream"]);
        let report = generate_report(&doc);
        let cov_pos = report.find("--- Coverage ---").expect("Coverage section");
        let unk_pos = report
            .find("--- Top-level Unidentified Streams ---")
            .expect("Unidentified section");
        assert!(
            cov_pos < unk_pos,
            "Coverage must render before the legacy Unidentified section"
        );
    }
}
