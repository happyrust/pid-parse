//! The order a sheet displays its objects in.
//!
//! A sheet's graphic records sit in its `Sheet<N>` stream in the order the
//! document handed out their oids, which says nothing about what is drawn
//! over what. The display order is stated elsewhere: the sheet's own object,
//! the `0x0114 JSheet` record of the storage's `PSMcluster0`, lists every
//! object the sheet owns directly — every record of the sheet stream whose
//! parent is the sheet — and it lists them in the order the sheet draws
//! them.
//!
//! # The record
//!
//! ```text
//! +0   u32  oid of the sheet (the `N` of `Sheet<N>`)
//! +4   u32  parent
//! +8   u32  0
//! +12  f64  range scale: 2.0 or 1.0 on the corpus
//! +20  u32  (0, 6, 9, 12 or 16 on the corpus; not read)
//! +24  u32  number of children
//! +28  one 14-byte entry per child:
//!        u32 oid, i16 x0, i16 y0, i16 x1, i16 y1, u16 flags
//! …    164 bytes: the sheet's name and its section's, then a fixed part
//!      that states the sheet's paper ([`decode_sheet_paper`])
//! ```
//!
//! An entry's four words are the child's range box in `scale × 2⁻¹⁵` m —
//! 0.061 mm where the scale is 2.0, 0.031 mm where it is 1.0 — and a child
//! with no extent states `(32767, 32767, −32768, −32768)`. On the eleven
//! acceptance sheets 5 707 of the 5 836 children with straight strokes or
//! circles on the page hold all of them inside their box to within one
//! unit; read in a fixed `2⁻¹⁴` m, none of the 704 on the two sheets that
//! state 1.0 does. The list is what a redraw walks, which is why each entry
//! comes with a box: it is filtered by the view's range.
//!
//! The list is the sheet's own. On all 71 drawings on hand — the eleven
//! acceptance sheets, the 53 of the plant backup and the seven fixtures —
//! it names exactly the records of the sheet stream whose parent is the
//! sheet, 85 836 in all, no more and no fewer.
//!
//! # The order
//!
//! A child is a graphic record that draws itself — a symbol placement, the
//! border frame, a point — or a record that draws nothing and owns the
//! records that do: a `0x00FA DependencyObject` owns the records that
//! depend on another — a label's text boxes, a connector's line string, an
//! arrow head's lines — and a `0x0021 ComplexString` or a `0x007B` Group
//! owns its members. Those are drawn where their owner stands in the list,
//! in the order the owner names them, and what one of them owns in turn is
//! drawn behind it ([`sheet_display_ranks`]).
//!
//! A new object goes to the end of the list and an object sent behind the
//! others to its front, so the list is neither the order of the oids nor
//! the order of the stream: a border frame placed again late in a drawing's
//! life stands hundreds of places down it (`O井场`: place 2 688 of 3 295),
//! and `I井场` lists one ball valve, oid 38159, ahead of its frame, oid 22.
//!
//! The eleven acceptance sheets with a vendor plot say it is the order of
//! drawing
//! (`docs/analysis/2026-11-11-a-sheet-draws-its-children-in-the-order-it-lists-them.md`).
//! On the nine whose plot was made from the file on hand, 98.3 to 99.3 % of
//! each plot's painted elements — 73 682 of 74 763 — are in this order when
//! an element is taken to be the record nearest it, and what is left is ink
//! two records share. Of the 10 928 records those plots paint, 10 429 first
//! appear in the list's order, 1 989 in the order of their oids and 1 990
//! in the order of the stream. `I井场`'s plot draws that valve first.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::sheet_records::{
    decode_igcomplexstring_at, decode_iggroup_at, parse_live_psm_header, SheetChain,
    PSM_TYPE_CODE_DEPENDENCY_OBJECT, PSM_TYPE_CODE_IGCOMPLEXSTRING2D, PSM_TYPE_CODE_IGGROUP,
    PSM_TYPE_CODE_JSHEET,
};

/// Bytes of a `JSheet` payload before its child entries.
pub const SHEET_CHILD_TABLE_AT: usize = 28;

/// Bytes of one child entry.
pub const SHEET_CHILD_ENTRY_LEN: usize = 14;

/// How many decoded tables [`find_sheet_child_table`] offers its caller
/// before it gives up on a cluster.
pub const SHEET_TABLE_CANDIDATES: usize = 64;

/// One object a sheet owns directly, as its `JSheet` record lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetChild {
    /// Oid of the child: a record of the sheet stream whose parent is the
    /// sheet.
    pub oid: u32,
    /// The child's range box, `x0, y0, x1, y1`, in the table's own units
    /// ([`SheetChildTable::unit_m`]). `(32767, 32767, −32768, −32768)` for a
    /// child with no extent.
    pub range: [i16; 4],
    /// The entry's last word, as stored. Not interpreted.
    pub flags: u16,
}

/// The paper a sheet is drawn on, as the tail of its `JSheet` record states
/// it.
///
/// On the 71 files on hand it is one of three: 0.42 × 0.594 m with code 33,
/// 0.594 × 0.841 m with 35, 0.841 × 1.189 m with 37 — ISO A2, A1 and A0 the
/// long way across. The eleven reference plots are printed on exactly this
/// paper, the A2 sheet included, whose border template is 0.33 × 0.57 mm
/// larger than it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetPaper {
    /// Height of the paper, metres.
    pub height_m: f64,
    /// The word between the two sizes, as stored: 33, 35 or 37 on the
    /// corpus. It moves with the size and is read as nothing else.
    pub size_code: u32,
    /// Width of the paper, metres.
    pub width_m: f64,
}

/// The child list of one `0x0114 JSheet` record.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetChildTable {
    /// Oid of the sheet; its records are in the stream `Sheet<oid>`.
    pub sheet_oid: u32,
    /// The `f64` ahead of the count, 2.0 or 1.0 on the corpus: the reach of
    /// a child's range box, in metres either side of the origin.
    pub range_scale: f64,
    /// The sheet's children, in the order the sheet draws them.
    pub children: Vec<SheetChild>,
    /// The paper the record states behind its entries, when it states one
    /// ([`decode_sheet_paper`]).
    pub paper: Option<SheetPaper>,
}

impl SheetChildTable {
    /// The length of one unit of a child's range box, in metres:
    /// `range_scale × 2⁻¹⁵`.
    #[must_use]
    pub fn unit_m(&self) -> f64 {
        self.range_scale / 32768.0
    }
}

