//! Low-level decoder for the `JSheetLayer` records stored in each
//! `PSMcluster0` record chain.

use crate::parsers::cluster_header::decode_psm_cluster0_body_records;

/// PSM type code for `JSheetLayer Object`.
pub const PSM_TYPE_CODE_JSHEET_LAYER: u16 = 0x0081;
/// PSM type code for `JSheetLayerManager Object`.
pub const PSM_TYPE_CODE_JSHEET_LAYER_MANAGER: u16 = 0x0042;

/// One fully decoded `JSheetLayer` record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetLayerDecoded {
    /// Full record byte range in the containing `PSMcluster0` stream.
    pub byte_range: std::ops::Range<usize>,
    /// Storage-local persistent object id.
    pub oid: u32,
    /// Storage-local parent reference from the PSM envelope.
    pub parent_ref: u32,
    /// Number of graphic objects the layer declares.
    pub object_count: u32,
    /// Layer number assigned by the writer (`u32::MAX` means unassigned).
    pub layer_number: u32,
    /// Authored sheet-layer name.
    pub name: String,
    /// Optional secondary authored name; empty throughout the current corpus.
    pub secondary_name: Option<String>,
    /// Final persisted word following both UTF-16 strings.
    pub trailing_word: u32,
}

/// One `JSheetLayerManager` identity from the same record chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetLayerManagerDecoded {
    /// Full record byte range in the containing `PSMcluster0` stream.
    pub byte_range: std::ops::Range<usize>,
    /// Storage-local persistent object id.
    pub oid: u32,
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn utf16_field(data: &[u8], at: usize) -> Option<(String, usize)> {
    let count = usize::try_from(u32_at(data, at)?).ok()?;
    if count > 128 {
        return None;
    }
    let bytes_start = at.checked_add(4)?;
    let bytes_len = count.checked_mul(2)?;
    let bytes_end = bytes_start.checked_add(bytes_len)?;
    let raw = data.get(bytes_start..bytes_end)?;
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let value = String::from_utf16(&units).ok()?;
    Some((value, bytes_end))
}

fn decode_layer_payload(
    payload: &[u8],
    byte_range: std::ops::Range<usize>,
) -> Option<SheetLayerDecoded> {
    let oid = u32_at(payload, 0)?;
    let parent_ref = u32_at(payload, 4)?;
    let object_count = u32_at(payload, 12)?;
    let layer_number = u32_at(payload, 16)?;
    let (name, after_name) = utf16_field(payload, 20)?;
    if name.is_empty() {
        return None;
    }
    let (secondary_name, after_secondary) = utf16_field(payload, after_name)?;
    let trailing_word = u32_at(payload, after_secondary)?;
    if after_secondary.checked_add(4)? != payload.len() {
        return None;
    }
    Some(SheetLayerDecoded {
        byte_range,
        oid,
        parent_ref,
        object_count,
        layer_number,
        name,
        secondary_name: (!secondary_name.is_empty()).then_some(secondary_name),
        trailing_word,
    })
}

/// Decode every `JSheetLayer` in a complete `PSMcluster0` chain.
///
/// The shared cluster walker enforces full-chain coverage; malformed layer
/// payloads are skipped individually without changing the record walk.
pub fn decode_sheet_layers(data: &[u8]) -> Vec<SheetLayerDecoded> {
    decode_psm_cluster0_body_records(data)
        .into_iter()
        .filter(|record| record.type_code == PSM_TYPE_CODE_JSHEET_LAYER)
        .filter_map(|record| decode_layer_payload(&record.raw_payload, record.byte_range))
        .collect()
}

/// Decode the identities of every `JSheetLayerManager` in a complete
/// `PSMcluster0` chain.
pub fn decode_sheet_layer_managers(data: &[u8]) -> Vec<SheetLayerManagerDecoded> {
    decode_psm_cluster0_body_records(data)
        .into_iter()
        .filter(|record| record.type_code == PSM_TYPE_CODE_JSHEET_LAYER_MANAGER)
        .filter_map(|record| {
            Some(SheetLayerManagerDecoded {
                byte_range: record.byte_range,
                oid: u32_at(&record.raw_payload, 0)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(name: &str, secondary: &str) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&12u32.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&103u32.to_le_bytes());
        out.extend_from_slice(&7u32.to_le_bytes());
        for value in [name, secondary] {
            let units: Vec<u16> = value.encode_utf16().collect();
            out.extend_from_slice(&(units.len() as u32).to_le_bytes());
            for unit in units {
                out.extend_from_slice(&unit.to_le_bytes());
            }
        }
        out.extend_from_slice(&9u32.to_le_bytes());
        out
    }

    #[test]
    fn layer_payload_decodes_both_names_and_requires_exact_tail() {
        let raw = payload("Default", "Alias");
        let decoded = decode_layer_payload(&raw, 10..10 + raw.len()).expect("valid layer");
        assert_eq!(decoded.oid, 12);
        assert_eq!(decoded.object_count, 103);
        assert_eq!(decoded.layer_number, 7);
        assert_eq!(decoded.name, "Default");
        assert_eq!(decoded.secondary_name.as_deref(), Some("Alias"));
        assert_eq!(decoded.trailing_word, 9);

        let mut with_tail = raw;
        with_tail.push(0);
        assert!(decode_layer_payload(&with_tail, 0..with_tail.len()).is_none());
    }

    #[test]
    fn empty_secondary_name_is_not_promoted() {
        let raw = payload("Labels", "");
        let decoded = decode_layer_payload(&raw, 0..raw.len()).expect("valid layer");
        assert_eq!(decoded.secondary_name, None);
    }
}
