# `0x3FE6 GLine2d` 不是记录族，是 ISO 页幅长宽比 `1/√2` 的高两字节

> 日期：2026-08-10
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费）
> 结论类型：**撤回**——Phase 14 以来当作记录族解码的 `0x3FE6`，经链式取证判定为
> `0x003D igSmartFrame2d` 载荷里一个 `f64` 字段造成的扫描伪命中。
> 前置：`2026-07-27-smartframe-003d-native-reader.md`（`0x003D` 布局，含 `+148` 长宽比）、
> `docs/plans/2026-08-07-phase39-render-gap-triage-cn.md` §1 F
> 工具：`examples/probe_gline2d_parameter_domain`

## 0. 一句话

Phase 39 把「离群坐标」记为缺口 F，病因写作「`GLine2d` 的参数域未解，退化成
从原点走一个源单位的 1000mm 直线」。**参数域没有未解**——解码器读的那对
`param_start` / `param_end` 就是 `[0.0021, 1.0]`，一字不差。问题在上一层：
**这三条记录不是记录。** 它们落在 `igSmartFrame2d` 页框记录的**内部**，
「类型码 `0x3FE6`」是该记录 `+148` 处那个 `f64` 长宽比（`1/√2 ≈ 0.7072`）的
**高两字节**。

## 1. Sheet 流是一条严格记录链，而这三条不在链上

`Sheet*` 流从 `+8` 起就是 `u16 type_word` + `u32 bytes_to_follow` + 载荷首尾相接的
纯记录链。**不滑动、不重同步地走一遍，100% 覆盖到流尾**：

| 流 | 记录数 | 覆盖 |
|---|---:|---|
| DWG-0201 `/Sheet6` | 359 | 29594 / 29594 字节（100%） |
| A01 `/Sheet6` | 19 | 1780 / 1780 字节（100%） |

三条 `GLine2d` 的偏移**一条都不在这 378 个记录起点里**。逐条查它们落在谁的肚子里：

| fixture | 命中偏移 | 所属记录 | 深入 |
|---|---:|---|---:|
| DWG-0201 | 17216 | `0x003D` @ 17056..17308 | **+160** |
| DWG-0201 | 28518 | `0x003D` @ 28358..28602 | **+160** |
| A01 | 579 | `0x003D` @ 419..663 | **+160** |

三条都是 `0x003D`，三条都是 **+160**，一个字节不差。这不是「附近」，是同一个字段。

## 2. 那个字段是页幅长宽比，早在 07-27 就写下来了

记录 `+160` = 载荷 `+154`。往前退 6 字节取一个 `f64`，起点是**载荷 `+148`**——
正是 `2026-07-27-smartframe-003d-native-reader.md` 记的那一条：

> 11 条 Linked/Embedded 全带 ISO A 系页幅（长宽比 `+148` 恒 ≈ 0.7072 = 1/√2）

实测读数：DWG-0201 `0.707235664`，A01 `0.707071179`。而 `1/√2` 落在 f64 的
`0x3FE6……` 桶里（该桶覆盖 `[0.6875, 0.71875)`），所以**它的高两字节字面就是
`E6 3F`**——小端读作 `0x3FE6`，也就是被当成类型码的那两个字节。

全语料交叉验证（`probe_gline2d_parameter_domain` 末段）：

```text
11 frame(s); 11 carry a ratio whose top word reads 0x3FE6; 3 of those pass
every remaining GLine2d rule.
```

**11 条页框，11 条的长宽比高位字都读作 `0x3FE6`。** 差别只在后续校验：另外 8 条在
「`bytes_to_follow` 合理 / 六个 f64 有限且在域内 / 方向是单位向量 /
`param_start < param_end`」这串规则里的某一条上掉出去，3 条全过。

## 3. 剩下的字段也都是碎片

以 A01 那条为例，把「记录」按解码器的读法摊开：

```text
E6 3F              <- "type_code"        实为 ratio 的高两字节
CC 00 00 00        <- "bytes_to_follow"  实为 ratio 之后的 4 字节
02 00 27 50        <- "oid"              实为一个 4 字节标签
00 00 00 00 00 00 F0 3F   1.0  ┐
00 00 00 00 00 00 00 00   0.0  │
00 00 00 00 00 00 00 00   0.0  ├ 一个放置矩阵：[1 0; 0 1] + 平移 + 齐次 1
00 00 00 00 00 00 F0 3F   1.0  │
00 00 B6 A0 F7 C6 B0 3E   1e-6 │
00 00 B6 A0 F7 C6 B0 3E   1e-6 │
00 00 00 00 00 00 F0 3F   1.0  ┘
```