/// Decode the payload of a `0x0114 JSheet` record.
///
/// `None` unless the payload holds the fixed part and every entry its count
/// states, and the scale ahead of the count is a positive finite number.
/// Bounds-checked and panic-free on arbitrary bytes.
#[must_use]
pub fn decode_sheet_child_table(payload: &[u8]) -> Option<SheetChildTable> {
    let word = |at: usize| -> Option<u32> {
        let bytes = payload.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let sheet_oid = word(0)?;
    let scale_bytes: [u8; 8] = payload.get(12..20)?.try_into().ok()?;
    let range_scale = f64::from_le_bytes(scale_bytes);
    if !range_scale.is_finite() || range_scale <= 0.0 {
        return None;
    }
    let count = usize::try_from(word(24)?).ok()?;
    let entries_len = count.checked_mul(SHEET_CHILD_ENTRY_LEN)?;
    let end = SHEET_CHILD_TABLE_AT.checked_add(entries_len)?;
    let entries = payload.get(SHEET_CHILD_TABLE_AT..end)?;
    let children = entries
        .as_chunks::<SHEET_CHILD_ENTRY_LEN>()
        .0
        .iter()
        .map(|entry| {
            let short = |at: usize| i16::from_le_bytes([entry[at], entry[at + 1]]);
            SheetChild {
                oid: u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]),
                range: [short(4), short(6), short(8), short(10)],
                flags: u16::from_le_bytes([entry[12], entry[13]]),
            }
        })
        .collect();
    Some(SheetChildTable {
        sheet_oid,
        range_scale,
        children,
        paper: payload.get(end..).and_then(decode_sheet_paper),
    })
}

/// Bytes of a `JSheet` tail ahead of the sheet's name.
const SHEET_TAIL_NAMES_AT: usize = 9;
/// Where the paper stands in the part of the tail behind the two names:
/// an `f64` height, a `u32`, an `f64` width.
const SHEET_PAPER_AT: usize = 61;
/// The largest side a paper is taken to have, metres. An A0 sheet is 1.189.
const SHEET_PAPER_MAX_M: f64 = 10.0;

/// Decode the paper a `JSheet` record states in what follows its entries.
///
/// The tail is nine bytes, the sheet's name and its section's as counted
/// UTF-16 strings, and a fixed part; the paper is 61 bytes into that part.
/// On the corpus the names are always `Sheet1` and `Section1` and the tail
/// 164 bytes, which puts the height at `+102`, the code at `+110` and the
/// width at `+114`; the names are walked rather than assumed, though no file
/// on hand has others to hold the walk to.
///
/// `None` when the tail is too short for that, or either size is not a
/// positive finite length under ten metres. Bounds-checked and panic-free on
/// arbitrary bytes.
#[must_use]
pub fn decode_sheet_paper(tail: &[u8]) -> Option<SheetPaper> {
    let mut at = SHEET_TAIL_NAMES_AT;
    for _name in 0..2 {
        let count = tail.get(at..at.checked_add(2)?)?;
        let units = usize::from(u16::from_le_bytes([count[0], count[1]]));
        at = at.checked_add(2)?.checked_add(units.checked_mul(2)?)?;
    }
    let paper = tail.get(at.checked_add(SHEET_PAPER_AT)?..)?;
    let side = |at: usize| -> Option<f64> {
        let bytes: [u8; 8] = paper.get(at..at + 8)?.try_into().ok()?;
        let metres = f64::from_le_bytes(bytes);
        (metres.is_finite() && metres > 0.0 && metres < SHEET_PAPER_MAX_M).then_some(metres)
    };
    let code: [u8; 4] = paper.get(8..12)?.try_into().ok()?;
    Some(SheetPaper {
        height_m: side(0)?,
        size_code: u32::from_le_bytes(code),
        width_m: side(12)?,
    })
}

/// Find the child table of the sheet `sheet_oid` in a storage's
/// `PSMcluster0`.
///
/// The document storage's `PSMcluster0` is not one contiguous record chain,
/// so the record is looked for where its bytes are: a live `0x0114` envelope
/// whose payload opens with the sheet's oid and decodes
/// ([`decode_sheet_child_table`]). `accept` is asked whether a decoded table
/// is the sheet's — the caller knows the sheet's records and can hold the
/// entries to them — and the first table it accepts is returned.
///
/// At most [`SHEET_TABLE_CANDIDATES`] tables are decoded and offered. A
/// drawing keeps one record per sheet and the odd retired copy; a cluster
/// built to offer thousands, each stating a count as long as the stream,
/// would otherwise cost the square of its length.
#[must_use]
pub fn find_sheet_child_table(
    cluster: &[u8],
    sheet_oid: u32,
    mut accept: impl FnMut(&SheetChildTable) -> bool,
) -> Option<SheetChildTable> {
    let needle = sheet_oid.to_le_bytes();
    let mut from = 0usize;
    let mut offered = 0usize;
    while let Some(found) = find_bytes(cluster.get(from..)?, &needle) {
        let body_start = from + found;
        from = body_start + 1;
        let Some(envelope) = body_start.checked_sub(6) else {
            continue;
        };
        let Some(header) = parse_live_psm_header(cluster, envelope) else {
            continue;
        };
        if header.type_code != PSM_TYPE_CODE_JSHEET {
            continue;
        }
        let Some(end) = body_start.checked_add(header.bytes_to_follow as usize) else {
            continue;
        };
        let Some(payload) = cluster.get(body_start..end) else {
            continue;
        };
        if let Some(table) = decode_sheet_child_table(payload) {
            if accept(&table) {
                return Some(table);
            }
            offered += 1;
            if offered >= SHEET_TABLE_CANDIDATES {
                return None;
            }
        }
    }
    None
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Where a record stands in its sheet's display order.
///
/// Ranks compare the way the sheet draws: by the place of the record's
/// top-level object in the sheet's child list, then by the record's place
/// among the records that object owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisplayRank {
    /// Index, in the sheet's child list, of the record itself when the sheet
    /// owns it directly, and of the listed object it hangs under otherwise.
    pub place: u32,
    /// `0` for a record the sheet owns directly. For a record another
    /// record owns, where a walk of that listed object's records reaches
    /// it, counted from one: each owner's records in the order the owner
    /// names them — the ones it does not name behind those, in stream
    /// order — and each followed by what it owns itself. A label's two
    /// text boxes under their `DependencyObject` are `1` and `2`; under a
    /// Group, a rectangle and its four lines are `1` to `5` and the group's
    /// next member `6`.
    pub inner: u32,
}

/// One record the native reader reads, as the chain walk found it.
struct Live<'a> {
    /// Offset of its envelope.
    at: usize,
    type_code: u16,
    oid: u32,
    /// The oid of the record that owns it, or of the sheet.
    parent: u32,
    payload: &'a [u8],
}

