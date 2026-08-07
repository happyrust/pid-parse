# Phase 38：语义上链 + 剩余显示缺口收口

> 日期：2026-08-06
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）两仓。
> 前序：Phase 37（页框 / 单位 / 样式表）已落地并出图核对，见第 1 节实测。
> 目标：把「一张 `.pid` 打开后看起来对不对」从**自证**推进到**他证**，
> 并收掉格式指南 §8 点名的三个显示缺口。

## 0. 现状（2026-08-06 实测，非引用历史文档）

### 0.1 两仓状态

| 仓 | HEAD | 分支 | 工作树 |
|---|---|---|---|
| `pid-parse` | `062f092 feat(style-link): follow a geometry record's index to the style it draws with` | `codex/phase32c-bundle-closeout` | 干净 |
| `OpenCADStudio` | `e9134c78 feat(io): draw a .pid at the widths, colours and text heights it states` | `main` | 干净（仅未跟踪 `teach/`） |

### 0.2 Phase 37 的六项缺口，现在剩几项

Phase 37 计划第 2 节列了 6 项。按今天的实测：

| # | Phase 37 时的缺口 | 现状 |
|---|---|---|
| 1 | 页框不画、页幅靠模板名猜 | **已关**。`572aa1d` 换源到 `0x003D`，`e7c8eb7d` 画 `PID-FRAME`；四张图 `page_mm` 全部为 `Some`，D06 与工艺管道-1 不再是 `None` |
| 2 | 单位换算是写死常数 | **已关**。`77262362` 按 `PidDrawingUnits` 缩放 |
| 3 | 文字字高 / 字体 | **已关**（字高）。`062f092` 的 `style_link` 两跳解析 + `e9134c78` 接进 OCS。**字体名仍是「最长 UTF-16 串」启发式，未定位偏移** |
| 4 | 线宽 / 颜色 / 线型 | **关了三分之二**。线宽 + 颜色已上链；**线型（虚线）仍未解码** |
| 5 | 离群实体 | **未关**。见 0.4 |
| 6 | 符号库 3 个未命中 | 不修（站点定制件，参考数据缺失，非解码缺陷）——判断不变 |

### 0.3 当前渲染实况（`.plot/dwg0201-0806-visible.*`，2026-08-06 09:42）

DWG-0201 可见图层导出 **424 段 / 5 圆 / 40 文字**。

字高分布（Phase 37 前是「40 条全是 2.50mm」这一个值）：

| 字高 | 条数 | 判读 |
|---|---|---|
| 3.175mm | 21 | 1/8″，样式表解出 |
| 2.5mm | 12 | **仍是 ISO 3098 回退**，样式表没给出值 |
| 2.4638mm | 3 | 样式表解出，但**不落在任何制图档位上**，值得单独看一眼 |
| 1.5mm | 2 | ISO 3098 |
| 3.5mm | 2 | ISO 3098 |

线样式 token（`@RRGGBB:WW`，WW 为百分之一毫米）：

| token | 行数 | 判读 |
|---|---|---|
| `@000000:35` | 32 | 0.35mm 黑 |
| `@808000:70` | 24 | 0.70mm 橄榄（工艺主管） |
| `@008000:18` | 4 | 0.18mm 绿 |
| `@FE0060:35` | 3 | 0.35mm 品红 |

合计 63 行带样式，其余按图层默认画。

### 0.4 四张图的解析/渲染普查（`examples/pid_probe`，2026-08-05 基线）

| fixture | 幅面 | 记录 | decoded / inferred / probe | 画出实体 | 离群 |
|---|---|---|---|---|---|
| DWG-0201 | A2 594.3×420.3 | 410 | 205 / 166 / **39** | 346 | 2 |
| DWG-0202 | A2 593.7×419.6 | 320 | 169 / 79 / **72** | 344 | 2 |
| D06 | A2 594.3×420.6 | 101 | 26 / 64 / **11** | 72 | 0 |
| 工艺管道-1 | A1 841.0×594.0 | 527 | 404 / 64 / **59** | 969 | 9 |

**这份普查是 Phase 37 样式上链之前跑的**，`.plot/` 已经是之后的了，两者不同步——
S0 的第一件事就是补一次 08-06 的普查，否则本 phase 的任何前后对比都没有基线。

