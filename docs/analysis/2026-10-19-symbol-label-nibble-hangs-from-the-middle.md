# 2026-10-19 — 符号体自带的标签挂在字盒中线上，不是基线上

范围：`pid-plot/src/model.rs::TextLine::from_symbol_anchor_code`，以及五张图
出图对比的文字数字。上一轮把图纸记录的竖直半字节改成标志位读法
（`2026-10-19-a-pair-of-flags-not-an-index.md`），符号体一侧故意保留了旧的
索引读法；这一轮把它也测掉了。剩下未对上的我方文字 38 条里，11 条属于这一
类（I 图 9 个 `M`，L / H 各 1 个 `S`），现在是 29 条。

## 量法

新探针 `pid-plot/examples/probe_symbol_label_offsets.rs`：对 `lay_out_drawing`
产出的每条带 `symbol` 来源的 `PaperShape::Text`，按字符串与最近位置配对参考
图上的 span，比较**我方实际基线**（`origin − 行位 × 字高`，随旋转）与参考
基线的差，按 `.sym` 文件与锚点类分组。

```text
cargo run -p pid-plot --example probe_symbol_label_offsets -- \
    "<drawing.pid>" "<reference.pdf>" "<corpus>"
```

改前（I 图，DETAIL=1 的逐条输出）：

| 符号 | 码 | 我方行位 | 条数 | 我方基线 − 参考基线 |
|---|---|---:|---:|---:|
| `MOV-GLV-Electrically Operated Valve with Flange 02.sym` | `0x10` | Bottom | 2 | **+1.06 / +1.11 mm** |
| `PV-Pressure Regulating Valve with Flange.sym` | `0x10` | Bottom | 6 | **+0.97 … +1.12 mm** |
| `Electric Motor Type 1.sym` | `0x10` | Bottom | 2 | **+1.57 / +1.68 mm** |
| `FT-Electromagnetic Flowmeter.sym` | 低半字节 `1` | Top | 1 | +0.83 mm（顶是对的，差的是字高） |
| `Turn off the system signal.sym` | `0x12` | Middle | 3 | −0.51 / −0.62（中线是对的） |

`0x10` 的标签字高 2.54 mm、参考 2.91 mm，差 1.06 mm ≈ 我方半个字高（1.27 mm）
减去 0.2 mm —— 落在 1 mm 匹配窗内。也就是说参考把它们画在**中线**上，而索引
读法把半字节 `0` 读成「基线」，整条高了约一个字高的一半。

## 规则

符号体这一侧没有「基线」这一类：

```text
nibble % 3 == 1  → 字盒顶
其余             → 字盒中线（0、2、5、8 都是）
```

和图纸记录的区别只在 `0`：图纸上 `0` 确实写在基线上（12 条，比值 0.00），
符号体上 `0` 与 `2` 一样落在中线。低半字节为 `1` 的顶类不动 —— 探针里它们的
`Top` 与参考一致。

## 结果（五张图，重新出图后重新对比）

| 图 | 文字对上（前） | 文字对上（后） | 参考总数 | 未对上（我方，前→后） |
|---|---:|---:|---:|---:|
| I井场 | 530 | **537** | 552 | 20 → 13 |
| 一级闪蒸分离器A | 144 | 144 | 152 | 5 → 5 |
| 一级闪蒸分离器B | 119 | 119 | 127 | 5 → 5 |
| 低温分离器A | 166 | **167** | 171 | 5 → 4 |
| 换热器CD | 249 | **250** | 250 | 3 → 2 |
| 合计 | 1208 | **1217** | 1252 | 38 → 29 |

覆盖率、多画毫米与 fit 一个数没动（只动文字落点）。I 图 +7：9 个 `M` 对上
7 个；L / H 各 +1，是那两个 `S`。`Electric Motor` 的两条从 +1.6 mm 收到
+0.55 / +0.66 mm，也在窗内。

## 还没对上的两条 `M`

I 图剩下的两条 `M` 在 `PV-Angle Throttle Control Valve.sym` 里，标签旋转
−90°。探针的逐条差值只适合正立的文字：它算的 y 位移对旋转标签没有意义
（真正的位移沿旋转后的行向），而且「按字符串 + 最近位置」配对在重复标签上
可能配到相邻的那一个。它们改前改后都未对上，数量没有变化；要坐实得先把
探针改成旋转感知的，属于下一轮。

## 改动

- `pid-plot/src/model.rs`：`TextLine::from_symbol_anchor_code` 的 `0` 从
  `Bottom` 改为 `Middle`，并在文档注释里写明五张图的测量；新增
  `a_symbol_label_nibble_hangs_from_the_middle` 钉住 `0x10 / 0x11 / 0x12 /
  0x15 / 0x18` 五个码。
- `pid-plot/tests/plot_compare.rs`：文字门限 530/144/119/166/249 →
  537/144/119/167/250。
- 证据探针 `pid-plot/examples/probe_symbol_label_offsets.rs`（`DETAIL=1`
  逐条输出）。
