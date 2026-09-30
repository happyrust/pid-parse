//! Low-level decoders for individual binary / XML fragments.
//!
//! Each submodule is a focused parser for one stream layout or
//! record family (cluster headers, `DocVersion` records, dynamic
//! attribute records, drawing XML, `JProperties`, magic sniffing,
//! PSM table rows, relationship / sheet probes, string scans,
//! tagged text lists, XML helpers). They are deliberately
//! stateless and side-effect-free so they are easy to unit-test.
//!
//! `parsers` is **not** intended as a public API surface for
//! end-users; consumers should go through the orchestrating layer
//! in [`crate::streams`] (or the top-level [`crate::api`] / CFB
//! [`crate::cfb`] entry points) instead. This module is `pub` only
//! so internal crates and tests can reach the primitives directly.

pub mod app_object;
pub mod cluster_header;
pub mod doc_version;
pub mod doc_version2;
pub mod drawing_xml;
pub mod dynamic_attr_records;
pub mod general_xml;
pub mod jproperties;
pub mod jsites_list;
pub mod magic;
pub mod psm_tables;
pub mod relationship_probe;
pub mod sheet_endpoint_records;
pub mod sheet_layers;
pub mod sheet_probe;
pub mod sheet_records;
pub mod string_scan;
pub mod summary;
pub mod tagged_stg_list;
pub mod undecoded_census;
pub mod view_filter_sets;
pub mod xml_util;

/// Upper bound for a pre-allocation whose size is a count read off disk:
/// never more elements than the bytes still unread could hold
/// (OCS spec `pid-import-next-round`, requirement 4.9 / design D2).
///
/// `count` is the stated element count, `remaining_bytes` the bytes left
/// from where the elements start to the end of the record or stream, and
/// `min_elem_bytes` the fewest bytes one element can occupy on the wire
/// (0 is read as 1). The result only caps a capacity hint: a `Vec` that
/// needs more still grows, so a clamp can lower an allocation but never
/// change what a decoder returns. What it stops is a corrupt count --
/// up to `u32::MAX` -- turning into one allocation that aborts on
/// wasm32 before the decoder reaches the bytes that would refuse it.
/// Usage: `Vec::with_capacity(bounded_capacity(count, end - at, 13))`.
pub(crate) fn bounded_capacity(
    count: usize,
    remaining_bytes: usize,
    min_elem_bytes: usize,
) -> usize {
    count.min(remaining_bytes / min_elem_bytes.max(1))
}

/// The `N` bytes at `off`, or `None` when they run past the end of `data`.
///
/// Backs the little-endian readers of `cluster_header`,
/// `dynamic_attr_records`, `sheet_endpoint_records` and `sheet_probe`, which
/// read 0 on `None`. Their callers bound-check `off` first. The fallback is
/// there only because such a guard (`at + len > data.len()`, `len` off disk)
/// wraps on a 32-bit `usize` (wasm32), where a raw index past the end would
/// panic and abort the page. Valid input never reaches it, so no decoded
/// result moves.
pub(crate) fn le_bytes<const N: usize>(data: &[u8], off: usize) -> Option<[u8; N]> {
    data.get(off..off.checked_add(N)?)?.try_into().ok()
}

#[cfg(test)]
mod bounded_capacity_tests {
    use super::bounded_capacity;

    /// Seed of the random triples (P-D22: fixed, so a failure reproduces).
    const SEED: u64 = 0x5EED_2403;

    /// Random triples drawn after the edge grid.
    const RANDOM_CASES: usize = 10_000;

    /// Zero, one, two, the top of `usize` and the widths an on-disk count
    /// comes in.
    const EDGES: [usize; 7] = [
        0,
        1,
        2,
        usize::MAX,
        usize::MAX - 1,
        u32::MAX as usize,
        u16::MAX as usize,
    ];

    /// Seeded xorshift64 (P-D22, no test dependency). The seed goes
    /// through splitmix64 first, and `| 1` keeps the state non-zero.
    struct XorShift64(u64);

    impl XorShift64 {
        fn new(seed: u64) -> Self {
            let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            Self((z ^ (z >> 31)) | 1)
        }

        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }

