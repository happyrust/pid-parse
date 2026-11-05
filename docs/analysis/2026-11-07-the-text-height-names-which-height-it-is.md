# 字高说的是字体的哪一个高（2026-11-07）

十一张参考图上，阀门、电机符号里的 `M`、`Turn off` 的 `S` 这一类 `symbol/Middle`
标签整体偏低：A0 三张 +0.094 … +0.098 个字高，A1 三张 +0.055 … +0.058，其余
+0.03 … +0.10。图纸自己的 Arial `Left/Ascender` 类也一律偏低 +0.018 … +0.039。
两类都不是偏移量的问题：是把「字高」一律当成大写字高来算。

## `JStyleTextChar` 的 `+41`

`style.dll` 的版本 3 读取器（`sub_10030A20`）在 `+42` 的字高 `f64` 之前单读一个
字节，存进对象 `+360`；字高的 setter（`IJStyleTextCharImp` 槽 6，`sub_10031530`）
把这个字节和字高一起写，并拒绝 `10..=14` 以外的码；getter（`sub_100305E0` →
`sub_1007B270`）按码 `11` / `12` / `13` 从一个三元组里取不同的分量来换算字高，
码 `14` 再乘 `18288`。所以字高本身不是一个尺寸，它是字体某一个高度的值，`+41`
说是哪一个。`style.dll` 里不做 cap/ascent 修正，这一步在渲染端。

（`+26` 的 `u16` 不是它：那是字段覆盖掩码，置位表示继承，`0xFFFF` 全继承。）

`examples/probe_which_height_a_text_char_names.rs` 走 `验收数据`、`test-file`、
`backup-plant-data` 1283 个 `.pid` / `.sym`：

| 码 | 合计 | 图纸 StyleCluster | JSite 副本 | `.sym` 体 |
|---|---:|---:|---:|---:|
| `0x0D` | 5273 | 505 | 368 | 4400 |
| `0x0C` | 675 | 2 | 76 | 597 |
| `0x0B` | 118 | 0 | 4 | 114 |

## 两个码各是哪个高

拿参考图上每条标签的 `Tf`（即 em）除以我方样式字高：

- `0x0D`：全部图纸文字，以及 `I-P`、`ESD`、`FT`、`SSV` 的符号文字。比值
  1.38 … 1.51。Arial 1.383（1/cap = 1/0.716 = 1.397），Arial Narrow 2.47 mm
  1.453、3.53 mm 1.440，宋体 1.468 … 1.49（1/0.684 = 1.462），仿宋 1.467 / 1.509
  （1/0.668 = 1.497）。**字高 = 大写字高。**
- `0x0C`：阀门、电机的 `M`（Arial 2.54 / 5.08 mm），`Turn off`、`3-Way` 的 `S`。
  比值 1.09 … 1.15，对 Arial 的 1/winAscent = 1/0.905 = 1.105。**字高 = 升部。**

基线落点也一样分成两档（参考基线到插入点的距离 / 我方样式字高）：

| 类 | 实测 | 按「字高 = cap」算 | 按码算 |
|---|---|---:|---:|
| `0x0C` 符号 Middle（PV / Motor / Turn off / 3-Way） | 0.37 … 0.48 | 0.50 | cap/(2·ascent) = 0.396 |
| `0x0D` Arial 3.18 图纸 Ascender（n = 192） | 1.267 | 1.32 | ascent/cap = 1.264 |
| `0x0D` Arial Narrow 2.47 图纸 Ascender（n = 57） | 1.337 | 1.32 | 0.9219/0.7158 = 1.288 |
| `0x0D` 图纸 Middle（n = 1429） | 0.509 | 0.50 | 0.50 |

旧的 1.32 是用 Arial 旧的大写字高 0.688 em 算的；`arial.ttf` 的 `OS/2 sCapHeight`
是 1467/2048 = 0.716，`H` 字形顶 0.7158。

## 改法

- `style_link`：新常量 `TEXT_CHAR_HEIGHT_MODE_OFFSET = 41`、
  `TEXT_HEIGHT_MODE_ASCENT = 0x0C`、`TEXT_HEIGHT_MODE_CAP = 0x0D`；
  `StyleRecord::char_height_mode`、`ResolvedTextHeight::height_mode`、
  `SymbolLettering::height_mode` 把字节带出来。
- `pid-plot`：`PaperShape::Text::height_basis: TextHeightBasis { Cap, Ascent }`，
  `0x0C` 为 `Ascent`，其余（含 `0x0B`、缺失）为 `Cap`。
- `write.rs`：每个嵌入字体读一次 `FaceHeights { ascent, descent, cap }`
  （`OS/2 usWinAscent / usWinDescent`，即 GDI 的 `tmAscent / tmDescent`；
  `sCapHeight`，`ARIALN.TTF` 的 OS/2 版本 1 没有就取 `H` 字形顶）。插入线的下落
  = 样式字高 × 该线的字体高度 / 样式字高所量的那个字体高度。`ASCENDER_HEIGHTS`
  （1.32）和 `DESCENDER_HEIGHTS`（0.31）两个常量删除。
- 字号（`Tf`）不动：符号文字的 7.2 pt 仍然按 `measured_tf` 的注释处理。

## 读数（`reference_plots_acceptance`，改前 → 改后，mean_down_heights）

| 页 | `symbol/Middle` | `Left/Ascender` |
|---|---|---|
| I | +0.094 → +0.001 | +0.026 → −0.007 |
| J K | +0.097 → +0.005 | +0.030 → −0.004 |
| L M | +0.098 → +0.006 | +0.033 → −0.002 |
| 闪蒸 A | +0.096 → −0.005 | +0.036 → −0.001 |
| 闪蒸 B | +0.095 → −0.006 | +0.034 → −0.001 |
| 低温 A | +0.076 → +0.012 | +0.033 → −0.002 |
| 空冷 A / B | +0.058 → +0.006 | +0.026 → −0.007 |
| 空冷 C | +0.055 → +0.008 | +0.026 → −0.007 |
| 换热 CD | +0.083 → +0.008 | +0.039 → +0.001 |
| A2 | +0.029 → −0.017 | +0.018 → −0.015 |

`symbol/Ascender` 同样从 +0.03 … +0.05 落到 ±0.014；`symbol/Middle` 的最大单条
偏移从 0.54 … 0.70 mm 落到 0.09 … 0.41 mm。coverage、extra、uncovered、文字配对数、
pen 一致数在十一张上都不变（这些标签原本就在 1 mm 配对窗口内）。

门禁收紧：`reference_plots_acceptance` 新增「≥ 10 条的类 `|mean_down| ≤ 0.04`」，
`plot_compare` 的 `CLASS_RATCHET_HEIGHTS` 0.10 → 0.05。

## 没有解决的

- `0x0B`（118 条，只在符号体里）没有在参考图上出现过，按 `Cap` 处理；
  `sub_1007B270` 的三元组分量没有读出具体含义。
- Arial Narrow 图纸 Ascender 实测 1.337，按字体表算 1.288，仍差 0.05 个字高
  （约 0.12 mm）；该类样本混在 `Left/Ascender` 里，均值已在 ±0.015 内。
