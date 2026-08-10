# Phase 39：剩余显示缺口分诊（D 字高残余 / E 填充 / F 离群坐标）

> 日期：2026-08-07
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）两仓。
> 前序：Phase 38（语义上链 + 虚线线型）已 S1–S6 全部落地并提交。
> 本计划是**只读分诊 + 排序**：先拿实测占比给 Phase 38 §6 留的三项排序，
> 再定各项该不该做、按什么次序做。**本轮不改任何解码/产品代码。**
>
> **2026-08-10 更新：S1 已执行，结果是负的，本计划的 D 项据此改判。**
> 见 `docs/analysis/2026-08-10-text-height-residue-is-one-sentinel-not-version-2.md`
> 与 `examples/probe_text_height_fallback`。下面 §1 的 D、§2、§3 已按实测改写；
> 原文保留在各处的「原判」里，因为判错的理由本身值得记着。

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

### D — 字高残余（**已重测：25/184，全是同一个哨兵值**）

**原判**：DWG-0201 的 `2.5×9` 是 ISO 3098 回退，病因是 `0x002C` 的 **version 2 路径未读**
（`sub_10002CFC` + `sub_10002CC0`），可见收益中、成本低（复用 S5 的 native-reader 手法）。

**2026-08-10 实测后改判**（`probe_text_height_fallback`）：

- **占比小一个数量级。** 全语料 184 条文字记录里 159 条解出真字高，回退 **25 条**；
  DWG-0201 只有 **1** 条。原判说的 `2.5×9` 里至少 8 条是**图纸自己声明的 2.5mm**
  ——这张图把 2.5mm 写了 10 次。真正受影响的只有工艺管道-1（43 条里 16 条）。
- **病因不是 version 2。** 记录里**根本没有逐条写版本号**：拿三个已知版本的族
  （`0x002C`/`0x002D` = 3、`0x002E` = 2）做对照，`+0..+13` 没有任何一列携带各族的版本。
  版本来自流/文档级 schema，「这条是 v2 还是 v3」在文件侧不可判定。
- **25 条回退是同一条模板样式。** 它们指向的三条 `JStyleTextChar`（四张图）除身份字段外
  **80 字节逐字节相同**，字高一律 `0.254mm` = `0.01″`。两跳链路 184/184 全通，
  没有一条是「指空 / 缺席 / 布局异常」。
- **字高没有第二个来源**：段落样式里没有落在制图区间的 `f64`；`igTextBox` 第三个尾部
  double 在被拒与解出两侧同分布，不是缩放系数。
- 可见收益：**低**（且拒收本身是对的）。成本：原估的 IDA 切片**解释不了任何一条回退**。

### E — 填充 `0x002A` / `0x002B`

- 占比实测：**可见占比 ≈ 0**。六图里没有一条被填充的几何；36 条填充样式定义无几何消费者。
- 可见收益：**≈ 0**（在当前语料上）。解出填充样式的颜色/图案，画面不会多出任何东西，
  因为没有面去填。
- 成本：一个 native-reader 取证切片（同 B/D）。
- 结论：**本期不做**。这不是「解不出来」，是「解出来也没得画」。→ §3 写 negative note。

### F — 离群坐标（**已查清：不是解码欠账，是扫描伪命中**）

**原判**：DWG-0201 2 条 `(2.1,-0.0)→(1000.0,-1.0)` 单位线已隔离在隐藏层
`PID-UNRESOLVED`；卡在 `GLine2d` 的参数域（起止参数）未解 → 退化成「原点走一个源单位」
的 1000mm 直线；成本中，是一条独立取证线。

**2026-08-10 实测后改判**（`probe_gline2d_parameter_domain`）：

- **参数域没有未解**。解码器读出的 `param_start` / `param_end` 就是 `[0.0021, 1.0]`。
- **这三条不是记录。** `Sheet*` 流从 `+8` 起是一条严格记录链，不滑动地走一遍
  **100% 覆盖到流尾**（DWG-0201 `/Sheet6` 359 条 / 29594 字节；A01 19 条 / 1780 字节），
  而三条 `GLine2d` 的偏移**一条都不在记录起点上**——它们各自落在一条
  `0x003D igSmartFrame2d` 记录内部的 **`+160`**，三条都是同一个偏移。
- **「类型码 `0x3FE6`」是页幅长宽比的高两字节。** 记录 `+160` = 载荷 `+154`，
  往前退 6 字节正是载荷 `+148`——`2026-07-27-smartframe-003d-native-reader.md`
  记的那个恒 `≈0.7072 = 1/√2` 的长宽比。`1/√2` 落在 f64 的 `0x3FE6……` 桶里，
  高两字节字面就是 `E6 3F`。全语料 **11 条页框、11 条的高位字都读作 `0x3FE6`**，
  其中 3 条恰好通过后续全部校验。
- 原判据是恒真式：候选集由 `GLine2d` 校验规则定义，再回头观察类型码一致。
  独立判据两条（链式归属、`radsrvitem` 权威类型枚举无 `0x3FE6`）都指向否定。
- 全部证据见 `docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`。

## 2. 排序结论

**原判**（08-07，按「可见收益 ÷ 成本」）：D 先做 → E 不做写 negative note → F 垫底。

**改判**（08-10，D 重测之后）：

1. **D 已结**，且不是按原计划结的：解码侧无事可做，`style_link` 拒收 `0.254mm` 的现行行为
   就是对的。剩下的唯一动作在消费侧——**让回退可见**（见 S2）。
