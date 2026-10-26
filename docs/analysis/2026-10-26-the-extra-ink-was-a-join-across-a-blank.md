# 多画的 12 mm 是合并桥接空白，不是墨（2026-10-26）

> 承接 `2026-10-23-pdf-export-recheck-after-the-border-template.md`（那篇记下 25.5 / 22.6 / 1.5 /
> 37.9 / 24.6 mm 的多画，除图框外「无一项归属」）。本文给出归属：**这些毫米大多不是纸上的墨**，
> 是比较器把同一条中心线上的两段合并成一条时，顺手跨过了符号体内部留白。
> 落地改动在 `pid-plot/src/compare.rs`：多画按落笔段采样，覆盖仍按合并后的中心线。

## 一句话

`join_centerlines` 用 `JOIN_GAP_MM = 3.0` 把相距 3 mm 以内的共线段并成一条 —— 虚线要靠它
才能整条参与比较。球阀体的两条斜线止于圆（半径 1.269 mm），圆内只有约 **1.8 mm** 空白，
于是被并成一条 8.52 mm 的线；采样器顺着它走进圆内的空白，把那段约 **1.5 mm** 的纸面记成我们的墨，
再按「最近图纸表笔划」归到球阀名下。

## 证据一：球阀体每一笔都压在参考墨上

按 0.25 mm 采样模型里所有 ball valve 图元、对参考出图查 0.5 mm 容差：

| 图 | 球阀墨量 | 未被参考覆盖 |
|---|---:|---:|
| I井场 | 1538.231 mm | **0.000 mm** |
| 一级闪蒸分离器A | 351.638 mm | **0.000 mm** |

半径 8 mm 范围内也找不到球阀附近的浮墨：A 上 0 笔，I 上只有 `Skid Mounted Equipment`
的四条横线与一条超声流量计弧。球阀一条都不欠。

## 证据二：被判多画的每一段都落在圆心

在 `split_extra` 里临时打印每一段被判多画的合并线（打印已回退），A 上全部同一形状：

```text
EXTRA 1.50 mm  run (207.71,332.03) -> (207.26,331.14)
                line (209.39,335.39) -> (205.58,327.78) len 8.52  near Ball Valve Type 2.sym
EXTRA 1.50 mm  run (207.71,367.26) -> (207.26,366.36)
                line (209.39,370.62) -> (205.58,363.00) len 8.52  near Ball Valve Type 2.sym
```

`run` 就是 8.52 mm 那条合并线的中段，正好是圆心；`dump_symbol_primitives` 打出的本体里
圆内没有任何图元 —— 纸上确实没墨。

## 证据三：处数乘得上

每处桥接 ≈ 1.50 mm：

| 图 | 阀 / 处 | 差 |
|---|---:|---:|
| I井场 | Type 1 八个 + Type 2 八个，另加 `Wellhead Christmas Tree 02` 一处 | 25.7 mm |
| 一级闪蒸分离器A | 2 阀 × 2 斜线 = 4 处 | 6.0 mm |
| 一级闪蒸分离器B | 无 | 0 |
| 低温分离器A | 4 处，其余 0.9 mm | 6.9 mm |
| 换热器CD | `Dual Action Cyl Act.sym` 一处 | 1.3 mm |

## 落地

- `drawn_lines(segments, fit)`：把我方落笔段直接映到参考空间，不做合并；
  多画（`cover` 的 extra 与 `split_extra` 两条路径）改用它采样。
- 覆盖与漏画仍走 `join_centerlines` —— 虚线、弧链必须整条参与，合并本来就是为了它们。
- `split_extra` 的入参改名 `our_strokes`，文档写明「不得传合并线」。
- 单测 `a_join_across_a_blank_is_not_ink`：两段共线、间隔 1.8 mm 的相同出图，
  先断言它们确实并成一条 21.8 mm 的中心线（合并行为本身要留着），再断言多画 < 0.2 mm。
- `pid-plot/tests/plot_compare.rs` 的上限同步收口：45 / 25 / 5 / 40 / 28 →
  **1 / 15.5 / 0.5 / 18.6 / 9.2 mm**，即读数上方留一处桥接（1.5 mm）的余量，
  单个符号体再漏回一条就过不去。反证：把 `compare.rs` 回退成合并线采样，
  闸门在 `I井场 extra 25.5 mm > 1.0 mm` 处失败。

## 验证

同一批 PDF（2026-10-25 21:52 导出），改动前后各跑一次 `compare_five_drawings`：

| 图 | 改前 | 改后 | 改后归属 |
|---|---:|---:|---|
| I井场 | 26 mm | **0.30 mm** | 无（groups 为空） |
| 一级闪蒸分离器A | 21 mm | **15.00 mm** | `/Sheet6` 15.00 |
| 一级闪蒸分离器B | 0 mm | 0.00 mm | 无 |
| 低温分离器A | 25 mm | **18.08 mm** | `/Sheet6` 18.08 |
| 换热器CD | 10 mm | **8.72 mm** | `/Sheet6` 8.72 |

覆盖率一格未动（0.9999 / 1.0000 × 4），文字仍全中。剩下的多画只剩**参考没画家具、我们画了**
这一项 —— 与 D5 的记录一致；`I井场` 的 `Wellhead Christmas Tree 02` 也是同一假象，不是墨。

> **2026-10-26 补充。** 这一项也不是绘图差：`A` 与 `低温分离器A` 的 15 mm 全部落在左列遮罩右缘之外 ——
> 矩形停在 `x = 70`/`72`，而模板左上角那个 50 × 10 mm 的框墨到 `75.125`。把右缘挪到模板自己的那条轨之后，
> 五张读数是 **0.30 / 0.00 / 0.00 / 3.08 / 8.72 mm**，`A` 归零，`低温分离器A` 只剩复式虚线的一处 45° 斜线标记。
> 见 `docs/analysis/2026-10-12-template-mask-samples.md` 的「2026-10-26 补」与
> `pid-plot/examples/probe_template_furniture_rails.rs`。
>
> **同日晚些的补充二：最后那 11.80 mm 也是我们多画。** 三处字形都盖在短于半个周期的管段末端，
> 参考在那些管段上只画管身；`complex_dash_positions` 改为 `L < period / 2` 时不落字形后，
> 五张多画 **12.11 → 0.305 mm**（只剩 `I井场` 的阈值碎墨），覆盖率逐位未变 —— 这一段也是判定的
> 关键：拿掉 11.8 mm 的墨没有碰到任何参考墨。见
> `docs/analysis/2026-10-26-the-hydraulic-glyph-comes-off-sub-half-period-stubs.md`。

## 复现

```text
cargo test -p pid-plot --lib compare
cargo test -p pid-plot --test compare_five_drawings five_drawings_write_compare_reports -- --nocapture
cargo test -p pid-plot --test plot_text_sources five_drawings_keep_text_sources -- --nocapture
```

后两条会把逐图报告写到 `<corpus>/plot/compare/*.json`，`extra_outside_by_symbol` 与
`uncovered_reference` 分列两个方向。
