# Phase 35: 渲染保真 — 符号身份、文字样式、符号本体

> 目标包创建于 2026-07-26。验收语言出处见 `goal-prompt.md`（grill 访谈）。
> Slice 35-A（`0x0059 igCircle2d` fixture 与布局证据）已于 2026-07-10 完成，
> 见 `docs/analysis/2026-07-10-phase35a-0059-igcircle2d-layout-evidence.md`。

## Goal

让 OpenCADStudio 打开本地 6 个 `.pid` fixture 时达到**信息完整性基准**：
所有已解码记录都被渲染且人工可读——文字有真实高度/旋转，符号有身份
（名称标签）与图形本体。为此 pid-parse 需要补齐三块解码，
OpenCADStudio 需要消费新增字段。

## Current Evidence（2026-07-26 核查）

### 已就绪的 API 面

- `PidGraphicKind::Text` 已有 `height` / `rotation` 字段，但
  `geometry.rs` 对 `igTextBox` 一律发 `0.0 / 0.0`（占位）。
- `PidGraphicKind::SymbolInstance` 已有 `symbol_path: Option<String>`，
  但一律发 `None`；插入点/2×2 矩阵分解（rotation + 带反射的 scale）
  已在早前修复中落地。
- `JSite` 模型已带 `symbol_path` / `symbol_name` / `local_symbol_path` /
  `has_ole_stream`；`crossref.rs` 的 `SymbolUsage` 能按路径聚合引用。
  缺的是 **`igSymbol2d` 记录 ↔ `JSite` 存储的实例级对应关系**。

### 文字样式证据

- `JStyleOverride`（PSM `0x0030`，RAD `style.dll`）13 个 DoIO 字段已
  完整解码；`+24..31` 是 rotation-angle 候选（跨 fixture 聚簇在
  `{0, π/2, 3π/2, 2π}`）；`+38..41` / `+56..59` 是 referenced-oid 候选，
  可能就是指向文字记录的连接件。
- `igTextBox` 的 `trailing_double_3`（常为 1.0）语义未定，是字高/缩放候选。

### 符号本体证据

- Phase 35-A：`0x0059` 全语料 616 条记录唯一长度桶 `btf=43`，
  payload `+18/+26/+34` 三个 f64 候选列 + terminal byte `00/01`，
  布局已 admitted；语义命名被门禁挡住（需 controlled-diff 或
  native-reader 证据）。
- Phase 34-E：曲线族在注册 fixture 的嵌套 `/JSite*/PSMcluster0` 流中
  就有实体（igCircle2d ×79、igArc2d ×29、igEllipticalArc2d ×4、
  igBSplineCurve2d ×2）——**符号图形本体内嵌在 `.pid` 自身**，
  不必依赖外部库也有解码对象。
- `.sym` 文件本身是 CFB，`/Sheet*` 流复用同一套 PSM 记录族
  （2-Way valve 的 Sheet6 内 `0x0018` 线记录与 `0x0059` 相邻成链），
  **现有 sheet 解码器可直接复用于 `.sym` 本体**。
- 备份库 `RefData~4~681.zip` 含完整参考符号库 616 个 `.sym`
  （含 `Equipment Components/Nozzles/Flanged Nozzle.sym` 等图纸实际
  引用项）；两个已入库 fixture（`Circle.sym`、
  `2-Way Angle Globe Valve.sym`）的许可边界见
  `test-file/symbols/PROVENANCE.md`。
- `dlls/` 内有 IDA 资产（`radsrvitem.dll.asm/.i64`、`core.dll.i64`、
  `ugeom2d1.dll`、`j2dsrv.dll` 等），native-reader 语义证据路线可行，
  与 Phase 16 用 `style.dll` 证明 `JStyleOverride` 的方法同源。

### OpenCADStudio 侧现状

- `src/io/pid.rs` 已把 normalized geometry 投影为 acadrust 实体
  （线/折线/圆/弧/文字/符号圆标记），有图层划分、单位换算
  （米→毫米）、`*Active` VPORT 取景与离群点过滤。
- 文字统一 2.5mm 高度导致密集标注重叠；符号只有 1.5mm 圆标记。

## Non-Goals

- 不宣称 vendor 格式全量解码完成；`0x0010` / `0x00FA` 仍不发几何。
- 不逐像素复刻 SmartSketch 渲染（字体、线宽、颜色表不在本 phase）。
- 不实现 `.pid` 写回（writer）路径。
- 不推送远程仓库；只做本地语义化提交。
- 未经证据门禁不把嵌套 JSite 几何提升为 top-level 文档几何
  （符号本体按“符号定义空间”单独建模，经实例矩阵变换渲染）。

## Done Means

1. `igSymbol2d` 实例带 `symbol_path`（crossref 命中时），OCS 渲染名称标签。
2. `igTextBox` 发真实 `height` / `rotation`（启发式证据链记录在案），
   OCS 渲染随之生效；无证据回退 2.5mm / 0°。
3. `0x0059 igCircle2d` 过语义门并落地解码器；`.sym` 本体解析 API 就绪，
   OCS 按实例矩阵渲染符号图形（至少覆盖 Circle.sym 对照组 +
   图纸引用命中的库符号）。
4. 6 个 fixture 全量断言零错误；golden 快照更新有据；
   `工艺管道及仪表流程-1.pid` 与 `A01.pid` 截图人工可读。
5. 全套质量门（build / test / clippy / fmt / missing-docs）绿色。
