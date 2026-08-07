# PID 解析加载现状快照（2026-07-27 实测）

> 用途：给下一步开发计划提供事实基线。所有数字均为本次实跑所得，
> 非引用历史文档。两仓：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）。

> **2026-08-07 更新（Phase 38 收口）**：07-27 的数字保留作历史基线，现状见各节的
> 08-07 批注。要点：Phase 37 把线宽/颜色/字高从「全默认」推到「读样式表」；
> Phase 38 S5 把**虚线线型** `0x002F` 也解出并接进 OCS（见
> `2026-08-07-jstyle-simple-dash-type-linetype.md`）；S1–S4 把发布语义
> （`_Data.xml` 的 `GraphicOID`，两跳）挂到实体上（见
> `2026-08-07-graphic-oid-is-the-semantic-join.md`）。§4 第 4 项「线型未解码」至此关闭。

## 1. 端到端实测：OCS 打开 6 个 fixture

命令：`PID_SYMBOL_LIBRARY=test-file/symbols-full`
+ `cargo run --release --example pid_probe -- <6 个 .pid>`

零失败，全部成功加载。

| Fixture | 实体总数 | GEOMETRY | POINT | SYMBOL | SYMBOL-LABEL | TEXT | 离群实体 |
|---|---:|---:|---:|---:|---:|---:|---:|
| DWG-0201GP06-01 | 317 | 65 | 75 | 120 | 20 | 37 | 2 |
| DWG-0202GP06-01 | 342 | 70 | 31 | 179 | 23 | 39 | 2 |
| 工艺管道及仪表流程-1 | 968 | 267 | 36 | 564 | 58 | 43 | 9 |
| D06 | 70 | 6 | 10 | 45 | 6 | 3 | 0 |
| publish A01 | 27 | 4 | 4 | 8 | 2 | 9 | 1 |
| publish DWG-0202 | 342 | 70 | 31 | 179 | 23 | 39 | 2 |

**文字高度/旋转分布：每张图都只有 1 个 distinct 值**
（`2.50mm rot=0`）——即 `igTextBox` 的 height/rotation 仍是占位，
OCS 全部回退到 ISO 3098 默认值。

> **2026-08-07 更新**：重跑 `pid_probe`（六图）。相较 07-27：多了 Phase 37 的
> `PID-FRAME`（每图 1 条闭合页框）；DWG-0201 的 `PID-CONNECTIVITY` 端点对
> （25，隐藏层）现在计入总数；字高不再是单值——五到六个 distinct
> （`3.175`=1/8″、`2.5`/`1.5`/`3.5` ISO、`2.464` 非档位），因 `style_link`
> 两跳解析已上线。离群数（探针口径 x>900 或 x<0）与 07-27 一致。
>
> | Fixture | 总数 | GEOMETRY | POINT | SYMBOL | SYMBOL-LABEL | TEXT | FRAME | 离群 |
> |---|---:|---:|---:|---:|---:|---:|---:|---:|
> | DWG-0201GP06-01 | 346 | 63 | 75 | 123 | 20 | 37 | 1 | 2 |
> | DWG-0202GP06-01 | 344 | 70 | 31 | 180 | 23 | 39 | 1 | 2 |
> | 工艺管道及仪表流程-1 | 969 | 267 | 36 | 564 | 58 | 43 | 1 | 9 |
> | D06 | 72 | 6 | 10 | 46 | 6 | 3 | 1 | 0 |
> | publish A01 | 28 | 3 | 4 | 8 | 2 | 9 | 1 | 1 |
> | publish DWG-0202 | 344 | 70 | 31 | 180 | 23 | 39 | 1 | 2 |
>
> **虚线**（Phase 38 S5，`pid-parse` 端到端探针口径，分母是已解析线样式条数）：
> DWG-0201 15/138、DWG-0202 9/101、D06 1/16 条带虚线；其余两图线样式全为实线。

## 2. 符号库（Phase 36 已落地）

`probe_sym_library_yield`：

- 库 618 个 `.sym`，读取失败 0，有本体 578，空本体 40；
- 图元 5636（线 5134 / 圆 315 / 弧 163 / 多段线 24）；
- 图纸放置 109 次 → 97 次可绘制（**89.0%**），904 图元；
- 未命中 3 个符号（12 次放置）全是 XaLNG 站点定制件，
  本地任何备份包都没有 → **参考数据缺失，非解码缺陷**。

## 3. `.pid` Sheet 流类型码覆盖（probe_psm_type_code_histogram）

已解码族占绝对多数。未解码的仅剩：

| type | 名称 | 全 fixture 命中 | 状态 |
|---|---|---:|---|
| `0x003D` | igSmartFrame2d | 12 | IdentifiedOnly，疑似图框/页面变换载体 |
| `0x0020` | igRectangle2d | 4 | IdentifiedOnly，Phase 34-B 负结论 |
| `0x0013` | igBoundary2d | 20 | Decoded 但**故意不发几何**（与成员线重复） |
| `0x0006`/`0x007B`/`0x0085` | — | 各 1（仅 A01） | 未定 |

**结论：`.pid` Sheet 流按记录族已接近全覆盖，剩余信息缺口不在
"还有多少记录没解"，而在"已解出的记录缺属性"。**

