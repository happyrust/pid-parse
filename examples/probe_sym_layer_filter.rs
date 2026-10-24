//! Explain which layer rule drops each stroke of one `.sym` body.
//!
//! `SymbolGeometry::displayed_primitives` filters a library body by three
//! rules: the sheet must be one of the body's normal views, the sheet's
//! `Top ViewFilterSet` must show the stroke's layer number, and the layer's
//! name must not belong to the family the sheet keeps for itself (`Label`,
//! `Heat Trace`, `Jacket`, `Construction`, `Dimension`, `HiddenObjects`).
//!
//! When a body comes out nearly empty, which of the three fired is not
//! visible from the outside. This probe prints the layer table, every view
//! set, and one row per layer with its stroke count and each rule's answer.
//!
//! ```text
//! cargo run --example probe_sym_layer_filter -- <symbol.sym>
//! ```
//!
//! Read-only.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use pid_parse::symbol_library::{
    read_symbol_geometry, SymbolGeometry, SymbolPrimitive, SymbolSheetView,
};

const HIDDEN: &[&str] = &[
    "heattrace",
    "hiddenobjects",
    "jacket",
    "label",
    "construction",
    "dimension",
    "deminsion",
];

fn hidden(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    HIDDEN.contains(&normalized.as_str())
}

fn bit_is_set(bitmap: &[u8], n: u16) -> bool {
    bitmap
        .get(usize::from(n / 8))
        .is_some_and(|byte| byte & (1 << (n % 8)) != 0)
}

fn kind(primitive: &SymbolPrimitive) -> &'static str {
    match primitive {
        SymbolPrimitive::Line { .. } => "line",
        SymbolPrimitive::Circle { .. } => "circle",
        SymbolPrimitive::Arc { .. } => "arc",
        SymbolPrimitive::Polyline { .. } => "polyline",
        SymbolPrimitive::Text { .. } => "text",
        SymbolPrimitive::BSpline { .. } => "bspline",
        SymbolPrimitive::Boundary { .. } => "boundary",
    }
}

fn supplementary(view: &SymbolSheetView) -> bool {
    view.layers
        .iter()
        .any(|(number, name)| hidden(name) && bit_is_set(&view.display, *number))
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: probe_sym_layer_filter <symbol.sym>");
        return ExitCode::from(2);
    };
    let body: SymbolGeometry = match read_symbol_geometry(&PathBuf::from(&path)) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::from(1);
        }
    };

    println!(
        "{}: {} primitives, {} layers, {} views",
        path,
        body.primitives.len(),
        body.sheet_layers.len(),
        body.sheet_views.len()
    );

    println!("\nlayer table (oid, number, name):");
    for layer in &body.sheet_layers {
        println!(
            "  oid={:<6} number={:<4} name={:?}",
            layer.oid, layer.layer_number, layer.name
        );
    }

    println!("\nview sets:");
    for view in &body.sheet_views {
        let bits: Vec<String> = (0..view.display.len() * 8)
            .filter(|n| bit_is_set(&view.display, *n as u16))
            .map(|n| n.to_string())
            .collect();
        println!(
            "  Sheet{}: {} bytes, {} bits set [{}], supplementary={}, listed={:?}",
            view.sheet_ref,
            view.display.len(),
            bits.len(),
            bits.join(","),
            supplementary(view),
            view.layers
        );
    }

    // Which sheet the filter selects, repeated from `displayed_primitives`.
    let sheets: Vec<u32> = {
        let mut seen: Vec<u32> = body
            .primitives
            .iter()
            .map(|styled| styled.sheet_ref)
            .chain(body.sheet_views.iter().map(|view| view.sheet_ref))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    };
    let view_of = |sheet: u32| body.sheet_views.iter().find(|v| v.sheet_ref == sheet);
    let normal: Vec<u32> = sheets
        .iter()
        .copied()
        .filter(|sheet| view_of(*sheet).is_some_and(|view| !supplementary(view)))
        .collect();
    let unknown: Vec<u32> = sheets
        .iter()
        .copied()
        .filter(|sheet| view_of(*sheet).is_none())
        .collect();
    let selected: Vec<u32> = if !normal.is_empty() {
        normal
    } else if sheets.contains(&6) {
        vec![6]
    } else if !unknown.is_empty() {
        unknown
    } else {
        sheets.clone()
    };
    println!("\nselected sheets: {selected:?}");

    #[derive(Default)]
    struct Row {
        strokes: usize,
        kinds: BTreeMap<&'static str, usize>,
        shown: usize,
    }
    let mut rows: BTreeMap<(u32, u32, u32, String), Row> = BTreeMap::new();
    for styled in &body.primitives {
        let key = (
            styled.sheet_ref,
            styled.sheet_layer_ref,
            styled.sheet_layer_number,
            styled.sheet_layer_name.clone().unwrap_or_default(),
        );
        let row = rows.entry(key).or_default();
        row.strokes += 1;
        *row.kinds.entry(kind(&styled.primitive)).or_default() += 1;
    }
    // Re-walk for the shown count so the per-rule answers below match the code.
    for styled in &body.primitives {
        let on_sheet = selected.contains(&styled.sheet_ref);
        let name_hidden = styled.sheet_layer_name.as_deref().is_some_and(hidden);
        let answer = view_of(styled.sheet_ref)
            .and_then(|view| {
                u16::try_from(styled.sheet_layer_number)
                    .ok()
                    .map(|n| (view, n))
            })
            .map(|(view, n)| bit_is_set(&view.display, n));
        let shown = on_sheet && answer.unwrap_or(true) && !name_hidden;
        if shown {
            let key = (
                styled.sheet_ref,
                styled.sheet_layer_ref,
                styled.sheet_layer_number,
                styled.sheet_layer_name.clone().unwrap_or_default(),
            );
            rows.entry(key).or_default().shown += 1;
        }
    }

    println!("\nper layer:");
    let mut dropped_by_view = 0usize;
    let mut dropped_by_name = 0usize;
    let mut dropped_by_sheet = 0usize;
    let mut shown_total = 0usize;
    for ((sheet, oid, number, name), row) in &rows {
        let on_sheet = selected.contains(sheet);
        let name_hidden = !name.is_empty() && hidden(name);
        let answer = view_of(*sheet).and_then(|view| {
            u16::try_from(*number)
                .ok()
                .map(|n| bit_is_set(&view.display, n))
        });
        let shown = row.shown;
        shown_total += shown;
        if !on_sheet {
            dropped_by_sheet += row.strokes;
        } else if name_hidden {
            dropped_by_name += row.strokes;
        } else if answer == Some(false) {
            dropped_by_view += row.strokes;
        }
        println!(
            "  Sheet{sheet} layer oid={oid} number={number} name={name:?}: {} strokes {:?}, view={}, hidden_name={}, shown={}",
            row.strokes,
            row.kinds,
            match answer {
                Some(true) => "on",
                Some(false) => "OFF",
                None => "no answer",
            },
            name_hidden,
            shown
        );
    }
    println!(
        "\nshown {shown_total} of {}: dropped by sheet {} / view-off {} / hidden name {}",
        body.primitives.len(),
        dropped_by_sheet,
        dropped_by_view,
        dropped_by_name
    );
    ExitCode::SUCCESS
}
