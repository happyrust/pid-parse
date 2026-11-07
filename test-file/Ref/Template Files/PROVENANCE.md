# Border-template fixture provenance

Like the symbol fixtures in `test-file/symbols/PROVENANCE.md`, these `.igr`
border templates come from user-owned SmartPlant reference data and are here for
internal parser testing. They grant no redistribution rights; confirm the
underlying project and vendor licence before publishing them outside this
repository.

| Repository fixture | ZIP entry | Size | SHA-256 |
|---|---|---:|---|
| `CPECCHB-SQ-A2.igr` | `CPECCHB-SQ-A2.igr` | 81,920 | `b4911b6466cd1f2792b98c03d4d66bdaf8b0dee42456b1463986e717c09284bb` |
| `CPE-SQ-A2.igr` | — (copied from the corpus) | 61,440 | `a36d62d6acb30a4a2d4843723b08cd867670fc7f4d553b5179b8bec253bf7e26` |
| `XA-A1.igr` | `XA-A1.igr` (another package, below) | 434,176 | `637d3968e52ebda0b98b68d19cdcfe248b6eabb89607335746ba03fd93c65a28` |

Source archive: `RefData~4~682.zip` at the root of a backup package (on this
machine, `D:\work\cad\pid-test-data`). The same archive carries
`CPECCHB-SQ-A2.pid`, the template's own authored sheet; that file is not
committed — `pid-plot/tests/border_template.rs` lays the `.igr` out through
`test-file/D06.pid` instead.

`CPE-SQ-A2.igr` was copied on 2026-11-01 from
`D:\work\cad\PID自研软件验证\Ref\Template Files\CPE-SQ-A2.igr`, the corpus's own
copy, so that `A01`'s embedded A2 frame has a template to answer it inside the
repository (§2.11 of the pid-plot issue). It is the same family as the A0/A1
files beside it; the corpus, unlike this directory, has carried it all along.

`XA-A1.igr` comes from a different backup package, the XaLNG plant's
`PLANT_XaLNG_1_1_3_p` (on this machine under `D:\pid\Programming Help`), as
the `XA-A1.igr` member of its own `RefData~4~682.zip`; the package keeps the
same file exploded at its root. `test-file/工艺管道及仪表流程-1.pid` is that
package's `180700/气化区/工艺管道及仪表流程图（一）.pid` byte for byte (SHA-256
`a3cceb23bc50c0b0188ed6524fe6215f9eb0ad4c19f8e235a3ce09256249fb38`), so this is
the template its frame links. Copied on 2026-11-07; the symbols the same
drawing names from that package are in `test-file/Ref/Symbols`.

`CPE-SQ-A0.igr` and `CPE-SQ-A1.igr` predate this note; their source is not
recorded here.
