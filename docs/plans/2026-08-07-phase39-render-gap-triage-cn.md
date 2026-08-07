# Phase 39：剩余显示缺口分诊（D 字高残余 / E 填充 / F 离群坐标）

> 日期：2026-08-07
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）两仓。
> 前序：Phase 38（语义上链 + 虚线线型）已 S1–S6 全部落地并提交。
> 本计划是**只读分诊 + 排序**：先拿实测占比给 Phase 38 §6 留的三项排序，
> 再定各项该不该做、按什么次序做。**本轮不改任何解码/产品代码。**

## 0. 现状（2026-08-07 实测）

### 0.1 两仓状态

| 仓 | HEAD | 工作树 |
|---|---|---|
| `pid-parse` | `24d4d9d docs: bring the load-status snapshot and bundle contract current to Phase 38` | 干净（仅 CRLF 噪声） |
| `OpenCADStudio` | `9c9a202c docs(pid): correct the README's symbol/style claims, and dump the dashed linetype` | 干净（仅 CRLF 噪声 + 未跟踪 `teach/`） |

Phase 38 收官：线宽/颜色/字高（Phase 37）+ 虚线线型 `0x002F`（S5）+ 发布语义
`GraphicOID`（S1–S4）都已上链并出图。剩下的显示缺口就是 D / E / F 三项。

### 0.2 本次分诊实测

**Sheet\* 流几何普查**（`examples/probe_psm_type_code_histogram`，六图）：已解码族占满，
未解码的跨图码只剩三个——`0x0013 igBoundary2d`（20，故意不发，与成员线重复）、
`0x003D igSmartFrame2d`（12，已用作页幅）、`0x0020 igRectangle2d`（4，Phase 34-B 负结论）。
**没有任何「填充/实心面」几何族出现在 Sheet 流里。**

**StyleCluster 侧填充样式**（格式指南 §5 记录表）：`0x002A JStyleSimpleFill` 25 条、
`0x002B JStyleHatchFill` 11 条——**样式定义在，但没有几何去引用它们**（Sheet 流里没有
被填充的面）。

## 1. 三项缺口的证据与占比

### D — 字高残余（12/40 走 2.5mm 回退，另有非档位值）

- 现状（DWG-0201，`pid_import` 断言口径）：字高分布 `3.175×21 / 2.5×9 / 2.464×3 / 1.5×2 / 3.5×2`。
  `3.175`（1/8″）与 ISO 档已由 `style_link` 两跳解出；**`2.5×9` 仍是 ISO 3098 回退**
  （字符样式没给出可用值），**`2.464×3` 解出了但不落任何制图档位**，值得单独看。
- 卡在哪（格式指南 §8.2）：`0x002C JStyleTextChar` 的 **version 2 路径未读**
  （`sub_10002CFC` + `sub_10002CC0`）；另有亚毫米记录 `0.254mm`（=0.01″，太小不可能是真字高）
  被 `style_link` **主动拒收**、消费方保留默认值。回退的那批多半就落在这两种情形里。
- 可见收益：**中**。把每张图约四分之一的文字从「统一 2.5mm」纠正到真实字高，肉眼可见。
- 成本：**低-中**。与 S5 **同一套** `style.dll` native-reader 手法（IDA 反编译版本分发的
  序列化器），`dlls/style.dll.i64` 还在，`0x002C` 的 version-3 路径已解、只差 version-2。

### E — 填充 `0x002A` / `0x002B`

- 占比实测：**可见占比 ≈ 0**。六图里没有一条被填充的几何；36 条填充样式定义无几何消费者。
- 可见收益：**≈ 0**（在当前语料上）。解出填充样式的颜色/图案，画面不会多出任何东西，
  因为没有面去填。
- 成本：一个 native-reader 取证切片（同 B/D）。
- 结论：**本期不做**。这不是「解不出来」，是「解出来也没得画」。→ §3 写 negative note。

### F — 离群坐标

- 现状：DWG-0201 2 条 `(2.1,-0.0)→(1000.0,-1.0)` 单位线、工艺管道-1 9 条负 x 文字；
  **已隔离在隐藏层 `PID-UNRESOLVED`**，不污染画面。
- 卡在哪：`GLine2d` 的参数域（起止参数）未解 → 退化成「原点走一个源单位」的 1000mm 直线。
  这是 `GLine2d` 解码本身的欠账，不是显示问题。
- 可见收益：**低**。这些记录本就隐藏，且坐标是真错的，纠正后也只是让几条隐藏线归位。
- 成本：中（`GLine2d` 参数域是一条独立取证线，与样式族的 native-reader 手法不同）。

## 2. 排序结论