/// The records of a sheet stream the native reader reads, in stream order,
/// as far as the stream's chain walks ([`SheetChain::walk`]).
///
/// A chain that stops gives the records before the stop: each one's length
/// came from the record before it, back to the header, so they are records
/// whatever follows them. A record the native reader skips is an earlier
/// copy of a live one and is left out, as is one too short to state an oid
/// and an owner.
fn live_records(sheet: &[u8]) -> Vec<Live<'_>> {
    let word = |payload: &[u8], at: usize| -> Option<u32> {
        let bytes = payload.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    };
    let mut records = Vec::new();
    for at in SheetChain::walk(sheet).starts {
        let Some(header) = parse_live_psm_header(sheet, at) else {
            continue;
        };
        let Some(end) = header
            .body_start
            .checked_add(header.bytes_to_follow as usize)
        else {
            continue;
        };
        let Some(payload) = sheet.get(header.body_start..end) else {
            continue;
        };
        let (Some(oid), Some(parent)) = (word(payload, 0), word(payload, 4)) else {
            continue;
        };
        records.push(Live {
            at,
            type_code: header.type_code,
            oid,
            parent,
            payload,
        });
    }
    records
}

/// The display rank of every live record of a sheet stream, by the offset
/// its envelope starts at.
///
/// A record whose parent is the sheet takes the place the table lists it at.
/// A record another record owns is drawn with its owner: it takes the place
/// of the listed object it hangs under, and among everything that object
/// owns it stands where a walk of the owners reaches it — each owner's
/// records behind the owner, in the order the owner names them, each
/// followed by its own. Three kinds of owner name what they own:
///
/// * a `0x00FA DependencyObject`, among the other words of its payload;
/// * a `0x0021 ComplexString` and a `0x007B` Group, in the member list that
///   is their payload — the same 14-byte entries the sheet's own list is
///   made of.
///
/// A record its owner does not name, and every record of an owner of
/// another kind, comes after the ones the owner names, in stream order. The
/// A2 reference plot is the witness for a `ComplexString`:
/// `CPECCHB-SQ-A2.igr` lists its sheet outline first and its inner frame —
/// a string of four lines — second, and the plot strokes the outline, then
/// the four lines as one path, then the title block.
///
/// The stream is read as far as its chain walks ([`SheetChain::walk`]). A
/// record before a stop is ranked as on a whole stream; one whose owner
/// lies past the stop keeps the owner's place when the list names the
/// owner, in stream order among the owner's records that were read.
///
/// A record the table does not reach — nothing it hangs under is listed —
/// has no rank and is left out. So is a record the native reader skips: it
/// is an earlier copy of a live one.
#[must_use]
pub fn sheet_display_ranks(sheet: &[u8], table: &SheetChildTable) -> BTreeMap<usize, DisplayRank> {
    let records = live_records(sheet);
    let place: HashMap<u32, u32> = table
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| Some((child.oid, u32::try_from(index).ok()?)))
        .collect();
    // The records each owner owns, by index, in stream order. The sheet's
    // own are placed by its list and are nobody's dependents, whatever oid
    // a record carries.
    let mut owned: HashMap<u32, Vec<usize>> = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        if record.parent != table.sheet_oid {
            owned.entry(record.parent).or_default().push(index);
        }
    }
    // The first record that carries each oid: the one an owner's oid names.
    let mut by_oid: HashMap<u32, usize> = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        by_oid.entry(record.oid).or_insert(index);
    }
    // What `owner` owns, in the order it draws them.
    let drawn_by = |owner: u32| -> Vec<usize> {
        let Some(dependents) = owned.get(&owner) else {
            return Vec::new();
        };
        let named = by_oid
            .get(&owner)
            .map(|index| named_dependents(sheet, &records[*index], dependents, &records))
            .unwrap_or_default();
        in_named_order(dependents, &named, &records)
    };
    let mut ranks = BTreeMap::new();
    let mut ranked = vec![false; records.len()];
    // The sheet's own records, each where the list has it.
    for (index, record) in records.iter().enumerate() {
        if record.parent == table.sheet_oid {
            if let Some(place) = place.get(&record.oid) {
                ranked[index] = true;
                ranks.insert(
                    record.at,
                    DisplayRank {
                        place: *place,
                        inner: 0,
                    },
                );
            }
        }
    }
    // Then what hangs under each object the list names, depth first: each
    // record, and behind it what it owns. The walk starts from the list and
    // not from the records read, so that a child whose own record lies past
    // a stop still places the records it owns that were read.
    for child in &table.children {
        let Some(place) = place.get(&child.oid).copied() else {
            continue;
        };
        let mut next = 1u32;
        let mut pending: Vec<usize> = drawn_by(child.oid).into_iter().rev().collect();
        while let Some(index) = pending.pop() {
            if std::mem::replace(&mut ranked[index], true) {
                continue;
            }
            ranks.insert(records[index].at, DisplayRank { place, inner: next });
            next = next.saturating_add(1);
            pending.extend(drawn_by(records[index].oid).into_iter().rev());
        }
    }
    ranks
}

