//! A damaged symbol file draws nothing its sound file does not.
//!
//! A `.sym` keeps its body on `Sheet*` streams — usually two, the view
//! `SmartPlant` draws and an alternate — and says which of them draws, and
//! on which layers, in the layer table and the view filter sets of its
//! `PSMcluster0`. Every one of those streams is a record chain. Until
//! 2026-11-12 the symbol reader walked a sheet by a rule of its own and
//! took it whole or not at all, and read the cluster only through a walker
//! that asks for the magic and a chain to the last byte. A sheet whose
//! chain stopped was then left out, and the choice of what to draw fell
//! through to the file's other sheet: the committed pressure gauge, one
//! circle on a sound file, drew its circle and a `PG`. A cluster that did
//! not read gave no layers and no view sets, and the same gauge drew two
//! circles, with nothing in the body's fault list to say why.
//!
//! What a drawing's own sheet does with a chain that does not hold is read
//! it as far as it holds: what is drawn is less, never other. This file
//! holds the symbol reader to the same, on every committed symbol file —
//! six on disk and the 616 of the committed `RefData~4~681.zip` — with each
//! of its chain streams damaged in turn, every way [`Damage`] lists:
//!
//! * **one walk.** The body's fault list is what the reader of a drawing
//!   says of the same bytes ([`StreamChainFault::of_stream`]), the root
//!   `PSMcluster0` included; and what the body holds of a stream whose
//!   chain stops is what it holds of the same stream cut off at the stop —
//!   the records the walk read, no more and no fewer.
//! * **less, never other.** Whatever [`SymbolGeometry::displayed`] gives
//!   for a file with a damaged sheet is part of what it gives for the sound
//!   file, primitive for primitive with its pen, layer and sheet. Where
//!   that cannot be had the body is not drawn, and says which of three ways
//!   ([`SymbolNotDrawn`]).
//! * **a fault that loses no record changes nothing.** A wrong magic over
//!   a chain that reaches the end, a header that counts fewer records than
//!   the chain holds: the body is the sound file's, and the fault is
//!   listed.
//! * **a cluster read short draws nothing.** Its layer table and its view
//!   sets decide which sheet and which layers draw, and what was not read
//!   of them cannot be made up.
//! * **a damaged style table adds no shape.** A `StyleCluster` gives pens,
//!   fill colours and lettering styles, and which sheet and which strokes
//!   draw is not its business: the body is always drawn, its strokes and
//!   lettering are the sound file's one for one, and every shape a fill is
//!   made of is a shape a fill of the sound file is made of. The pens are
//!   whatever the table gave — the styles before a stop, and none at all
//!   under a first word that is not the magic, which the style reader, a
//!   drawing's and a symbol's alike, takes for no table — and what names a
//!   style that was not read draws with the default. One thing changes in
//!   number: two boundary records that state one region in one colour
//!   draw it once, and when the table gives only one of the two its colour
//!   both draw. That is a second fill of a region the sound file fills,
//!   under or over the first, and it is the outcome [`Outcome::More`] in
//!   the tally: 28 damages of two files of the archive
//!   (`Supply Scope Break.sym`, 8; `DV-Deluge Control Valve with
//!   Flange.sym`, 20), none on disk.
//!
//! * **a stream with no byte is read short, and a file with no root
//!   cluster is not drawn.** The chain walk has no word for an empty
//!   stream — it read every byte there is — and until 2026-11-13 the
//!   symbol reader took its word. A sheet emptied to no bytes was a sheet
//!   of no strokes, and the body fell through to the file's other sheet as
//!   it did before the rules above; a root `PSMcluster0` emptied, or gone
//!   from the file, was no layer table and no view sets, and the gauge
//!   drew two circles. Neither was listed. A chain stream of a symbol file
//!   opens with its header whatever it holds, so the reader takes one with
//!   no byte for one cut off inside its header, and a file with no root
//!   cluster for one whose root cluster was read short, and lists each
//!   ([`StreamChainFault::of_an_empty_stream`],
//!   [`StreamChainFault::of_a_missing_stream`]). No committed symbol file
//!   is either: the sweep holds every one to a root cluster and to no
//!   fault.
//!
//! The tallies are pinned, by the stream's role and the kind of damage, so
//! that a rule that changes shows here.
//!
//! One rule more is the library's, not the body's: **a damaged copy is not
//! passed over for another root's.** A library searches its roots in order
//! and moves on from a copy that holds nothing to draw; a copy of which
//! nothing was read is not such a copy
//! ([`a_damaged_copy_is_not_passed_over_for_another_roots`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pid_parse::parsers::sheet_records::SheetChain;
use pid_parse::symbol_library::{
    read_symbol_geometry_from_bytes, symbol_geometry_of_package, StyledPrimitive, SymbolGeometry,
    SymbolLibrary, SymbolNotDrawn, SymbolPrimitive,
};
use pid_parse::writer::{PidWriter, StreamReplacement, WritePlan};
use pid_parse::{PidPackage, StreamChainFault};

/// The committed symbol library of a backup package.
#[cfg(feature = "refdata-archive")]
const ARCHIVE: &str = "test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip";

/// The committed pressure gauge: one displayed circle on `/Sheet6`, its
/// normal view, and the lettering `PG` on `/Sheet68`.
const GAUGE: &str =
    "test-file/symbols/Instrumentation/Off-Line/Without Implied Components/Pressure/PG-Pressure Gauge.sym";
/// A body kept on a sheet with no view set, beside a normal view that is a
/// header and nothing else: the body is the sheet that keeps the most
/// strokes.
const CIRCLE: &str = "test-file/symbols/Design/Annotation/Graphics/Circle.sym";
/// Nine displayed strokes on `/Sheet6`, its normal view.
const VALVE: &str = "test-file/symbols/Piping/Valves/Angle/2-Way Angle Globe Valve.sym";

/// The root cluster of a symbol file: its layer table and its view sets.
const ROOT_CLUSTER: &str = "/PSMcluster0";
/// How the tally names a file read with no root cluster at all.
const NO_ROOT_CLUSTER: &str = "the root cluster not in the file";

/// One way a chain stream is damaged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Damage {
    /// The record at this index states a length of zero: the walk stops on
    /// it.
    ZeroLength(usize),
    /// The record's type word carries the `0x4000` flag, which no record
    /// opens with.
    Flagged(usize),
    /// The record's type word is `0x3FE6`, past the class table.
    NoSuchClass(usize),
    /// The last record states one byte more than the stream holds.
    LastTooLong,
    /// Four bytes after the last record.
    StrayBytes,
    /// The stream opens with another word than the chain magic.
    WrongMagic,
    /// The header counts one live record more than the chain holds.
    CountOneMore,
    /// The header counts one live record fewer than the chain holds.
    CountOneFewer,
    /// The stream ends where its last record starts, under the header it
    /// had.
    CutBeforeLast,
    /// The stream is its header and nothing else, the count as it was.
    CutToHeader,
    /// The stream is five bytes: shorter than a header.
    CutIntoHeader,
    /// The stream holds no byte at all. The walk does not stop — there is
    /// nothing to stop on — and says nothing is wrong; the reader of a
    /// symbol file does ([`fault_of`]).
    Emptied,
}

