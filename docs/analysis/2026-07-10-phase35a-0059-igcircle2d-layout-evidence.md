# Phase 35-A：`0x0059 igCircle2d` fixture 与布局证据门

> 日期：2026-07-10
> 范围：固化两个 `.sym` fixture，新增只读布局探针，扫描本地完整语料；
> 不新增 parser / DTO / schema / writer / byte-audit claim / normalized
> geometry。

## 结论

**`layout-admitted; semantic proof required`**。

全部 616 条 chain-validated `0x0059` 记录只有一个长度 bucket：
`bytes_to_follow = 43`。payload 均可按位置表示为：

```text
+0..+18    positional prefix（本切片不命名字段）
+18..+26   f64 candidate A
+26..+34   f64 candidate B
+34..+42   f64 candidate C
+42        terminal byte
```

三个 f64 候选列在 616/616 records 中均为 finite；terminal byte 仅出现
`0x00` / `0x01`。raw type 为 `0x0059`（614 条）或带 `0x8000` flag 的
`0x8059`（2 条），两者 BTF 与候选列位置一致。因此记录边界和位置布局已经
足以进入后续 semantic-verification slice，但当前证据不能把 A/B/C 命名为
`center.x`、`center.y` 或 `radius`。

## Fixture provenance

两个入库 fixture 均来自：

`test-file/backup-test/DWG-0202GP06-01_p/RefData~4~681.zip`

| Fixture | Size | SHA-256 | `0x0059` records |
|---|---:|---|---:|
| `Design/Annotation/Graphics/Circle.sym` | 40,960 | `9cbd906a30cdcd8b9d09b51b7fea5cdb6aae82f163eb6aedf363aac0770346b5` | 2 |
| `Piping/Valves/Angle/2-Way Angle Globe Valve.sym` | 49,152 | `54b386afc74ede07c2447783ab20345c424ac7f3dd8f79a93236d8e9d9edd60a` | 6 |

逐记录运行结果：8 条全部 `btf=43`；其中 7 条位于 `.sym /Sheet*`
（`symbol_sheet`），`Circle.sym /PSMcluster0` 的 1 条归为 `other`。完整
来源、内部测试许可边界与 hash 见 `test-file/symbols/PROVENANCE.md`。

逐记录位置证据（A/B/C 对应 payload `+18/+26/+34`，仍不命名语义）：

| Fixture / stream | Range | Prev / next | Raw / flags | A | B | C | Tail |
|---|---|---|---|---:|---:|---:|---:|
| `Circle.sym /Sheet12` | `[8..57)` | none / none | `0059/0000` | 0.1016 | 0.127 | 0.00381 | `00` |
| `Circle.sym /PSMcluster0` | `[2457..2506)` | `0067@[2431..2457)` / `0036@[2506..2540)` | `0059/0000` | 0.1016 | 0.127 | 0.00762 | `00` |
| `2-Way... /Sheet6` | `[120..169)` | `0018@[64..120)` / `0059@[169..218)` | `0059/0000` | 0 | 0 | 0.000952 | `01` |
| `2-Way... /Sheet6` | `[169..218)` | `0059@[120..169)` / `0059@[218..267)` | `0059/0000` | 0 | 0 | 0.000648 | `01` |
| `2-Way... /Sheet6` | `[218..267)` | `0059@[169..218)` / `0018@[267..323)` | `0059/0000` | 0 | 0 | 0.000342000025781076 | `01` |
| `2-Way... /Sheet63` | `[8..57)` | none / `0018@[57..113)` | `0059/0000` | 0 | 0 | 0.000342000025781076 | `01` |
| `2-Way... /Sheet63` | `[393..442)` | `0018@[337..393)` / `0059@[442..491)` | `0059/0000` | 0 | 0 | 0.000648 | `01` |
| `2-Way... /Sheet63` | `[442..491)` | `0059@[393..442)` / none | `0059/0000` | 0 | 0 | 0.000952 | `01` |

对应 payload hex：

