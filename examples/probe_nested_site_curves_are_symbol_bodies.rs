//! Are the circles and arcs inside a nested `JSite` the body of a symbol the
//! drawing places?
//!
//! `decode_igcircles` / `decode_igarcs` read 12 + 12 curve records out of the
//! nested `JSite<N>/PSMcluster0` storages of the four fixtures, and the
//! values themselves look like symbol glyphs rather than page content: ten of
//! the twelve circles are centred on the origin, 6.35 mm is the half-inch
//! instrument balloon, and the arcs come in mirrored pairs. The 2026-08-31
//! coverage-gap note left the storage-to-page transform unproven and asked
//! for a probe that tests the nested site as an embedded symbol definition.
//!
//! This is that probe. It needs no transform to run: a symbol's `.sym` body
//! is in symbol-local coordinates, and so -- if the hypothesis holds -- is
//! the nested site. So compare them raw. For every fixture:
//!
//! 1. Resolve every symbol the drawing places (`PidGraphicKind::SymbolInstance`
//!    with a `symbol_path`) against the reference library, and collect the
//!    circle and arc primitives of each body.
//! 2. For every curve record a nested site holds, look for a body primitive
//!    that is the same shape at the same place, to a nanometre. Print which
//!    symbol(s) it belongs to, or that nothing matches.
//! 3. The other direction: of the symbols matched, how many of their own
//!    circles and arcs the site reproduces -- a full copy of the body, or a
//!    partial one.
//!
//! A clean match says the nested `LdcSite` is the drawing's embedded copy of
//! a symbol definition, and the transform to the page is the one the
//! `igSymbol2d` placements of that symbol already carry. No match leaves the
//! gap where it was.
//!
//! ```powershell
//! cargo run --quiet --example probe_nested_site_curves_are_symbol_bodies
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pid_parse::model::JSiteNestedGeometry;
use pid_parse::symbol_library::{SymbolLibrary, SymbolPrimitive};
use pid_parse::{build_normalized_geometry, PidGeometryConfidence, PidGraphicKind, PidParser};

const FIXTURES: [&str; 4] = [
    "test-file/D06.pid",
    "test-file/DWG-0201GP06-01.pid",
    "test-file/DWG-0202GP06-01.pid",
    "test-file/工艺管道及仪表流程-1.pid",
];

/// The reference library every fixture was drawn against.
const LIBRARY: &str = "test-file/symbols-full";

/// Two coordinates are the same place when they agree to a nanometre: far
/// below drafting tolerance, far above f64 noise at these magnitudes.
const SAME_MM: f64 = 1e-9;

/// One curve, from either side, reduced to what can be compared.
#[derive(Clone, Copy, PartialEq)]
enum Curve {
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    Arc {
        center: (f64, f64),
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
}

impl Curve {
    fn same_as(&self, other: &Curve) -> bool {
        let close = |a: f64, b: f64| (a - b).abs() <= SAME_MM;
        match (self, other) {
            (
                Curve::Circle { center, radius },
                Curve::Circle {
                    center: c,
                    radius: r,
                },
            ) => close(center.0, c.0) && close(center.1, c.1) && close(*radius, *r),
            (
                Curve::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                },
                Curve::Arc {
                    center: c,
                    radius: r,
                    start_angle: s,
                    end_angle: e,
                },
            ) => {
                close(center.0, c.0)
                    && close(center.1, c.1)
                    && close(*radius, *r)
                    && close(*start_angle, *s)
                    && close(*end_angle, *e)
            }
            _ => false,
        }
    }

    fn text(&self) -> String {
        match self {
            Curve::Circle { center, radius } => {
                format!("circle c=({:.4},{:.4}) r={:.5}", center.0, center.1, radius)
            }
            Curve::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => format!(
                "arc    c=({:.4},{:.4}) r={:.5} {:.3}..{:.3}",
                center.0, center.1, radius, start_angle, end_angle
            ),
        }
    }
}

fn site_curves(curves: &JSiteNestedGeometry) -> Vec<Curve> {
    curves
        .circles
        .iter()
        .map(|c| Curve::Circle {
            center: (c.center_x, c.center_y),
            radius: c.radius,
        })
        .chain(curves.arcs.iter().map(|a| Curve::Arc {
            center: (a.center_x, a.center_y),
            radius: a.radius,
            start_angle: a.start_angle,
            end_angle: a.end_angle,
        }))
        .collect()
}