impl Damage {
    /// Whether the damage leaves every record of the stream to be read: the
    /// chain reaches the stream's end and holds no fewer live records than
    /// the header states.
    fn loses_no_record(self) -> bool {
        matches!(self, Self::WrongMagic | Self::CountOneFewer)
    }

    /// Whether the walk stops short of the stream's end.
    fn stops_the_walk(self) -> bool {
        matches!(
            self,
            Self::ZeroLength(_)
                | Self::Flagged(_)
                | Self::NoSuchClass(_)
                | Self::LastTooLong
                | Self::StrayBytes
                | Self::CutIntoHeader
        )
    }

    /// The kind without its record index, for a tally.
    fn kind(self) -> &'static str {
        match self {
            Self::ZeroLength(_) => "a length of zero",
            Self::Flagged(_) => "a type word flagged 0x4000",
            Self::NoSuchClass(_) => "a type word past the class table",
            Self::LastTooLong => "the last record one byte too long",
            Self::StrayBytes => "stray bytes at the end",
            Self::WrongMagic => "a wrong magic",
            Self::CountOneMore => "a header count one too many",
            Self::CountOneFewer => "a header count one too few",
            Self::CutBeforeLast => "cut off before the last record",
            Self::CutToHeader => "cut off after the header",
            Self::CutIntoHeader => "cut off inside the header",
            Self::Emptied => "emptied of every byte",
        }
    }

    /// The damaged stream, or `None` where the stream has nothing for the
    /// damage to change.
    fn apply(self, data: &[u8], starts: &[usize]) -> Option<Vec<u8>> {
        let mut damaged = data.to_vec();
        match self {
            Self::ZeroLength(index) => {
                let at = *starts.get(index)?;
                damaged[at + 2..at + 6].copy_from_slice(&0u32.to_le_bytes());
            }
            Self::Flagged(index) => {
                let at = *starts.get(index)?;
                damaged[at + 1] |= 0x40;
            }
            Self::NoSuchClass(index) => {
                let at = *starts.get(index)?;
                damaged[at..at + 2].copy_from_slice(&0x3FE6u16.to_le_bytes());
            }
            Self::LastTooLong => {
                let at = *starts.last()?;
                let length = u32::from_le_bytes(damaged[at + 2..at + 6].try_into().ok()?);
                damaged[at + 2..at + 6].copy_from_slice(&(length + 1).to_le_bytes());
            }
            Self::StrayBytes => damaged.extend_from_slice(&[0xEE; 4]),
            Self::WrongMagic => damaged
                .get_mut(..4)?
                .copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]),
            Self::CountOneMore | Self::CountOneFewer => {
                let stated = u32::from_le_bytes(damaged.get(4..8)?.try_into().ok()?);
                let changed = if self == Self::CountOneMore {
                    stated.checked_add(1)?
                } else {
                    stated.checked_sub(1)?
                };
                damaged[4..8].copy_from_slice(&changed.to_le_bytes());
            }
            Self::CutBeforeLast => damaged.truncate(*starts.last()?),
            Self::CutToHeader => {
                starts.first()?;
                damaged.truncate(8);
            }
            Self::CutIntoHeader => {
                if damaged.len() < 8 {
                    return None;
                }
                damaged.truncate(5);
            }
            Self::Emptied => damaged.clear(),
        }
        Some(damaged)
    }
}

/// Every damage made to a stream of `records` records: a stop at each
/// record in turn; the two type words no record opens with at the first,
/// the middle and the last; and each of the damages that are of the stream
/// rather than of a record.
fn damages(records: usize) -> Vec<Damage> {
    let mut all: Vec<Damage> = (0..records).map(Damage::ZeroLength).collect();
    let mut at = vec![0, records / 2, records.saturating_sub(1)];
    at.dedup();
    for index in at {
        if index < records {
            all.push(Damage::Flagged(index));
            all.push(Damage::NoSuchClass(index));
        }
    }
    all.extend([
        Damage::LastTooLong,
        Damage::StrayBytes,
        Damage::WrongMagic,
        Damage::CountOneMore,
        Damage::CountOneFewer,
        Damage::CutBeforeLast,
        Damage::CutToHeader,
        Damage::CutIntoHeader,
        Damage::Emptied,
    ]);
    all
}

/// What a reader of a symbol file lists of the stream at `path` holding
/// `damaged`: the chain walk's word; and for a stream with no byte, which
/// the walk reads as an empty chain with nothing wrong, the reader's own.
fn fault_of(path: &str, damage: Damage, damaged: &[u8]) -> Option<StreamChainFault> {
    StreamChainFault::of_stream(path, damaged)
        .or_else(|| (damage == Damage::Emptied).then(|| StreamChainFault::of_an_empty_stream(path)))
}

/// What a stream of a symbol file is to the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    /// A `Sheet*` stream: strokes and lettering.
    Sheet,
    /// A `PSMcluster0`: the layer table and the view filter sets.
    Cluster,
    /// A `StyleCluster`: the pens, fills and lettering styles.
    Styles,
}

fn role_of(stream_path: &str) -> Option<Role> {
    match stream_path.rsplit('/').next()? {
        "PSMcluster0" => Some(Role::Cluster),
        "StyleCluster" => Some(Role::Styles),
        name if name.starts_with("Sheet") => Some(Role::Sheet),
        _ => None,
    }
}

/// What became of the body under one damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    /// Drawn, and all of what the sound file draws.
    Whole,
    /// Drawn, and less than the sound file draws.
    Less,
    /// Drawn, with an outline of the sound file drawn once more than the
    /// sound file draws it. Only a damaged style table does this, and only
    /// to a fill: see the head of this file.
    More,
    /// Not drawn.
    NotDrawn(NotDrawn),
}

/// [`SymbolNotDrawn`], ordered for a tally's key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NotDrawn {
    ClusterShort,
    NothingToShow,
    PickAmongShortSheets,
}

impl From<SymbolNotDrawn> for NotDrawn {
    fn from(why: SymbolNotDrawn) -> Self {
        match why {
            SymbolNotDrawn::ClusterShort => Self::ClusterShort,
            SymbolNotDrawn::NothingToShow => Self::NothingToShow,
            SymbolNotDrawn::PickAmongShortSheets => Self::PickAmongShortSheets,
        }
    }
}

/// What is drawn, pens and colours left out — which is what is compared
/// where the style table is the damaged stream: the shapes the fills are
/// made of, and every other primitive's shape, in order.
fn fills_and_strokes(drawn: &[StyledPrimitive]) -> (Vec<SymbolPrimitive>, Vec<SymbolPrimitive>) {
    let mut fills = Vec::new();
    let mut strokes = Vec::new();
    for styled in drawn {
        match &styled.primitive {
            SymbolPrimitive::Boundary { members, .. } => fills.extend(members.iter().cloned()),
            other => strokes.push(other.clone()),
        }
    }
    (fills, strokes)
}

