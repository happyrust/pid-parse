use crate::model::PidDocument;
use std::fmt::Write;

pub fn generate_report(doc: &PidDocument) -> String {
    let mut out = String::new();

    writeln!(out, "=== PID Document Report ===\n").ok();

    writeln!(out, "Streams: {}", doc.streams.len()).ok();
    writeln!(out, "JSites:  {}", doc.jsites.len()).ok();
    writeln!(out, "Clusters: {}", doc.clusters.len()).ok();
    writeln!(
        out,
        "Sheet streams: {}",
        doc.sheet_streams.len()
    )
    .ok();
    writeln!(
        out,
        "Unknown streams: {}",
        doc.unknown_streams.len()
    )
    .ok();

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
                writeln!(out, "    [PROBE] table_offset=0x{:04X}, method={}, entries={}, end=0x{:04X}",
                    pi.string_table_offset, pi.detection_method, pi.entries_parsed, pi.end_offset).ok();
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
            writeln!(out, "  [PROBE] body_start=0x{:04X}, markers={}, records={}, bytes_scanned={}",
                ps.body_start_offset, ps.marker_count, ps.records_extracted, ps.bytes_scanned).ok();
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

    out
}
