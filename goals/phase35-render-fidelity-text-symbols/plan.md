# Plan: Phase 35 渲染保真 — 符号身份、文字样式、符号本体

> 创建于 2026-07-26。Slice 35-A 已完成（2026-07-10）。
> 排序原则：确定性高、用户可见收益大的切片先行（标签 → 文字 → 本体）。

## 1. Approach

```text
igSymbol2d ↔ JSite 实例级关联（探针）
  -> SymbolInstance.symbol_path 发真值 -> OCS 名称标签       （35-C, 35-F1）
igTextBox 样式字段（trailing_double_3 + JStyleOverride 关联探针）
  -> Text.height / rotation 发真值 -> OCS 文字样式           （35-D, 35-F1）
0x0059 语义证据（IDA native reader 优先，语料统计交叉验证）
  -> SheetIgCircle2dDecoded -> .sym 本体解析 API
  -> 符号库解析器（备份库提取 + 路径映射）
  -> OCS 按实例矩阵渲染符号本体                              （35-B, 35-E, 35-F2）
全 fixture 断言 + golden + 截图 -> 本地提交（不推送）        （35-G, 35-H）
```

双仓协同：pid-parse 出解码与 API，OpenCADStudio
（`D:\work\plant-code\cad\OpenCADStudio`）消费。OCS 侧切片标注 `[OCS]`。

## 2. Slices

