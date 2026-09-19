//! Does the parametric chain close on a cached body?
//!
//! Three families of the definition cache have been read separately and
//! never joined end to end:
//!
//! * `0x00BD JSymbolInformation` names variables (`Left` / `Right` / …) and
//!   each variable's `0x00C7 Double Value`; `0x00EA Variables` groups the
//!   values; `0x006F Standard Relation` carries a formula whose first
//!   operand is the output and whose other operands are the inputs
//!   (`2026-08-27-the-recordless-182-referrers-are-symbolinformation.md`:
//!   the output is a `0x0115 JDim` on 13 of 13 relations).
//! * `0x0115 JDim` now decodes: a value in metres, the line it measures,
//!   the group it belongs to
//!   (`2026-09-14-jdim-is-a-framed-record-whose-blocks-follow-the-dimension-kind.md`,
//!   `2026-09-15-tag-188-members-land-in-jdim-reference-slots.md`).
//! * The bodies themselves: `2026-09-07-placement-tail-names-the-cached-definition.md`
//!   reads the `Parametric Manifold` placement of `DWG-0201` as naming
//!   `/JSite396` sheet 113 (Imagineer Document), whose two arcs are
//!   r = 35.59 mm against the library's 20.32 mm default, and `/JSite329`
//!   sheet 49 as the unplaced library-default template.
//!
//! Plan J3 asks whether these are one story: template dimension = 20.32,
//! instance dimension = 35.59, the instance's arcs sitting on the
//! dimension's endpoints, and every tag-188 edge a `JDim -> constrained
//! geometry` edge that lands on a concrete `igLine2d`. This probe lays the
//! chain out per storage and per body, with no sampling, and lets the
//! numbers say which parts hold.
//!
//! Since plan K1 (2026-09-19) the pairing and the names live on the
//! projection -- `PidSymbolDefinition::template` / `::variables`,
//! `PidSymbolDimension::name` / `::formula` -- and section 4 reads them
//! from there; sections 2 and 3 still join the raw records, as the
//! evidence the projection was built on.
//!
//! ```powershell
//! cargo run --example probe_parametric_chain_resolves_a_cached_body
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pid_parse::model::{JSite, PidDocument};
use pid_parse::PidParser;

const FIXTURES: [&str; 5] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
    "test-file/export-test/publish-data/A01/A01.pid",
];

fn mm(m: f64) -> String {
    format!("{:.3}", m * 1000.0)
}

const INCH_M: f64 = 0.0254;

/// Evaluate a `Standard Relation` formula over its inputs.
///
/// The corpus writes `0E$1`, `0E$1+0.01`, `0E$1+0.1`, `0E($1+$2)/10` and
/// `0E$1/2`: a `0E` prefix, `$n` for the n-th input, decimal constants and
/// `+ - * /` with parentheses. Whatever unit the inputs are handed in is
/// the unit the constants are read in -- which is the question section 2
/// puts to the numbers.
fn eval_formula(formula: &str, inputs: &[f64]) -> Option<f64> {
    struct P<'a> {
        s: &'a [u8],
        i: usize,
        inputs: &'a [f64],
    }
    impl P<'_> {
        fn peek(&self) -> Option<u8> {
            self.s.get(self.i).copied()
        }
        fn expr(&mut self) -> Option<f64> {
            let mut v = self.term()?;
            while let Some(op @ (b'+' | b'-')) = self.peek() {
                self.i += 1;
                let r = self.term()?;
                v = if op == b'+' { v + r } else { v - r };
            }
            Some(v)
        }
        fn term(&mut self) -> Option<f64> {
            let mut v = self.factor()?;
            while let Some(op @ (b'*' | b'/')) = self.peek() {
                self.i += 1;
                let r = self.factor()?;
                v = if op == b'*' { v * r } else { v / r };
            }
            Some(v)
        }
        fn factor(&mut self) -> Option<f64> {
            match self.peek()? {
                b'(' => {
                    self.i += 1;
                    let v = self.expr()?;
                    (self.peek()? == b')').then(|| self.i += 1)?;
                    Some(v)
                }
                b'-' => {
                    self.i += 1;
                    Some(-self.factor()?)
                }
                b'$' => {
                    self.i += 1;
                    let start = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.i += 1;
                    }
                    let n: usize = std::str::from_utf8(&self.s[start..self.i])
                        .ok()?
                        .parse()
                        .ok()?;
                    self.inputs.get(n.checked_sub(1)?).copied()
                }
                _ => {
                    let start = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
                        self.i += 1;
                    }
                    std::str::from_utf8(&self.s[start..self.i])
                        .ok()?
                        .parse()
                        .ok()
                }
            }
        }
    }
    let body = formula.strip_prefix("0E")?;
    let mut p = P {
        s: body.as_bytes(),
        i: 0,
        inputs,
    };
    let v = p.expr()?;
    (p.i == body.len()).then_some(v)
}

