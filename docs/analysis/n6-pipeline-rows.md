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
