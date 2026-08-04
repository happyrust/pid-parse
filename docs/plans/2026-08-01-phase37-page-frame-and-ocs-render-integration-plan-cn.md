# Phase 37：页框上链路 + OpenCADStudio 渲染集成收尾

> 日期：2026-08-01
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）两仓。
> 目标：把已经拿到证据、但还停在 parser 层的 `0x003D` 页框成果接到
> `NormalizedPidGeometry` → `PidPageTransform` → OCS 图框渲染这条链路上，
> 并把 OCS 侧在飞的 `.pid` 导入改动收口成可回归的基线。

## 0. 2026-08-03 复测更新（本节覆盖第 1 节中已过期的部分）

`OpenCADStudio` 已合并上游 `origin/main`（29 个提交，含 Fluent i18n），
合并提交 `49455b44`，唯一冲突在 `Cargo.lock`，按上游版重取后用
`cargo metadata` 重解，`pid-parse` 路径依赖保留。

S0 的编译门禁已通过：`cargo check --tests --examples` 绿，
`cargo test --test pid_import` 5/5 通过。上游 API 漂移造成的 5 个测试编译错误
已修（`HatchModel` 新增 `line_weight_px`、`BlockCache::build` 增到 6 参、
`export_pdf` 增到 13 参）。第 1.1 节列出的 `Cargo.toml` index 残留与
`examples/_pid_geom_stats.rs` 已不存在。

**S0 剩余**：`src/io/pid.rs`（+295）、`tests/pid_import.rs`（新增）、
`examples/pid_probe.rs`（+86）、`.gitignore`、`Cargo.toml` 仍未提交。

### 0.1 四张图的实测解析情况（2026-08-03，`examples/pid_probe`）

| fixture | 幅面 | 证据条数 | decoded / inferred / probe | 画出实体 | `page_mm` | 内容 x 范围 | 内容 y 范围 |
|---|---|---|---|---|---|---|---|
| DWG-0201 | A2 | 410 | 205 / 186 / 19 | 365 | `Some(594.0, 420.0)` 标称 | 33.98 … 583.19 | 6.95 … 402.02 |
| DWG-0202 | A2 | 320 | 169 / 109 / 42 | 373 | `Some(594.0, 420.0)` 标称 | **-25.64** … 583.70 | 5.27 … 409.87 |
| 工艺管道-1 | A1 | 527 | 404 / 111 / 12 | 1015 | **`None`** | **-8.03** … 826.86 | 13.34 … 551.31 |
| D06 | A2 | 101 | 26 / 67 / 8 | 74 | **`None`** | 75.11 … 377.43 | 48.23 … 285.46 |

四张图 100% 的实体都是 `units = unknown` + `page_transform = unavailable`，
文字高度全是 `2.50mm` 这一个值（importer 的 ISO 3098 回退）。

**四张图里有两张拿不到页幅**：D06 与工艺管道-1 没有 `Template` tag，模板名启发式
直接退化成 `None`，于是既不按页取景、还多推一条 units/page-transform 警告。
这是把页幅换源到 `0x003D` 最直接的收益——不是把 594.0 改成 594.3 这种精度问题，
而是**从没有变成有**。

### 0.2 页框记录实测（`examples/probe_smartframe_variant`）

| fixture | 页框记录数 | 状态 | `+76 × +84`（m） | 换算（mm） |
|---|---|---|---|---|
| DWG-0201 | **6**（`/Sheet6`，extent 完全相同） | Linked | 0.594305 × 0.420314 | 594.31 × 420.31 |
| DWG-0202 | 1 | Linked | 0.593746 × 0.419629 | 593.75 × 419.63 |
| 工艺管道-1 | 1 | Linked | 0.841000 × 0.594000 | 841.00 × 594.00 |
| D06 | 1 | Linked | 0.594328 × 0.420566 | 594.33 × 420.57 |
| A01 | 1 + 1 嵌套 | Embedded / LocallyLinked | 0.594001 × 0.420001 / 1e-6 | 594.00 × 420.00 |

**对 S1 的直接影响**：DWG-0201 有 6 条页框记录，`page_extent_m()` 对这 6 条
全部返回 `Some`，且 extent 逐比特相同。所以选取规则不是"挑一条"，而是
**按 extent 去重后要求唯一**；去重后仍有多个不同 extent 才是需要停下写
negative note 的情况。这条规则有证据支撑，不属于第 5 节禁止的无据启发式。

## 1. 现状（2026-08-01 实测，非引用历史文档）

