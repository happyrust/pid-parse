# 2026-10-19 — 每个文字类别的平均位移，比残差表更早看见偏差

范围：`pid-plot/src/compare.rs`（`CompareReport::text_offsets`）、
`pid-plot/tests/plot_compare.rs`（按类别的新门限）、两个对比入口的打印。

## 为什么要有它

1 mm 的配对窗是这个对比器的优点，也是它的盲区：**整类标签一起偏 0.3 个字高、
仍然全部落在窗内**。上一轮修掉的那个顶锚点问题就是这样藏了两轮 —— I 图 58 条
顶部锚定的标签一直高 0.79 mm，每一条都能配上，`text_matched` 一个数没动。
残差表只能看见「掉出窗外的那些」，看不见「集体站偏了的那些」。

## 报什么

每配上一对，把「我方 run 起点 − 参考 run 起点」投到**参考 run 自己的书写坐标系**
（`along` 沿字向、`down` 朝降部），再除以参考字高 —— 于是旋转标签与正立标签同
量，2.5 mm 与 5 mm 的字也同量。按**字号规则**归类：

```text
Left/Middle     图纸记录，左锚 + 中线
Right/Ascender  图纸记录，右锚 + ascender 线
symbol/Middle   符号体自带标签（锚点由 .sym 记录给出，不带左/中/右）
unknown         拼接过、找不到对应来源的那几条
```

每类给出 `count`、`mean_along_heights`、`mean_down_heights`、`max_shift_mm`，
写进报告 JSON（`text_offsets`）并打印在 `plot_compare_all` 与 `pid_plot_compare`。

## 量到的

I 图（最大的一张）：

| 类别 | n | along | down | max |
|---|---:|---:|---:|---:|
| Center/Middle | 437 | **−0.053** | −0.002 | 0.69 |
| Left/Ascender | 57 | −0.042 | **+0.026** | 0.48 |
| Left/Middle | 20 | +0.000 | −0.020 | 0.17 |
| symbol/Middle | 12 | −0.035 | **+0.094** | 0.69 |
| Center/Bottom | 4 | −0.036 | −0.005 | 0.17 |
| Center/Top | 5 | −0.010 | +0.006 | 0.10 |

五张图的 `Center/Middle` 都在 −0.05…−0.06，`Left/Ascender` 的 down 都在
+0.03…+0.04，`Left/Middle` 在 −0.02…−0.03 —— 也就是说上一轮把顶部锚点从
1.0 改成 1.32 之后，`Left/Ascender` 这一类确实站到了 0.03 个字高（≈0.08 mm）
以内，而不是「刚好落在窗内」。

## 门限

`pid-plot/tests/plot_compare.rs` 对**样本 ≥ 10 条**的类别断言
`|along| ≤ 0.10`、`|down| ≤ 0.10` 个字高（0.10 个字高在 2.5 mm 字上就是
0.25 mm —— 本来就在配对窗内，这正是它存在的意义）。五张图当前最大 0.057。

门限本身验过会响：把常数临时改成 0.02 跑一次，

```text
I井场 注采阀组工艺及自控流程图 Center/Middle along -0.053 body heights over 437 labels
test result: FAILED
```

失败信息带类别、方向、数值和样本数，不是一句 `assertion failed`。改回 0.10 后
全绿。

## 它第一次抓到的东西：居中标签整体左偏 0.053 个字高

`Center/Middle` 是最大的类（I 图 437 条，五张图 800+ 条），mean along 稳定在
**−0.053…−0.057**，即我方文字起点比参考左 0.05 个字高（2.5 mm 字上约
0.14 mm）。原因是同一族的老问题：**我方按「推进宽度」居中，参考按「墨迹」居中**。
推进宽度含首尾两个 side bearing，所以居中时我方整条被再推左约半个
（推进 − 墨迹）≈ 0.11 个字高。右侧锚点那一半在上一轮已经用「右对齐到最后一个
有墨的字」抵掉了一部分，居中这一半还没动 —— 要修得先拿到字形的墨迹范围：
`pdfcraft_fonts::pdf::Metrics` 只暴露推进宽度与字体级的 `ascent` / `descent`，
没有逐字 bbox，得从嵌入的字形轮廓算，是下一轮的事。

> 后续（2026-10-20）：这一条读错了。把「半个推进差」与 `along` 并列量出来以后，
> −0.053 里几乎全部是**我方 run 比参考宽**（差 0.35 mm，半个就是 0.18 mm），而宽度
> 多出来的部分来自图纸字段的**前导空格**——旧代码只 `trim_end()`，居中时那半个空格
> 把整条往左顶了 0.41 mm。去掉两端填充后 I 图 `Center/Middle` 收到 −0.026。墨迹包围盒
> 不是这条的主因。见 `docs/analysis/2026-10-20-a-pad-is-not-part-of-the-label.md`。

同族的还有 `symbol/Middle` 的 +0.09（符号标签整体低 0.2 mm）和 `symbol/Top`
的 −0.18…−0.20（n≤5，样本太少不下结论）。

## 改动

- `pid-plot/src/compare.rs`：`TextSource` 增 `anchor` / `line`；`match_text` 在
  每次配对（逐条与拼接两种）时记一个位移；`CompareReport::text_offsets` 新字段，
  JSON 里 `#[serde(default)]`。
- 新单测：`a_paired_run_reports_its_offset_in_body_heights`（0.2 个字高的位移读数
  与换算）、`a_paired_run_is_classed_by_its_lettering_rule`（`Right/Ascender`）。
- `pid-plot/tests/plot_compare.rs`：类别均值门限 + 两个常数。
- 打印：`plot_compare_all` 每张图后列出类别，`pid_plot_compare` 同样。
