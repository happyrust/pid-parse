# Border-template fixture provenance

Like the symbol fixtures in `test-file/symbols/PROVENANCE.md`, these `.igr`
border templates come from user-owned SmartPlant reference data and are here for
internal parser testing. They grant no redistribution rights; confirm the
underlying project and vendor licence before publishing them outside this
repository.

| Repository fixture | ZIP entry | Size | SHA-256 |
|---|---|---:|---|
| `CPECCHB-SQ-A2.igr` | `CPECCHB-SQ-A2.igr` | 81,920 | `b4911b6466cd1f2792b98c03d4d66bdaf8b0dee42456b1463986e717c09284bb` |

Source archive: `RefData~4~682.zip` at the root of a backup package (on this
machine, `D:\work\cad\pid-test-data`). The same archive carries
`CPECCHB-SQ-A2.pid`, the template's own authored sheet; that file is not
committed — `pid-plot/tests/border_template.rs` lays the `.igr` out through
`test-file/D06.pid` instead.

`CPE-SQ-A0.igr` and `CPE-SQ-A1.igr` predate this note; their source is not
recorded here.