### 1.1 两仓状态

| 仓 | HEAD | 工作树 |
|---|---|---|
| `pid-parse` | `15041f4 feat(sheet): decode the OLE frame a drawing borders itself with` | 干净 |
| `OpenCADStudio` | `96805e6d Merge remote-tracking branch 'origin/main'`（领先 origin 17 个提交） | **脏** |

`OpenCADStudio` 未收口的东西：

- `src/io/pid.rs` 同时有 staged（+184/-33）与 unstaged（+111/-41）两层改动；
- `Cargo.toml` 在 index 里留着 `UU` 未合并条目（文件内容本身没有冲突标记，
  `MERGE_HEAD` 也不存在，是残留的 index 阶段项）；
- 未跟踪：`tests/pid_import.rs`（5 个 `.pid` 回归断言）、
  `examples/_pid_geom_stats.rs`（临时统计探针）、`.plot/`（10 个基线快照）。

**`cargo check --tests --examples` 当前失败**，2 个错误全在
`tests/block_hatch_export.rs:292` —— `HatchModel` 缺 `aci` 字段。这与 `.pid`
无关（是同一天另一处在飞改动），但它挡住 `cargo test`，任何 PID 切片跑门禁
前都会先撞上。

### 1.2 解析链路（已经做到哪）

`.pid` 是 CFB 容器，`/Sheet*` 流是 6 字节 envelope 的 PSM 记录链。
`build_normalized_geometry()` 输出 `NormalizedPidGeometry { entities,
warnings, page_dimensions_mm }`，每个实体带 `PidGraphicKind` +
`PidGeometryConfidence`（Decoded / Inferred / ProbeOnly）+ 字节级 provenance +
`PidCoordinateContext`。

按记录族算，Sheet 流已接近全覆盖：线 / 折线 / 圆 / 弧 / 文字 / 符号 / 点 /
标注全部有 emitter，`radsrvitem.dll sub_56448F70` 的 type_code→类名枚举坐实了
全部 `ig*` 身份。符号本体（Phase 36）也已落地：618 个 `.sym` 零解析失败、
6679 个图元、图纸放置 109 次中 97 次可绘制（89%）。

`0x003D igSmartFrame2d` 的解码在上个提交刚落地：`sheet_records.rs` 有
`IGSMARTFRAME_WIDTH_AT = 76` / `HEIGHT_AT = 84` / `ASPECT_AT = 148` 与三态
OLE 标志判定，注册进了 `sheet_families` 的 `decoded_igsmartframes`。
`docs/analysis/2026-07-27-smartframe-003d-native-reader.md` 已把
`ROADMAP-PAGE-TRANSFORM` 要求的七项（坐标空间 / 单位 / 方向 / 原点 / 比例 /
边界 / 溯源）逐项证完，五张图四条边全部落在页内。

### 1.3 显示链路（`OpenCADStudio/src/io/pid.rs`，788 行）

`load_pid()` 把 normalized geometry 投影成 acadrust 实体，分 8 个图层：

| 图层 | 颜色 | 默认 | 内容 |
|---|---|---|---|
| `PID-GEOMETRY` | 白 | 可见 | 线 / 折线 / 圆 / 弧 |
| `PID-TEXT` | 绿 | 可见 | `igTextBox` 文字 |
| `PID-SYMBOL` | 青 | 可见 | 符号本体，未命中库时退化为 1.5mm 圆点 |
| `PID-POINT` | 品红 | 可见 | `igPoint2d` |
| `PID-SYMBOL-LABEL` | 灰 | **隐藏** | 从 `.sym` 路径派生的名称标签 |
| `PID-ANNOTATION` | 黄 | **隐藏** | `JStyleOverride` 锚点方向短线 |
| `PID-CONNECTIVITY` | 蓝 | **隐藏** | 两端都在页内的 endpoint pair |
| `PID-UNRESOLVED` | 红 | **隐藏** | 参数域未解出的 `GLine2d` 单位线 |

取景由 importer 自己造 `*Active` VPORT，取 decoded 包围盒与 `page_dimensions_mm`
的并集。

### 1.4 当前渲染实况（`.plot/` 快照）

| 图 | 幅面 | 可见图层实体 |
|---|---|---|
| DWG-0201 | A2 | 线 134 / 圆 5 / 折线 44 / 文字 40 |
| 工艺管道及仪表流程-1 | A1 | 线 455 / 圆 47 / 折线 329 / 文字 43 |

（`pid_plot_dump` 把弧采样成折线输出，所以 CSV 里 `arcs` 恒为 0，不是缺陷。）

