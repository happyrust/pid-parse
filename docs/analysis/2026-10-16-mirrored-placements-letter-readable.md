# 镜像放置的符号文字：SmartPlant 把字形归一化到可读区间（2026-10-16）

符号实例的负 Y 缩放（镜像放置）会同时镜像体里的文字。SmartPlant 出图
并不把字印成反字：它把字形旋转归一化进 `[-π/2, π/2)` 的可读区间，并
按"被吸收掉的镜像"翻转文字锚点（左↔右、上↔下）。

## 实测依据

`probe_mirrored_text`（`examples/`，枚举 `scale[1] < 0` 且体含非 `NULL`
文字的实例）在五张参考图里共命中 15 个镜像实例，其中只有 3 个带真实
文字，全部验证了这条规则：

| 实例 | 位置 mm | placement rot | 体文字 | 参考表现 |
|---|---|---|---|---|
| `ESD Interlock.sym` | HE (689.7, 435.9) | π | `ESD`,`I` | 正读，居中于盒下 |
| `3-Way Solenoid Valve.sym` | HE (668.9, 435.9) | +π/2 | `S` | `∽` 居中 |
| `SSV Sample Valve Type 2.sym` | A (207.5, 349.3) | −π/2 | `D` | 右读竖排，碗口朝上 |

其余 12 个镜像实例（`Off-Unit SQ`、`DCS Func Access`、`LT-DP`、
`Disc Single-Func`、`Remarks`）体文字全是 `NULL` 占位，不参与验证。

## 归一化规则（OpenCADStudio 实现，`io/pid/symbols.rs`）

镜像放置时：

1. `raw = placement.rotation − lettering.rotation_rad`（镜像把自带角
   反号，与弧角的镜像方式一致）；
2. `emit = (raw + π/2).rem_euclid(π) − π/2`，把方向收进可读区间；
3. `delta = emit − placement.rotation`，由它推回被吸收的翻转：
   `cos δ < −0.5` → 水平锚点左右互换；`cos δ > 0.5` → 竖直锚点上下
   互换，然后重算 `alignment_point`。

三个实测点的推出结果：

- ESD（rot=π）：`emit=0`，`δ=−π` → 水平翻转，文字正读、锚点右移——
  与参考一致；
- SSV `D`（rot=−π/2）：`emit=−π/2`，`δ=0` → 竖直翻转——D 碗内弧与
  参考一致；
- 电磁阀 `S`（rot=+π/2）：`emit=−π/2`，`δ=−π` → 水平翻转——`∽` 居中
  且不倒置，与参考一致。

非镜像放置完全不受影响：旋转照原样合成，锚点不翻。

## 注意

这是**渲染层**规则，不在解析层。`SymbolLettering.rotation_rad` 与
`anchor_code` 按记录原值给出；归一化发生在 OpenCADStudio 的
`shape_primitive`（`io/pid/symbols.rs` 文字臂）。回归测试：
`mirrored_placement_lettering_stays_readable`（π 镜像正读 + 水平翻
锚、±π/2 镜像保右读 + 竖直翻锚、非镜像不受影响）。
