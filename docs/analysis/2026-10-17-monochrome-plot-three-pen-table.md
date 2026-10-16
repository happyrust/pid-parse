# 单色出图笔宽表：参考 PDF 只用三支笔

> 日期：2026-10-17
> 性质：导出保真度证据（非解析器发现）——决定对参考图笔宽的测量结论，
> 与 OpenCADStudio `f37fc2628` 的实现一一对应。

## 结论

五张参考 PDF 的矢量描边宽度**只出现三个值**：

| PDF 宽度 | 毫米 | 用途（实测） |
|---|---:|---|
| 0.48 pt | 0.169 mm | 细线：标注、符号细节、标题栏 0.25 mm 规则线 |
| 0.96 pt | 0.339 mm | 中线：一般管线、符号体 |
| 1.92 pt | 0.678 mm | 粗线：工艺主管线、设备轮廓 |

源 `.pid` 文件声明的宽度走 ISO 128 阶梯（0.10/0.13/0.18/0.25/0.35/
0.50/0.70 mm…），若按字面值出图会产生十余种 PDF 线宽——与参考不符。
参考出图器显然跑了一张**三支笔的笔宽表**（等价 AutoCAD 单色 CTB）。

## 档位边界（实测锚点）

- `≤ 0.30 mm → 0.169 mm`（细笔）：标题栏 0.25 mm 规则线在 ref 中打印为细笔，
  锁定下界。
- `≤ 0.55 mm → 0.339 mm`（中笔）：0.50 mm 的符号缓存描边落在 0.96 pt。
- `> 0.55 mm → 0.678 mm`（粗笔）：0.70 mm 工艺主线打印为 1.92 pt。

## 实现位置（OpenCADStudio）

`src/io/pid/styles.rs` 的 `plot_pen_mm`：`apply_symbology`（sheet 线活）
与 `paint_symbol_stroke`（符号缓存体描边）共用同一映射，写入 DXF
`LineWeight`（毫米百分之一）。`paper_style_scale` 在归并**之前**乘入——
缩放把声明值推过档位边界时才换笔。

- 文本实体在样式臂提前返回，不吃线宽。
- 虚线行为不变：放置样式显式 dash 覆盖体描边 dash，solid 放置保留体 dash。
- 页面颜色与体颜色规则不变。

## 残余：0.75 pt 默认档

导出仍有少量 0.75 pt（≈0.265 mm）三段式路径——全部是 ~0.3 mm 的亚毫米
小勾画（line_weight_px=1 的默认权重，未经符号样式臂）。300 dpi 裁剪验证：
这些小勾画压在更粗的主描边之下，视觉不可见，参考图同位置亦无可分离墨水。
定性为**亚可见噪声**，不值得再追。

## 验证

- W/A/HE 导出笔宽分布从 11+ 档收敛为 4 档：0.482 / 0.964 / 1.928 pt
  三主档 + 0.75 pt 默认勾画档，与参考的三档结构一致。
- 全页像素指标（pdfcompare_mono）与改动前逐位相同——该指标由像素墨水比
  驱动，±0.15 pt 的宽度归并在光栅化层面不可分辨；矢量笔宽分布才是本改动的
  权威判据。
- OCS `cargo test -p OpenCADStudio pid`：102 过 0 失败（含改写的
  `paper_style_scale_multiplies_declared_sizes_and_leaves_lettering` 与
  `a_cached_strokes_dash_is_its_own_and_its_colour_its_placements`
  期望值）。

## 对应提交

- OpenCADStudio `f37fc2628` —— `io/pid: snap declared stroke widths onto
  the three-plotter-pen table`