fn site_id(site: &JSite) -> Option<u32> {
    site.name.strip_prefix("JSite")?.parse().ok()
}

/// `(definition site, definition sheet) -> (symbol names, placement count)`
/// across every top-level Sheet stream.
fn placements(doc: &PidDocument) -> BTreeMap<(u32, u32), (BTreeSet<String>, usize)> {
    let symbol_of_site: BTreeMap<u32, String> = doc
        .jsites
        .iter()
        .filter_map(|site| {
            Some((
                site_id(site)?,
                site.symbol_name
                    .clone()
                    .unwrap_or_else(|| "<no .sym>".to_string()),
            ))
        })
        .collect();
    let mut out: BTreeMap<(u32, u32), (BTreeSet<String>, usize)> = BTreeMap::new();
    for sheet in &doc.sheet_streams {
        let Some(geometry) = sheet.geometry.as_ref() else {
            continue;
        };
        for placement in &geometry.decoded_igsymbols {
            let entry = out
                .entry((
                    placement.definition_site_ref,
                    placement.definition_sheet_ref,
                ))
                .or_default();
            entry.0.insert(
                symbol_of_site
                    .get(&placement.jsite_ref)
                    .cloned()
                    .unwrap_or_else(|| format!("<JSite{} unknown>", placement.jsite_ref)),
            );
            entry.1 += 1;
        }
    }
    out
}

fn section_1_placements(doc: &PidDocument) {
    println!("\n=== 1. which cached body each placed symbol names ===");
    for ((site, sheet), (names, count)) in placements(doc) {
        let storage = doc
            .jsites
            .iter()
            .find(|candidate| site_id(candidate) == Some(site));
        let body = storage
            .and_then(|s| s.nested_geometry.as_ref())
            .and_then(|nested| nested.definition(sheet));
        let summary = match (storage.and_then(|s| s.nested_geometry.as_ref()), body) {
            (Some(nested), Some(definition)) => {
                let on = |layer: u32| definition.layers.binary_search(&layer).is_ok();
                format!(
                    "{} arcs {:?} mm, {} circles {:?} mm, {} lines, {} dimensions {:?} mm",
                    nested.arcs.iter().filter(|a| on(a.sheet_layer_ref)).count(),
                    nested
                        .arcs
                        .iter()
                        .filter(|a| on(a.sheet_layer_ref))
                        .map(|a| mm(a.radius))
                        .collect::<Vec<_>>(),
                    nested
                        .circles
                        .iter()
                        .filter(|c| on(c.sheet_layer_ref))
                        .count(),
                    nested
                        .circles
                        .iter()
                        .filter(|c| on(c.sheet_layer_ref))
                        .map(|c| mm(c.radius))
                        .collect::<Vec<_>>(),
                    nested
                        .lines
                        .iter()
                        .filter(|l| on(l.sheet_layer_ref))
                        .count(),
                    nested
                        .dimensions
                        .iter()
                        .filter(|d| on(d.sheet_layer_ref))
                        .count(),
                    nested
                        .dimensions
                        .iter()
                        .filter(|d| on(d.sheet_layer_ref))
                        .map(|d| mm(d.value_m))
                        .collect::<Vec<_>>(),
                )
            }
            _ => "body not resolved".to_string(),
        };
        println!(
            "  /JSite{site:<5} sheet {sheet:<6} x{count:<3} {:<48} {summary}",
            names.iter().cloned().collect::<Vec<_>>().join(" | ")
        );
    }
}

