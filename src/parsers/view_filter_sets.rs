//! Low-level decoder for the `0x0057 Top ViewFilterSet` records stored in
//! each `PSMcluster0` record chain: the per-view layer display state.
//!
//! A view filter set belongs to one `JSheet` (`+16`) and carries, for the
//! layers that sheet's `JSheetLayerManager` registers, two bitmaps indexed by
//! layer number -- the first is the display state, the second reads as the
//! locate (selectability) state -- plus a table of `{name, layer number}`
//! naming the layers it governs. The record closes exactly under this layout
//! on every one of the 53 records of the reference corpus (four main
//! fixtures + A01); see `docs/analysis/2026-09-14-viewfilterset-carries-the-layer-display-state.md`.
//!
//! ```text
//! +0   u32 oid ; +4 u32 0 ; +8 u32 0 ; +12 u32 2 ; +16 u32 JSheet ; +20 u32 1 ;
//! +24  u32 1 ; +28 u32 0 ; +32 u32 active layer number ; +36 u16 0
//! then 6 x { u8 0xFF ; u16 len ; len bytes }        -- bitmaps over layer numbers
//! then u16 n ; u16 2 ; n x override                 -- per-layer display overrides
//!      override = { u16 layer ; u8 kind ; u8 1 ; u16 0 ;
//!                   [kind & 2: u32 COLORREF ; f64 line width] ; u32 }
//! then 12 x u8 0
//! then u32 count ; count x { u32 chars ; UTF-16 name ; u16 layer number }
//! ```
//!
//! Evidence grade: corpus. The bitmaps' meaning is read off the corpus
//! (`Hidden` / `HiddenObjects` off on every top-level sheet, `Default` on
//! everywhere, `Dimension` / `Construction` off in every symbol definition,
//! `WaterMark` displayed but not locatable on one sheet); the field names of
//! the override entry are this crate's, pending the native reader in
//! `viewfil.dex`.

use crate::parsers::cluster_header::decode_psm_cluster0_body_records;

/// PSM type code for `Top ViewFilterSet Object` (the per-sheet set).
pub const PSM_TYPE_CODE_VIEW_FILTER_SET: u16 = 0x0057;

/// Number of length-prefixed bitmaps a set carries.
const BITMAPS: usize = 6;
/// Zero bytes between the override table and the layer table.
const RESERVED: usize = 12;

/// One fully decoded `Top ViewFilterSet` record.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewFilterSetDecoded {
    /// Full record byte range in the containing `PSMcluster0` stream.
    pub byte_range: std::ops::Range<usize>,
    /// Storage-local persistent object id.
    pub oid: u32,
    /// Storage-local id of the `JSheet` this set is the view state of.
    pub sheet_ref: u32,
    /// Payload `+32`. Equals the layer number of the layer named `Default`
    /// on all 53 corpus records; read as the active layer's number.
    pub active_layer_number: u32,
    /// Bitmap 1: bit `n` set means layer number `n` is displayed.
    pub display: Vec<u8>,
    /// Bitmap 2: bit `n` set reads as layer number `n` being locatable.
    /// Identical to [`Self::display`] on most sheets; differs where a layer
    /// is shown but not selectable, or hidden but still selectable.
    pub locate: Vec<u8>,
    /// Bitmaps 3..6, raw. All ones and of constant size (2, 2, 1, 1 bytes)
    /// across the corpus; unread.
    pub further_bitmaps: Vec<Vec<u8>>,
    /// Per-layer display overrides, in record order.
    pub overrides: Vec<LayerDisplayOverrideDecoded>,
    /// The layers the set governs: `(name, layer number)`, in record order.
    pub layers: Vec<ViewFilterSetLayerDecoded>,
}

/// One entry of the set's layer table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewFilterSetLayerDecoded {
    /// Authored layer name.
    pub name: String,
    /// The layer's number: the bit both bitmaps are read at.
    pub layer_number: u16,
}

/// One per-layer display override. Only [`Self::layer_number`] has a
/// corpus-level reading with a control; the rest is carried as read.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerDisplayOverrideDecoded {
    /// The layer the override applies to.
    pub layer_number: u16,
    /// Kind byte; `3` carries a colour and a width, `1` carries neither.
    pub kind: u8,
    /// Win32 `COLORREF` (`0x00BBGGRR`) when the kind carries one.
    pub colour: Option<u32>,
    /// Line width in metres when the kind carries one.
    pub line_width: Option<f64>,
    /// Closing word of the entry (`11` beside a colour, `0` without).
    pub trailing_word: u32,
}

