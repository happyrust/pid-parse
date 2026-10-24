# 2026-10-20 — A pad is not part of the label

The class table's `Center/Middle` row had been reporting **−0.053 body heights**
along the run on the I drawing (437 labels, 0.13 mm at a 2.5 mm body) since the
mean started being reported. Every one of those labels paired inside the 1 mm
window, so nothing in the residual list showed it. This note records what the
figure was made of and what moved it.

## Two readings, and the one that fit

A centred run starts at `insertion − width/2`, so a run that starts left of the
reference's has two possible causes:

1. the reference steps back by a different width — it centres on **ink**, or on
   a narrower font, so its start sits right of ours; or
2. the anchor rule itself is off by a constant.

`examples/probe_center_anchor_width.rs` separates them. It pairs runs the way
the comparator does (same text, within a millimetre, after the fit) and prints,
per class, the along-axis error beside **half the advance difference** — the
error a pure run-width mismatch would give on its own. On the I drawing:

| class | n | along | half advance difference | residual |
|---|---:|---:|---:|---:|
| `Center/Middle` | 435 | −0.193 mm | −0.176 mm | −0.017 mm |

Nearly all of it was width, not the rule. The residual — 0.005 body heights —
is the anchor rule's own error, and it is zero within noise.

## Where the extra width came from

It was not the font. Of the 546 paired runs in the I drawing, **141 carried
whitespace the drawing had stored**: 105 with a leading space (`" 1001102A"`),
36 with a trailing one, some with both. The writer already trimmed the tail —
that is what `a_padded_label_right_aligns_on_its_inked_end` recorded — but the
head was measured in. A centre anchor steps back by half the width it measures,
so a leading space moved the run 0.41 mm (0.11 body) left of where the visible
glyphs belong.

The reference plot letters the visible glyphs. For the L drawing's
`0103501A` tags the reference's own span is `" 0103501A"` and it centres on that
whole advance, 13.64 mm; ours measured 14.23 mm with the pad and 13.32 mm
without, so dropping the pad lands 0.15 mm from the reference's start where
keeping it landed 0.29 mm. The pad is the field's storage, not the label.

## What changed

`show_text` now trims **both ends** before the run is measured or drawn, and a
run that trims to nothing writes no text at all. `TextOffsetClass` gained
`mean_advance_heights` — our advance minus the reference's, in reference body
heights — and both printers show it as `width`, so a width difference can never
again be read as a placement error. The class ratchet now holds `along` to
**0.05** body heights (it was 0.10 for both axes); `down` stays at 0.10 because
`symbol/Middle` measures 0.094.

Class means after the change:

| drawing | `Center/Middle` along | width | labels |
|---|---:|---:|---:|
| I井场 | −0.026 | +0.034 | 437 |
| 集注站 A | −0.039 | +0.065 | 74 |
| 集注站 B | −0.037 | +0.030 | 67 |
| 集注站 L | −0.034 | +0.054 | 97 |
| 集注站 H | −0.029 | +0.058 | 189 |

## What is left

`width` still reads **+0.03 to +0.07** body heights on `Center/Middle` — our
face sets the same label a percent or two wider than the reference's, on digits
more than on letters (`NC` matches to 0.04%, `1001102A` does not). Half of that
difference is the −0.02 to −0.04 now showing in `along`; the anchor rule's own
share is an order of magnitude smaller. That is a font-metric gap, not a rule:
the next round has to compare our per-glyph advances against the face the
reference plot actually embedded.

## The two labels the L drawing lost

The trim made our two `0103501A` tags touch: they sit 13.22 mm apart and the
nine characters now ink 13.32 mm. Our reader glues runs across a gap it cannot
see, so it reads one 18-character span and neither reference tag can pair with
it. PyMuPDF splits at every text object, which is why the reference keeps its
pair apart even though *its* runs overlap by 0.43 mm. Both tags are drawn where
the reference puts them; the count is 165 of 167 because this crate's reader
has no text-object signal to split on. The baseline records the blind spot.

## Guard rails

* `padding_at_either_end_is_dropped_before_the_anchor_is_measured` — a run
  padded on both sides anchors exactly where the tight run does, for a centre
  and a right anchor.
* `a_run_of_spaces_writes_no_text` — a value that trims to nothing draws
  nothing.
* `a_wider_run_reports_its_width_beside_its_shift` — 1 mm of extra advance on a
  2.5 mm body reports `width +0.4` and `along −0.2`, so the two figures cannot
  be confused.
* `five_drawings_hold_the_ratchet` — `along` ≤ 0.05 body heights per class with
  ten or more labels. Before this round the I drawing's `Center/Middle` sat at
  −0.053 and the gate would have failed.
