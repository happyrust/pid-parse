# Phase 34-E: 缺失曲线几何家族的 fixture 扩充计划（本地语料重大发现）

> 日期：2026-07-07
> 范围：Phase 34-E 规划 slice（`goals/phase34-full-sheet-geometry-decode/plan.md`
> §2 定义的「Fixture expansion plan」）。只读调研 + 一个新的常驻扫描探针
> （`examples/probe_curve_family_corpus_scan.rs`），无 parser / DTO /
> schema / writer / byte-audit / confidence 改动。
> 结论先行：**五个「缺失」曲线家族有四个根本不缺 fixture** —— 记录一直
> 存在于本地语料中，只是躲在 Sheet-only 流过滤器扫不到的地方。

## 1. 结论速览

| 家族 | 类型码 | 34-A 时的状态 | 本次发现 | 新状态 |
|---|---|---|---|---|
| igCircle2d | `0x0059` | NeedsFixture（Sheet 流零命中） | 注册 fixture 嵌套 `/JSite*\PSMcluster0` 有 **79** 条；备份符号库 **616** 条 | **LocalEvidenceAvailable** |
| igArc2d | `0x0061` | NeedsFixture | 注册 fixture 嵌套流 **29** 条；备份符号库 **279** 条 | **LocalEvidenceAvailable** |
| igEllipticalArc2d | `0x007E` | NeedsFixture | A01.pid 嵌套流 **4** 条；备份符号库 **50** 条 | **LocalEvidenceAvailable** |
| igBSplineCurve2d | `0x005D` | NeedsFixture | DWG-0202 嵌套流 **2** 条；备份符号库 **55** 条 | **LocalEvidenceAvailable** |
| igEllipse2d | `0x0063` | NeedsFixture | 注册 fixture **0** 条；仅备份符号库 **44** 条（如 `Ellipse.sym`） | **NeedsSymFixtureExtraction** |

「Sheet 流零命中」的旧结论仍然成立且已复跑复核（§2.1）——它只对
**顶层 `Sheet*` 流**成立。曲线记录实际生活在三类此前未扫描的容器里：

1. 注册 `.pid` fixture 的**嵌套 `/JSiteNNN\PSMcluster0` 流**（符号实例
   的图形定义簇）；
2. 已入库的原始 CFB 容器 `test-file/backup-test/*/RefData~4~683` 的
   `/StyleCluster` 流（igCircle2d×34 + igArc2d×6，两份备份内容相同）；
3. 备份 zip 内的**符号定义文件 `.sym`**（`RefData~4~681.zip`，1826 个
   `.sym`，其中 270 个 CFB 含曲线记录；`.sym` 自身就是小型 CFB，用的
   还是 `Sheet6` 流名）。

## 2. 证据（2026-07-07 实跑）

### 2.1 Sheet-only 直方图复核（旧口径，零命中确认）

```powershell
cargo run --quiet --example probe_psm_type_code_histogram
```

六 fixture 全部 `Sheet*` 流中，`0x0059 / 0x0061 / 0x0063 / 0x007E /
0x005D` **零出现**（直方图仅列出既有 decoded 家族 + `0x0013 / 0x003D /
0x0020` 三个已定性候选）。34-A 库存口径无误。

### 2.2 全流扫描：注册语料（新口径）

```powershell
cargo run --quiet --example probe_curve_family_corpus_scan -- test-file
```

```text
files scanned: 56, valid CFB containers: 8, files with curve hits: 7
  0x0059 igCircle2d: 79   0x0061 igArc2d: 29
  0x0063 igEllipse2d: 0   0x007E igEllipticalArc2d: 4
  0x005D igBSplineCurve2d: 2
```

| 容器 | 命中 | 流 |
|---|---|---|
| `D06.pid` | igCircle2d×4 | `/JSite145\PSMcluster0` (20 711 B) |
| `DWG-0201GP06-01.pid` | igCircle2d×5 + igArc2d×7 | `/JSite329\PSMcluster0` (45 755 B), `/JSite396\PSMcluster0` (3 996 B) |
| `DWG-0202GP06-01.pid`（+publish 孪生） | igCircle2d×1 + igArc2d×5 + igBSplineCurve2d×1 | `/JSite793\PSMcluster0` (39 319 B) |
| `A01.pid` | igEllipticalArc2d×4 | `/JSite39\PSMcluster0` (8 826 B), `/JSite121\PSMcluster0` (3 105 B) |
| `backup-test/{DWG-0202GP06-01_p,TEST02_p}/RefData~4~683` | igCircle2d×34 + igArc2d×6（两份相同） | `/StyleCluster` (48 032 B) |

### 2.3 全流扫描：备份 zip 解压语料

```powershell
# PlantData~2~711.zip + RefData~4~681.zip 解压至 %TEMP%\pid34e 后：
cargo run --quiet --example probe_curve_family_corpus_scan -- $env:TEMP\pid34e
```

```text
files scanned: 626, valid CFB containers: 624, files with curve hits: 270
  0x0059: 616   0x0061: 279   0x0063: 44   0x007E: 50   0x005D: 55
```

代表性样本（`.sym` 是小 CFB，流名同为 `Sheet*`）：