## 1. 缺口清单（按 可见收益 ÷ 实现成本 排序）

| # | 缺口 | 证据状态 | 卡在哪 | 归属 |
|---|---|---|---|---|
| A | **无声丢弃**：181 条 probe-only 记录（四张图合计）与 `igDimension`/`igBalloon`/`igLeader`/`0x00FF` 一样，进不了画面也**不说一声** | 格式指南 §8.3 已给出判据：按原生图形谓词 `radsrvitem.dll!sub_56449950` 分「图形类 / 非图形类」，只对前者告警 | 只差写 | 两仓 |
| B | **虚线线型未解码**：全部线条画成实线 | `0x002F` 已知是线型样式族，未取证 | 需要一个 `style.dll` native-reader 切片（方法已验证三次） | `pid-parse` |
| C | **语义完全没上链**：画出来的东西不知道自己是什么。选中一根管子问不出管线号 | **新发现，见第 2 节**：`DWG-0202GP06-01_Data.xml` 有 39 条 `PIDRepresentation`，各带一个 `GraphicOID`（103…7342，39 个互不相同）；Sheet 侧每条记录带 `graphic_oid: Option<u32>` | 只差把两边 join 起来验证 | 两仓 |
| D | **12/40 条文字仍走 2.5mm 回退**；另有 3 条解出 2.4638mm 这种非档位值 | 已定位（`0x002C` 的亚毫米记录 0.254mm 被 `style_link` 拒收） | 需要 `0x002C` 的 version 2 路径（`sub_10002CFC`/`sub_10002CC0`，未读） | `pid-parse` |
| E | **填充未解码**：`0x002A`/`0x002B` | 未取证 | 同 B，一个取证切片 | `pid-parse` |
| F | **离群实体**：0201 有 2 条 `(2.1,-0.0)→(1000.0,-1.0)`，工艺管道-1 有 9 条负 x 文字 | 已定位到 `unresolved_unit_line` / 参数域未解 | 只做了图层隔离，坐标本身仍错 | 两仓 |
| G | **文档漂移**：OCS README 仍写「symbol bodies … are marked rather than drawn」（早就画了）；`2026-07-27-pid-load-status-snapshot.md` 正文表格仍是 07-27 版 | — | 只差改 | 两仓 |

## 2. 本 phase 的主体：`GraphicOID` 语义上链（缺口 C）

这是唯一一条能把「显示 P&ID」从**画线**推到**看图**的路，也是第一次能用
**厂商自己的输出**来量本项目的解码完整度，而不是自己跟自己比。

### 2.1 证据

`test-file/export-test/publish-data/DWG-0202GP06-01/` 下同时有：

- `DWG-0202GP06-01.pid`（249,856 B）—— 本项目 Sheet 解码的输入；
- `DWG-0202GP06-01_Data.xml`（69,491 B）—— **SmartPlant 自己发布的同一张图的语义模型**。

XML 里的对象分布：

```text
39 PIDRepresentation      16 PIDSignalPort       11 PIDNote
 7 PIDPipingConnector      7 PIDProcessPoint      5 PIDBranchPoint
 5 PIDPipeline             4 PIDPipingBranchPoint 4 PIDPipingPort
 3 PIDNozzle               2 PIDControlSystemFunction
 2 PIDPipingComponent      1 PIDDrawing           1 PIDProcessVessel
 1 PIDSignalConnector
```

每条 `PIDRepresentation` 的形状是：

```xml
<PIDRepresentation>
   <IObject UID="83AF76CA19444A2EB9706A138A8A4E96"/>
   <IDrawingRepresentation GraphicOID="3406"/>
</PIDRepresentation>
```

39 个 `GraphicOID` 互不相同，落在 `103 … 7342`。
Sheet 侧 `PidGraphicEntity.graphic_oid` 是 `Option<u32>`，
由 `geometry.rs` 从记录的 `record.oid` 填入（7 处几何族全部填）。
**两边是同一个小整数空间，量级、稀疏度、唯一性都对得上。**

### 2.2 假设与验证方式

> **H38-1**：`_Data.xml` 的 `IDrawingRepresentation/@GraphicOID`
> 与 Sheet 记录的 `graphic_oid` 是同一个 OID 空间，
> 一条 `PIDRepresentation` 对应该 OID 的一条或一组 Sheet 记录。