fn section_2_chain(doc: &PidDocument, site: &JSite) {
    let nested = site.nested_geometry.as_ref();
    let info = site.symbol_information.as_ref();
    let has_dimensions = nested.is_some_and(|n| !n.dimensions.is_empty());
    if info.is_none() && !has_dimensions {
        return;
    }
    println!("\n=== 2. the chain inside {} ===", site.path);
    let layers = doc.sheet_layers.get(&site.path);
    let layer_name = |oid: u32| -> String {
        layers
            .and_then(|layers| layers.iter().find(|layer| layer.oid == oid))
            .map_or_else(|| "?".to_string(), |layer| layer.name.clone())
    };

    // Every persist id this probe can put a class to.
    let mut class: BTreeMap<u32, String> = BTreeMap::new();
    if let Some(info) = info {
        for record in &info.symbol_informations {
            class.insert(record.oid, "SymbolInformation".into());
        }
        for record in &info.double_values {
            class.insert(record.oid, format!("DoubleValue {}", record.value));
        }
        for record in &info.variable_groups {
            class.insert(record.oid, "Variables".into());
        }
        for record in &info.relations {
            class.insert(record.oid, "StandardRelation".into());
        }
    }
    if let Some(nested) = nested {
        for d in &nested.dimensions {
            class.insert(d.oid, format!("JDim {} mm", mm(d.value_m)));
        }
        for l in &nested.lines {
            class.insert(l.oid, "Line".into());
        }
        for a in &nested.arcs {
            class.insert(a.oid, format!("Arc r {} mm", mm(a.radius)));
        }
        for c in &nested.circles {
            class.insert(c.oid, format!("Circle r {} mm", mm(c.radius)));
        }
        for s in &nested.sheets {
            class.insert(*s, "JSheet".into());
        }
    }
    let name_of =
        |oid: u32| -> String { class.get(&oid).cloned().unwrap_or_else(|| "?".to_string()) };

    if let Some(info) = info {
        println!(
            "  -- symbol information ({}) --",
            info.symbol_informations.len()
        );
        for record in &info.symbol_informations {
            let variables: Vec<String> = record
                .variables
                .iter()
                .map(|v| format!("{}={} (-> {})", v.name, v.value, v.value_ref))
                .collect();
            println!(
                "  SymbolInformation {:<5} parent {:<5} extents ({}, {}) {}",
                record.oid,
                record.parent_ref,
                record.extents.0,
                record.extents.1,
                if variables.is_empty() {
                    "(no variables)".to_string()
                } else {
                    variables.join("  ")
                }
            );
        }
        println!("  -- variable groups ({}) --", info.variable_groups.len());
        for record in &info.variable_groups {
            let members: Vec<String> = record
                .members
                .iter()
                .map(|m| format!("{m} {}", name_of(*m)))
                .collect();
            println!("  Variables {:<5} [{}]", record.oid, members.join(", "));
        }
        println!("  -- double values ({}) --", info.double_values.len());
        let named_by: BTreeMap<u32, Vec<String>> = info
            .symbol_informations
            .iter()
            .flat_map(|record| {
                record
                    .variables
                    .iter()
                    .map(move |v| (v.value_ref, format!("{}.{}", record.oid, v.name)))
            })
            .fold(BTreeMap::new(), |mut acc, (oid, name)| {
                acc.entry(oid).or_default().push(name);
                acc
            });
        for record in &info.double_values {
            println!(
                "  DoubleValue {:<5} parent {:<5} = {:<12} named by {}",
                record.oid,
                record.parent_ref,
                record.value,
                named_by
                    .get(&record.oid)
                    .map_or_else(|| "nothing".to_string(), |names| names.join(", "))
            );
        }
        println!("  -- standard relations ({}) --", info.relations.len());
        for record in &info.relations {
            let operands: Vec<String> = record
                .operands
                .iter()
                .enumerate()
                .map(|(index, oid)| {
                    format!(
                        "{}{oid} {}",
                        if index == 0 { "out=" } else { "in=" },
                        name_of(*oid)
                    )
                })
                .collect();
            println!(
                "  Relation {:<5} sig {:<10} formula {:<24} {}",
                record.oid,
                record.signature,
                record.formula,
                operands.join("  ")
            );
        }

        // The formula against the stored values, in metres and in inches:
        // whichever unit makes every relation of the storage close is the
        // unit the constants are written in.
        println!("  -- does the formula reproduce the output? --");
        let value_of = |oid: u32| -> Option<f64> {
            info.double_values
                .iter()
                .find(|d| d.oid == oid)
                .map(|d| d.value)
                .or_else(|| {
                    nested?
                        .dimensions
                        .iter()
                        .find(|d| d.oid == oid)
                        .map(|d| d.value_m)
                })
        };
        for record in &info.relations {
            let Some((&out, ins)) = record.operands.split_first() else {
                continue;
            };
            let Some(expected) = value_of(out) else {
                println!(
                    "  Relation {:<5} output {out} has no value this probe knows",
                    record.oid
                );
                continue;
            };
            let inputs: Option<Vec<f64>> = ins.iter().map(|oid| value_of(*oid)).collect();
            let Some(inputs) = inputs else {
                println!(
                    "  Relation {:<5} an input has no value this probe knows",
                    record.oid
                );
                continue;
            };
            let in_m = eval_formula(&record.formula, &inputs);
            let in_inch = eval_formula(
                &record.formula,
                &inputs.iter().map(|v| v / INCH_M).collect::<Vec<_>>(),
            )
            .map(|v| v * INCH_M);
            let verdict = |v: Option<f64>| match v {
                Some(v) if (v - expected).abs() < 1e-9 => "matches".to_string(),
                Some(v) => format!("gives {} mm", mm(v)),
                None => "does not parse".to_string(),
            };
            println!(
                "  Relation {:<5} {:<20} output {} mm: read in metres {}, read in inches {}",
                record.oid,
                record.formula,
                mm(expected),
                verdict(in_m),
                verdict(in_inch)
            );
        }
    }

    if let Some(nested) = nested {
        println!("  -- dimensions ({}) --", nested.dimensions.len());
        for d in &nested.dimensions {
            let line = nested.lines.iter().find(|l| l.oid == d.measured_oid);
            let measured = match line {
                Some(l) => {
                    let len =
                        ((l.end_x - l.start_x).powi(2) + (l.end_y - l.start_y).powi(2)).sqrt();
                    format!(
                        "line {} ({:.4}, {:.4})-({:.4}, {:.4}) len {} mm{}",
                        l.oid,
                        l.start_x,
                        l.start_y,
                        l.end_x,
                        l.end_y,
                        mm(len),
                        if (len - d.value_m).abs() < 1e-9 {
                            " = value"
                        } else {
                            " != value"
                        }
                    )
                }
                None => format!(
                    "{} {} (not a line the cache carries)",
                    d.measured_oid,
                    name_of(d.measured_oid)
                ),
            };
            println!(
                "  JDim {:<5} sheet {:<5} layer {:<5} {:<12} = {:>8} mm  measures {measured}  group {}",
                d.oid,
                d.parent_ref,
                d.sheet_layer_ref,
                layer_name(d.sheet_layer_ref),
                mm(d.value_m),
                d.group_ref
                    .map_or_else(|| "-".to_string(), |g| g.to_string())
            );
        }

        println!("  -- bodies ({}) --", nested.definitions.len());
        let named = placements(doc);
        for definition in &nested.definitions {
            let on = |layer: u32| definition.layers.binary_search(&layer).is_ok();
            let placed = site_id(site)
                .and_then(|id| named.get(&(id, definition.sheet_oid)))
                .map_or_else(
                    || "named by no placement".to_string(),
                    |(names, count)| {
                        format!(
                            "named by {count} placement(s) of {}",
                            names.iter().cloned().collect::<Vec<_>>().join(" | ")
                        )
                    },
                );
            let arcs: Vec<String> = nested
                .arcs
                .iter()
                .filter(|a| on(a.sheet_layer_ref))
                .map(|a| {
                    format!(
                        "arc {} r {} c ({:.4}, {:.4}) [{:.3}..{:.3}]",
                        a.oid,
                        mm(a.radius),
                        a.center_x,
                        a.center_y,
                        a.start_angle,
                        a.end_angle
                    )
                })
                .collect();
            let dims: Vec<String> = nested
                .dimensions
                .iter()
                .filter(|d| on(d.sheet_layer_ref))
                .map(|d| format!("JDim {} = {} mm", d.oid, mm(d.value_m)))
                .collect();
            println!(
                "  sheet {:<5} manager {:<5} layers {:?}: {} lines, {} circles, {} arcs, {} texts, {} dims -- {placed}",
                definition.sheet_oid,
                definition.manager_oid,
                definition.layers,
                nested
                    .lines
                    .iter()
                    .filter(|l| on(l.sheet_layer_ref))
                    .count(),
                nested
                    .circles
                    .iter()
                    .filter(|c| on(c.sheet_layer_ref))
                    .count(),
                arcs.len(),
                nested
                    .texts
                    .iter()
                    .filter(|t| on(t.sheet_layer_ref))
                    .count(),
                dims.len(),
            );
            for arc in &arcs {
                println!("      {arc}");
            }
            for dim in &dims {
                println!("      {dim}");
            }
        }
    }
}