- `Piping/Valves/Angle/2-Way Angle Globe Valve.sym` — igCircle2d×6，
  `/Sheet6` 仅 683 B、`/Sheet63` 491 B —— 理想的最小受控 fixture；
- `Piping/Valves/Angle/Angle Check Valve.sym` — igArc2d×4；
- `Design/Annotation/Graphics/Circle.sym`、`Ellipse.sym` —— 注记图形
  符号，igEllipse2d 的直接来源；
- PlantData 内的 4 个 `.pid` 图纸（`222.pid`、`DWG-0201GP01-01.pid`、
  `DWG-0201GP06-01.pid`、`DWG-0202GP06-01.pid`）同样在
  `/JSite*\PSMcluster0` 携带曲线（如 `DWG-0201GP01-01.pid`
  igCircle2d×15 + igEllipticalArc2d×1）。

### 2.4 为什么此前一直「看不见」

`probe_psm_type_code_histogram` / `probe_psm_undecoded_shapes` 与
geometry fixture 库存全部以 `path.contains("Sheet")` 过滤流名——嵌套
`/JSite*\PSMcluster0` 与 `/StyleCluster` 不含 "Sheet"，`.sym` 文件则
根本不在注册表里。曲线家族因此被误判为「本地语料不存在」。

## 3. 修订后的 fixture 扩充计划

优先级从「向外获取」整体反转为「就地取材」：

| 步骤 | 内容 | 产出 / 门禁 |
|---|---|---|
| E-1（推荐首步） | 从 `RefData~4~681.zip` 提取一小组代表性 `.sym`（每家族 1–2 个、优先最小文件：Circle/Ellipse/Angle-valve 系列）入库 `test-file/symbols/`，登记到 fixture 注册表（新 category `symbol_ref`） | 每家族 ≥1 个受控小 fixture；`.sym` 与 `.pid` 同为 CFB+`Sheet*` 流，现有 parser 入口可直接读 |
| E-2 | 对每个家族跑七层模板第 1 步：`probe_<family>_shape` 字节 dump（`.sym` 顶层 `Sheet6` 流 ownership 干净，优于嵌套 JSite） | 字段偏移 / 校验规则 / 拒绝样本齐备后才允许开解码器 slice |
| E-3 | 嵌套 `/JSite*\PSMcluster0` 纳入扫描口径（histogram / undecoded-shapes 增加可选全流模式），但**投影语义门禁**继续生效：嵌套 JSite 是 0x0020 ownership-gate 的同一语境，layout 证明可用、几何发射需另证 ownership | 概率探针只读；不改变 normalized geometry |
| E-4 | 家族解码器按证据逐个开 slice（建议顺序：igCircle2d 616 样本→igArc2d 279→igEllipse2d 44→igEllipticalArc2d 50→igBSplineCurve2d 55），每个 slice 独立走七层模板 + 全门禁 | 逐家族 evidence-gated，禁止合并大爆炸 slice |
| E-5（降级为兜底） | 外部获取：联网核实（2026-07-07，docs.hexagonppm.com / Octave Institute）**无公开可下载的独立样例工程**——参考数据与模板随产品安装介质/许可分发；用户侧 SmartPlant 受控作图（画圆/弧/椭圆存盘取回）仍是补充受控 fixture 的可行路径（`test-file/controlled-diff/` 协议现成） | 仅当本地样本不足以钉死某字段时启用 |

### 许可与入库注意

`.sym` 来自用户自有工程备份的 Reference Data（`ZGSY P&ID Reference
Data`），与既有 `.pid` fixture 同源同权——内部测试用途一致；不额外
引入第三方分发物。入库时保持原文件名与来源路径记录。

## 4. 每家族就绪缺口（E-2 需钉死的最小集）

以 igLine2d 18 字节前缀先例（`oid / parent_ref / remaining_header /
sub_type / index`）为参照，每家族需用探针证明：

- 类型码 + `bytes_to_follow` 尺寸分布（定长或公式）；
- 前缀是否复用 18 字节 IGDS 头（`remaining_header` 取值）；
- 几何字段假设（**待验证，非事实**）：circle 预期 center+radius 或
  center+圆周点；arc 预期 center/radius/角域或三点式；ellipse 预期
  center+两轴（或轴端点）+旋转；elliptical-arc 预期 ellipse 参数+角域；
  bspline 预期 degree/knot/control-point 计数 + 变长数组（唯一预期
  变长家族，参照 igLineString2d 的 count 校验先例）；
- 有限性 / 域界 / 退化拒绝规则与 6–12 个单测；
- byte-audit / schema / panic-safety 接入点（同 34-D igBoundary2d 清单）。

## 5. 边界维持

- 本 slice 零生产代码改动；新增的 `probe_curve_family_corpus_scan`
  是只读探针（走既有 chain-validated walk，不进管线）。
- `ROADMAP-PAGE-TRANSFORM`、`0x0020` ownership-gate、`0x0010/0x00FA/
  0x0030` audit 边界、`0x003D` StructuralCandidate 全部不变。
- 嵌套 JSite 流的**几何投影语义**在 E-3 前保持未证明状态——本发现
  解锁的是「字节布局证据源」，不是「直接发射几何的授权」。
