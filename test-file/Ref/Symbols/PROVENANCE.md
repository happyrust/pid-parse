# Project symbol provenance

Like the fixtures in `test-file/symbols/PROVENANCE.md`, these `.sym` files come
from user-owned SmartPlant reference data and are here for internal testing.
They grant no redistribution rights; confirm the underlying project and vendor
licence before publishing them outside this repository.

They are the XaLNG plant's own symbols that `test-file/工艺管道及仪表流程-1.pid`
names and that neither the five-drawing corpus's `Ref` nor the bundled
`test-file/symbols-full` carries. That fixture is the package's
`180700/气化区/工艺管道及仪表流程图（一）.pid` byte for byte (SHA-256
`a3cceb23bc50c0b0188ed6524fe6215f9eb0ad4c19f8e235a3ce09256249fb38`); the
template its frame links is `test-file/Ref/Template Files/XA-A1.igr`.

Source archive: `RefData~4~681.zip` of the `PLANT_XaLNG_1_1_3_p` backup package
(on this machine under `D:\pid\Programming Help`; 2,855 entries). The package
keeps the same files exploded beside the archive, with the same hashes.

| Repository fixture | ZIP entry | Size | SHA-256 |
|---|---|---:|---|
| `Design/Annotation/Labels/Xa Item Note & Label.sym` | same path | 94,208 | `5552ab94a6f7f7c9ae5c5c1101c32a03fc0b705eef4afb11b22e426c8530fbd7` |
| `Piping/Piping OPC's/Xa chu.sym` | same path | 45,056 | `1a94242cc04424cec80b0b059d06e1f586695e6d6de15dd4076ca07d7d6a72bf` |
| `Piping/Piping OPC's/Xa.sym` | same path | 45,056 | `1d863e8df3061d6e06c3e3481785b6cc783bb58fa722646cd6480a93fc689004` |

`pid-plot`'s symbol search takes this directory after the bundled library
(`symbol_library` in `pid-plot/src/model.rs`), so these three names resolve in
any checkout; no other library in the tree carries them.
