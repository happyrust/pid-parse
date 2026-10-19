# 2026-10-19 — A boxed `igTextBox` letters from its box's left edge

Scope: the kind-2 `igTextBox` tail (`src/parsers/sheet_records.rs`), the
normalized projection (`PidGraphicKind::Text`), and `pid-plot`'s text
placement. The numbers below are the five-drawing comparison's own three
figures, before and after the change.

## What the comparison said before

Once the vertical anchor landed (`2026-10-15-vertical-anchor-nibble-settles-label-placements.md`)
the largest single class left was a set of title-block fields. 58 unmatched
*our*-side runs across the five drawings, all of them `DWG-…` drawing numbers,
and every one of them out by the same kind of amount — the reference ink
starts a consistent 8.7 mm to the left of ours, except for the one string with
an embedded space (`DWG-0100集 05-35`), which is out by 7.7 mm instead. A
constant offset that shrinks by half the extra width is the signature of an
alignment mismatch, not of a missing glyph.

Read on the plot rather than in the model:

```
                        our ink left      reference ink left
DWG-0100集05-37            92.99 mm            84.33 mm
DWG-0100集 05-35           92.06 mm            84.33 mm
DWG-0100集05-21            90.67 mm            81.96 mm
DWG-0100集05-42            90.08 mm            81.45 mm
```

The reference's left edges are equal for the two widths in a cell, and ours
move by half the string width — ours is centred on the insertion point, theirs
is not.

## The tail was already carrying the answer

`docs/analysis/2026-08-22-igtextbox-tail-kind-2-and-formatting-runs.md` closed
the "kind 2 is a second tail format" question and left the extra bytes open:
32 or 40 of them, not decoded. The native sizer
(`IGDSFactoryTextPointRectShape::GetSize`) states a kind-2 tail as 68 bytes
plus one 8-byte field per bit of an optional flag word, and every title-block
record's flag word (`tail + 36 = 0xC0000000`) sets no optional bit, so its tail
is exactly 68. Reading the whole 68 instead of the 36 stops at the placement
block:

| tail offset | bytes | field |
|---|---:|---|
| `+0` / `+8` | 16 | insertion point |
| `+16` / `+24` | 16 | text direction `(cos, sin)` |
| `+32` | 4 | trailer, anchor code in its last byte |
| `+36` | 4 | optional-field flag word |
| **`+40`** | 8 | **text box width** |
| **`+48`** | 8 | **text box height** |
| `+56` / `+60` / `+64` | 12 | three counters (`0x15`, `0x14`, `0x14` on a 14-character label) |

All 58 records state the same box: **45.307 × 3.689 mm**. The box is a size,
not the string's own advance width — a 14-character and a 15-character label in
the same cell share it, and they are 27.9 mm and 29.6 mm wide.

The insertion point is the box's **middle** — these records all state anchor
code `0x48`, i.e. centre/middle — and the affected strings measure
`insertion − reference ink left = 22.65 ± 0.05 mm`, which is half of 45.307 to
the last digit the reader can resolve. So the reference letters the string from
the box's **left edge**, and 45.307/2 is why the old placement was out by
8.7 mm: half a box minus half a word.

## The rule, stated so it generalises

The anchor code still says where inside the box the insertion point sits; the
run starts at the box's left edge whichever hot point that is. So the horizontal
shift is `box_width × {left 0, centre ½, right 1}`, and a boxed record that
states a left anchor does not move at all. Vertical is untouched — a boxed label
sits on the *letter height's* own anchor line (measured 0.03–0.17 mm), not on
half of the box's 3.689 mm — so the low nibble keeps the meaning the 2026-10-15
page gave it.

Measured on the corpus, per cell, `insertion − reference ink left`:

| boxed record | offset from insertion |
|---|---:|
| 55 of the 58, all four drawings' columns | −22.65 mm (box left) |
| `DWG-0100集05-42`, two rows of drawing A | −5.01 mm |
| `DWG-0000集05-07`, one row of I井场 | −5.01 mm |

The three exceptions are the open end of this page; see below.

## What it moved

Text runs matched, of the reference's total outside the masks:

| drawing | before | after | reference |
|---|---:|---:|---:|
| I井场 | 524 | 525 | 552 |
| A | 125 | **141** | 152 |
| B | 103 | **116** | 127 |
| 低温分离器A | 147 | **159** | 171 |
| 换热器CD | 236 | **249** | 250 |
| total | 1135 | **1190** | 1252 |

Unmatched *our*-side runs fall from 117 to 56. Coverage, extra ink and the fit
do not move: no segment changed, only where 55 labels start.

`pid-plot/tests/plot_compare.rs` is re-ratcheted upward to the new numbers.

## The three records that do not follow

Three boxed records are placed at the box's *right* edge instead —
`insertion − box/2 − (box − string width)`, i.e. `box_right − string_width`.
They share everything with the 55 that do follow: same tail kind, same box, same
anchor code `0x48`, same string in two of the cases. What separates them is
only which row of the table they sit on:

```
A   DWG-0100集05-42   insertion 689.9 mm   reference ink left 684.78
A   DWG-0100集05-37   insertion 693.8 mm   reference ink left 671.07
I井场 DWG-0000集05-07  insertion 1098.0 mm  reference ink left 1093.07
I井场 DWG-0000集05-07  insertion 1091.8 mm  reference ink left 1069.24
```

The same string, in the same column, one row apart, letters differently — so
this is a per-cell property that the record does not carry, not a second reading
of a field we have. Both rows stay mismatched either way, which is why the rule
above is stated for the box's left edge and this is recorded rather than
guessed at.

## What changed

- **Parser** — `SheetIgTextBoxDecoded::text_box_width_m` / `text_box_height_m`,
  read from tail `+40` / `+48` for kind-2 tails with room for them. A
  non-finite, non-positive or absurd pair reports `0.0`, which leaves the
  record placed by its insertion point as before; kind-1 tails carry no box at
  all. Unit tests in `parsers::sheet_records` cover the decode, the absent
  case and the unusable values.
- **Projection** — `DecodedIgTextBoxRecord` mirrors the two fields, and
  `PidGraphicKind::Text` gains `text_box_width`, absent (and so omitted from
  the serialized geometry) wherever there is no box.
- **Plot** — `pid-plot/src/model.rs` shifts the run to the box's left edge and
  anchors it left whenever a box is present.

## Guard rails

- `examples/probe_igtextbox_text_box.rs` — the evidence probe: tail kinds and
  the box per record, five drawings, 58 records, one box.
- `pid-plot`'s `a_boxed_record_letters_from_the_box_left_edge` and
  `a_left_anchored_boxed_record_does_not_move` pin the placement rule, and
  `pid-plot/tests/plot_compare.rs` pins the re-ratcheted numbers.
- `tests/golden/geometry/*.json` is re-blessed: the diff is exactly an added
  `text_box_width` on 24 text entities across four fixtures — the title blocks
  of the test-file corpus, plus boxed unit labels (`℃`, `m`, `MPag`) and one
  multi-line note. Their anchor codes span left, centre and right, so the
  re-bless is where the three cases of the rule are visible at once.