2. **E 不做**，写 negative note 封存：占比实测为 0，等出现「带填充的 fixture」再翻案。（不变）
3. ~~**F 升为剩余项里的第一名**~~ **F 已查清并关闭（08-10）**：它不是 `GLine2d`
   解码欠账，那三条「记录」是 `igSmartFrame2d` 页幅长宽比造成的扫描伪命中。
   没有参数域要解。剩下的是一次**撤回**动作（见 S4）。

## 3. 推荐执行序列

```text
S0  基线确认（沿用 Phase 38 S6 的 08-07 census）
 -> S1  [pid-parse] 缺口 D 归因                              ✅ 2026-08-10 完成，负结论
 -> S2  [OCS] 让字高回退可见 + 把重测后的分布钉进断言        ✅ 2026-08-10 完成
 -> S4  [pid-parse] 缺口 F 归因                              ✅ 2026-08-10 完成，判定为伪命中
 -> S5  [两仓] 撤回 0x3FE6 GLine2d 家族                      ✅ 2026-08-10 完成
 -> S3  [pid-parse] E 写 negative note（填充零可见占比）    ← 当前，Phase 39 最后一项
```

### S1：缺口 D 归因 ✅ 完成（负结论）

**原计划**：按 S5 已验证三次的路子（type code → CLSID → `style.dll` vtable → 版本分发
序列化器 → 字段偏移）读 `0x002C` 的 **version-2** 路径（`sub_10002CFC` / `sub_10002CC0`），
把回退桶里的记录解出来。

**实际执行**：先按 §5 Stop-And-Challenge 的第一刀问「判据成不成立」，结果这一刀就把
切片本身否掉了——版本号不逐条写，回退桶也不是 version-2 造成的。全部实测见
`docs/analysis/2026-08-10-text-height-residue-is-one-sentinel-not-version-2.md`。
`0x002C` 的 version-2 路径**移出关键路径**：它既不可从文件侧判定，也解释不了任何一条回退。

### S2：OCS 侧让回退可见

`style_link` 拒收之后，25 条真位号被画成 `TEXT_HEIGHT_MM` 而日志里一个字都没有。
`report_import` 已经会按名字报告解不出来的图形族，字高回退应当同等对待。

**Done**：导入日志说出「多少条文字保留了回退字高」；`pid_import` 的
`lettering_carries_the_height_the_drawing_states` 断言注释更新——`2.5×9` 里至多 1 条是回退，
其余是图纸自己声明的 2.5mm，这一点原来写反了。`2.464mm` 仍是解出来的真实值
（DWG-0201 3 条、DWG-0202 4 条），不落制图档位但**不是**解码残差。

### S4：缺口 F 归因 ✅ 完成（判定为伪命中）

**原计划**：只在 D 落地且有余力时做 `GLine2d` 参数域取证；不成立就维持
`PID-UNRESOLVED` 隐藏隔离，写负结论。

**实际执行**：先问「这三条记录到底在不在记录链上」，答案是不在——判定见 §1 F 与
`docs/analysis/2026-08-10-gline2d-is-the-iso-page-ratio-not-a-record.md`。
按计划的 Stop 规则，负结论已写；隐藏隔离暂时维持，直到 S5 把源头撤掉。

### S5：撤回 `0x3FE6 GLine2d` 家族 ✅ 完成

根因是 `GLine2dDecoder` 走 `PsmRecordDecoder::scan`（逐偏移试、失败滑一字节）。
Sheet 流已证明是 100% 干净的记录链，判据换成**链成员资格**：新增
`sheet_record_starts`（走不通整条流就返回空，不退化成扫描），
`decode_primitive_lines` 只在记录起点上尝试 `decode_at`。

**已落地**：

| 仓 | 改动 |
|---|---|
| `pid-parse` | `decode_primitive_lines` 链式化；`PSM_TYPE_CODE_GLINE2D` 文档写明撤回理由 |
| `pid-parse` | 合成 fixture 的 `bytes_to_follow` 改为 60（原来的 48 让记录声明的长度短于它实际占的字节，不是链能跨过的记录）；所有 `rejects_*` 测试改走 `synthetic_sheet_stream`，否则它们会因为「没有流头」而空过 |
| `pid-parse` | 新增 `primitive_line_inside_another_records_payload_is_not_a_record`，把语料失败缩成一个单元测试 |
| `pid-parse` | golden 重新 bless：DWG-0201 少 2 个实体、A01 少 1 个，纯删除 |
| `pid-parse` | 两个具名测试反向：`dwg0201_emits_no_gline2d_lines_and_keeps_its_inferred_floor`、`primitive_line_decoder_finds_no_gline2d_and_holds_invariants_if_it_ever_does` |
| `OpenCADStudio` | 删掉 `unresolved_unit_line` 特判、`LAYER_UNRESOLVED` 与取景例外；测试改为 `no_line_spans_the_sheet_and_the_diagnostic_layer_is_gone` |

byte-audit 那三段区间随之归还给 `0x003D`。

**没有顺手做**：把 `scan` 整体换成链式走查。其余家族的伪命中率没测过，
那是独立一轮，每族都需要自己的语料对照。

### S3：E 的 negative note

产物：`docs/analysis/2026-08-07-fill-has-no-visible-consumer.md`（或并入现状快照）。
写清：六图 Sheet 流无填充几何、36 条填充样式无引用、占比 0；翻案的**最便宜验证**是
先证「某条几何的 `index` 指到 `0x002A`/`0x002B`」再谈解码，而不是先解样式。

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