/// Whether every one of `part` is one of `whole`, each of `whole` standing
/// for one of `part` at most.
fn is_part_of<T: PartialEq>(part: &[T], whole: &[T]) -> bool {
    let mut left: Vec<&T> = whole.iter().collect();
    part.iter().all(|one| {
        left.iter()
            .position(|other| *other == one)
            .map(|found| left.swap_remove(found))
            .is_some()
    })
}

/// What the sweep of a set of files read.
#[derive(Default)]
struct Sweep {
    files: usize,
    streams: BTreeMap<Role, usize>,
    damages: usize,
    /// Changes the chain walk finds nothing wrong with, which are passed
    /// over.
    no_fault: usize,
    /// (role, kind of damage, outcome) -> damages.
    outcomes: BTreeMap<(Role, &'static str, Outcome), usize>,
    /// What does not hold, by rule, with the first few places.
    broken: BTreeMap<&'static str, (usize, Vec<String>)>,
}

impl Sweep {
    fn fails(&mut self, rule: &'static str, place: impl FnOnce() -> String) {
        let entry = self.broken.entry(rule).or_default();
        entry.0 += 1;
        if entry.1.len() < 6 {
            entry.1.push(place());
        }
    }

    /// The tally as the pins spell it, one line an outcome.
    fn lines(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .map(|((role, kind, outcome), count)| {
                let outcome = match outcome {
                    Outcome::NotDrawn(why) => format!("Outcome::NotDrawn(NotDrawn::{why:?})"),
                    drawn => format!("Outcome::{drawn:?}"),
                };
                format!("(Role::{role:?}, {kind:?}, {outcome}, {count}),")
            })
            .collect()
    }

    /// The tally as the pins spell it.
    fn pins(&self) -> Vec<(Role, &'static str, Outcome, usize)> {
        self.outcomes
            .iter()
            .map(|((role, kind, outcome), count)| (*role, *kind, *outcome, *count))
            .collect()
    }
}

/// The body's own word on what is drawn, as owned values.
fn shown(body: &SymbolGeometry) -> Result<Vec<StyledPrimitive>, SymbolNotDrawn> {
    body.displayed()
        .map(|primitives| primitives.into_iter().cloned().collect())
}

/// How a damaged file is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Through {
    /// From the bytes of a copy of the file with the stream replaced, as a
    /// symbol library reads a file: [`read_symbol_geometry_from_bytes`].
    Bytes,
    /// From the parsed file with the stream's bytes replaced in memory:
    /// [`symbol_geometry_of_package`]. The same body, without writing and
    /// parsing a compound file a damage — which is what lets every stream
    /// of 616 files be damaged every way in a test.
    Package,
}

/// The body of `package` with the stream at `path` holding `data`.
fn read_damaged(
    package: &mut PidPackage,
    path: &str,
    data: &[u8],
    through: Through,
) -> SymbolGeometry {
    match through {
        Through::Package => {
            let stream = package.streams.get_mut(path).expect("the stream");
            let original = std::mem::replace(&mut stream.data, data.to_vec());
            let body = symbol_geometry_of_package(package);
            package.streams.get_mut(path).expect("the stream").data = original;
            body
        }
        Through::Bytes => {
            let plan = WritePlan {
                stream_replacements: vec![StreamReplacement {
                    path: path.to_string(),
                    new_data: data.to_vec(),
                }],
                ..WritePlan::default()
            };
            let copy = PidWriter::write_to_bytes(package, &plan)
                .unwrap_or_else(|error| panic!("{path}: the damaged copy is written: {error}"));
            read_symbol_geometry_from_bytes(&copy)
                .unwrap_or_else(|error| panic!("{path}: the damaged copy reads: {error}"))
        }
    }
}