按「可见收益 ÷ 成本」，且优先复用已验证手法：

1. **D 先做**。收益中、成本低（S5 同款刀），是唯一还能明显改善画面的一项。
2. **E 不做**，写 negative note 封存：占比实测为 0，等出现「带填充的 fixture」再翻案。
3. **F 垫底 / 可选**。已隔离、收益低；且它是 `GLine2d` 解码欠账，更适合跟一次
   几何解码专项一起做，而不是塞进这一期显示收口。

## 3. 推荐执行序列

```text
S0  基线确认（沿用 Phase 38 S6 的 08-07 census；无需重跑，除非 D 落地后要核对字高分布）
 -> S1  [pid-parse] 0x002C version-2 字高路径 native-reader 取证 + style_link 接住（缺口 D）
 -> S2  [OCS] 复核字高分布：回退桶应收缩，2.464 那类非档位值给出解释或标注
 -> S3  [pid-parse] E 写 negative note（填充零可见占比），并给「若要翻案」留一条最便宜的验证路
 -> S4  （可选）[pid-parse] F：GLine2d 参数域取证；不成立就维持隐藏隔离
```

### S1：`0x002C` version-2 字高（缺口 D）

按 S5 已验证三次的路子：type code → CLSID → `style.dll` vtable → 版本分发序列化器 → 字段偏移。
`0x002C` 的 version-3 路径（`B=26`、字高 `+42`）已是 native-reader；本切片读 **version-2**
路径（`sub_10002CFC` / `sub_10002CC0`），把回退桶里的记录解出来。

**Done**：证据到 native-reader；`style_link` 对 version-2 的 `0x002C` 也返回字高（或明确
判定该记录确实无有效字高）；`0.254mm` 那类要么解释、要么继续按「非字高」拒收并标注。
DWG-0201 的 `2.5mm` 回退桶收缩，`pid_import` 的字高断言更新到新分布。

**Stop**：读不出就停下写 negative note。**不准**按「P&ID 正文一般 2.5mm」这种制图惯例
往回退桶里填值——那正是现在的行为，且已标注为回退，不是解码。

### S2：OCS 侧字高复核

`pid_import` 的 `lettering_carries_the_height_the_drawing_states` 断言随 S1 更新；
若 `2.464mm` 查清是某种真实字高（而非解码残差），记进分析文档。

### S3：E 的 negative note

产物：`docs/analysis/2026-08-07-fill-has-no-visible-consumer.md`（或并入现状快照）。
写清：六图 Sheet 流无填充几何、36 条填充样式无引用、占比 0；翻案的**最便宜验证**是
先证「某条几何的 `index` 指到 `0x002A`/`0x002B`」再谈解码，而不是先解样式。

### S4（可选）：F 的 GLine2d 参数域

只在 D 落地且有余力时做。不成立就维持 `PID-UNRESOLVED` 隐藏隔离，写负结论。

## 4. Gate 命令（沿用两仓门禁）

`pid-parse`：

```powershell
cargo build  --locked --workspace --all-targets
cargo test   --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs   # bash 门禁在本机不可用，用它替代
```

`OpenCADStudio`：

```powershell
cargo check --tests --examples
cargo test --locked --test pid_import -- --nocapture
cargo fmt --all -- --check
# clippy -D warnings 因存量债今天不可能全绿：改跑不带 -D，确认我方文件零命中
cargo clippy --locked --all-targets 2>&1 | Select-String "io.pid.rs|pid_import.rs"
$env:PID_SYMBOL_LIBRARY = "..\pid-parse\test-file\symbols-full"
cargo run --release --example pid_probe -- <六张 .pid>
```

## 5. Stop-And-Challenge

1. 想按制图惯例给字高/填充/线型**填**任何一个解不出来的值，而不标注为启发式。
2. 想在没有「被填充几何」的证据下先解填充样式——先证消费者存在，再谈解码。
3. 想给 `0x0013`/`0x003D`/`0x0020` 这些无 fixture 支撑的族硬写解码器。
4. 任何 parser promotion 缺字节区间、fixture ratchet、panic-safety 或 byte-audit movement。

## 6. 明确不做（本 phase）

| 项 | 为什么 |
|---|---|
| E 填充解码 | 可见占比实测为 0，解了也没得画；等带填充的 fixture |
| 字体名偏移 | 「最长 UTF-16 串」启发式已能读对，优先级低于字高档位 |
| `0x0013` / `0x003D` 尾部 / `0x0020` | 已有负结论或覆盖不足，不在关键路径 |
| OCS clippy 存量债（1062 条 empty-line-after-doc） | 是仓主的机械清理决策，不塞进显示收口 |