两处旁证：

- **「oid」在三条上完全相同**（`02 00 27 50` = 1344733186），跨两张互不相干的图。
  对象标识符不会这样。它与 `probe_igsymbol2d_jsite_link` 里那个放置矩阵标签
  `02 00 A7 50` 是同一形状的 4 字节标签。
- **校验之所以全过，是矩阵的形状恰好符合**：单位阵的 `1.0` 被读成方向向量的 x 分量
  （模长自然是 1），齐次项的 `1.0` 被读成 `param_end`，平移分量被读成 `param_start`，
  于是 `param_start < param_end` 也成立。三条校验规则**同时**被一个单位矩阵满足，
  这不是巧合的堆叠，是同一个原因。

## 4. 原判据是恒真式

`PSM_TYPE_CODE_GLINE2D` 的文档写：

> 每个「18 字节 PSM 头之后跟 48 字节、且满足 `GLine2d` 校验规则」的记录，
> 其 `type_code` 都等于 `0x3FE6`。

候选集**是被这套校验规则定义出来的**，然后回头观察它们的类型码一致——这是循环论证，
不是交叉验证。真正的独立判据有两条，两条都指向否定：

1. **链式归属**（本文 §1）：不在记录起点上。
2. **`radsrvitem.dll` 的权威类型枚举**（`sub_56448F70`，见
   `2026-07-27-ugeom2d1-curve-readers-ida.md` 结论 3）覆盖 `0x06`..`0x117`，
   **没有 `0x3FE6`**。

`radsrvitem.dll` 里确实有一个 `GLine2d::Validate`（`sub_56524C50`），六个 double 的
参数式布局也确实是它的——**类是真的，盘上这三条不是它的实例**。

## 5. 影响

- **缺口 F 消失。** 没有参数域要解；`GLine2d` 参数域取证这条线可以关掉。
- **那三条 1000mm 直线是幻影**，不是「解错的真记录」。OCS 把它们隔离在隐藏层
  `PID-UNRESOLVED` 是当时能做的最好处置，但正确的处置是**根本不发**。
- **byte-audit 有重复记账**：这三条各自宣称 954 / 954 / 210 字节的区间，
  而那些字节属于它们所在的 `0x003D` 记录。
- **`0x003D` 页框的解码没有问题**：页幅 `+76/+84` 与长宽比 `+148` 都读对了，
  正是「读对了」才让高位字露出来。

## 6. 该怎么修（未执行，另开动作）

根因是 `GLine2dDecoder` 走的是 `PsmRecordDecoder::scan`——**逐偏移试、失败滑一字节**。
既然 Sheet 流已证明是 100% 干净的记录链，正确的判据是**链成员资格**：
`decode_primitive_lines` 先走链，只在记录起点上尝试 `decode_at`。
这样这三条自动消失，且判据从「长得像」换成「就在那儿」。

这是一次**撤回**，会改动公开产物，需要单独一轮：

| 受影响 | 变化 |
|---|---|
| `tests/golden/geometry/*.json` | DWG-0201 少 2 个实体、A01 少 1 个，需 `UPDATE_GEOMETRY_GOLDEN=1` 重新 bless |
| `tests/parse_real_files.rs` | `dwg0201_emits_decoded_primitive_lines_without_inferred_regression` 与 `primitive_line_decoder_emits_decoded_lines_with_provenance` 断言「至少一条」，须改写为断言零条并说明原因 |
| `OpenCADStudio` | `unresolved_unit_line` 特判与 `PID-UNRESOLVED` 图层上的单位线不再有来源；`tests/pid_import.rs` 的 `unresolved_unit_lines_are_kept_off_the_drawing` 须改写 |
| byte-audit | 三段区间归还给 `0x003D` |

**不建议**顺手把 `scan` 换成链式走查——其余家族的伪命中率没有测过，那是独立一轮
的事，需要各自的语料对照。

## 7. 复现

```powershell
cargo run --example probe_gline2d_parameter_domain
```

前半段把三条记录整条摊开（链式归属、类型字所在 `f64`、六个 double、尾部字节），
末段在全语料 `0x003D` 上统计长宽比高位字与后续校验的通过情况。