/// Damage each chain stream of one symbol file every way, and hold each
/// reading to the rules of this file's header.
fn sweep_file(label: &str, bytes: &[u8], through: Through, sweep: &mut Sweep) {
    let mut package =
        PidPackage::from_bytes(bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
    let sound = symbol_geometry_of_package(&package);
    assert_eq!(
        sound,
        read_symbol_geometry_from_bytes(bytes).unwrap_or_else(|error| panic!("{label}: {error}")),
        "{label}: the body of the parsed file is the body read from its bytes"
    );
    assert!(
        sound.chain_faults.is_empty(),
        "{label}: a sound file lists no fault: {:#?}",
        sound.chain_faults
    );
    let sound_shown = shown(&sound).unwrap_or_else(|why| panic!("{label}: not drawn: {why:?}"));
    let (sound_fills, sound_strokes) = fills_and_strokes(&sound_shown);
    sweep.files += 1;

    let streams: Vec<(String, Role)> = package
        .streams
        .keys()
        .filter_map(|path| Some((path.clone(), role_of(path)?)))
        .collect();
    for (path, role) in streams {
        *sweep.streams.entry(role).or_default() += 1;
        let original = package.streams[&path].data.clone();
        let starts = SheetChain::walk(&original).starts;
        for damage in damages(starts.len()) {
            let Some(damaged) = damage.apply(&original, &starts) else {
                continue;
            };
            let place = || format!("{label} {path}: {damage:?}");
            let chain = SheetChain::walk(&damaged);
            assert_eq!(
                chain.stop.is_some(),
                damage.stops_the_walk(),
                "{}: whether the walk stops",
                place()
            );
            // What the walk finds nothing wrong with is no fault to either
            // reader: a stream cut off before a last record that is a
            // superseded copy still holds every live record its header
            // states.
            let Some(fault) = fault_of(&path, damage, &damaged) else {
                assert!(
                    matches!(damage, Damage::CutBeforeLast | Damage::CutToHeader),
                    "{}: no fault",
                    place()
                );
                sweep.no_fault += 1;
                continue;
            };
            sweep.damages += 1;

            let body = read_damaged(&mut package, &path, &damaged, through);

            // One walk: the body lists the fault the reader of a drawing
            // states of these bytes, and no other.
            if body.chain_faults != [fault] {
                sweep.fails("the fault list is the chain walk's word", || {
                    format!(
                        "{}: listed {:?}",
                        place(),
                        body.chain_faults
                            .iter()
                            .map(|fault| fault.stream_path.as_str())
                            .collect::<Vec<_>>()
                    )
                });
            }
            // One walk: what is read of a stream whose chain stops is what
            // is read of the stream cut off at the stop.
            if damage.stops_the_walk() {
                let cut = read_damaged(&mut package, &path, &damaged[..chain.end], through);
                if (&body.primitives, &body.sheet_layers, &body.sheet_views)
                    != (&cut.primitives, &cut.sheet_layers, &cut.sheet_views)
                {
                    sweep.fails(
                        "a stopped stream gives what the same stream cut at the stop gives",
                        || {
                            format!(
                                "{}: {} primitive(s), {} layer(s), {} view set(s) against {}, {}, {}",
                                place(),
                                body.primitives.len(),
                                body.sheet_layers.len(),
                                body.sheet_views.len(),
                                cut.primitives.len(),
                                cut.sheet_layers.len(),
                                cut.sheet_views.len()
                            )
                        },
                    );
                }
            }

            let drawn = shown(&body);
            let outcome = match &drawn {
                Ok(part) if part.len() == sound_shown.len() => Outcome::Whole,
                Ok(part) if part.len() > sound_shown.len() => Outcome::More,
                Ok(_) => Outcome::Less,
                Err(why) => Outcome::NotDrawn((*why).into()),
            };
            *sweep
                .outcomes
                .entry((role, damage.kind(), outcome))
                .or_default() += 1;

            match (&drawn, role) {
                // A damaged style table adds no shape, and the body is drawn
                // whatever the damage.
                (Ok(part), Role::Styles) => {
                    let (fills, strokes) = fills_and_strokes(part);
                    if strokes != sound_strokes {
                        sweep.fails("a damaged style table changes no stroke", || {
                            format!(
                                "{}: {} stroke(s) for {}",
                                place(),
                                strokes.len(),
                                sound_strokes.len()
                            )
                        });
                    }
                    let new = fills
                        .iter()
                        .filter(|shape| !sound_fills.contains(shape))
                        .count();
                    if new != 0 {
                        sweep.fails("a damaged style table fills no new shape", || {
                            format!(
                                "{}: {new} of the {} shape(s) its fills are made of",
                                place(),
                                fills.len()
                            )
                        });
                    }
                }
                (Err(why), Role::Styles) => {
                    sweep.fails("a damaged style table leaves the body drawn", || {
                        format!("{}: {why:?}", place())
                    });
                }
                // A fault that loses no record changes nothing of the body.
                _ if damage.loses_no_record() => {
                    if body.primitives != sound.primitives || drawn.as_ref() != Ok(&sound_shown) {
                        sweep.fails("a fault that loses no record changes nothing", || {
                            format!(
                                "{}: {} primitive(s) for {}, shown {:?} for {}",
                                place(),
                                body.primitives.len(),
                                sound.primitives.len(),
                                drawn.as_ref().map(Vec::len),
                                sound_shown.len()
                            )
                        });
                    }
                }
                // A cluster read short draws nothing.
                (_, Role::Cluster) => {
                    if drawn != Err(SymbolNotDrawn::ClusterShort) {
                        sweep.fails("a cluster read short draws nothing", || {
                            format!("{}: {:?}", place(), drawn.as_ref().map(Vec::len))
                        });
                    }
                }
                // Less, never other: each primitive with its pen, its layer
                // and its sheet.
                (Ok(part), Role::Sheet) => {
                    if !is_part_of(part, &sound_shown) {
                        sweep.fails("what a damaged sheet draws, the sound file draws", || {
                            format!(
                                "{}: {} shown, of which {} the sound file does not show",
                                place(),
                                part.len(),
                                part.iter().filter(|one| !sound_shown.contains(one)).count()
                            )
                        });
                    }
                }
                (Err(why), Role::Sheet) => {
                    if *why == SymbolNotDrawn::ClusterShort {
                        sweep.fails("a body is not drawn for the reason that is its own", || {
                            format!("{}: {why:?}", place())
                        });
                    }
                }
            }
        }
    }

    // The file with no root cluster at all. Every committed symbol file
    // keeps one; a file that does not says nothing of which sheet and which
    // layers draw, like one whose root cluster was read short, and is
    // listed so and not drawn.
    let root = package
        .streams
        .remove(ROOT_CLUSTER)
        .unwrap_or_else(|| panic!("{label}: a symbol file with no {ROOT_CLUSTER}"));
    let body = match through {
        Through::Package => symbol_geometry_of_package(&package),
        Through::Bytes => {
            let copy = PidWriter::write_to_bytes(&package, &WritePlan::default())
                .unwrap_or_else(|error| panic!("{label}: the copy with no root cluster: {error}"));
            read_symbol_geometry_from_bytes(&copy)
                .unwrap_or_else(|error| panic!("{label}: the copy with no root cluster: {error}"))
        }
    };
    package.streams.insert(ROOT_CLUSTER.to_string(), root);
    sweep.damages += 1;
    let place = || format!("{label}: no {ROOT_CLUSTER}");
    if body.chain_faults != [StreamChainFault::of_a_missing_stream(ROOT_CLUSTER)] {
        sweep.fails("the fault list is the chain walk's word", || {
            format!(
                "{}: listed {:?}",
                place(),
                body.chain_faults
                    .iter()
                    .map(|fault| fault.stream_path.as_str())
                    .collect::<Vec<_>>()
            )
        });
    }
    let drawn = shown(&body);
    let outcome = match &drawn {
        Ok(part) if part.len() == sound_shown.len() => Outcome::Whole,
        Ok(part) if part.len() > sound_shown.len() => Outcome::More,
        Ok(_) => Outcome::Less,
        Err(why) => Outcome::NotDrawn((*why).into()),
    };
    *sweep
        .outcomes
        .entry((Role::Cluster, NO_ROOT_CLUSTER, outcome))
        .or_default() += 1;
    if drawn != Err(SymbolNotDrawn::ClusterShort) {
        sweep.fails("a cluster read short draws nothing", || {
            format!("{}: {:?}", place(), drawn.as_ref().map(Vec::len))
        });
    }
}

/// The symbol files the repository keeps on disk, the machine-local library
/// left out.
fn committed_on_disk() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                walk(&path, out);
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("sym"))
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(Path::new("test-file/symbols"), &mut files);
    walk(Path::new("test-file/Ref/Symbols"), &mut files);
    files
}

fn hold(sweep: &Sweep, what: &str) {
    eprintln!(
        "{what}: {} symbol files, {:?} streams, {} damages ({} more changes are no fault to the \
         chain walk)",
        sweep.files, sweep.streams, sweep.damages, sweep.no_fault
    );
    for line in sweep.lines() {
        eprintln!("  {line}");
    }
    let broken: Vec<String> = sweep
        .broken
        .iter()
        .map(|(rule, (count, places))| {
            format!(
                "{rule}: {count} damage(s), the first of them:\n    {}",
                places.join("\n    ")
            )
        })
        .collect();
    assert!(
        broken.is_empty(),
        "{what}: {} rule(s) do not hold over {} damages of {} files:\n  {}",
        broken.len(),
        sweep.damages,
        sweep.files,
        broken.join("\n  ")
    );
}