/// The oids `owner` names as the records it owns, in the order it names
/// them; empty for an owner that names none.
fn named_dependents(
    sheet: &[u8],
    owner: &Live<'_>,
    dependents: &[usize],
    records: &[Live<'_>],
) -> Vec<u32> {
    match owner.type_code {
        PSM_TYPE_CODE_DEPENDENCY_OBJECT => {
            let oids: Vec<u32> = dependents.iter().map(|index| records[*index].oid).collect();
            named_in_order(owner.payload, &oids)
        }
        PSM_TYPE_CODE_IGCOMPLEXSTRING2D => decode_igcomplexstring_at(sheet, owner.at)
            .map(|string| string.members.iter().map(|member| member.oid).collect())
            .unwrap_or_default(),
        PSM_TYPE_CODE_IGGROUP => decode_iggroup_at(sheet, owner.at)
            .map(|group| group.members.iter().map(|member| member.oid).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `dependents` — indices into `records`, in stream order — with the ones
/// `named` names first, in the order it names them, and the rest behind
/// them as the stream holds them.
fn in_named_order(dependents: &[usize], named: &[u32], records: &[Live<'_>]) -> Vec<usize> {
    if named.is_empty() {
        return dependents.to_vec();
    }
    // Where each oid stands among the dependents; an oid two of them carry
    // is taken in stream order.
    let mut slots: HashMap<u32, std::collections::VecDeque<usize>> = HashMap::new();
    for (slot, index) in dependents.iter().enumerate() {
        slots
            .entry(records[*index].oid)
            .or_default()
            .push_back(slot);
    }
    let mut taken = vec![false; dependents.len()];
    let mut order = Vec::with_capacity(dependents.len());
    for oid in named {
        if let Some(slot) = slots
            .get_mut(oid)
            .and_then(std::collections::VecDeque::pop_front)
        {
            taken[slot] = true;
            order.push(dependents[slot]);
        }
    }
    order.extend(
        dependents
            .iter()
            .zip(&taken)
            .filter(|(_, taken)| !**taken)
            .map(|(index, _)| *index),
    );
    order
}

/// The display order of one sheet stream: what [`sheet_display_order`] reads
/// off a storage's `PSMcluster0` and the sheet's own stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SheetDisplayOrder {
    /// Oid of the sheet.
    pub sheet_oid: u32,
    /// How many children the sheet's `JSheet` record lists.
    pub children: usize,
    /// The paper the record states, when it states one.
    pub paper: Option<SheetPaper>,
    /// The rank of every live record of the stream the list reaches, by the
    /// offset its envelope starts at ([`sheet_display_ranks`]).
    pub ranks: BTreeMap<usize, DisplayRank>,
}

/// Read a sheet's display order: its `JSheet` record out of the storage's
/// `PSMcluster0`, held to the sheet's own stream, and every record's rank.
///
/// The table has to be the sheet's. On every stream of the corpora it
/// names exactly the live records of the stream whose parent is the sheet,
/// and a table every one of whose children is such a record is taken
/// first, wherever in the cluster it stands. A stream read short of its
/// list — its chain stops ([`SheetChain::walk`]), or it was cut off at a
/// record — holds fewer records than the list names; the list is then the
/// sheet's when it names every record of the sheet's the walk did read,
/// and it ranks those ([`sheet_display_ranks`]).
///
/// A stream the walk reads no record of the sheet's from, and a cluster
/// that holds no such table, give `None`, and the caller keeps whatever
/// order it had.
#[must_use]
pub fn sheet_display_order(
    cluster: &[u8],
    sheet: &[u8],
    sheet_oid: u32,
) -> Option<SheetDisplayOrder> {
    let mut owned_by_sheet: Vec<u32> = live_records(sheet)
        .iter()
        .filter(|record| record.parent == sheet_oid)
        .map(|record| record.oid)
        .collect();
    if owned_by_sheet.is_empty() {
        return None;
    }
    owned_by_sheet.sort_unstable();
    let whole = find_sheet_child_table(cluster, sheet_oid, |table| {
        !table.children.is_empty()
            && table
                .children
                .iter()
                .all(|child| owned_by_sheet.binary_search(&child.oid).is_ok())
    });
    let table = whole.or_else(|| {
        find_sheet_child_table(cluster, sheet_oid, |table| {
            let mut listed: Vec<u32> = table.children.iter().map(|child| child.oid).collect();
            listed.sort_unstable();
            owned_by_sheet
                .iter()
                .all(|oid| listed.binary_search(oid).is_ok())
        })
    })?;
    Some(SheetDisplayOrder {
        sheet_oid,
        children: table.children.len(),
        paper: table.paper,
        ranks: sheet_display_ranks(sheet, &table),
    })
}

/// The oids of `dependents` in the order `payload` first names each.
///
/// A `DependencyObject` names the records it owns as `u32` oids among its
/// other words, which sit on two-byte boundaries after the twelve-byte
/// sub-header. The payload's grammar is not decoded here; an oid is read
/// wherever a whole `u32` equals one the object owns, which a `u16` field
/// cannot counterfeit unless its neighbour is zero.
fn named_in_order(payload: &[u8], dependents: &[u32]) -> Vec<u32> {
    let owned: HashSet<u32> = dependents.iter().copied().collect();
    let mut named: HashSet<u32> = HashSet::with_capacity(dependents.len());
    let mut order: Vec<u32> = Vec::with_capacity(dependents.len());
    let mut at = 12usize;
    while let Some(bytes) = payload.get(at..at + 4) {
        let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if owned.contains(&value) && named.insert(value) {
            order.push(value);
            at += 4;
        } else {
            at += 2;
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::{
        decode_sheet_child_table, decode_sheet_paper, find_sheet_child_table, sheet_display_ranks,
        DisplayRank, SheetChild, SheetChildTable, SheetPaper, SHEET_TABLE_CANDIDATES,
    };

    /// A `JSheet` payload: the fixed part, `children` entries, and a tail.
    fn sheet_payload(sheet_oid: u32, scale: f64, children: &[(u32, [i16; 4], u16)]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&sheet_oid.to_le_bytes());
        payload.extend_from_slice(&4u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&scale.to_le_bytes());
        payload.extend_from_slice(&9u32.to_le_bytes());
        payload.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for (oid, range, flags) in children {
            payload.extend_from_slice(&oid.to_le_bytes());
            for word in range {
                payload.extend_from_slice(&word.to_le_bytes());
            }
            payload.extend_from_slice(&flags.to_le_bytes());
        }
        payload.extend_from_slice(&sheet_tail("Sheet1", "Section1", [0.594, 0.841], 35));
        payload
    }

    /// What follows the entries: nine bytes, the sheet's name and its
    /// section's as counted UTF-16, and a fixed part of 123 bytes that
    /// states the paper — 164 bytes in all for `Sheet1` / `Section1`, as on
    /// every file of the corpus. The bytes are those of the 闪蒸B sheet.
    fn sheet_tail(sheet: &str, section: &str, paper_m: [f64; 2], code: u32) -> Vec<u8> {
        let mut tail = vec![0x01, 0x01, 0x02, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        for name in [sheet, section] {
            let units: Vec<u16> = name.encode_utf16().collect();
            tail.extend_from_slice(&(units.len() as u16).to_le_bytes());
            for unit in units {
                tail.extend_from_slice(&unit.to_le_bytes());
            }
        }
        let fixed_at = tail.len();
        tail.extend_from_slice(&[0x07, 0x00, 0x00, 0x00]);
        tail.extend_from_slice(&[0u8; 19]);
        tail.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        tail.extend_from_slice(&[0u8; 30]);
        tail.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(tail.len() - fixed_at, 61);
        tail.extend_from_slice(&paper_m[0].to_le_bytes());
        tail.extend_from_slice(&code.to_le_bytes());
        tail.extend_from_slice(&paper_m[1].to_le_bytes());
        tail.extend_from_slice(&0f64.to_le_bytes());
        tail.extend_from_slice(&1f64.to_le_bytes());
        tail.extend_from_slice(&1f64.to_le_bytes());
        tail.extend_from_slice(&[0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00]);
        tail.extend_from_slice(&[0x06, 0x00, 0x00, 0x00, 0x13, 0x00, 0x00, 0x00]);
        assert_eq!(tail.len() - fixed_at, 123);
        tail
    }

    #[test]
    fn the_tail_of_a_jsheet_states_the_paper() {
        // The A1 sheet of the corpus: 164 bytes behind the entries.
        let tail = sheet_tail("Sheet1", "Section1", [0.594, 0.841], 35);
        assert_eq!(tail.len(), 164);
        assert_eq!(
            decode_sheet_paper(&tail),
            Some(SheetPaper {
                height_m: 0.594,
                size_code: 35,
                width_m: 0.841,
            })
        );
        // The table carries it.
        let payload = sheet_payload(6, 2.0, &[(22, [0, 0, 1, 1], 0)]);
        let table = decode_sheet_child_table(&payload).expect("a table");
        assert_eq!(table.paper.map(|paper| paper.width_m), Some(0.841));
        // The sizes stand behind the two names, wherever those end.
        let renamed = sheet_tail("图纸", "A longer section name", [0.42, 0.594], 33);
        assert_ne!(renamed.len(), 164);
        assert_eq!(
            decode_sheet_paper(&renamed).map(|paper| (paper.height_m, paper.width_m)),
            Some((0.42, 0.594))
        );
        // The paper ends 122 bytes into this tail. Cut anywhere short of
        // that the tail states none, and so does one whose sizes are not
        // sizes; the table is still a table.
        for len in 0..tail.len() {
            assert_eq!(
                decode_sheet_paper(&tail[..len]).is_some(),
                len >= 122,
                "{len} bytes"
            );
        }
        for bad in [0.0, -0.594, f64::NAN, f64::INFINITY, 1.0e9] {
            let tail = sheet_tail("Sheet1", "Section1", [bad, 0.841], 35);
            assert_eq!(decode_sheet_paper(&tail), None, "{bad}");
        }
        let mut cut = payload.clone();
        cut.truncate(payload.len() - 60);
        let table = decode_sheet_child_table(&cut).expect("the entries are whole");
        assert_eq!((table.children.len(), table.paper), (1, None));
        // A name count that runs past the tail does not panic.
        let mut overrun = tail.clone();
        overrun[9..11].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(decode_sheet_paper(&overrun), None);
    }

    fn record(type_code: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&type_code.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    /// A graphic record's payload: oid, parent, layer, then padding.
    fn graphic(oid: u32, parent: u32) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&parent.to_le_bytes());
        payload.extend_from_slice(&8u32.to_le_bytes());
        payload.extend_from_slice(&[0u8; 20]);
        payload
    }

    /// A `DependencyObject` payload naming `dependents` among other words,
    /// the way oid 2864 of the 闪蒸B sheet names its two text boxes.
    fn dependency(oid: u32, sheet: u32, driver: u32, dependents: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&sheet.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&[0x00, 0x00, 0x02, 0x00]);
        payload.extend_from_slice(&driver.to_le_bytes());
        payload.extend_from_slice(&[0x01, 0x00]);
        payload.extend_from_slice(&[0x5a, 0x04, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0xfb, 0xff]);
        payload.extend_from_slice(&[0x02, 0x00]);
        for dependent in dependents {
            payload.extend_from_slice(&dependent.to_le_bytes());
            payload.extend_from_slice(&[0x00, 0x00]);
        }
        payload.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x07, 0x00]);
        payload
    }

    fn sheet_stream(records: &[Vec<u8>]) -> Vec<u8> {
        let mut stream = vec![0u8; 8];
        for record in records {
            stream.extend_from_slice(record);
        }
        stream
    }

    #[test]
    fn a_jsheet_payload_lists_its_children_with_their_boxes() {
        // The first three entries of the 闪蒸B sheet: the border frame, the
        // vessel, and the dependency object of one of its nozzles.
        let payload = sheet_payload(
            6,
            2.0,
            &[
                (22, [-12, -12, 13790, 9744], 0x0000),
                (865, [4217, 5378, 7340, 6261], 0x0006),
                (1276, [4776, 6260, 4860, 6323], 0x0021),
                (550, [32767, 32767, -32768, -32768], 0x0000),
            ],
        );
        let table = decode_sheet_child_table(&payload).expect("a table");
        assert_eq!(table.sheet_oid, 6);
        assert!((table.range_scale - 2.0).abs() < 1e-12);
        assert_eq!(
            table.children,
            vec![
                SheetChild {
                    oid: 22,
                    range: [-12, -12, 13790, 9744],
                    flags: 0
                },
                SheetChild {
                    oid: 865,
                    range: [4217, 5378, 7340, 6261],
                    flags: 6
                },
                SheetChild {
                    oid: 1276,
                    range: [4776, 6260, 4860, 6323],
                    flags: 0x21
                },
                SheetChild {
                    oid: 550,
                    range: [32767, 32767, -32768, -32768],
                    flags: 0
                },
            ]
        );
        // The frame's box is the A1 sheet, 841 × 594 mm, and a margin.
        let unit_mm = table.unit_m() * 1000.0;
        assert!((unit_mm - 0.061_035).abs() < 1e-6, "{unit_mm}");
        assert!((f64::from(table.children[0].range[2]) * unit_mm - 841.67).abs() < 0.01);
        assert!((f64::from(table.children[0].range[3]) * unit_mm - 594.73).abs() < 0.01);
    }

    #[test]
    fn a_payload_shorter_than_its_count_is_no_table() {
        let payload = sheet_payload(6, 2.0, &[(22, [0, 0, 1, 1], 0), (23, [0, 0, 1, 1], 0)]);
        // The fixed part alone, the fixed part and half an entry, and a count
        // the payload cannot hold.
        assert!(decode_sheet_child_table(&payload[..27]).is_none());
        assert!(decode_sheet_child_table(&payload[..28 + 20]).is_none());
        let mut too_many = payload.clone();
        too_many[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_sheet_child_table(&too_many).is_none());
        // A scale that is not a positive number.
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut bad = payload.clone();
            bad[12..20].copy_from_slice(&scale.to_le_bytes());
            assert!(decode_sheet_child_table(&bad).is_none(), "{scale}");
        }
        // No entry at all is a table of none.
        let empty = sheet_payload(6, 1.0, &[]);
        assert_eq!(
            decode_sheet_child_table(&empty).map(|table| table.children.len()),
            Some(0)
        );
        // Arbitrary bytes do not panic.
        for len in 0..64 {
            let _ = decode_sheet_child_table(&vec![0xffu8; len]);
        }
    }

    #[test]
    fn the_table_is_found_behind_padding_and_past_a_retired_copy() {
        // The document storage's cluster is not one chain: the record sits
        // among other bytes. An earlier copy the native reader skips (type
        // word `0x8000` set) and a record of another sheet come first.
        let wanted = sheet_payload(6, 2.0, &[(22, [0, 0, 1, 1], 0), (865, [0, 0, 1, 1], 6)]);
        let retired = sheet_payload(6, 2.0, &[(865, [0, 0, 1, 1], 6)]);
        let other = sheet_payload(7, 2.0, &[(30, [0, 0, 1, 1], 0)]);
        let mut cluster = vec![0u8; 37];
        cluster.extend_from_slice(&record(0x0114 | 0x8000, &retired));
        cluster.extend_from_slice(&[0u8; 5]);
        cluster.extend_from_slice(&record(0x0114, &other));
        cluster.extend_from_slice(&[0xaa; 3]);
        cluster.extend_from_slice(&record(0x0114, &wanted));
        cluster.extend_from_slice(&[0u8; 9]);
        let table = find_sheet_child_table(&cluster, 6, |_| true).expect("the live table");
        assert_eq!(
            table.children.iter().map(|c| c.oid).collect::<Vec<_>>(),
            vec![22, 865]
        );
        let other = find_sheet_child_table(&cluster, 7, |_| true).expect("the other sheet");
        assert_eq!(other.children[0].oid, 30);
        // A table the caller refuses is passed over, and no sheet 8 exists.
        assert!(find_sheet_child_table(&cluster, 6, |_| false).is_none());
        assert!(find_sheet_child_table(&cluster, 8, |_| true).is_none());
    }

    #[test]
    fn a_cluster_offers_only_so_many_tables() {
        // A table the caller wants behind `refused` tables it does not.
        let cluster_with = |refused: usize| {
            let mut cluster = Vec::new();
            for _ in 0..refused {
                let decoy = sheet_payload(6, 2.0, &[(1, [0, 0, 1, 1], 0)]);
                cluster.extend_from_slice(&record(0x0114, &decoy));
            }
            let wanted = sheet_payload(6, 2.0, &[(22, [0, 0, 1, 1], 0)]);
            cluster.extend_from_slice(&record(0x0114, &wanted));
            cluster
        };
        let is_wanted = |table: &SheetChildTable| table.children[0].oid == 22;
        let mut asked = 0usize;
        let found = find_sheet_child_table(&cluster_with(SHEET_TABLE_CANDIDATES - 1), 6, |table| {
            asked += 1;
            is_wanted(table)
        });
        assert!(found.is_some(), "the last table within the budget is found");
        assert_eq!(asked, SHEET_TABLE_CANDIDATES);
        // One more refusal and the search stops before it reaches the table.
        let mut asked = 0usize;
        let found = find_sheet_child_table(&cluster_with(SHEET_TABLE_CANDIDATES), 6, |table| {
            asked += 1;
            is_wanted(table)
        });
        assert!(found.is_none());
        assert_eq!(asked, SHEET_TABLE_CANDIDATES);
    }

    /// The 闪蒸B sheet in small. The stream is in oid order; the sheet draws
    /// the vessel, its nozzle, then a gauge and the gauge's two text boxes in
    /// the order the label's dependency object names them — which is not the
    /// order of their oids.
    #[test]
    fn a_record_ranks_where_its_owner_stands_in_the_sheets_list() {
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x003d, &graphic(22, sheet)),  // border frame
            record(0x004d, &graphic(351, 2864)),  // text "LG"
            record(0x00ce, &graphic(726, 1276)),  // nozzle
            record(0x00ce, &graphic(865, sheet)), // vessel
            record(0x00ce, &graphic(921, sheet)), // gauge
            record(0x00fa, &dependency(1276, sheet, 865, &[726])),
            record(0x004d, &graphic(2694, 2864)), // text " 0112101B"
            record(0x00fa, &dependency(2864, sheet, 921, &[2694, 351])),
            // A record whose owner the sheet does not list.
            record(0x0018, &graphic(3000, 2999)),
            // An earlier copy of the vessel the native reader skips.
            record(0x00ce | 0x8000, &graphic(865, sheet)),
        ]);
        let table = decode_sheet_child_table(&sheet_payload(
            sheet,
            2.0,
            &[
                (22, [0, 0, 1, 1], 0),
                (865, [0, 0, 1, 1], 6),
                (1276, [0, 0, 1, 1], 0x21),
                (921, [0, 0, 1, 1], 6),
                (2864, [0, 0, 1, 1], 0),
            ],
        ))
        .expect("a table");
        let ranks = sheet_display_ranks(&stream, &table);
        // By stream offset, in stream order: frame, "LG", nozzle, vessel,
        // gauge, [dependency], " 0112101B", [dependency]. The dependency
        // objects stand in the list themselves and draw nothing; they are
        // ranked like any child.
        let in_stream_order: Vec<DisplayRank> = ranks.values().copied().collect();
        let rank = |place, inner| DisplayRank { place, inner };
        assert_eq!(
            in_stream_order,
            vec![
                rank(0, 0), // frame
                rank(4, 2), // "LG": second of the label's two
                rank(2, 1), // nozzle
                rank(1, 0), // vessel
                rank(3, 0), // gauge
                rank(2, 0), // the nozzle's dependency object
                rank(4, 1), // " 0112101B": first of the label's two
                rank(4, 0), // the label's dependency object
            ]
        );
        // Sorted by rank the records are in the order the sheet draws them.
        let mut drawn: Vec<(DisplayRank, usize)> =
            ranks.iter().map(|(at, rank)| (*rank, *at)).collect();
        drawn.sort();
        let offsets: Vec<usize> = ranks.keys().copied().collect();
        let drawn_as: Vec<usize> = drawn
            .iter()
            .map(|(_, at)| offsets.iter().position(|o| o == at).expect("an offset"))
            .collect();
        // frame, vessel, [dep], nozzle, gauge, [dep], " 0112101B", "LG"
        assert_eq!(drawn_as, vec![0, 3, 5, 2, 4, 7, 6, 1]);
    }

    /// The table is held to the sheet's own stream: a `JSheet` record of the
    /// same oid whose children are not that stream's is another storage's or
    /// not a record at all, and is passed over.
    #[test]
    fn a_table_is_the_sheets_only_when_it_lists_the_sheets_own_records() {
        use super::sheet_display_order;
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x003d, &graphic(22, sheet)),
            record(0x00ce, &graphic(865, sheet)),
            record(0x00ce, &graphic(726, 1276)),
            record(0x00fa, &dependency(1276, sheet, 865, &[726])),
        ]);
        let listing = |children: &[(u32, [i16; 4], u16)]| {
            let mut cluster = vec![0u8; 11];
            cluster.extend_from_slice(&record(0x0114, &sheet_payload(sheet, 2.0, children)));
            cluster
        };
        let own = [
            (865, [0, 0, 1, 1], 6),
            (22, [0, 0, 1, 1], 0),
            (1276, [0, 0, 1, 1], 0x21),
        ];
        let order = sheet_display_order(&listing(&own), &stream, sheet).expect("the order");
        assert_eq!((order.sheet_oid, order.children), (6, 3));
        // The vessel first, then the frame, then the nozzle under its
        // dependency object: four records ranked.
        let ranks: Vec<(u32, u32)> = order
            .ranks
            .values()
            .map(|rank| (rank.place, rank.inner))
            .collect();
        assert_eq!(ranks, vec![(1, 0), (0, 0), (2, 1), (2, 0)]);

        // A child that is not a record of the stream, one the sheet does not
        // own directly, and no child at all: none is this sheet's list.
        // (Each also leaves out a record the stream holds, so none is the
        // list of a stream read short of it either.)
        for refused in [
            vec![(865, [0, 0, 1, 1], 6), (9999, [0, 0, 1, 1], 0)],
            vec![(865, [0, 0, 1, 1], 6), (726, [0, 0, 1, 1], 0)],
            vec![],
        ] {
            assert!(
                sheet_display_order(&listing(&refused), &stream, sheet).is_none(),
                "{refused:?}"
            );
        }
        // The sheet's own table behind one that is refused is still found.
        let mut both = listing(&[(9999, [0, 0, 1, 1], 0)]);
        both.extend_from_slice(&listing(&own));
        assert!(sheet_display_order(&both, &stream, sheet).is_some());
    }

    #[test]
    fn a_dependent_its_owner_does_not_name_comes_after_those_it_names() {
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x0018, &graphic(40, 50)),
            record(0x0018, &graphic(41, 50)),
            record(0x0018, &graphic(42, 50)),
            // Names 42 only.
            record(0x00fa, &dependency(50, sheet, 0, &[42])),
        ]);
        let table = SheetChildTable {
            sheet_oid: sheet,
            range_scale: 2.0,
            children: vec![SheetChild {
                oid: 50,
                range: [0, 0, 1, 1],
                flags: 0,
            }],
            paper: None,
        };
        let ranks: Vec<u32> = sheet_display_ranks(&stream, &table)
            .values()
            .map(|rank| rank.inner)
            .collect();
        // 40 and 41 after 42, in stream order; the owner itself at 0.
        assert_eq!(ranks, vec![2, 3, 1, 0]);
    }

    /// A `ComplexString` payload: the curve sub-header, three bytes, the
    /// member count, the range scale, then one 14-byte entry per member —
    /// the layout of oid 32 of `CPECCHB-SQ-A2.igr`, the template's inner
    /// frame.
    fn complex_string(oid: u32, sheet: u32, members: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&sheet.to_le_bytes());
        payload.extend_from_slice(&8u32.to_le_bytes());
        payload.extend_from_slice(&[0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x01, 0x06, 0x00]);
        payload.extend_from_slice(&(members.len() as u32).to_le_bytes());
        payload.extend_from_slice(&1f64.to_le_bytes());
        for member in members {
            payload.extend_from_slice(&member.to_le_bytes());
            payload.extend_from_slice(&[0x6f, 0x04, 0xd0, 0x00, 0xd3, 0x4a, 0xff, 0x00, 0, 0]);
        }
        payload.extend_from_slice(&[0x00, 0x02, 0x00, 0x00]);
        payload
    }

    /// A Group payload: oid, parent, layer, a word, the member count, the
    /// range scale, then one 14-byte entry per member.
    fn group(oid: u32, sheet: u32, members: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&oid.to_le_bytes());
        payload.extend_from_slice(&sheet.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&(members.len() as u32).to_le_bytes());
        payload.extend_from_slice(&1f64.to_le_bytes());
        for member in members {
            payload.extend_from_slice(&member.to_le_bytes());
            payload.extend_from_slice(&[0, 0, 0, 0, 0x18, 0x4c, 0xd3, 0x35, 0x06, 0x00]);
        }
        payload.extend_from_slice(&[0x00, 0x02, 0x00, 0x00]);
        payload
    }

    fn listing(sheet: u32, oids: &[u32]) -> SheetChildTable {
        SheetChildTable {
            sheet_oid: sheet,
            range_scale: 1.0,
            children: oids
                .iter()
                .map(|oid| SheetChild {
                    oid: *oid,
                    range: [0, 0, 1, 1],
                    flags: 0,
                })
                .collect(),
            paper: None,
        }
    }

    /// `CPECCHB-SQ-A2.igr` in small. Its sheet lists the outline, then the
    /// inner frame — a `ComplexString` of four lines — then the title
    /// block, and the A2 reference plot strokes them in that order: the
    /// outline, the frame's four lines as one path, the first labels.
    #[test]
    fn a_curve_ranks_where_its_complex_string_stands_in_the_order_it_is_listed() {
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x0084, &graphic(31, sheet)), // the outline
            record(0x0021, &complex_string(32, sheet, &[36, 33, 34, 35, 37])),
            record(0x0018, &graphic(33, 32)),
            record(0x0018, &graphic(34, 32)),
            record(0x0018, &graphic(35, 32)),
            record(0x0018, &graphic(36, 32)),
            record(0x0019, &graphic(37, 32)), // a constraint between two of them
            record(0x004d, &graphic(41, sheet)),
        ]);
        let ranks: Vec<(u32, u32)> = sheet_display_ranks(&stream, &listing(sheet, &[31, 32, 41]))
            .values()
            .map(|rank| (rank.place, rank.inner))
            .collect();
        // In stream order. The string lists its last line first, so that
        // line is drawn first of the four; the constraint draws nothing and
        // is ranked like any member.
        assert_eq!(
            ranks,
            vec![
                (0, 0), // the outline
                (1, 0), // the string
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 1), // oid 36, listed first
                (1, 5),
                (2, 0), // the label
            ]
        );
    }

    /// A border of the `-W` templates: a Group of two rectangles, each of
    /// which owns its four lines. What an owner owns is drawn behind it and
    /// ahead of the next thing its own owner lists.
    #[test]
    fn what_a_group_member_owns_is_drawn_with_the_member() {
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x0018, &graphic(10, sheet)),
            record(0x007b, &group(20, sheet, &[22, 21])),
            record(0x0020, &graphic(21, 20)),
            record(0x0020, &graphic(22, 20)),
            record(0x0018, &graphic(23, 21)),
            record(0x0018, &graphic(24, 21)),
            record(0x0018, &graphic(25, 22)),
            record(0x0018, &graphic(26, 22)),
            record(0x0018, &graphic(30, sheet)),
        ]);
        let ranks: Vec<(u32, u32)> = sheet_display_ranks(&stream, &listing(sheet, &[20, 10, 30]))
            .values()
            .map(|rank| (rank.place, rank.inner))
            .collect();
        assert_eq!(
            ranks,
            vec![
                (1, 0), // oid 10
                (0, 0), // the group
                (0, 4), // rectangle 21, listed second
                (0, 1), // rectangle 22, listed first
                (0, 5), // 21's lines, behind it
                (0, 6),
                (0, 2), // 22's lines, behind it
                (0, 3),
                (2, 0), // oid 30
            ]
        );
    }

    /// A record that names itself, or the sheet, as its owner is drawn
    /// once, and a ring of owners with no way in from the sheet not at all.
    #[test]
    fn a_ring_of_owners_is_walked_once() {
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x0018, &graphic(sheet, sheet)), // carries the sheet's own oid
            record(0x0018, &graphic(11, sheet)),
            record(0x00fa, &dependency(12, 12, 0, &[12])), // owns itself
            record(0x00fa, &dependency(13, 14, 0, &[14])),
            record(0x00fa, &dependency(14, 13, 0, &[13])),
        ]);
        let table = listing(sheet, &[sheet, 11, 12, 13]);
        let ranks: Vec<(u32, u32)> = sheet_display_ranks(&stream, &table)
            .values()
            .map(|rank| (rank.place, rank.inner))
            .collect();
        // The first two are the sheet's and stand where the list has them;
        // oid 12 is listed and never read as the sheet's, so what it owns —
        // itself — is drawn at its place; 13 is listed likewise and owns 14,
        // which owns 13.
        assert_eq!(ranks, vec![(0, 0), (1, 0), (2, 1), (3, 2), (3, 1)]);
    }

    /// A stream whose chain stops is ranked as far as it was read: the
    /// records before the stop stand where the list has them, and a record
    /// whose owner lies past the stop is drawn at the owner's place — the
    /// list still names it — as the stream holds it.
    #[test]
    fn a_chain_that_stops_ranks_the_records_before_the_stop() {
        use super::sheet_display_order;
        let sheet = 6;
        let records = [
            record(0x003d, &graphic(22, sheet)),
            record(0x004d, &graphic(351, 2864)), // owned by a record past the stop
            record(0x00ce, &graphic(726, 1276)), // owned by one before it
            record(0x00fa, &dependency(1276, sheet, 865, &[726])),
            record(0x00ce, &graphic(865, sheet)), // the walk stops on this one
            record(0x004d, &graphic(2694, 2864)),
            record(0x00fa, &dependency(2864, sheet, 921, &[2694, 351])),
        ];
        let whole = sheet_stream(&records);
        let mut broken = whole.clone();
        let stop = 8 + records[..4].iter().map(Vec::len).sum::<usize>();
        broken[stop + 2..stop + 6].copy_from_slice(&0u32.to_le_bytes());
        let children = [
            (22, [0, 0, 1, 1], 0),
            (865, [0, 0, 1, 1], 6),
            (1276, [0, 0, 1, 1], 0x21),
            (2864, [0, 0, 1, 1], 0),
        ];
        let mut cluster = vec![0u8; 11];
        cluster.extend_from_slice(&record(0x0114, &sheet_payload(sheet, 2.0, &children)));

        let intact = sheet_display_order(&cluster, &whole, sheet).expect("the order");
        let ranks = |order: &super::SheetDisplayOrder| -> Vec<(usize, u32, u32)> {
            order
                .ranks
                .iter()
                .map(|(at, rank)| (*at, rank.place, rank.inner))
                .collect()
        };
        let at = |index: usize| 8 + records[..index].iter().map(Vec::len).sum::<usize>();
        assert_eq!(
            ranks(&intact),
            vec![
                (at(0), 0, 0),
                (at(1), 3, 2), // "351", named second by its owner
                (at(2), 2, 1),
                (at(3), 2, 0),
                (at(4), 1, 0),
                (at(5), 3, 1),
                (at(6), 3, 0),
            ]
        );
        // The same list, held to the four records the walk read. The label
        // box keeps its owner's place and is the only one of the owner's
        // that was read.
        let short =
            sheet_display_order(&cluster, &broken, sheet).expect("the order, as far as read");
        assert_eq!(
            (short.children, short.paper),
            (intact.children, intact.paper)
        );
        assert_eq!(
            ranks(&short),
            vec![(at(0), 0, 0), (at(1), 3, 1), (at(2), 2, 1), (at(3), 2, 0)]
        );
        // The stream cut off where the walk stopped reads the same.
        let cut = &whole[..stop];
        let cut_order =
            sheet_display_order(&cluster, cut, sheet).expect("the order of a cut stream");
        assert_eq!(ranks(&cut_order), ranks(&short));
        // A stream the walk reads nothing of has no order.
        let mut dead = whole.clone();
        dead[10..14].copy_from_slice(&0u32.to_le_bytes());
        assert!(sheet_display_order(&cluster, &dead, sheet).is_none());
    }

    /// A list that names records the stream does not hold is the sheet's
    /// only when it names every record the stream does hold; and a list
    /// that names exactly the stream's records is taken ahead of it.
    #[test]
    fn a_list_longer_than_the_stream_is_the_sheets_when_it_names_all_the_stream_holds() {
        use super::sheet_display_order;
        let sheet = 6;
        let stream = sheet_stream(&[
            record(0x003d, &graphic(22, sheet)),
            record(0x00ce, &graphic(865, sheet)),
        ]);
        let cluster_of = |tables: &[&[u32]]| {
            let mut cluster = vec![0u8; 5];
            for oids in tables {
                let children: Vec<(u32, [i16; 4], u16)> =
                    oids.iter().map(|oid| (*oid, [0, 0, 1, 1], 0)).collect();
                cluster.extend_from_slice(&record(0x0114, &sheet_payload(sheet, 2.0, &children)));
                cluster.extend_from_slice(&[0u8; 3]);
            }
            cluster
        };
        let places = |cluster: &[u8]| -> Option<Vec<u32>> {
            sheet_display_order(cluster, &stream, sheet)
                .map(|order| order.ranks.values().map(|rank| rank.place).collect())
        };
        // Longer than the stream, and naming both of its records.
        assert_eq!(
            places(&cluster_of(&[&[900, 865, 22, 901]])),
            Some(vec![2, 1])
        );
        // Longer than the stream and missing one of its records: not this
        // sheet's list.
        assert_eq!(places(&cluster_of(&[&[900, 865, 901]])), None);
        // The list of exactly the stream's records wins, wherever it stands.
        assert_eq!(
            places(&cluster_of(&[&[900, 865, 22, 901], &[865, 22]])),
            Some(vec![1, 0])
        );
        assert_eq!(
            places(&cluster_of(&[&[865, 22], &[900, 22, 865, 901]])),
            Some(vec![1, 0])
        );
    }

    #[test]
    fn a_stream_the_walk_reads_nothing_of_ranks_nothing() {
        let table = listing(6, &[22]);
        assert!(sheet_display_ranks(&[], &table).is_empty());
        assert!(sheet_display_ranks(&[0u8; 7], &table).is_empty());
        // A first record whose length runs past the stream.
        let mut stream = vec![0u8; 8];
        stream.extend_from_slice(&[0xce, 0x00, 0xff, 0xff, 0x00, 0x00, 0x01]);
        assert!(sheet_display_ranks(&stream, &table).is_empty());
        for len in 0..80 {
            let _ = sheet_display_ranks(&vec![0x5au8; len], &table);
        }
    }
}