| Slice | Purpose | Output | Status |
|---|---|---|---|
| 35-A | `0x0059` fixture 固化 + 布局证据 | 2 个 `.sym` fixture、只读探针、616 记录唯一 `btf=43` 布局 | Complete（2026-07-10） |
| 35-B | `0x0059 igCircle2d` 语义证明 + 解码器 | IDA/语料双证据 → `SheetIgCircle2dDecoded`、byte-audit claim、schema、panic-safety、ratchet | Pending |
| 35-C | `igSymbol2d` ↔ `JSite` 实例关联 | 关联探针 + `SymbolInstance.symbol_path` 发真值 + 测试 | Pending |
| 35-D | `igTextBox` 高度/旋转 | 样式字段探针（`trailing_double_3`、`JStyleOverride` referenced-oid）+ `Text.height/rotation` 发真值 | Pending |
| 35-E | `.sym` 本体解析 + 符号库解析器 | 公开 API：`.sym` → 归一化符号定义几何；库根映射（备份库提取目录 + `\\...\Symbols\` 前缀重映射） | Pending |
| 35-F1 | [OCS] 标签 + 文字样式集成 | `PID-SYMBOL` 名称标签图层；文字消费 height/rotation | Pending |
| 35-F2 | [OCS] 符号本体渲染 | 按实例 2×2 矩阵 + 插入点实例化符号定义几何（含反射） | Pending |
| 35-G | 端到端验收 | 6 fixture 零错误断言、golden 重祝福、gongyi/A01 截图 | Pending |
| 35-H | 契约同步 + 本地提交 | atlas / roadmap / task_plan / 本包状态一致；两仓语义化提交 | Pending |

## 3. Slice 细则

### 35-B `0x0059` 语义证明 + 解码器

- 证据优先级：
  1. **IDA native reader**（首选）：在 `dlls/` 现有反编译资产中定位
     `igCircle2d` 的序列化读取序（候选载体 `ugeom2d1.dll` /
     `radsrvitem.dll.asm`；方法同 Phase 16 `style.dll` →
     `JStyleOverride`）。
  2. **语料统计交叉验证**（辅助/兜底）：616 条记录上验证候选列不变量
     （如 C 列恒正且量级毫米级 → radius；A/B 落在符号页面范围 → center；
     `Circle.sym` 对照组 A=0.1016 B=0.127 C=0.00381/0.00762 的
     几何自洽解释）。
- 兜底与 Q3 精神一致：若 IDA 证据短期拿不到，允许以统计证据 +
  `Circle.sym` 渲染对照落地解码器，但必须在 rustdoc 与分析文档中
  标注证据等级为 heuristic，且保留升级路径。
- 落地物：`SheetIgCircle2dDecoded` DTO、`decode_igcircles`、
  byte-audit Decoded claim、schema needles、panic-safety、
  跨 fixture ratchet 测试；`.sym` `/Sheet*` 域发归一化 `Circle`，
  嵌套 JSite 域维持所有权门禁（不投影 top-level）。
- 曲线族其余成员（`0x0061` igArc2d、`0x0063` igEllipse2d、
  `0x007E` / `0x005D`）各自独立切片，按 35-B 模板复制，
  本 phase 只承诺 `0x0059`，其余按剩余预算推进。

### 35-C `igSymbol2d` ↔ `JSite` 实例关联

- 新探针：对 6 fixture 枚举 `igSymbol2d.oid / parent_ref /
  sub_type_word`，与 `JSite` 存储名、`JSite` 内部记录 oid、
  `SymbolUsage.jsite_names` 交叉比对，找实例级连接件。
- 命中则 `geometry.rs` 发 `symbol_path: Some(...)`（含 `symbol_name`
  可读名）；探不到稳定关联时回退“最近插入点/所属分组”启发式，
  并在文档标注证据等级。
- 测试：`parse_real_files` 断言 gongyi/A01 的具名实例数下界。

### 35-D `igTextBox` 高度/旋转

- 探针内容：
  1. `trailing_double_3` 跨 fixture 分布（是否恒 1.0，是否与字号档位相关）；
  2. `JStyleOverride.referenced_oid_a/b` 是否命中 `igTextBox.oid`；
     命中率、以及 `rotation_angle` 聚簇值与图面竖排文字的对应；
  3. `igTextBox` payload 未命名区间的 f64 扫描（字高候选）。
- 采纳启发式映射（Q3 已授权），置信不足的记录保持 `0.0` 让 OCS 回退。
- 单位契约：`Text.height` 按源单位（米）发，OCS 侧统一 ×1000。

### 35-E `.sym` 本体解析 + 符号库解析器

- 公开 API（形态实施时定，倾向 `pid_parse::symbol` 模块）：
  输入 `.sym` 路径 → 输出符号定义空间的归一化 2D 几何
  （复用现有 sheet 解码器栈 + 35-B 圆解码器）。
- 库解析器：`symbol_path`（`\\WIN-SPID\...\Symbols\X\Y.sym` 或
  `Piping\Valves\...`）→ 本地根目录拼接；默认根按序探测
  `test-file/symbols/`、备份库提取目录；根可由调用方注入
  （OCS 端做成配置）。
- 备份库提取脚本/说明落在 `test-file/symbols/PROVENANCE.md` 更新中，
  不把 616 个 `.sym` 全量入库 git（体积与许可考虑），只按需增补
  个别 fixture。

### 35-F [OCS] 集成

- F1：`PID-SYMBOL` 图层旁加名称标签（文字实体，插入点偏移符号标记，
  高度 2.5mm 起步）；`Text` 消费 `height`（×1000）与 `rotation`。
- F2：符号定义几何按 `insertion + rotation + scale`（含负 y-scale
  反射）实例化为普通实体（不引入 acadrust block 定义，除非实现中
  证明 block 路径更省；决策点见 §6）。
- 保持既有取景（VPORT + 离群点过滤）不回退。

### 35-G 验收

- pid-parse：`cargo test --locked` 全绿；golden 更新逐文件核对
  计数与首尾实体。
- OCS：`examples/pid_probe.rs` 对 6 fixture 零错误、实体计数下界断言；
  GUI 打开 gongyi/A01 截图（DPI-aware 流程沿用既有脚本）人工检查
  文字不重叠、符号有形。

### 35-H 契约同步 + 提交

- 同步 `docs/analysis/2026-06-19-authoritative-pid-format-atlas.md`、
  `docs/plans/2026-06-19-pid-parser-roadmap-gates.md`、`task_plan.md`
  与本包状态语言。
- 提交策略：pid-parse 按切片语义分（35-B/C/D/E 各一或合理合并）；
  OCS 一到两笔（labels+text、symbol bodies）。作者身份
  `git -c user.name=happyrust -c user.email=golinuxlove@gmail.com`，
  不推送。

## 4. Evidence Rules

沿用 Phase 34 规则，任何 parser 提升必须给出：

- stream path 与半开 byte ranges；
- 记录类型与 payload 大小；fixture id 与计数分布；
- 期望解码值；malformed/截断拒绝测试；
- byte-audit 位移；公共输出变更的 schema/DTO 测试；panic-safety 条目。

新增本 phase 规则：**启发式落地必须显式标注证据等级**
（`ida-proven` / `corpus-statistical` / `heuristic`），写进 rustdoc
与对应分析文档，保留后续升级为强证据的路径。

## 5. Verification

pid-parse 全套门：

```powershell
cargo test --locked --lib parsers::sheet_records -- --nocapture
cargo test --locked --test parse_real_files -- --nocapture
cargo test --locked --test parser_panic_safety -- --nocapture
cargo test --locked --lib schema -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

OpenCADStudio：

```powershell
cargo build --release
cargo run --release --example pid_probe -- <fixture>   # 6 个逐一
# GUI 截图：docs/screenshots/ 下沿用 DPI-aware 抓屏脚本
```

## 6. Decision Points

| Point | Default |
|---|---|
| IDA 里短期找不到 `igCircle2d` reader | 降级语料统计 + Circle.sym 对照，标注 `corpus-statistical`，不阻塞 35-E |
| `igSymbol2d` ↔ `JSite` 无稳定连接件 | 启发式（插入点/分组）+ 标注，标签仍要落地 |
| `trailing_double_3` 恒 1.0 无区分度 | 字高回退 2.5mm，只落地 rotation（若 JStyleOverride 关联成立） |
| OCS 实例化用 block 还是内联实体 | 默认内联实体；实现中若 block 明显更省再切换 |
| 曲线族其余 4 家（0x0061/0x0063/0x007E/0x005D） | `0x0059` 落地后按剩余预算逐个独立切片，不合并投机解码 |
| 备份库 616 `.sym` 是否入库 | 不全量入库；仅按需增补个别 fixture 并更新 PROVENANCE |
| 提交/推送 | 本地语义化提交，不推送（grill Q5） |