/// Every damage of every chain stream of the six symbol files on disk,
/// each damaged file read from the bytes of a rewritten copy, as a symbol
/// library reads it — and read again from the parsed file with the stream
/// replaced, which is how the archive's 616 are read below: the two give
/// the same outcomes.
#[test]
fn a_damaged_symbol_file_on_disk_draws_less_or_nothing() {
    let files = committed_on_disk();
    assert_eq!(files.len(), 6, "the committed symbol files: {files:?}");
    let mut sweep = Sweep::default();
    let mut in_memory = Sweep::default();
    for file in &files {
        let bytes = std::fs::read(file).unwrap_or_else(|error| panic!("{file:?}: {error}"));
        let label = file.display().to_string();
        sweep_file(&label, &bytes, Through::Bytes, &mut sweep);
        sweep_file(&label, &bytes, Through::Package, &mut in_memory);
    }
    hold(&sweep, "on disk, from the bytes of each damaged copy");
    hold(&in_memory, "on disk, from the parsed file");
    assert_eq!(
        (sweep.pins(), sweep.damages, sweep.no_fault),
        (in_memory.pins(), in_memory.damages, in_memory.no_fault),
        "the two ways of reading a damaged file"
    );
    assert_eq!(sweep.pins(), ON_DISK, "the outcomes, on disk");
}

/// The same for the 616 symbol files of the committed archive.
#[cfg(feature = "refdata-archive")]
#[test]
fn a_damaged_symbol_file_of_an_archive_draws_less_or_nothing() {
    let members = pid_parse::refdata_archive::archive_members(Path::new(ARCHIVE), "sym")
        .expect("the committed archive is readable");
    let mut sweep = Sweep::default();
    for member in &members {
        let label = member.label().display().to_string();
        let bytes = pid_parse::refdata_archive::read_archive_file(member)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        sweep_file(&label, &bytes, Through::Package, &mut sweep);
    }
    hold(&sweep, "the archive");
    assert_eq!(sweep.files, 616, "the symbol files of {ARCHIVE}");
    assert_eq!(sweep.pins(), ARCHIVED, "the outcomes, in the archive");
}

/// The outcomes of [`a_damaged_symbol_file_on_disk_draws_less_or_nothing`]:
/// the stream's role, the kind of damage, what became of the body, how
/// many damages. Exact.
#[rustfmt::skip]
const ON_DISK: &[(Role, &str, Outcome, usize)] = &[
    (Role::Sheet, "a header count one too few", Outcome::Whole, 10),
    (Role::Sheet, "a header count one too many", Outcome::Whole, 8),
    (Role::Sheet, "a header count one too many", Outcome::NotDrawn(NotDrawn::NothingToShow), 2),
    (Role::Sheet, "a header count one too many", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "a length of zero", Outcome::Whole, 52),
    (Role::Sheet, "a length of zero", Outcome::Less, 38),
    (Role::Sheet, "a length of zero", Outcome::NotDrawn(NotDrawn::NothingToShow), 4),
    (Role::Sheet, "a length of zero", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 17),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::Whole, 14),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::Less, 5),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::NothingToShow), 4),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 4),
    (Role::Sheet, "a type word past the class table", Outcome::Whole, 14),
    (Role::Sheet, "a type word past the class table", Outcome::Less, 5),
    (Role::Sheet, "a type word past the class table", Outcome::NotDrawn(NotDrawn::NothingToShow), 4),
    (Role::Sheet, "a type word past the class table", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 4),
    (Role::Sheet, "a wrong magic", Outcome::Whole, 12),
    (Role::Sheet, "cut off after the header", Outcome::Whole, 4),
    (Role::Sheet, "cut off after the header", Outcome::NotDrawn(NotDrawn::NothingToShow), 4),
    (Role::Sheet, "cut off after the header", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "cut off before the last record", Outcome::Whole, 6),
    (Role::Sheet, "cut off before the last record", Outcome::Less, 2),
    (Role::Sheet, "cut off before the last record", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "cut off inside the header", Outcome::Whole, 4),
    (Role::Sheet, "cut off inside the header", Outcome::NotDrawn(NotDrawn::NothingToShow), 6),
    (Role::Sheet, "cut off inside the header", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "emptied of every byte", Outcome::Whole, 4),
    (Role::Sheet, "emptied of every byte", Outcome::NotDrawn(NotDrawn::NothingToShow), 6),
    (Role::Sheet, "emptied of every byte", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "stray bytes at the end", Outcome::Whole, 8),
    (Role::Sheet, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::NothingToShow), 2),
    (Role::Sheet, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Sheet, "the last record one byte too long", Outcome::Whole, 6),
    (Role::Sheet, "the last record one byte too long", Outcome::Less, 2),
    (Role::Sheet, "the last record one byte too long", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 2),
    (Role::Cluster, "a header count one too few", Outcome::Whole, 6),
    (Role::Cluster, "a header count one too many", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "a length of zero", Outcome::NotDrawn(NotDrawn::ClusterShort), 186),
    (Role::Cluster, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::ClusterShort), 18),
    (Role::Cluster, "a type word past the class table", Outcome::NotDrawn(NotDrawn::ClusterShort), 18),
    (Role::Cluster, "a wrong magic", Outcome::Whole, 6),
    (Role::Cluster, "cut off after the header", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "cut off before the last record", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "cut off inside the header", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "emptied of every byte", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "the last record one byte too long", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Cluster, "the root cluster not in the file", Outcome::NotDrawn(NotDrawn::ClusterShort), 6),
    (Role::Styles, "a header count one too few", Outcome::Whole, 6),
    (Role::Styles, "a header count one too many", Outcome::Whole, 6),
    (Role::Styles, "a length of zero", Outcome::Whole, 108),
    (Role::Styles, "a type word flagged 0x4000", Outcome::Whole, 18),
    (Role::Styles, "a type word past the class table", Outcome::Whole, 18),
    (Role::Styles, "a wrong magic", Outcome::Whole, 6),
    (Role::Styles, "cut off after the header", Outcome::Whole, 6),
    (Role::Styles, "cut off before the last record", Outcome::Whole, 6),
    (Role::Styles, "cut off inside the header", Outcome::Whole, 6),
    (Role::Styles, "emptied of every byte", Outcome::Whole, 6),
    (Role::Styles, "stray bytes at the end", Outcome::Whole, 6),
    (Role::Styles, "the last record one byte too long", Outcome::Whole, 6),
];