        /// In `[0, n)` for `n > 0`. The modulo bias does not matter here.
        fn below(&mut self, n: usize) -> usize {
            (self.next_u64() % n as u64) as usize
        }

        /// One component: an edge value, a full-width `usize`, or a value
        /// of at most 64, so the element-size division has small divisors
        /// to round with.
        fn component(&mut self) -> usize {
            match self.below(3) {
                0 => EDGES[self.below(EDGES.len())],
                1 => self.next_u64() as usize,
                _ => self.below(65),
            }
        }
    }

    /// Asserts both halves of Property 3 for one triple and returns the
    /// capacity; the case index and the triple go into the failure message.
    fn assert_bounded(
        case: usize,
        count: usize,
        remaining_bytes: usize,
        min_elem_bytes: usize,
    ) -> usize {
        let capacity = bounded_capacity(count, remaining_bytes, min_elem_bytes);
        assert!(
            capacity <= count,
            "case {case}: bounded_capacity({count}, {remaining_bytes}, {min_elem_bytes}) = {capacity} > count {count}"
        );
        let elem = min_elem_bytes.max(1);
        let bytes = capacity as u128 * elem as u128;
        assert!(
            bytes <= remaining_bytes as u128,
            "case {case}: bounded_capacity({count}, {remaining_bytes}, {min_elem_bytes}) = {capacity}, and {capacity} x {elem} B = {bytes} B > remaining {remaining_bytes} B"
        );
        capacity
    }

    /// Feature: pid-import-next-round, Property 3: 预分配有界
    ///
    /// **Validates: Requirements 4.9**
    ///
    /// Cases 0 to 342 are every triple of `EDGES` (7 cubed); the next
    /// `RANDOM_CASES` are drawn from `SEED`, one component at a time.
    #[test]
    fn capacity_fits_count_and_remaining_bytes() {
        let mut case = 0;
        for &count in &EDGES {
            for &remaining_bytes in &EDGES {
                for &min_elem_bytes in &EDGES {
                    assert_bounded(case, count, remaining_bytes, min_elem_bytes);
                    case += 1;
                }
            }
        }

        let mut rng = XorShift64::new(SEED);
        let (mut clamped, mut unclamped) = (0, 0);
        for _ in 0..RANDOM_CASES {
            let count = rng.component();
            let remaining_bytes = rng.component();
            let min_elem_bytes = rng.component();
            if assert_bounded(case, count, remaining_bytes, min_elem_bytes) < count {
                clamped += 1;
            } else {
                unclamped += 1;
            }
            case += 1;
        }
        // The draws land on both sides of the `min`, so neither half of the
        // property passes only because its side never ran.
        assert!(
            clamped > 0 && unclamped > 0,
            "{RANDOM_CASES} random cases: {clamped} clamped, {unclamped} unclamped"
        );
    }

    #[test]
    fn capacity_examples() {
        // A corrupt `u32::MAX` count over 100 bytes of 13-byte elements.
        assert_eq!(bounded_capacity(u32::MAX as usize, 100, 13), 7);
        // `min_elem_bytes` 0 reads as 1, so a small count stands.
        assert_eq!(bounded_capacity(5, 100, 0), 5);
    }
}

#[cfg(test)]
mod le_bytes_tests {
    use super::le_bytes;

    #[test]
    fn reads_in_range_and_refuses_past_the_end() {
        let data = [1, 2, 3, 4, 5];
        assert_eq!(le_bytes::<4>(&data, 0), Some([1, 2, 3, 4]));
        assert_eq!(le_bytes::<4>(&data, 1), Some([2, 3, 4, 5]));
        assert_eq!(le_bytes::<4>(&data, 2), None);
        assert_eq!(le_bytes::<2>(&data, 5), None);
        // Offsets a wrapped 32-bit guard can hand over: `off + N` overflows
        // too, and the read refuses instead of panicking.
        assert_eq!(le_bytes::<4>(&data, usize::MAX - 1), None);
        assert_eq!(le_bytes::<2>(&data, usize::MAX), None);
    }
}