```text
Circle /Sheet12 [8..57): A0 01 00 00 0C 00 00 00 0E 00 00 00 00 00 09 00 00 00 A7 0A 46 25 75 02 BA 3F A8 C6 4B 37 89 41 C0 3F 2D 73 BA 2C 26 36 6F 3F 00
Circle /PSMcluster0 [2457..2506): 0B 01 00 00 06 00 00 00 05 00 00 00 00 00 09 00 00 00 A7 0A 46 25 75 02 BA 3F A8 C6 4B 37 89 41 C0 3F 2D 73 BA 2C 26 36 7F 3F 00
2-Way /Sheet6 [120..169): 19 00 00 00 06 00 00 00 08 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 97 4B D2 6E F4 31 4F 3F 01
2-Way /Sheet6 [169..218): 1A 00 00 00 06 00 00 00 08 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 9B 25 67 67 D1 3B 45 3F 01
2-Way /Sheet6 [218..267): 1B 00 00 00 06 00 00 00 08 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB C5 0B ED CE 69 36 3F 01
2-Way /Sheet63 [8..57): B9 06 00 00 3F 00 00 00 26 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 AB C5 0B ED CE 69 36 3F 01
2-Way /Sheet63 [393..442): 68 07 00 00 3F 00 00 00 26 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 9B 25 67 67 D1 3B 45 3F 01
2-Way /Sheet63 [442..491): 69 07 00 00 3F 00 00 00 26 00 00 00 00 00 0F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 97 4B D2 6E F4 31 4F 3F 01
```

## Corpus evidence

### 当前 `test-file`

加入两个 fixture 后的全目录扫描结果：

```text
files scanned: 59, valid CFB containers: 10, non-CFB files: 49, streams scanned: 432
0x0059 records: 89
  symbol_sheet: 7
  nested_pid_jsite: 13
  other: 69
```

`other=69` 包含现有两个备份 `StyleCluster` 的 68 条记录，以及
`Circle.sym /PSMcluster0` 的 1 条记录。Phase 34-E 的旧 scanner 将“一条
record 后面存在一个 plausible header”视为 chain，得到 79 条聚合计数
（11 nested PID + 68 StyleCluster）；本切片要求候选后的完整 PSM chain
一直延伸到 stream EOF，得到 13 条 nested PID。当前 89 条还包括新增的
8 条 fixture records。

### 临时完整语料

临时目录分别解压：

- `RefData~4~681.zip`：715 archive entries，618 extracted files，其中
  616 个 `.sym`；
- `PlantData~2~711.zip`：使用同一 `DWG-0202GP06-01_p` 下仅含本地 PID
  corpus 的归档，放入独立 `plantdata/` 子目录，避免覆盖同名路径。

聚合结果：

```text
files scanned: 626, valid CFB containers: 624, non-CFB files: 2, streams scanned: 14405
0x0059 records: 616
  symbol_sheet: 510
  nested_pid_jsite: 24
  other: 82

symbol_sheet: raw 0x0059=508, raw 0x8059=2
nested_pid_jsite: raw 0x0059=24
other: raw 0x0059=82
```

所有 source domain 都只有 `btf=43`。关键位置统计：

| Domain | Records | `f64@+18` finite | `f64@+26` finite | `f64@+34` finite | Terminal `00/01` |
|---|---:|---:|---:|---:|---:|
| symbol_sheet | 510 | 510/510 | 510/510 | 510/510 | 215 / 295 |
| nested_pid_jsite | 24 | 24/24 | 24/24 | 24/24 | 16 / 8 |
| other | 82 | 82/82 | 82/82 | 82/82 | 81 / 1 |

Phase 34-E 曾写“1826 个 `.sym`”；本次直接检查当前 source archive，实际
是 616 个 `.sym`。旧数字不作为 Phase 35 fixture gate，记录计数 616 则由
RefData + 小型 PlantData corpus 复跑确认。

## Probe behavior

`examples/probe_igcircle2d_shape.rs`：

- 接受一个 file 或 directory，递归读取 CFB streams；
- 从每个候选起点验证连续 PSM records，只有整条 chain 延伸到 stream EOF
  才计入；
- 目录、文件和 stream I/O 错误直接失败；无 CFB magic 的文件单独计数，
  带 CFB magic 但结构损坏的文件也直接失败，不静默缩小 corpus；
- 输出 half-open range、raw type/flags、BTF、前后记录、payload hex；
- 按 source domain + BTF 汇总每个 payload offset 的 f64 finite 比例、
  distinct bits、min/max，并汇总 terminal byte；
- 只报告 positional candidate，不输出几何语义字段名。

单元测试覆盖 canonical chain、截断 header、异常 BTF、伪邻接、plausible
neighbor 后 chain 中断、adversarial no-panic、non-finite f64 报告、terminal
byte 统计、`.pid`-限定的 nested JSite 分类和完整 CFB magic 判定。

## Phase 35-B gate

本切片只证明稳定布局。启动 `SheetIgCircle2dDecoded` 前仍必须取得以下之一：

1. controlled fixture diff，能把一次已知 circle 编辑稳定映射到 A/B/C 与
   terminal byte；或
2. IDA native `igCircle2d` reader，证明各字段读取顺序和含义。

在此之前维持 `IdentifiedOnly / NeedsParser`，不修改 atlas / roadmap
confidence，不将 nested JSite 投影为 top-level document geometry，也不修改
既有 `PidGraphicKind::Circle`。