/// Where a body holds both arcs and dimensions, how the two relate: is an
/// arc's radius one of the body's dimension values, and does an arc sit
/// on the line a dimension measures (its centre or an endpoint on the
/// line's endpoints)? Reported for every (arc, dimension) pair of such a
/// body, so the answer is a count, not a claim. The plan expected the
/// instance's arcs to sit on a dimension's endpoints; the storages that
/// hold arcs and dimensions together decide what the relation really is.
fn section_3_arcs_against_dimensions(doc: &PidDocument) {
    println!("\n=== 3. arcs against dimensions, every body that has both ===");
    let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6;
    let mut bodies = 0usize;
    for site in &doc.jsites {
        let Some(nested) = site.nested_geometry.as_ref() else {
            continue;
        };
        for definition in &nested.definitions {
            let on = |layer: u32| definition.layers.binary_search(&layer).is_ok();
            let arcs: Vec<_> = nested
                .arcs
                .iter()
                .filter(|a| on(a.sheet_layer_ref))
                .collect();
            let dims: Vec<_> = nested
                .dimensions
                .iter()
                .filter(|d| on(d.sheet_layer_ref))
                .collect();
            if arcs.is_empty() || dims.is_empty() {
                continue;
            }
            bodies += 1;
            println!("  {} sheet {}:", site.path, definition.sheet_oid);
            for arc in &arcs {
                let radius_is: Vec<String> = dims
                    .iter()
                    .filter(|d| (d.value_m - arc.radius).abs() < 1e-9)
                    .map(|d| format!("JDim {}", d.oid))
                    .collect();
                println!(
                    "    arc {} r {} mm centre ({:.4}, {:.4}): radius equals {}",
                    arc.oid,
                    mm(arc.radius),
                    arc.center_x,
                    arc.center_y,
                    if radius_is.is_empty() {
                        "no dimension of this body".to_string()
                    } else {
                        radius_is.join(", ")
                    }
                );
                let arc_ends = [
                    (
                        arc.center_x + arc.radius * arc.start_angle.cos(),
                        arc.center_y + arc.radius * arc.start_angle.sin(),
                    ),
                    (
                        arc.center_x + arc.radius * arc.end_angle.cos(),
                        arc.center_y + arc.radius * arc.end_angle.sin(),
                    ),
                ];
                for d in &dims {
                    let Some(line) = nested.lines.iter().find(|l| l.oid == d.measured_oid) else {
                        continue;
                    };
                    let ends = [(line.start_x, line.start_y), (line.end_x, line.end_y)];
                    let on_centre: Vec<String> = ends
                        .iter()
                        .filter(|e| close(**e, (arc.center_x, arc.center_y)))
                        .map(|e| format!("({:.4}, {:.4})", e.0, e.1))
                        .collect();
                    let on_ends: Vec<String> = ends
                        .iter()
                        .flat_map(|e| arc_ends.iter().map(move |a| (e, a)))
                        .filter(|(e, a)| close(**e, **a))
                        .map(|(e, _)| format!("({:.4}, {:.4})", e.0, e.1))
                        .collect();
                    if !on_centre.is_empty() || !on_ends.is_empty() {
                        println!(
                            "      JDim {} ({} mm) measures line {}: line endpoint on arc centre {}, on arc endpoint {}",
                            d.oid,
                            mm(d.value_m),
                            line.oid,
                            if on_centre.is_empty() {
                                "-".to_string()
                            } else {
                                on_centre.join(" ")
                            },
                            if on_ends.is_empty() {
                                "-".to_string()
                            } else {
                                on_ends.join(" ")
                            }
                        );
                    }
                }
            }
        }
    }
    if bodies == 0 {
        println!("  no body of this drawing holds both arcs and dimensions");
    }
}