/// The same for the archive.
#[cfg(feature = "refdata-archive")]
#[rustfmt::skip]
const ARCHIVED: &[(Role, &str, Outcome, usize)] = &[
    (Role::Sheet, "a header count one too few", Outcome::Whole, 1131),
    (Role::Sheet, "a header count one too many", Outcome::Whole, 642),
    (Role::Sheet, "a header count one too many", Outcome::Less, 6),
    (Role::Sheet, "a header count one too many", Outcome::NotDrawn(NotDrawn::NothingToShow), 107),
    (Role::Sheet, "a header count one too many", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 480),
    (Role::Sheet, "a length of zero", Outcome::Whole, 4817),
    (Role::Sheet, "a length of zero", Outcome::Less, 3381),
    (Role::Sheet, "a length of zero", Outcome::NotDrawn(NotDrawn::NothingToShow), 960),
    (Role::Sheet, "a length of zero", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 8587),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::Whole, 1125),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::Less, 263),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::NothingToShow), 477),
    (Role::Sheet, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 1272),
    (Role::Sheet, "a type word past the class table", Outcome::Whole, 1125),
    (Role::Sheet, "a type word past the class table", Outcome::Less, 263),
    (Role::Sheet, "a type word past the class table", Outcome::NotDrawn(NotDrawn::NothingToShow), 477),
    (Role::Sheet, "a type word past the class table", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 1272),
    (Role::Sheet, "a wrong magic", Outcome::Whole, 1235),
    (Role::Sheet, "cut off after the header", Outcome::Whole, 274),
    (Role::Sheet, "cut off after the header", Outcome::Less, 7),
    (Role::Sheet, "cut off after the header", Outcome::NotDrawn(NotDrawn::NothingToShow), 398),
    (Role::Sheet, "cut off after the header", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 452),
    (Role::Sheet, "cut off before the last record", Outcome::Whole, 527),
    (Role::Sheet, "cut off before the last record", Outcome::Less, 75),
    (Role::Sheet, "cut off before the last record", Outcome::NotDrawn(NotDrawn::NothingToShow), 74),
    (Role::Sheet, "cut off before the last record", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 449),
    (Role::Sheet, "cut off inside the header", Outcome::Whole, 315),
    (Role::Sheet, "cut off inside the header", Outcome::Less, 7),
    (Role::Sheet, "cut off inside the header", Outcome::NotDrawn(NotDrawn::NothingToShow), 433),
    (Role::Sheet, "cut off inside the header", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 480),
    (Role::Sheet, "emptied of every byte", Outcome::Whole, 315),
    (Role::Sheet, "emptied of every byte", Outcome::Less, 7),
    (Role::Sheet, "emptied of every byte", Outcome::NotDrawn(NotDrawn::NothingToShow), 433),
    (Role::Sheet, "emptied of every byte", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 480),
    (Role::Sheet, "stray bytes at the end", Outcome::Whole, 642),
    (Role::Sheet, "stray bytes at the end", Outcome::Less, 6),
    (Role::Sheet, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::NothingToShow), 107),
    (Role::Sheet, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 480),
    (Role::Sheet, "the last record one byte too long", Outcome::Whole, 529),
    (Role::Sheet, "the last record one byte too long", Outcome::Less, 75),
    (Role::Sheet, "the last record one byte too long", Outcome::NotDrawn(NotDrawn::NothingToShow), 75),
    (Role::Sheet, "the last record one byte too long", Outcome::NotDrawn(NotDrawn::PickAmongShortSheets), 452),
    (Role::Cluster, "a header count one too few", Outcome::Whole, 658),
    (Role::Cluster, "a header count one too many", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "a length of zero", Outcome::NotDrawn(NotDrawn::ClusterShort), 21879),
    (Role::Cluster, "a type word flagged 0x4000", Outcome::NotDrawn(NotDrawn::ClusterShort), 1974),
    (Role::Cluster, "a type word past the class table", Outcome::NotDrawn(NotDrawn::ClusterShort), 1974),
    (Role::Cluster, "a wrong magic", Outcome::Whole, 658),
    (Role::Cluster, "cut off after the header", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "cut off before the last record", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "cut off inside the header", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "emptied of every byte", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "stray bytes at the end", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "the last record one byte too long", Outcome::NotDrawn(NotDrawn::ClusterShort), 658),
    (Role::Cluster, "the root cluster not in the file", Outcome::NotDrawn(NotDrawn::ClusterShort), 616),
    (Role::Styles, "a header count one too few", Outcome::Whole, 658),
    (Role::Styles, "a header count one too many", Outcome::Whole, 658),
    (Role::Styles, "a length of zero", Outcome::Whole, 14556),
    (Role::Styles, "a length of zero", Outcome::More, 20),
    (Role::Styles, "a type word flagged 0x4000", Outcome::Whole, 1971),
    (Role::Styles, "a type word flagged 0x4000", Outcome::More, 3),
    (Role::Styles, "a type word past the class table", Outcome::Whole, 1971),
    (Role::Styles, "a type word past the class table", Outcome::More, 3),
    (Role::Styles, "a wrong magic", Outcome::Whole, 658),
    (Role::Styles, "cut off after the header", Outcome::Whole, 658),
    (Role::Styles, "cut off before the last record", Outcome::Whole, 657),
    (Role::Styles, "cut off before the last record", Outcome::More, 1),
    (Role::Styles, "cut off inside the header", Outcome::Whole, 658),
    (Role::Styles, "emptied of every byte", Outcome::Whole, 658),
    (Role::Styles, "stray bytes at the end", Outcome::Whole, 658),
    (Role::Styles, "the last record one byte too long", Outcome::Whole, 657),
    (Role::Styles, "the last record one byte too long", Outcome::More, 1),
];

/// One symbol file read from the bytes of a copy with one stream replaced:
/// the way a library reads it.
fn read_with(source: &str, stream: &str, damage: Damage) -> Option<(SymbolGeometry, Vec<u8>)> {
    let bytes = std::fs::read(source).unwrap_or_else(|error| panic!("{source}: {error}"));
    let package =
        PidPackage::from_bytes(&bytes).unwrap_or_else(|error| panic!("{source}: {error}"));
    let original = &package
        .streams
        .get(stream)
        .unwrap_or_else(|| panic!("{source} has no {stream}"))
        .data;
    let damaged = damage.apply(original, &SheetChain::walk(original).starts)?;
    let plan = WritePlan {
        stream_replacements: vec![StreamReplacement {
            path: stream.to_string(),
            new_data: damaged.clone(),
        }],
        ..WritePlan::default()
    };
    let copy = PidWriter::write_to_bytes(&package, &plan)
        .unwrap_or_else(|error| panic!("{source} {stream} {damage:?}: {error}"));
    let body = read_symbol_geometry_from_bytes(&copy)
        .unwrap_or_else(|error| panic!("{source} {stream} {damage:?}: {error}"));
    Some((body, damaged))
}

/// What a body draws, each primitive by the kind of shape it is and what it
/// says, in order; or why it is not drawn.
fn drawn(body: &SymbolGeometry) -> Result<Vec<String>, SymbolNotDrawn> {
    body.displayed().map(|primitives| {
        primitives
            .into_iter()
            .map(|styled| match &styled.primitive {
                SymbolPrimitive::Line { .. } => "line".to_string(),
                SymbolPrimitive::Circle { .. } => "circle".to_string(),
                SymbolPrimitive::Arc { .. } => "arc".to_string(),
                SymbolPrimitive::Polyline { .. } => "polyline".to_string(),
                SymbolPrimitive::BSpline { .. } => "bspline".to_string(),
                SymbolPrimitive::Text { text, .. } => format!("text {text:?}"),
                SymbolPrimitive::Boundary { .. } => "boundary".to_string(),
            })
            .collect()
    })
}