fn body_curves(primitives: &[SymbolPrimitive]) -> Vec<Curve> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            SymbolPrimitive::Circle { center, radius } => Some(Curve::Circle {
                center: *center,
                radius: *radius,
            }),
            SymbolPrimitive::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Some(Curve::Arc {
                center: *center,
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            }),
            _ => None,
        })
        .collect()
}

fn symbol_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

fn main() {
    if !Path::new(LIBRARY).exists() {
        println!("skip: {LIBRARY} is absent; nothing to compare against");
        return;
    }
    let mut fixtures_run = 0usize;
    let mut total_site_curves = 0usize;
    let mut total_matched = 0usize;

    for fixture in FIXTURES {
        if !Path::new(fixture).exists() {
            println!("skip: {fixture} is absent");
            continue;
        }
        let doc = match PidParser::new().parse_file(fixture) {
            Ok(doc) => doc,
            Err(error) => {
                println!("skip: {fixture} did not parse: {error}");
                continue;
            }
        };
        fixtures_run += 1;
        let geometry = build_normalized_geometry(&doc);

        // 1. Every symbol the drawing places, with its body's curves.
        let mut placed: BTreeMap<String, usize> = BTreeMap::new();
        for entity in &geometry.entities {
            if entity.confidence != PidGeometryConfidence::Decoded {
                continue;
            }
            if let PidGraphicKind::SymbolInstance {
                symbol_path: Some(path),
                ..
            } = &entity.kind
            {
                *placed.entry(path.clone()).or_default() += 1;
            }
        }
        let mut library = SymbolLibrary::new(LIBRARY);
        let mut bodies: BTreeMap<String, Vec<Curve>> = BTreeMap::new();
        let mut unresolved = Vec::new();
        for path in placed.keys() {
            match library.resolve(path) {
                Some(body) => {
                    let primitives: Vec<SymbolPrimitive> = body
                        .primitives
                        .iter()
                        .map(|p| p.primitive.clone())
                        .collect();
                    bodies.insert(path.clone(), body_curves(&primitives));
                }
                None => unresolved.push(symbol_name(path).to_string()),
            }
        }
        let library_curves: usize = bodies.values().map(Vec::len).sum();
        println!(
            "\n=== {} : {} placed symbols ({} distinct), {} resolve in the library and carry {} circles/arcs between them{}",
            fixture.rsplit('/').next().unwrap_or(fixture),
            placed.values().sum::<usize>(),
            placed.len(),
            bodies.len(),
            library_curves,
            if unresolved.is_empty() {
                String::new()
            } else {
                format!("; unresolved: {}", unresolved.join(", "))
            }
        );

        // 2. Every curve a nested site holds, against every body.
        for site in doc
            .jsites
            .iter()
            .filter(|site| site.nested_geometry.is_some())
        {
            let curves = site_curves(site.nested_geometry.as_ref().expect("filtered"));
            println!(
                "  {}  ({} curves; JProperties .sym = {})",
                site.path,
                curves.len(),
                site.symbol_path
                    .as_deref()
                    .or(site.local_symbol_path.as_deref())
                    .map(symbol_name)
                    .unwrap_or("-")
            );
            let mut matched_symbols: BTreeSet<&str> = BTreeSet::new();
            let mut matched_here = 0usize;
            for curve in &curves {
                let owners: Vec<&str> = bodies
                    .iter()
                    .filter(|(_, body)| body.iter().any(|b| curve.same_as(b)))
                    .map(|(path, _)| symbol_name(path))
                    .collect();
                if owners.is_empty() {
                    println!("     {}   -> no placed symbol draws this", curve.text());
                } else {
                    matched_here += 1;
                    matched_symbols.extend(owners.iter().copied());
                    println!("     {}   -> {}", curve.text(), owners.join(" | "));
                }
            }
            total_site_curves += curves.len();
            total_matched += matched_here;

            // 3. The other direction, for the symbols that matched.
            for name in &matched_symbols {
                let (path, body) = bodies
                    .iter()
                    .find(|(path, _)| symbol_name(path) == *name)
                    .expect("matched name came from bodies");
                let reproduced = body
                    .iter()
                    .filter(|b| curves.iter().any(|c| c.same_as(b)))
                    .count();
                println!(
                    "     <- {name}: the site reproduces {reproduced} of the body's {} circles/arcs; placed {} time(s)",
                    body.len(),
                    placed.get(path).copied().unwrap_or_default()
                );
            }
        }
    }

    if fixtures_run == 0 {
        println!("no fixture available; nothing to measure");
        return;
    }
    println!(
        "\n=== verdict: {total_matched} of {total_site_curves} nested-site curves are, to a nanometre, a circle or arc of a symbol body the same drawing places"
    );
}