/// Template against instance, for every parametric symbol the drawing
/// places -- read off the projection, now that K1 put the pairing on it.
///
/// [`pid_parse::PidSymbolDefinition::template`] names, for a placed
/// parametric body, the template body it was placed from (paired through
/// the instance's `value_ref`s when they resolve in the template storage,
/// by variable names and values when they resolve nowhere); the template's
/// dimensions carry the variable that drives each and the formula doing
/// it. What is left for the probe is the evidence the DTO does not hold:
/// that the instance is the body a placement names and the template is
/// not, and the two bodies line for line and arc for arc, so "the instance
/// was resized" is a diff, not a guess.
fn section_4_template_vs_instance(doc: &PidDocument) {
    use pid_parse::symbol_library::SymbolPrimitive;

    println!("\n=== 4. template against placed instance ===");
    let named = placements(doc);
    let geometry = pid_parse::build_normalized_geometry(doc);
    let describe = |body: &pid_parse::PidSymbolDefinition| -> String {
        let points: Vec<(f64, f64)> = body
            .primitives
            .iter()
            .filter_map(|p| match p {
                SymbolPrimitive::Line { start, end } => Some([*start, *end]),
                _ => None,
            })
            .flatten()
            .collect();
        let arcs: Vec<String> = body
            .primitives
            .iter()
            .filter_map(|p| match p {
                SymbolPrimitive::Arc { radius, .. } => Some(mm(*radius)),
                _ => None,
            })
            .collect();
        let mut it = points.into_iter();
        match it.next() {
            Some(first) => {
                let (x0, y0, x1, y1) = it.fold(
                    (first.0, first.1, first.0, first.1),
                    |(x0, y0, x1, y1), (x, y)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                );
                format!(
                    "bbox x {x0:.7}..{x1:.7} y {y0:.7}..{y1:.7} = {} x {} mm (half {} x {}), arcs r {:?}",
                    mm(x1 - x0),
                    mm(y1 - y0),
                    mm((x1 - x0) / 2.0),
                    mm((y1 - y0) / 2.0),
                    arcs
                )
            }
            None => "no lines".to_string(),
        }
    };
    let mut pairs = 0usize;
    for instance in &geometry.symbol_definitions {
        let Some(template_ref) = instance.template else {
            continue;
        };
        let Some(template) = geometry.symbol_definition(template_ref) else {
            println!(
                "  /JSite{} sheet {} names template /JSite{} sheet {}, which is not a body of the projection",
                instance.reference.site, instance.reference.sheet, template_ref.site, template_ref.sheet
            );
            continue;
        };
        let by_refs = instance.variables.iter().all(|i| {
            template
                .variables
                .iter()
                .any(|t| t.value_ref == i.value_ref)
        });
        let same_values = instance.variables.len() == template.variables.len()
            && instance.variables.iter().all(|i| {
                template
                    .variables
                    .iter()
                    .any(|t| t.name == i.name && (t.value_m - i.value_m).abs() < 1e-12)
            });
        let placed = |reference: pid_parse::PidSymbolDefinitionRef| {
            named.get(&(reference.site, reference.sheet)).map_or_else(
                || "placed by nothing".to_string(),
                |(names, count)| {
                    format!(
                        "placed x{count} as {}",
                        names.iter().cloned().collect::<Vec<_>>().join(" | ")
                    )
                },
            )
        };
        println!(
            "  instance /JSite{} sheet {} ({}) <- template /JSite{} sheet {} ({}): paired {}",
            instance.reference.site,
            instance.reference.sheet,
            placed(instance.reference),
            template.reference.site,
            template.reference.sheet,
            placed(template.reference),
            if by_refs {
                format!(
                    "through value_refs {:?}, which resolve in the template storage",
                    instance
                        .variables
                        .iter()
                        .map(|v| v.value_ref)
                        .collect::<Vec<_>>()
                )
            } else {
                format!(
                    "by variable names and values (value_refs {:?} resolve in no storage)",
                    instance
                        .variables
                        .iter()
                        .map(|v| v.value_ref)
                        .collect::<Vec<_>>()
                )
            }
        );
        println!(
            "    variables {}: {}",
            if same_values { "identical" } else { "DIFFER" },
            instance
                .variables
                .iter()
                .map(|v| format!("{}={} mm", v.name, mm(v.value_m)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "    template dimensions ({}, {} named): {}; instance dimensions: {}",
            template.dimensions.len(),
            template
                .dimensions
                .iter()
                .filter(|d| d.name.is_some())
                .count(),
            template
                .dimensions
                .iter()
                .map(|d| {
                    format!(
                        "JDim {} {}={} mm [{}]",
                        d.oid,
                        d.name.as_deref().unwrap_or("-"),
                        mm(d.value_m),
                        d.formula.as_deref().unwrap_or("no relation")
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            instance.dimensions.len()
        );
        println!(
            "    template sheet {}: {}",
            template.reference.sheet,
            describe(template)
        );
        println!(
            "    instance sheet {}: {}",
            instance.reference.sheet,
            describe(instance)
        );
        pairs += 1;
    }
    // Records the projection paired with nothing: variables in a storage
    // without relations whose body no template claims.
    for site in &doc.jsites {
        let Some(info) = site.symbol_information.as_ref() else {
            continue;
        };
        if !info.relations.is_empty() {
            continue;
        }
        let paired_here = geometry
            .symbol_definitions
            .iter()
            .filter(|body| body.template.is_some() && Some(body.reference.site) == site_id(site))
            .count();
        let with_variables = info
            .symbol_informations
            .iter()
            .filter(|r| !r.variables.is_empty())
            .count();
        if with_variables != paired_here {
            println!(
                "  {}: {with_variables} SymbolInformation record(s) with variables, {paired_here} body(ies) paired with a template",
                site.path
            );
        }
    }
    if pairs == 0 {
        println!("  no parametric instance in this drawing");
    }
}

fn main() {
    for fixture in FIXTURES {
        if !Path::new(fixture).exists() {
            println!("\n{fixture}: absent");
            continue;
        }
        let doc = PidParser::new()
            .parse_file(fixture)
            .unwrap_or_else(|e| panic!("{fixture}: {e}"));
        println!("\n==================== {fixture} ====================");
        section_1_placements(&doc);
        for site in &doc.jsites {
            section_2_chain(&doc, site);
        }
        section_3_arcs_against_dimensions(&doc);
        section_4_template_vs_instance(&doc);
    }
}