/// The committed pressure gauge, stream by stream and damage by damage,
/// read from the bytes of a damaged copy as a library reads a file.
///
/// The sound gauge draws one circle: the first record of `/Sheet6`, its
/// normal view. `/Sheet68` holds the same circle and the lettering `PG` on
/// a layer nothing hides, and the root cluster's view set of sheet 6 is
/// what keeps a second, heat-trace circle of `/Sheet6` off the page. With
/// `/Sheet6` left out the reader used to draw `/Sheet68` — the circle and
/// `PG` — and with the cluster unread both circles. Neither is drawn now:
/// the gauge is its one circle, or nothing.
#[test]
fn the_pressure_gauge_is_its_circle_or_nothing() {
    let sound = read_symbol_geometry_from_bytes(&std::fs::read(GAUGE).expect("the gauge"))
        .expect("the gauge reads");
    assert_eq!(drawn(&sound), Ok(vec!["circle".to_string()]));
    let circle = Ok(vec!["circle".to_string()]);
    let records = |stream: &str| {
        let package = PidPackage::from_bytes(&std::fs::read(GAUGE).expect("the gauge"))
            .expect("the gauge parses");
        SheetChain::walk(&package.streams[stream].data).starts.len()
    };
    assert_eq!(
        (
            records("/Sheet6"),
            records("/Sheet68"),
            records("/PSMcluster0")
        ),
        (14, 2, 30)
    );

    for stream in ["/Sheet6", "/Sheet68", "/PSMcluster0", "/StyleCluster"] {
        for damage in damages(records(stream)) {
            let Some((body, damaged)) = read_with(GAUGE, stream, damage) else {
                continue;
            };
            let expected = match (stream, damage) {
                // The cluster read short: which sheet and which layers draw
                // cannot be told.
                ("/PSMcluster0", damage) if !damage.loses_no_record() => {
                    Err(SymbolNotDrawn::ClusterShort)
                }
                // The normal view read short of its first record, the
                // circle: it keeps its place and shows nothing.
                (
                    "/Sheet6",
                    Damage::ZeroLength(0)
                    | Damage::Flagged(0)
                    | Damage::NoSuchClass(0)
                    | Damage::CutToHeader
                    | Damage::CutIntoHeader
                    | Damage::Emptied,
                ) => Err(SymbolNotDrawn::NothingToShow),
                // Everything else: the circle.
                _ => circle.clone(),
            };
            assert_eq!(
                drawn(&body),
                expected,
                "the gauge with {stream} damaged, {damage:?}"
            );
            assert_eq!(
                body.chain_faults,
                vec![fault_of(stream, damage, &damaged).expect("a fault")],
                "the gauge with {stream} damaged, {damage:?}: the fault the body lists"
            );
        }
    }

    // The gauge with no root cluster at all: as with the cluster read
    // short, nothing says which sheet and which layers draw. Until
    // 2026-11-13 this file drew two circles and listed no fault.
    let mut package = PidPackage::from_bytes(&std::fs::read(GAUGE).expect("the gauge"))
        .expect("the gauge parses");
    package
        .streams
        .remove(ROOT_CLUSTER)
        .expect("the gauge keeps a root cluster");
    let copy = PidWriter::write_to_bytes(&package, &WritePlan::default())
        .expect("the gauge with no root cluster is written");
    let body = read_symbol_geometry_from_bytes(&copy).expect("the gauge with no root cluster");
    assert_eq!(
        drawn(&body),
        Err(SymbolNotDrawn::ClusterShort),
        "the gauge with no root cluster"
    );
    assert_eq!(
        body.chain_faults,
        vec![StreamChainFault::of_a_missing_stream(ROOT_CLUSTER)],
        "the gauge with no root cluster: the fault the body lists"
    );
}

/// The two faults the chain walk has no word for, in the reader's own.
///
/// The walk reads an empty stream as an empty chain with nothing wrong, and
/// a stream that is not there it is never handed. What a reader of a
/// symbol file lists for them says where and why as any fault does: a
/// stream that stops at its first byte for want of a header, and a stream
/// the file does not hold.
#[test]
fn an_emptied_stream_and_a_missing_one_are_said_in_the_readers_own_words() {
    assert!(SheetChain::walk(&[]).is_sound(), "the walk's word");
    assert_eq!(StreamChainFault::of_stream("/Sheet6", &[]), None);

    let emptied = StreamChainFault::of_an_empty_stream("/Sheet6");
    assert_eq!(
        emptied.describe_framing(),
        "the record chain of /Sheet6 does not hold: it stops at +0 of 0 bytes, where the stream \
         is shorter than the 8-byte header a chain opens with: the chain read 0 record(s) before \
         the stop, and the 0 byte(s) from it on are not read"
    );
    // The same sentence a stream cut off inside its header gets, for the
    // five bytes it has.
    let cut = StreamChainFault::of_stream("/Sheet6", &[0x44, 0xF5, 0x90, 0x6C, 0x00])
        .expect("five bytes are no header");
    assert_eq!(
        cut.describe_framing(),
        emptied
            .describe_framing()
            .replace("+0 of 0 bytes", "+0 of 5 bytes")
            .replace("the 0 byte(s) from it on", "the 5 byte(s) from it on")
    );

    let missing = StreamChainFault::of_a_missing_stream(ROOT_CLUSTER);
    assert_eq!(
        missing.describe_framing(),
        "the record chain of /PSMcluster0 does not hold: the file holds no such stream"
    );
    assert_ne!(emptied.stop, missing.stop);
    for fault in [&emptied, &missing] {
        assert_eq!(
            (fault.stream_len, fault.unread_bytes, fault.records_read),
            (0, 0, 0)
        );
        assert!(fault.describe().starts_with(&fault.describe_framing()));
        // Each survives the trip a page's account takes it on.
        let json = serde_json::to_string(fault).expect("a fault serializes");
        assert_eq!(
            &serde_json::from_str::<StreamChainFault>(&json).expect("and reads back"),
            fault
        );
    }
    assert!(
        serde_json::to_string(&missing)
            .expect("a fault serializes")
            .contains("\"reason\":\"no_stream\""),
        "{missing:?}"
    );
}

/// A body that is the sheet keeping the most strokes: `Circle.sym` keeps
/// its one circle on `/Sheet12`, which no view set governs, beside a normal
/// view `/Sheet6` that is a header and nothing else. Either sheet read
/// short and the pick cannot be made as on the sound file — `/Sheet12`'s
/// count is not known, and `/Sheet6` may have held the stroke that makes it
/// the body — so the symbol is not drawn. It is never drawn from a sheet
/// the sound file would not have picked.
#[test]
fn a_body_picked_by_its_strokes_is_not_drawn_from_a_short_sheet() {
    let sound = read_symbol_geometry_from_bytes(&std::fs::read(CIRCLE).expect("the circle"))
        .expect("the circle reads");
    assert_eq!(drawn(&sound), Ok(vec!["circle".to_string()]));
    for (stream, records, why) in [
        ("/Sheet12", 1, SymbolNotDrawn::PickAmongShortSheets),
        ("/Sheet6", 0, SymbolNotDrawn::NothingToShow),
    ] {
        let mut read = 0;
        for damage in damages(records) {
            let Some((body, _)) = read_with(CIRCLE, stream, damage) else {
                continue;
            };
            read += 1;
            let expected = if damage.loses_no_record() {
                Ok(vec!["circle".to_string()])
            } else {
                Err(why)
            };
            assert_eq!(drawn(&body), expected, "{stream}, {damage:?}");
        }
        assert!(read >= 4, "{stream}: {read} damages read");
    }
}