验证按本仓 §0 的证据规矩来，**先只读探针，不改解码器**：

1. 写 `examples/probe_graphic_oid_join.rs`，对 DWG-0202 把两边 OID 求交；
2. 判据不是命中率——OID 在文档内是稠密小整数，命中率是弱判据（§8.1 那条教训）。
   判据是**落到哪一类记录**：39 个 OID 应当无一落到样式/依赖这类非图形族上；
3. 零假设：从 Sheet 全体 OID 里随机取 39 个，全部落进图形族的概率。
   写进 negative/positive note。

**A01 是第二份样本**：`export-test/publish-data/A01/` 同样是 `.pid` + `_Data.xml` 配对。
两张图都成立才算跨 fixture。

### 2.3 成立之后能拿到什么

| 产出 | 说明 |
|---|---|
| **第一个他证的完整度数字** | 「39 条发布表示里画出了 N 条」。今天所有覆盖率都是自证的（byte-audit、记录数），这是第一个来自厂商输出的 |
| **实体带标签** | 管线号、设备位号、仪表位号从 `_Data.xml` 挂到画出的实体上 |
| **OCS 侧可选中可查询** | 选中一根管子 → 属性面板出管线号。这是「显示 P&ID」与「显示一堆线」的分界 |

### 2.4 不成立怎么办

写 negative note，本 phase 退化为 A/B/D/G 四项收口。**不要**改判据去凑命中率
——§8.1 记着上一次 `index` 被误判负的两个 bug 就是这么来的。

## 3. 推荐执行序列

```text
S0  基线补齐（不含新功能）
 -> S1  [pid-parse] GraphicOID join 只读取证（H38-1）
 -> S2  [两仓] 无声丢弃 → 点名告警（缺口 A）
 -> S3  [pid-parse] 语义侧 API：OID → 对象标签/类型（H38-1 成立才做）
 -> S4  [OCS] 标签落到实体 + 属性面板可查（S3 之后）
 -> S5  [pid-parse] 虚线线型 0x002F 取证（缺口 B，可与 S3 并行）
 -> S6  基线快照 + 契约同步 + 文档去漂移（缺口 G）
```

D（字高残余）、E（填充）、F（离群坐标）留到 Phase 39，理由写在第 6 节。

### S0：基线补齐

不引入任何新功能。

1. 用 08-06 的 HEAD 重跑 `examples/pid_probe`，出 `.plot/probe-2026-08-06.txt`，
   四张图逐图层计数与 08-05 那份对比，**样式上链带来的差异要能逐条说清**；
2. 记进 `goals/phase35-render-fidelity-text-symbols/progress.jsonl`
   （Phase 37 S4 欠的那条）。

**Done**：`.plot/probe-2026-08-06.txt` 存在；差异有解释；两仓门禁全绿。
**Stop**：普查数字与 `.plot/` 的 CSV 对不上时先查工具，不要改基线。

### S1：`GraphicOID` join 只读取证

产物：`examples/probe_graphic_oid_join.rs` +
`docs/analysis/2026-08-06-graphic-oid-is-the-semantic-join.md`。

**Done**：DWG-0202 与 A01 两张图各出一张
「39 个 OID → 落到哪个记录族 / 未命中几个」的表；
零假设概率算出来写进文档；结论明确写 **成立 / 不成立**。

**Stop**：只准用「落到哪一类」当判据，不准用命中率。
两张图结论相反时停下，不要挑一张信。

### S2：无声丢弃 → 点名告警

按格式指南 §8.3 的现成判据：

1. `pid-parse`：未知 type code 按原生图形谓词分「图形类 / 非图形类」，
   **只对图形类**推一条点名警告（type code + 命中次数 + 流路径）；
2. OCS：`report_import()` 把这些警告透传到命令行，
   格式跟现有 "geometry decode remains partial …" 那几条一致。

**Done**：四张图各自打印出自己丢了什么；
`igDimension`/`igBalloon`/`igLeader`/`0x00FF` 全语料 0 命中这件事，
从「代码里知道」变成「用户能看到」。

**Stop**：**不要**顺手给这些族写解码器——无 fixture 可验证（§8.3 原话）。

### S3：语义侧 API（H38-1 成立才做）

产物：`pid-parse` 公开一个 OID → 语义对象的只读查询，
输入是 `.pid` 旁边的 `_Data.xml`（**可选依赖**：没有它，解析行为一字不变）。