> **2026-08-04 更新**：本节第 2、3 项已关闭，第 5 项已定性为非缺口；第 3 节的
> 三个「未定」type code 也已定名。逐条见 §4 各项的更新批注，出处见
> `2026-08-04-inferred-points-negative-note.md`、
> `2026-08-04-annotation-families-risk.md`、
> `2026-08-04-graphicgroup-tail-property-block.md`。
>
> 另外，**第 3 节把 `0x0006` / `0x007B` / `0x0085` 记为「未定」已过时**：
> `radsrvitem.dll!sub_56448F70` 的完整类表给出
> `0x0006 = igPointOnRelation2d`、`0x0085 = igTangentRelation2d`（都是参数约束，
> 永不可画）、`0x007B = igGroup`（容器，无几何）。三个都不是几何缺口。

## 4. 真正的信息缺口（按用户可见收益排序）

1. **文字样式（字高/字体/旋转）**。Phase 35-D 已证明字高**不在**
   `igTextBox` 记录内（146 条记录全偏移扫描无命中），12 字节 trailer
   末 4 字节大端 u32 是跨图纸稳定的**样式 id**（id=56 三张图全对应中文，
   id=64 全对应管道号）。样式表应在 `/StyleCluster`（`0x005A`），
   Phase 29 判定其 prefix 无 parser 证据 → **卡在需要 native reader**。
2. ~~**符号内文本**~~ **已关闭（Phase 36 + OCS `place_primitive`）**。`.sym` 本体
   里的文本已读出并绘制，模板占位 `NULL` 由 `carries_a_label` 过滤。
3. ~~**图框 / 页面变换**~~ **已关闭（Phase 37 S1–S3）**。页幅改从 `0x003D`
   实测读取，四张图全部拿到（D06 与工艺管道从 `None` 变成有值），
   `PidPageTransform` 升为 `Available`，`PID-FRAME` 图层画出闭合页框，
   两个经验常数由数据提供。
4. ~~**线宽 / 颜色 / 线型**~~ **已关闭（Phase 37 + Phase 38 S5）**。几何 payload
   `+14` 的 index 命名同文档 `StyleCluster` 里的样式 id：`0x002E SimpleLine` 给
   线宽（`+34`）与颜色（`+50`），`0x002F SimpleDashType` 给虚线图案，线样式经
   `+54` 命名它。见 `2026-08-05-geometry-index-is-the-style-link.md` 与
   `2026-08-07-jstyle-simple-dash-type-linetype.md`。OCS 按样式表画线宽/颜色，
   并把虚线映到 `PID-DASH-<n>` linetype；四张图 558/558 条可绘制记录零未解析。
   **2026-08-04 进展（存档）**：曾排除几何图元本身承载样式——`igLine2d`（50 字节）
   与 `igPoint2d`（34 字节）定长且字节全额入账；后证实样式经 `+14` 的 index 外链。
5. ~~**离群实体**~~ **非缺口**。实测四张图**可见图层零离群**：探针报的
   都在隐藏层上（`PID-UNRESOLVED` 的单位线、`PID-SYMBOL-LABEL` 的页外标签）。

**2026-08-04 新增（本表原先没有）**：

6. **标注族语料缺口**。`igDimension`(277) / `igBalloon`(279) / `igLeader`(280)
   与未命名的 `0xFF` 都通过原生图形谓词 `sub_56449950`，即出现即应绘制，
   但全语料 0 命中且当前会**静默丢弃**。建议加图形类未知 type code 的点名告警，
   不建议现在写解码器（无 fixture 可验证）。

## 5. 关键能力变更：IDA headless 已打通（2026-07-27 实测）

roadmap `docs/plans/2026-06-19-pid-parser-roadmap-gates.md` 记录的
限制是：「2026-06-21 live MCP refresh 只有 `sppid.dll`、`core.dll`
可达，开其它 IDB 被 MCP 工具面缺 `idalib_open` 挡住」。

**该限制已不成立。** 本机：

- IDA Professional 9.2 装在 `D:\IDA Professional 9.2\`；
- Python `idapro`（idalib）0.0.8 + `ida-pro-mcp` 2.0.0 已装；
- `tools/idalib_smoke.py` 实测 headless 打开 `dlls/sppid.dll.i64`
  成功（158 函数 / 543 符号），无需 GUI、无需 MCP、可脚本化批量跑。

含义：`ugeom2d1.dll`（2D 几何库，弧/圆/椭圆的天然载体）、
`j2dsrv.dll`、`radsrvitem.dll.i64`、`core.dll.i64` 都可以直接开来找
序列化读取序。Phase 35-D 与 Phase 29 共同指向的
**`/StyleCluster` `0x005A` reader** 这个 IDA target request，现在有了
执行手段。

## 6. 仓库状态

- `pid-parse` HEAD `a50a22a feat(symbols): decode .sym library geometry`；
  工作树脏：`src/symbol_library.rs`（多根搜索 + 4 个新测试）、
  `examples/probe_sym_library_yield.rs`、`AGENTS.md`，以及若干未跟踪
  的 `docs/agents/` 与分析文档。**未提交。**
- `OpenCADStudio` HEAD `7f005070 feat(io): draw .pid symbol placements`；
  工作树脏：`src/io/pid.rs`（`discover_symbol_library` 改多根）。**未提交。**
- 目标包 `goals/phase35-render-fidelity-text-symbols/progress.jsonl`
  只记到 35-C，**落后于实际进度**（35-D 探针、Phase 36 符号库均已完成
  并提交），需要补记。