/// A directory of this test's own under the temporary directory, removed
/// when the guard is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "pid-parse-sym-chain-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A library of two roots that both hold the symbol a placement names. The
/// library answers with the first root's copy, and moves on to the next
/// root only from a copy that holds nothing to draw — a text-only body,
/// say, beside the drawn version in another root. A copy of which nothing
/// was *read* is not that: `Circle.sym` with the one record of `/Sheet12`
/// behind a stop holds no primitive, and until 2026-11-12 the library
/// passed it over and drew the second root's copy, with no fault to say so.
/// It is the library's answer now, with its fault, and is not drawn — the
/// file standing in for the damaged one is no better than the sheet
/// standing in for the damaged one.
///
/// The same copy with `/Sheet12` emptied to no bytes was passed over until
/// 2026-11-13, when the reader came to take an empty stream of a symbol
/// file for one read short ([`a_copy_with_an_emptied_sheet_is_not_passed_over_either`]).
#[test]
fn a_damaged_copy_is_not_passed_over_for_another_roots() {
    let scratch = Scratch::new("two-roots");
    let relative = Path::new("Design").join("Annotation").join("Graphics");
    let stored = r"\\WIN-SPID\Plant\Ref\Symbols\Design\Annotation\Graphics\Circle.sym";
    let (first, second) = (scratch.0.join("first"), scratch.0.join("second"));
    let sound = std::fs::read(CIRCLE).expect("the circle");
    let (_, damaged_sheet) =
        read_with(CIRCLE, "/Sheet12", Damage::ZeroLength(0)).expect("a record to stop on");
    let package = PidPackage::from_bytes(&sound).expect("the circle parses");
    let damaged = PidWriter::write_to_bytes(
        &package,
        &WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: "/Sheet12".to_string(),
                new_data: damaged_sheet.clone(),
            }],
            ..WritePlan::default()
        },
    )
    .expect("the damaged copy is written");
    for (root, bytes) in [(&first, &damaged), (&second, &sound)] {
        let dir = root.join(&relative);
        std::fs::create_dir_all(&dir).expect("the root");
        std::fs::write(dir.join("Circle.sym"), bytes).expect("the copy");
    }
    let fault = StreamChainFault::of_stream("/Sheet12", &damaged_sheet).expect("a fault");

    // The damaged copy first: it is the answer, nothing of it read.
    let mut library = SymbolLibrary::with_roots([&first, &second]);
    let body = library
        .resolve(stored)
        .expect("the library holds the symbol");
    assert_eq!(body.chain_faults, vec![fault]);
    assert!(body.primitives.is_empty(), "{:?}", body.primitives);
    assert_eq!(drawn(body), Err(SymbolNotDrawn::PickAmongShortSheets));

    // The sound copy first: it is the answer, and the damaged one is not
    // read at all.
    let mut library = SymbolLibrary::with_roots([&second, &first]);
    let body = library
        .resolve(stored)
        .expect("the library holds the symbol");
    assert!(body.chain_faults.is_empty(), "{:?}", body.chain_faults);
    assert_eq!(drawn(body), Ok(vec!["circle".to_string()]));
}

/// [`a_damaged_copy_is_not_passed_over_for_another_roots`] for a copy whose
/// one drawn sheet holds no byte. The chain walk finds nothing wrong with
/// such a stream, so the copy read as a sound file with nothing to draw —
/// the very thing a library moves on from — and the second root's file was
/// drawn, with no fault to say so. It is read short now: the first root's
/// copy is the answer, lists the emptied sheet, and is not drawn.
#[test]
fn a_copy_with_an_emptied_sheet_is_not_passed_over_either() {
    let scratch = Scratch::new("two-roots-emptied");
    let relative = Path::new("Design").join("Annotation").join("Graphics");
    let stored = r"\\WIN-SPID\Plant\Ref\Symbols\Design\Annotation\Graphics\Circle.sym";
    let (first, second) = (scratch.0.join("first"), scratch.0.join("second"));
    let sound = std::fs::read(CIRCLE).expect("the circle");
    let package = PidPackage::from_bytes(&sound).expect("the circle parses");
    let emptied = PidWriter::write_to_bytes(
        &package,
        &WritePlan {
            stream_replacements: vec![StreamReplacement {
                path: "/Sheet12".to_string(),
                new_data: Vec::new(),
            }],
            ..WritePlan::default()
        },
    )
    .expect("the copy with an emptied sheet is written");
    for (root, bytes) in [(&first, &emptied), (&second, &sound)] {
        let dir = root.join(&relative);
        std::fs::create_dir_all(&dir).expect("the root");
        std::fs::write(dir.join("Circle.sym"), bytes).expect("the copy");
    }

    let mut library = SymbolLibrary::with_roots([&first, &second]);
    let body = library
        .resolve(stored)
        .expect("the library holds the symbol");
    assert_eq!(
        body.chain_faults,
        vec![StreamChainFault::of_an_empty_stream("/Sheet12")]
    );
    assert!(body.primitives.is_empty(), "{:?}", body.primitives);
    assert_eq!(drawn(body), Err(SymbolNotDrawn::PickAmongShortSheets));

    let mut library = SymbolLibrary::with_roots([&second, &first]);
    let body = library
        .resolve(stored)
        .expect("the library holds the symbol");
    assert!(body.chain_faults.is_empty(), "{:?}", body.chain_faults);
    assert_eq!(drawn(body), Ok(vec!["circle".to_string()]));
}

/// A normal view of several strokes read as far as its chain holds: the
/// angle globe valve draws nine strokes of `/Sheet6`, and with the chain
/// stopped at each record in turn it draws the ones before the stop —
/// first none, which is not a symbol and is not drawn, then one more for
/// every drawable record the walk gets past.
#[test]
fn a_stopped_normal_view_draws_the_strokes_before_the_stop() {
    let sound = read_symbol_geometry_from_bytes(&std::fs::read(VALVE).expect("the valve"))
        .expect("the valve reads");
    let whole = drawn(&sound).expect("the valve is drawn");
    assert_eq!(whole.len(), 9, "{whole:?}");
    let mut seen = Vec::new();
    for stop in 0..13 {
        let (body, _) = read_with(VALVE, "/Sheet6", Damage::ZeroLength(stop)).expect("a record");
        match drawn(&body) {
            Ok(part) => {
                assert_eq!(part, whole[..part.len()], "stopped at record {stop}");
                seen.push(part.len());
            }
            Err(why) => {
                assert_eq!(
                    why,
                    SymbolNotDrawn::NothingToShow,
                    "stopped at record {stop}"
                );
                seen.push(0);
            }
        }
    }
    assert_eq!(
        seen,
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 9, 9],
        "the strokes drawn with the chain stopped at each record in turn"
    );
}