**Done**：`PidDocument` 之外新增的这层不影响任何既有 golden；
missing-docs ratchet 不上涨；`_Data.xml` 缺失时软降级并说明。

**Stop**：**不要**把 `_Data.xml` 变成解析 `.pid` 的前置条件。
`.pid` 单独打开必须仍然工作——那是这个 importer 的立身之本。

### S4：OCS 侧标签与属性

1. 语义标签写进实体的扩展数据，不新增图层（图层已经 9 个了）；
2. 属性面板显示：位号 / 类型 / 管线号；
3. `_Data.xml` 不存在时，属性面板不出现该分组，不报错。

**Done**：`tests/pid_import.rs` 加断言：有 `_Data.xml` 时 N 个实体带标签，
无 `_Data.xml` 时导入结果与今天逐字节一致。

### S5：虚线线型 `0x002F`

按 Phase 16 / Phase 37 已验证三次的路子走：
type code → `psm_type_clsid.py` → CLSID → `style.dll` vtable → 序列化器 → 字段偏移。

**Done**：证据等级到 native-reader；
`style_link` 多返回一个线型；OCS 把它映射到已有 linetype 表。

**Stop**：`style.dll` 里读不到就停下写 negative note。
**不要**按「虚线通常是仪表信号线」这种制图惯例去猜——那是启发式，
本仓 §0 明令要标注，而这一条根本不该进代码。

### S6：基线与契约

1. `.plot/` 快照重出，记进 `goals/`；
2. `docs/analysis/2026-07-27-pid-load-status-snapshot.md` 正文表格更新到 08-06；
3. `docs/pid-export-bundle-contract.md` 同步 S2/S3 的语义变化；
4. **OCS `README.md` 改掉「symbol bodies … are marked rather than drawn」**
   —— 符号本体从 `3c24b0e3` 起就在画了，109 次放置 97 次可绘制（89%）。

## 4. Gate 命令

Planning gate：

```powershell
plannotator annotate docs/plans/2026-08-06-phase38-semantic-link-and-render-gap-closeout-cn.md --gate --json
```

`pid-parse` 实现门禁：

```powershell
cargo build  --locked --workspace --all-targets
cargo test   --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

`OpenCADStudio` 实现门禁：

```powershell
cargo check --tests --examples
cargo test --locked --test pid_import -- --nocapture
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
$env:PID_SYMBOL_LIBRARY = "..\pid-parse\test-file\symbols-full"
cargo run --release --example pid_probe
cargo run --release --example pid_plot_dump -- ..\pid-parse\test-file\DWG-0201GP06-01.pid
```

## 5. Stop-And-Challenge

必须停下的情况：

1. 想用**命中率**证 H38-1。§8.1 记着 `index` 那次误判负的两个 bug，
   同一个坑不踩第二次：判据是「落到哪一类」。
2. 想把 `_Data.xml` 变成 `.pid` 解析的必需输入。
3. 想给 `igDimension`/`igBalloon`/`igLeader`/`0x00FF` 写没有 fixture 的解码器。
4. 想按制图惯例（虚线=仪表线、字高=2.5mm）填任何一个解不出来的值，
   而不标注为启发式。
5. 想「修」那 3 个未命中的站点定制符号。
6. 想在 S0 没跑出 08-06 基线之前开始 S1——前后对比没有基线就不成立。
7. 任何 parser promotion 缺少字节区间、fixture ratchet、panic-safety
   或 byte-audit movement。

## 6. 明确不做（本 phase）

| 项 | 为什么留到 Phase 39 |
|---|---|
| D 字高残余 12/40 | 卡在 `0x002C` 的 version 2 路径未读，是一个独立取证切片，与本 phase 主线无关 |
| E 填充 `0x002A`/`0x002B` | 同上；且四张 fixture 上填充的可见占比未测，收益未知——**Phase 39 之前先测一下占比** |
| F 离群坐标 | 参数域未解，属于 `GLine2d` 解码本身的欠账，不是显示问题 |
| 字体名偏移 | 目前用「最长 UTF-16 串」启发式，能读对；优先级低于线型 |
| `0x0020` / `0x0010` / `0x00FA` 尾部 | 已有负结论或覆盖率不足，不在关键路径上 |
