# A2 的 8 pt Arial 管线号缺一行 Tf

日期：2026-11-07

## 结论

清单 #8（A2 `轻烃分离器I` 的 Left/Ascender 类 15 条，mean_advance −0.32 字高）的原因：两条 8 pt Arial
管线号 `65-AI-010901-I2`、`200-LFL-0109201-B2` 被我们按样式字高直接写成 8 pt（2.822 mm），
而参考图写的是 `Tf 11.16`。`plotted_size_pt` 的实测表 `measured_tf` 里只有 Arial 9 pt → 12.45，
没有 8 pt 这一行。字号小了 1.40 倍，这两条字串因此窄了 8.2 mm 和 10.9 mm，把整类的均值拉到 −0.32。

同一张表也没有 Arial 14.4 pt（5.08 mm）。`ESD Interlock.sym` 的 `I` 就是这个字号，参考图写
`Tf 20.19`（A2、低温A、空冷 A/B/C、换热 CD 都有）。

两行都是参考图上直接读出来的 Tf，比例和已有的 9 pt → 12.45 一致（11.16 / 8 = 1.395，
12.45 / 9 = 1.383，20.19 / 14.4 = 1.402）：

| 字体 | 样式字号 | 参考 Tf | 出处 |
|---|---|---|---|
| Arial | 8 pt | 11.16 | A2 两条管线号 |
| Arial | 14.4 pt | 20.19 | 6 张图上 `ESD Interlock` 的 `I` |

## 读数（对比 b70ab7b）

| 图 | 类 | mean_advance | max_shift (mm) |
|---|---|---|---|
| A2 | Left/Ascender（15） | −0.320 → −0.004 | 0.42 → 0.42 |
| A2 | symbol/Middle | −0.016 → +0.024 | 0.35 → 0.19 |
| 低温A | symbol/Middle | −0.028 → −0.001 | 0.28 → 0.19 |
| 空冷A / 空冷B | symbol/Middle | −0.033 → +0.003 | 0.35 → 0.22 |
| 空冷C | symbol/Middle | −0.044 → −0.004 | 0.35 → 0.19 |
| 换热CD | symbol/Middle | +0.009 → +0.025 | 0.29 → 0.17 |

coverage、extra、文字配对数、pen 读数在 11 张图上都没有变化。

## 没改的部分

`measured_tf` 的注释说过，符号体自身的 7.2 pt 标签故意不跟参考图的 Tf 走：我们能加载的 Arial 每个字比
参考图宽 14.6%。A0 三张图上的 `M`（2.540 mm w800，参考 2.913）和电机的 `M`（4.085 mm，参考 4.551），
以及 `Turn off the system signal` 的 `S`（5.08 mm Arial Narrow，参考 5.535 / 5.602），都属于这一类，
这次没有动。

Arial Narrow 在参考图里比 Windows 自带的 `arialn.ttf` 窄 4–5%（参考嵌入子集的 `/W` 给出的数字是
456/1000 em，和 `arialn.ttf` 相同，差值由每个字串自己的 `Tc` 补上，`Tc` 在 −0.10…−0.32 之间，随字串变化）。
按单字反解的字宽也不是 hmtx 的常数倍，而且没有找到能复现的像素取整规则（`hdmx`、按 dpi 取整都试过），
所以 Center/Middle 类 +0.03…+0.06 的 advance 余量留在清单里。
