# 符号体的「顶」是升部线，镜像后是降部线（2026-11-05）

A2（`集注站 轻烃分离器I气出口接口位置改造`）上 `SSV Sample Valve Type 2.sym`
的 `D` 离参考 1.005 mm，刚过 `TEXT_MM` 1.0。11 月 4 日记下的读数是沿书写方向
−0.04 mm、横向 +0.88 mm，对不上 0 / 0.5 / 1 / 1.32 个字高中的任何一档。

## 十一张一起读

`probe_symbol_label_offsets` 走验收目录的十一张，只看顶类（符号低半字节 `1`）
和底类的标签。`down/ref_height` 是我方基线减参考基线，除以参考字高：

| 类 | 页 | 条数 | 改前 `down/ref_h` |
|---|---:|---:|---|
| `I-P` / `P-I Converter` 的 `I` `P`（Left / Top） | 8 | 26 | −0.18 … −0.21 |
| `ESD Interlock`（Left、Right / Top） | 6 | 13 | −0.18 … −0.21 |
| `FT-Electromagnetic Flowmeter` 的 `M`（Left / Top） | 3 | 3 | −0.19 … −0.22 |
| `SSV Sample Valve Type 2` 的 `D`（镜像放置，Left / Bottom） | 2 | 2 | **+0.20**（A2 +0.88 mm，闪蒸 A +0.91 mm） |

（`空冷器A` 上 `ESD Interlock` 的一对 `−4.66 / +4.28` 是配对探针把两条同字标签对调了，
两条的 `down_mm` 均值 `−0.85` 与其它页同档。）

所有顶类都**低**了同一个比例：0.2 个参考字高。我方字高 3.17 mm，参考 4.39 mm；
0.2 × 4.39 = 0.88 mm ≈ 0.32 × 3.17 mm —— 正好是图纸 `igTextBox` 顶那一档
`ASCENDER_HEIGHTS = 1.32` 比一个字高多出来的那一截。所以符号顶和图纸顶是**同一条
线**：Arial 的升部（0.905 em）对字体表存的大写字高（0.688 em）。10 月 19 日把
符号顶留在「一个字高」，是因为当时只有 I 图一个 `M` 能读（+0.83 mm，被记成「差在
字高」）。

`D` 是同一件事的反面。它的放置竖直镜像，`flip_vertical` 把 `Top` 翻成 `Bottom`，
标签落在插入点上；参考把它画在插入点**上方** 0.88 / 0.91 mm。镜像把字盒翻过来，
留在插入点上的边是字盒的下沿 —— 降部线：Arial 降部 0.212 em / 0.688 = 0.31 个
字高，3.17 × 0.31 = 0.98 mm。

## 改法

- `TextLine::from_symbol_anchor_code`：低半字节 `1` → `Ascender`（原 `Top`）。
- 新增 `TextLine::Descender`，`baseline_origin` 里是 `−0.31` 个字高（基线在插入点
  上方）；`flip_vertical` 让 `Ascender` ↔ `Descender` 互换（原先 `Ascender` 折成
  `Bottom`）。
- 图纸 `igTextBox` 的读法（`from_anchor_code`）不动。

## 读数

| | 改前 | 改后 |
|---|---|---|
| 顶类 `down/ref_h`（42 条，11 张） | −0.18 … −0.22 | **+0.01 … +0.05** |
| `D`（A2 / 闪蒸 A） | +0.88 / +0.91 mm | **−0.10 / −0.08 mm** |
| A2 `text_matched` | 213 / 225 | **214 / 225** |
| 其余十张的文字、覆盖率、多画、笔宽 | — | 不变 |

`min_text_matched` 213 → 214。A2 仍未配对的 11 条：气泡上 9 条（`.pid` 记录了拖动
后的位置）和两句管线文字（字符串是 `.pid` 自己的）。`TEXT_MM` 仍是 1.0。

## 复现

```text
PID_PARSE_SELF_CHECK_DIR / 语料 = D:\work\cad\pid-test-data\验收数据
DETAIL=1 cargo run -p pid-plot --example probe_symbol_label_offsets -- "<图名>.pid" "<图名>.pdf" "<语料>"
cargo test -p pid-plot --test reference_plots_acceptance
```
