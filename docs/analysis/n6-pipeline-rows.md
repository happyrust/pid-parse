# N6 管线行：I井场 PIDPipeline / PipingConnectors 对上三张表（2026-10-12）

> 任务 t-1985。只读 `D:\work\cad\pid-test-data\Export.dmp`（`EXPORT:V12.01`，74,489,856 字节）。不跑 `imp`，不连 Oracle，不改 `src`，不重写 `D:\work\cad\PID自研软件验证\publish`。
> 探针 `examples/probe_oracle_exp_pipeline.rs`。`cargo run --example probe_oracle_exp_pipeline` 退出 0。
> 既有测试 NOT RUN：只读探针，没有改 `src`，没有跑 `cargo test`。

## 一句话

`T_PIPELINE` 和 `T_PLANTITEMGROUP` 是空表。`T_PIPERUN` 有 4,686 行，I井场 106 条 `PipingConnectors` 的管段端全部是 `SP_ID`，79 个 `PIDPipeline` UID 在这三张表的列里是 0。同一行拼不出两端，有序和对调都是 0。

## 行格式

解码从 `examples/probe_oracle_exp_rows.rs` 拷来，口径见 `docs/analysis/n5-exp-rows.md`。`APPLICATIONS` 第一行是 `ID=1`、`NAME=SmartPlant PID`、`SHORTNAME=SPPID`，一共 5 行。钉不住探针就退出 1。字符列是字符集 2000、形式 2（AL16UTF16，UTF-16BE）。`0xFFFE` 是 NULL，`0xFFFF` 是表尾。

## 图纸

三张表都没有图纸列，`DocUID` `5633AF4121EE4F4D9FE78E90722CC050` 在单元格里是 0 次。I井场行不靠图纸列挑：参考这份 XML 里的 `PIDPipingConnector` UID 能落进 `T_PIPERUN.SP_ID` 的行算这张图的管段行（169 行，其中 106 行是 `PipingConnectors` 的 `UID1`）。`PIDPipeline` 的 79 个 UID 对不上任何单元格，所以管线行是 0。另外 4 张集注站图不在这份参考里，剩下的 4,517 行 `T_PIPERUN` 不算进 I井场，也不往下分。

## 三张表

| 表 | 行 | 列 | 非空列 |
|---|---:|---:|---:|
| `T_PIPELINE` | 0 | 5 | 0 |
| `T_PIPERUN` | 4,686 | 293 | 12 |
| `T_PLANTITEMGROUP` | 0 | 6 | 0 |

空表的表头仍然读到了。`T_PIPELINE`：`SP_ID`、`OPERFLUIDCODE`、`TAGSEQUENCENO`、`TAGSUFFIX`、`FLUIDSYSTEM`。`T_PLANTITEMGROUP`：`SP_ID`、`ISOWNER`、`PLANTITEMGROUPTYPE`、`PLANTITEMGROUPCLASS`、`SP_WSSITEID`、`ISSHARED`。0 行，所以这 11 列全空。

`T_PIPERUN` 非空列：

| 列 | 类型 | 非空 |
|---|---|---:|
| `SP_ID` | 字符 | 4,686 |
| `NOMINALDIAMETER` | 数 | 2,648 |
| `INSULPURPOSE` | 数 | 21 |
| `OPERFLUIDCODE` | 数 | 3,604 |
| `PIPINGMATERIALSCLASS` | 字符 | 3,564 |
| `PIPERUNTYPE` | 数 | 4,686 |
| `TAGSEQUENCENO` | 字符 | 3,135 |
| `FLOWDIRECTION` | 数 | 1,563 |
| `FLUIDSYSTEM` | 数 | 3,620 |
| `PIPERUNCLASS` | 数 | 4,686 |
| `INSULATIONSPEC` | 字符 | 1,497 |
| `SP_CONNECTORSZEROLENGTH` | 数 | 4,686 |

另外 281 列全空。字符列都是字符集 2000、形式 2。

## 管线 UID

参考 `PIDPipeline` 79 个。单元格按去掉花括号、连字符、空白再转大写之后，长度 32 的十六进制才算同一个 UID。