两张图**都没有图框**，内容悬在深色背景中间；文字全是同一个 2.5mm 高度。

## 2. 缺口清单（按 可见收益 ÷ 实现成本 排序）

| # | 缺口 | 证据状态 | 卡在哪 |
|---|---|---|---|
| 1 | **页框不画、页幅靠模板名猜** | 解码已完成，证据七项齐备 | **只差接线**，见下 |
| 2 | 单位换算是写死常数 | 同上，`0.594305 → 594mm` 已证明是米 | 只差接线 |
| 3 | 文字字高 / 字体 | 硬阻塞 | 字高不在 `igTextBox` 内；样式表在 `/StyleCluster 0x005A`，需要 `style.dll` native reader，**本机未装 SmartPlant** |
| 4 | 线宽 / 颜色 / 线型 | 完全未解码 | 需要独立取证 phase |
| 5 | 离群实体（最多 9 个/图） | 已定位 | `unresolved_unit_line` 只做了图层隔离，记录坐标本身仍错 |
| 6 | 符号库 3 个未命中（12 次放置） | 已定性 | XaLNG 站点定制件，**参考数据缺失，非解码缺陷**，不修 |

**第 1、2 项是本 phase 的主体**：解码侧的活已经干完并提交了，
`geometry.rs:363 infer_page_dimensions()` 却还在从 `drawing_meta.tags["Template"]`
里做子串匹配（`contains("A2")` → 返回标称 `594.0 × 420.0`），
**完全没有读 `decoded_igsmartframes`**。后果有三层：

1. 用的是标称值而不是实测值（594.0 vs 594.3）；
2. 没有 `Template` tag 的图直接退化成 `None`，此时既不取页幅取景，
   还会推一条 "coordinate units and page transforms are unavailable" 警告；
3. `PidPageTransform` 至今恒为 `Unavailable`，于是 OCS 只能把
   `MM_PER_SOURCE_UNIT = 1000.0` 和 `on_sheet()` 的 `-100..=2000` 写死在自己
   源码里——两个本可以由数据给出的经验常数。

## 3. 推荐执行序列

```text
S0 地基收口（不含新功能）
  -> S1 [pid-parse] 页幅来源换成 0x003D，PidPageTransform 升为 Available
  -> S2 [OCS] 画图框 + 按真实页矩形取景与过滤
  -> S3 [OCS] 去掉两个经验常数，改由 PidPageTransform 提供
  -> S4 基线快照 + 回归 ratchet + 契约同步
  -> S5（授权后）字高：取回 style.dll，或按 style id 做有据回退
```

### S0：地基收口

不引入任何新功能，只让仓库回到能跑门禁的状态。

1. ~~清掉 `Cargo.toml` 的 index 残留~~ —— 已随 2026-08-03 的上游合并解决；
2. ~~修 `HatchModel` 缺字段~~ —— 已修，同时修掉上游漂移带来的
   `BlockCache::build` / `export_pdf` 签名变化（共 5 个编译错误）；
3. **仍未做**：把 `src/io/pid.rs` 的改动 + `tests/pid_import.rs` +
   `examples/pid_probe.rs` 收成语义化提交；`.plot/` 已进 `.gitignore`。

Done：`cargo check --tests --examples` 绿（已达成）；
`cargo test --test pid_import` 5/5（已达成）；`git status` 只剩预期内容。

### S1：`page_dimensions_mm` 换源 + `PidPageTransform::Available`

产物：

1. `geometry.rs` 读 `decoded_igsmartframes`，取 `is_page_frame()` 为真的记录，
   页幅 = `extent_width_m / extent_height_m` × 1000；
2. 模板名启发式**降级为回退**（没有 `0x003D` 页框记录时才用），不删除；
3. `PidPageTransform` 由 `Unavailable` 升为 `Available { scale: 1.0,
   origin: (0,0), bounds: (0,0)-(w,h) }`，`PidDrawingUnits::Known { unit: "m" }`；
4. 一张图有多条候选页框记录时的选取规则要写死并测到（按
   `docs/analysis/2026-07-27-smartframe-003d-native-reader.md`：
   嵌套 `/JSite*/Sheet*` 里那条 LocallyLinked、尺寸退化为 `1e-6` 的**不是页面**）。
   规则按第 0.2 节的实测定为：取 `page_extent_m()` 为 `Some` 的记录，
   **按 extent 去重**；去重后唯一则采用（DWG-0201 的 6 条同尺寸记录走这条），
   去重后仍有多个不同 extent 则不猜、退回模板名回退并推警告。