impl ViewFilterSetDecoded {
    /// Whether layer number `n` is displayed, or `None` when the bitmap does
    /// not reach that far.
    pub fn is_displayed(&self, layer_number: u16) -> Option<bool> {
        bit(&self.display, layer_number)
    }

    /// Whether layer number `n` is locatable, under the second bitmap's
    /// reading; `None` when the bitmap does not reach that far.
    pub fn is_locatable(&self, layer_number: u16) -> Option<bool> {
        bit(&self.locate, layer_number)
    }
}

fn bit(bitmap: &[u8], layer_number: u16) -> Option<bool> {
    let byte = bitmap.get(usize::from(layer_number / 8))?;
    Some(byte & (1 << (layer_number % 8)) != 0)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(data: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// Decode one payload under the layout above; `None` the moment the bytes
/// refuse it, including a record that does not end exactly where the layer
/// table does.
fn decode_payload(
    payload: &[u8],
    byte_range: std::ops::Range<usize>,
) -> Option<ViewFilterSetDecoded> {
    for (at, expected) in [(4, 0), (8, 0), (12, 2), (20, 1), (24, 1), (28, 0)] {
        if u32_at(payload, at)? != expected {
            return None;
        }
    }
    if u16_at(payload, 36)? != 0 {
        return None;
    }
    let oid = u32_at(payload, 0)?;
    let sheet_ref = u32_at(payload, 16)?;
    let active_layer_number = u32_at(payload, 32)?;

    let mut at = 38usize;
    let mut bitmaps = Vec::with_capacity(BITMAPS);
    for _ in 0..BITMAPS {
        if *payload.get(at)? != 0xFF {
            return None;
        }
        let len = usize::from(u16_at(payload, at + 1)?);
        let start = at.checked_add(3)?;
        let end = start.checked_add(len)?;
        bitmaps.push(payload.get(start..end)?.to_vec());
        at = end;
    }

    let override_count = usize::from(u16_at(payload, at)?);
    if u16_at(payload, at + 2)? != 2 {
        return None;
    }
    at += 4;
    let mut overrides = Vec::with_capacity(override_count);
    for _ in 0..override_count {
        let layer_number = u16_at(payload, at)?;
        let kind = *payload.get(at + 2)?;
        if *payload.get(at + 3)? != 1 || u16_at(payload, at + 4)? != 0 {
            return None;
        }
        at += 6;
        let (colour, line_width) = if kind & 2 != 0 {
            let colour = u32_at(payload, at)?;
            let width = f64_at(payload, at + 4)?;
            at += 12;
            (Some(colour), Some(width))
        } else {
            (None, None)
        };
        let trailing_word = u32_at(payload, at)?;
        at += 4;
        overrides.push(LayerDisplayOverrideDecoded {
            layer_number,
            kind,
            colour,
            line_width,
            trailing_word,
        });
    }

    if payload
        .get(at..at + RESERVED)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return None;
    }
    at += RESERVED;

    let count = usize::try_from(u32_at(payload, at)?).ok()?;
    at += 4;
    let mut layers = Vec::with_capacity(count.min(256));
    for _ in 0..count {
        let chars = usize::try_from(u32_at(payload, at)?).ok()?;
        if chars > 128 {
            return None;
        }
        let start = at.checked_add(4)?;
        let end = start.checked_add(chars.checked_mul(2)?)?;
        let units: Vec<u16> = payload
            .get(start..end)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        let name = String::from_utf16(&units).ok()?;
        if name.is_empty() {
            return None;
        }
        let layer_number = u16_at(payload, end)?;
        layers.push(ViewFilterSetLayerDecoded { name, layer_number });
        at = end + 2;
    }
    if at != payload.len() {
        return None;
    }

    let mut bitmaps = bitmaps.into_iter();
    let display = bitmaps.next()?;
    let locate = bitmaps.next()?;
    Some(ViewFilterSetDecoded {
        byte_range,
        oid,
        sheet_ref,
        active_layer_number,
        display,
        locate,
        further_bitmaps: bitmaps.collect(),
        overrides,
        layers,
    })
}

/// Decode every `Top ViewFilterSet` in a complete `PSMcluster0` chain.
///
/// The shared cluster walker enforces full-chain coverage; a payload that
/// refuses the layout is skipped individually, so a caller comparing this
/// count with the raw `0x0057` count sees exactly which records refused.
pub fn decode_view_filter_sets(data: &[u8]) -> Vec<ViewFilterSetDecoded> {
    decode_psm_cluster0_body_records(data)
        .into_iter()
        .filter(|record| record.type_code == PSM_TYPE_CODE_VIEW_FILTER_SET)
        .filter_map(|record| decode_payload(&record.raw_payload, record.byte_range))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(name: &str) -> Vec<u8> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let mut out = (units.len() as u32).to_le_bytes().to_vec();
        for unit in units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    /// A top-level set the way D06 writes one, reduced to three layers:
    /// `Default` #0 on, `Labels` #1 on, `Hidden` #5 off; `Hidden` shown but
    /// not locatable would be a different sheet, so here locate = display.
    fn payload(with_override: bool, trailing: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for word in [19u32, 0, 0, 2, 6, 1, 1, 0, 0] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
        for bitmap in [
            vec![0b1101_1111u8, 0xFF],
            vec![0b1101_1111u8, 0xFF],
            vec![0xFF, 0xFF],
            vec![0xFF, 0xFF],
            vec![0xFF],
            vec![0xFF],
        ] {
            out.push(0xFF);
            out.extend_from_slice(&(bitmap.len() as u16).to_le_bytes());
            out.extend_from_slice(&bitmap);
        }
        out.extend_from_slice(&(u16::from(with_override)).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        if with_override {
            out.extend_from_slice(&15u16.to_le_bytes());
            out.push(3);
            out.push(1);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0x0094_9494u32.to_le_bytes());
            out.extend_from_slice(&0.00018f64.to_le_bytes());
            out.extend_from_slice(&11u32.to_le_bytes());
        }
        out.extend_from_slice(&[0u8; 12]);
        out.extend_from_slice(&3u32.to_le_bytes());
        for (name, number) in [("Default", 0u16), ("Labels", 1), ("Hidden", 5)] {
            out.extend_from_slice(&utf16(name));
            out.extend_from_slice(&number.to_le_bytes());
        }
        out.extend_from_slice(trailing);
        out
    }

    #[test]
    fn a_set_decodes_its_bitmaps_overrides_and_layer_table() {
        let raw = payload(true, &[]);
        let set = decode_payload(&raw, 0..raw.len()).expect("valid set");
        assert_eq!(set.oid, 19);
        assert_eq!(set.sheet_ref, 6);
        assert_eq!(set.active_layer_number, 0);
        assert_eq!(set.display, [0b1101_1111, 0xFF]);
        assert_eq!(set.further_bitmaps.len(), 4);
        assert_eq!(
            set.layers,
            [
                ViewFilterSetLayerDecoded {
                    name: "Default".into(),
                    layer_number: 0
                },
                ViewFilterSetLayerDecoded {
                    name: "Labels".into(),
                    layer_number: 1
                },
                ViewFilterSetLayerDecoded {
                    name: "Hidden".into(),
                    layer_number: 5
                },
            ]
        );
        assert_eq!(set.is_displayed(0), Some(true));
        assert_eq!(set.is_displayed(5), Some(false));
        assert_eq!(set.is_displayed(16), None, "past the bitmap");
        assert_eq!(set.is_locatable(5), Some(false));
        assert_eq!(set.overrides.len(), 1);
        let override_ = &set.overrides[0];
        assert_eq!(override_.layer_number, 15);
        assert_eq!(override_.kind, 3);
        assert_eq!(override_.colour, Some(0x0094_9494));
        assert_eq!(override_.line_width, Some(0.00018));
        assert_eq!(override_.trailing_word, 11);
    }

    #[test]
    fn a_set_without_overrides_decodes_and_a_stray_byte_refuses() {
        let raw = payload(false, &[]);
        let set = decode_payload(&raw, 0..raw.len()).expect("valid set");
        assert!(set.overrides.is_empty());
        assert_eq!(set.layers.len(), 3);

        let with_tail = payload(false, &[0]);
        assert!(
            decode_payload(&with_tail, 0..with_tail.len()).is_none(),
            "the account must close exactly"
        );
    }

    #[test]
    fn a_wrong_constant_or_bitmap_flag_refuses() {
        let mut raw = payload(false, &[]);
        raw[12..16].copy_from_slice(&3u32.to_le_bytes());
        assert!(decode_payload(&raw, 0..raw.len()).is_none(), "form number");

        let mut raw = payload(false, &[]);
        raw[38] = 0x00;
        assert!(decode_payload(&raw, 0..raw.len()).is_none(), "bitmap flag");
    }
}