| 表 | 命中 | 缺 |
|---|---:|---:|
| `T_PIPELINE` | 0 | 79 |
| `T_PIPERUN` | 0 | 79 |
| `T_PLANTITEMGROUP` | 0 | 79 |
| 三张表合计 | 0 | 79 |

缺的就是这 79 个全部。没有「在别的列里、只是没叫 `SP_ID`」的命中。

## 组成关系

参考 `PipingConnectors` 106 条，两端类全是 `PIDPipingConnector` → `PIDPipeline`。`UID2` 去重之后就是上面那 79 个管线 UID（`pipeline_uid_set_eq_rel_uid2=true`）。参考里另有 `PIDPipingConnector` 对象 169 个，比这 106 条多出来的 63 个没有这条关系；169 个对象 UID 也都在 `T_PIPERUN.SP_ID` 里。

两端是否能由这三张表的列拼出：同一行里两个不同的列，规范化后等于 `UID1` 和 `UID2`，顺序对调也算。

| 表 | 有序 | 对调 | 两种里有一种 |
|---|---:|---:|---:|
| `T_PIPELINE` | 0 | 0 | 0 |
| `T_PIPERUN` | 0 | 0 | 0 |
| `T_PLANTITEMGROUP` | 0 | 0 | 0 |
| 三张表合计 | 0 | 0 | 0 |

管段端单独能对上：106 个 `UID1` 都是 `T_PIPERUN.SP_ID`，缺 0。管线端对不上：79 个 `UID2` 在三张表里是 0。没有第二列可以和对调，所以拼出是 0。没有再去打开别的表。

## 复现

```powershell
cargo run --example probe_oracle_exp_pipeline
```

可选两个参数：dump、参考 `_Data.xml`。缺省就是上面这两个路径。

## 全表搜（2026-10-12，t-23）

> 上面三张表的结论不动。这一节把同一份 dump 里其余的表也打开。探针 `examples/probe_oracle_exp_pipeline_scan.rs`。`cargo run --example probe_oracle_exp_pipeline_scan` 退出 0。既有测试 NOT RUN：没有改 `src`，没有跑 `cargo test`。

行解码从 `examples/probe_oracle_exp_rows.rs` 拷来，口径仍是上面的「行格式」。拷贝上补了三种原来没遇到的列：类型 12（DATE，表头多一个 `u16`，格子仍是 `u16` 长度）、类型 113（长列：列顺序里先放一段短数据，这一行其余列写完再跟剩余载荷）、类型 `0xA0`（字符列，表头只有字符集和形式，没有单独的最大长度；格子仍是 `u16` 长度，`0xFFFE` 是 NULL，形式 2 按 UTF-16BE 解）。标识列 = 列名里带 `ID` 的字符列。UID 比对沿用上一节：去花括号、连字符、空白，转大写，长度 32 的十六进制。

同一行的有序 / 对调按列的先后：前面的列等于 `UID1`、后面的列等于 `UID2` 是有序；反过来是对调。106 条的 `UID2` 和 79 个 `PIDPipeline` 是同一批（`pipeline_uid_set_eq_rel_uid2`）。

| 项 | 数 |
|---|---:|
| `INSERT` | 154 |
| 按行读完 | 154 |
| 没读成 | 0 |
| 标识列 | 264 |
| 其中字符集形式 2 | 264 |
| 79 个管线 UID 命中 | 0 |
| 79 个管线 UID 缺 | 79 |
| 106 条同一行有序 | 0 |
| 106 条同一行对调 | 0 |
| 106 条同一行两种里有一种 | 0 |

154 个 `INSERT`、126 个表名（字典表如 `APPLICATIONS`、`MAX_ID` 有第二份甚至第四份）。探针打的是 `inserts=154 insert_names=126`。`T_MODELITEM`、`T_REPRESENTATION`、`T_PLANTITEM` 都在里面，标识列命中是 0，没有单独再列。没有任何一列命中，所以没有「表 / 列 / 行数」可以填。

