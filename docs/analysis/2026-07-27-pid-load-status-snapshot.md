# PID 解析加载现状快照（2026-07-27 实测）

> 用途：给下一步开发计划提供事实基线。所有数字均为本次实跑所得，
> 非引用历史文档。两仓：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）。

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

## 4. 真正的信息缺口（按用户可见收益排序）

1. **文字样式（字高/字体/旋转）**。Phase 35-D 已证明字高**不在**
   `igTextBox` 记录内（146 条记录全偏移扫描无命中），12 字节 trailer
   末 4 字节大端 u32 是跨图纸稳定的**样式 id**（id=56 三张图全对应中文，
   id=64 全对应管道号）。样式表应在 `/StyleCluster`（`0x005A`），
   Phase 29 判定其 prefix 无 parser 证据 → **卡在需要 native reader**。
2. **符号内文本**。`.sym` 语料里 `0x004D igTextBox` 有 1490 条未解，
   40 个 `.sym` 解出空本体（推测纯文本标签类符号）。符号上的固定标注
   （如阀门位号模板）当前完全不画。
3. **图框 / 页面变换**。`0x003D igSmartFrame2d` 带 A2 级页面标量
   （0.594/0.420），但 `PidPageTransform` 仍是 `Unavailable`，
   图纸边框不画，单位靠"米→毫米 ×1000"的经验常数。
4. **线宽 / 颜色 / 线型**。完全未解码，OCS 全部按默认渲染。
5. **离群实体**（最多 9 个/图）。`unresolved_unit_line` 只做了取景过滤，
   记录本身仍以错误坐标进入文档。

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