Done：6 个 fixture 的 `page_dimensions_mm` 与该分析文档的实测表逐张对上
（594.3×420.3 / 593.7×419.6 / 841.0×594.0 / 594.3×420.6 / 594.0×420.0）；
**D06 与工艺管道-1 不再返回 `None`**，"coordinate units and page transforms
are unavailable" 那条警告随之消失；golden 重祝福有据；byte-audit movement 可解释。

Stop：若某个 fixture 选不出唯一页框记录，**停下写 negative note**，
不要临时加"取最大的那条"这类无证据启发式。

### S2：OCS 画图框

产物：

1. 新增 `PID-FRAME` 图层（**默认可见**——它是图纸的一部分，不是诊断证据），
   按 `(0,0)-(w,h)` 画闭合 `LwPolyline`；
2. `frame_drawing()` 改为在有页矩形时直接按页取景，不再取并集；
3. `on_sheet()` 的判据从写死区间改成"页矩形加一个页边距"。

Done：`.plot/` 重出图，DWG-0201 与工艺管道-1 的边框把内容不多不少地框住；
`tests/pid_import.rs` 加断言：`PID-FRAME` 存在、可见、是闭合四边形、
面积等于页幅。

Stop：图框**只画一个矩形**。标题栏 / 会签栏是图纸自己的线work，不合成。

### S3：去掉两个经验常数

`MM_PER_SOURCE_UNIT` 与 `on_sheet()` 的区间改由 `PidCoordinateContext`
提供；拿不到时保留现有常数作回退，并在日志里说明走了回退。

Done：`grep` 不到"经验常数"性质的裸数字；无页框的输入仍能打开。

### S4：基线与契约

1. `.plot/` 快照重出并记进 `goals/`；
2. `docs/analysis/2026-07-27-pid-load-status-snapshot.md` 的第 4 节缺口表
   按本 phase 结果更新（第 1、3 项应当移出）；
3. `goals/phase35-render-fidelity-text-symbols/progress.jsonl` 补记 35-F；
4. `docs/pid-export-bundle-contract.md` 同步 `PidPageTransform` 的语义变化。

### S5：字高（需要单独授权）

两条路，**都不是本 phase 默认动作**：

- **A（推荐）**：取回 `style.dll`（装 SmartPlant 或从备份包里捞），
  按 Phase 16 证 `JStyleOverride` 的同一套 native-reader 方法解
  `/StyleCluster 0x005A`。证据等级能到 native-reader。
- **B**：用已证明跨图纸稳定的 style id（56=中文、64=管道号、21=常规标注）
  做语料统计聚类，给每类一个有据的字高。证据等级封顶
  corpus-statistical，且**必须**在代码与文档里标注为启发式。

## 4. Gate 命令

Planning gate：

```powershell
plannotator annotate docs/plans/2026-08-01-phase37-page-frame-and-ocs-render-integration-plan-cn.md --gate --json
```

`pid-parse` 实现门禁（S1）：

```powershell
cargo test --locked --lib parsers::sheet_records -- --nocapture
cargo test --locked --lib geometry -- --nocapture
cargo test --locked --test parse_real_files -- --nocapture
cargo test --locked --lib byte_audit -- --nocapture
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

`OpenCADStudio` 实现门禁（S0 / S2 / S3）：

```powershell
cargo check --tests --examples
cargo test --locked --test pid_import -- --nocapture
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
$env:PID_SYMBOL_LIBRARY = "..\pid-parse\test-file\symbols-full"
cargo run --release --example pid_plot_dump -- ..\pid-parse\test-file\DWG-0201GP06-01.pid
```

## 5. Stop-And-Challenge

必须停下的情况：

1. 想在页框记录选取上加没有证据的启发式（"取最大的那条"）。
2. 想把 `0x0013 igBoundary2d` / `0x0010` / `0x00FA` emit 成可绘制几何。
3. 想合成标题栏 / 会签栏 / 图框内的分格线。
4. 想在没有 `style.dll` 或 controlled fixture 的情况下给字高编一个值，
   而不标注为启发式。
5. 想"修"那 3 个未命中的站点定制符号——那是参考数据缺失，不是解码缺陷。
6. 任何 parser promotion 缺少字节区间、fixture ratchet、panic-safety
   或 byte-audit movement。
7. S0 没做完就开始 S1/S2——当前 `cargo test` 是红的，红着跑不出可信门禁。
