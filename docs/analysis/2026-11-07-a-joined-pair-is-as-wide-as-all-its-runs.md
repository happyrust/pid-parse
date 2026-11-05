# 合并成一串的标签只量到第一串的宽度

日期：2026-11-07

## 结论

清单 #9（I / JK 的 Center/Bottom 类 mean_advance −0.279 / −0.197 字高）不是出图问题，是比对报告自己的读数错了。

格网参考号 `B2` `B5` 是两条各自居中的标签，两边出图的位置一致：

| | 原点 (mm) | advance (mm) |
|---|---|---|
| 参考 `B2B5`（一串） | (778.43, 717.13) | 7.903 |
| 我们的 `B2` | (778.33, 717.23) | 3.598 |
| 我们的 `B5` | (782.77, 717.05) | 3.598 |

参考图把它们读回成一串 `B2B5`，我们读回两串。`match_text` 第二遍按拼接把它们配对，但 `record_offset`
只拿了我们的第一串（`B2`）去量宽度，于是这一对记成「窄了 4.3 mm = 1.2 字高」。I 图有 1 对，JK 有 3 对
（`B2B5`、两个 `D7D5`），LM 有 2 对，正好是这三张图 Center/Bottom 和 Center/Top 的负 advance。
反过来，参考图拆成多串、我们合成一串时，也只量参考的第一串，读成我们宽了：`unknown` 类的 +0.29…+1.34
就是这么来的。

## 修改

`record_offset` 改为接收两边配上的整串（只有一串时就是一串）。宽度从第一串的原点量到最远一串的末端，方向取第一串的书写方向。
单测 `a_label_fused_into_one_run_reports_the_whole_split_width` 钉住 `B2B5` 这种情况。

`reference_plots_acceptance` 新增门禁：10 条以上的类，|mean_advance| ≤ 0.10 字高。修复后最宽的一类是
烃水露点页的 Left/Ascender，−0.092。修复前 A2 Left/Ascender 是 −0.320，I 的 Center/Bottom 是 −0.279，这道门禁都会拦下。

## 读数（对比 80c1c96）

| 图 | 类 | n | mean_advance |
|---|---|---|---|
| I | Center/Bottom | 4 | −0.279 → +0.030 |
| JK | Center/Bottom | 11 | −0.197 → +0.028 |
| JK | Center/Top | 12 | −0.068 → +0.027 |
| LM | Center/Bottom | 11 | −0.086 → +0.026 |
| LM | Center/Top | 15 | −0.059 → +0.028 |
| 闪蒸A | unknown | 7 | +0.617 → +0.038 |

Center/Middle 类也有 +0.003…+0.029 的变化，都是同样的拼接配对。coverage、extra、文字配对数、
along / down 读数没有变化，这次只改了报告。

剩下的是全语料共有的 +0.03…+0.06：我们的 SimSun 和 Arial Narrow 比参考图宽 1–3%（见
`2026-11-07-the-a2-8pt-arial-was-missing-its-tf.md` 的 Arial Narrow 一节）。