`T_DRAWINGVERSION` 15 行。`SP_PROJECTNAME` 就是类型 `0xA0`，字符集 2000、形式 2，这 15 行全是 NULL（`rows=15 SP_PROJECTNAME non_null=0 charset=Some(2000) form=Some(2)`）。这张表名字里带 `ID` 的字符列是 `SP_ID`、`SP_DRAWINGID`、`SP_REVISIONID`，已经算进上面 264 列。整份 dump 的字节查找是 `byte_scan utf16be=0 ascii=0 raw16=0 of=79`。

解码钉：`T_PIPERUN.SP_ID` 仍是 106/106 个 `PipingConnectors` 的 `UID1`。管线端在全部标识列里是 0，同一行拼不出两端。

结论：这份 dump 里没有管线。

```powershell
cargo run --example probe_oracle_exp_pipeline_scan
```

可选两个参数：dump、参考 `_Data.xml`。缺省与上一节相同。

## 合成（2026-10-14，N9-2）

上面两节的结论不变：这份 dump 里没有管线行。发布时按 `PipeLinePublisher` 的口径，从本图已经发出的 `PIDPipingConnector` 合成。实现在 `src/publish/pipeline.rs` 的 `attach_pipelines`。

- 连接器 UID 对上 `T_PIPERUN.SP_ID`。`TAGSEQUENCENO` 为空的不入线。
- 名字是 `{公称直径}-{流体代码}-{TAGSEQUENCENO}-{PIPINGMATERIALSCLASS}[-{INSULATIONSPEC}]`。公称直径在 1 到 4999 写成 `{n} mm`；大于等于 5000 写成 `(代码 − 5000) / 32` 的带分数英寸，约分后是 `1"`、`3/4"`、`1-1/2"`。直径空着就留空槽，例如 `-PG-1001105-F9`。保温规格空着就不加最后一段。
- 流体代码查 `CODELISTS` 19 的短文本。查不到就省掉这一段并计数。同名合并成一条 `PIDPipeline`。
- UID 是 UUID v5：RFC 4122 的 DNS 名字空间上取 `pid-parse/pipeline`，再混上厂名，再混上管线名。同厂同名得到同一个 UID，32 位大写十六进制。
- `FluidSystem` 从 `SPPIDDataMap.xml`（或 `RefData~4~804.zip` 里的这一条）把码映成 `@{GUID}`。不给映射、码为空、或映射缺了，这一项就不写。不写 `OperFluidCode`。
- 关系 UID 是 `PCN-{连接器}-{管线}`，`DefUID` 为 `PipingConnectors`，`UID1` 是连接器，`UID2` 是管线。
- `--diff-against` 对管线和 `PipingConnectors` 按名字配对。参考里空着的 `FluidSystem` 单列，不算错。没有 `DrawingNo` 的空图不挂管线。不给 `--oracle-exp`、也不给备份目录时，`.pid` 模式不走这条。

I井场钉在 `ijingchang_oracle_exp_matches_the_pinned_reference`：对象 3 093、值 3 346、关系 4 107，按名 79 / 106，`FluidSystem` 多 23，流体代码缺 0、映射缺 0。本轮没重跑这条。

整包（2026-10-14）：`pid_publish_xml D:\work\cad\pid-test-data --plant P01` 退出 0。53 / 53，失败 0，空图 2（`test/U01/D03`、`D04`，管线 0）。51 张合计对象 80 085、值 87 419、关系 106 705、管线 2 395、`PipingConnectors` 3 128。对象比没挂管线时的 77 690 多 2 395，关系比 103 577 多 3 128。管道端口 9 557、信号端口 22 021、连通 19 371、OPC 476、管口 372、注释 114 不变。`cargo test --locked -p pid-parse --test publish_from_pid backup_directory_publishes_the_whole_package` 1 过，50.19 秒。交付目录在 2026-10-14 01:57–01:58 已按这个合计写过：`pid-test-data\publish` 的 `_Index.json` 是上面这组数；`PID自研软件验证\publish` 只换了 I井场三件，`_Data.xml` / `_Meta.xml` 与整包 SHA256 相同，`_Coverage.json` 只差 `source`。另外 4 张集注站三件仍是 2026-10-12 11:40:21。
