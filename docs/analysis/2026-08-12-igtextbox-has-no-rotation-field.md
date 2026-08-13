# ~~igTextBox 记录里没有旋转角：登记为 Coverage Gap~~（已撤回）

> **⚠ 本文结论于 2026-08-13 撤回。旋转角就在记录里。**
>
> 正确结论见 `2026-08-13-igtextbox-rotation-is-a-direction-pair.md`：旋转存成
> **单位方向向量 `(cos, sin)`**，`cos` 在 `text_end+16`（就是本文判为
> 「scale/marker」的那个槽位），`sin` 在 `text_end+24`（本文判为「flag」的那个）。
> 164 条记录无一例外满足 `cos² + sin² = 1`，解出 0°/90°/180°。
>
> **错在哪**：本文分别量了每个槽位，各自都只取离散的 `{0, ±1}`，于是判定
> 「都不是角度」。这两句单独看都对，错在没把它们**配对**——两列计数互补，
> 那是一个向量的两个分量，不是两个无关的标志位。问题问错了：不该问
> 「这个字段是不是角度」，该问「这组字段是不是一个方向」。
>
> 下文保留原样，作为判错过程的记录。

> 日期：2026-08-12
> 阶段：W7 取证（不改解码器，不接线）
> 产物：`examples/probe_igtextbox_rotation_candidate`
> 相关：`OpenCADStudio` 的 `IgTextBoxEmitter` 目前 `rotation: 0.0` 硬编码

## 问题

`OpenCADStudio` 把每一条 `.pid` 标签都水平摆放：`geometry.rs` 的 `IgTextBoxEmitter`
发 `PidGraphicKind::Text { rotation: 0.0, .. }`，因为 `igTextBox` 解码器解出了插入点
和文本，却没有角度。沿竖管跑的标签因此全部躺平。接线之前必须先**找到**一个角度、
并**证明**它。本文是「找」这一半的结论。

## 取证方法

`examples/probe_igtextbox_rotation_candidate` 遍历五张 fixture 的全部
**被接受** `igTextBox` 记录（146 条），量它们自己字节里所有可能是角度的位置。
payload 布局（见 `decode_igtextbox_payload`）：

```text
  0..31    head（oid、aux 对、sub_type、index、12 未解字节、+30 长度）
  32..     UTF-16LE 文本，text_length 个码元
  text_end +0..7   f64  trailing_double_1  （insertion.x）
  text_end +8..15  f64  trailing_double_2  （insertion.y）
  text_end +16..23 f64  trailing_double_3  （结构体注释「often 1.0」）
  text_end +24..35 12 字节，未解
```

角度是连续量：一个真的旋转字段必须在带旋转的标签上取到 `[0, 2π)`（或角度制
`[0, 360)`）里的中间值，在水平标签上取 0。判据 `is_angle_like` 即「有限、非零、
`< 2π`、且不恰好是 `±1`」。

## 结果：记录内两个候选都不是角度

| 候选 | 取值分布（146 条） | angle-like |
|---|---|---:|
| `trailing_double_3`（text_end+16） | `1.0` × 115，`-1.0` × 1，其余 30 条为 `1e-13`~`1e-17` 量级的浮点噪声（本质 0） | **0** |
| f64 @ text_end+24（未解 12 字节的前 8） | `0` × 110，`1.0` × 30，其余噪声 | **0** |

没有一条记录、没有一个候选取到哪怕一个真正的角度值。`trailing_double_3` 是个
`{0, ±1}` 的 scale/marker（偶尔 `-1.0`，像是镜像标志），text_end+24 是个 `{0, 1}`
的 flag。**旋转角不在 `igTextBox` 记录里。**

## 结论：Coverage Gap，源头最可能在样式链

这与字高同构：字高也不在 `igTextBox` 记录里，而在样式链上
（`igTextBox.index → JStyleTextPara +38 → JStyleTextChar +42`，见
`style_link::text_heights_for_file`）。旋转角最可能的下一站是同一条链上某个
未解字段，或另一族样式记录。定位它需要独立取证，超出本轮范围。

因此登记为 **Coverage Gap**（`CONTEXT.md` 术语）：`igTextBox` 的旋转角是一个
「已识别但无已证结构解码器」的字段，`rotation: 0.0` 是记录内证据支持的诚实兜底，
不是猜测。

## 为什么不先接一个候选凑合

- **记录内无源可接**：上表已排除记录自身的每一个候选。接 `trailing_double_3`
  会把 scale 当角度，把 115 条水平标签转 57°（1 弧度）——比躺平错得更离谱。
- **纯 corpus 不接线**：即便将来在样式链上找到一个「看起来像角度」的偏移，也必须
  有 native reader（`style.dll` / `radsrvitem.dll` 的读取序）或 controlled fixture
  （在 SmartPlant 里旋转一条标签重新发布、做字节差分）交叉验证才接。`GLine2d` 与
  `JStyleOverride` 锚点两次撤回都是「corpus 统计看着漂亮、原生读序一查就翻车」。
- **接线会改 golden snapshot**：`rotation` 从 0 变为实测值是行为变化，无两证不动。

## 将来接线的验收路径

1. 在 `JStyleTextPara` / `JStyleTextChar`（或其邻近样式记录）定位一个在带旋转标签上
   非零、在水平标签上为零、值域落在 `[0, 2π)` 的字段；
2. native reader 或 controlled fixture 二证其一坐实读取序；
3. `style_link` 新增 `text_rotations_for_file`（键 `(stream, graphic_oid)`，与字高同构），
   或在 `igTextBox` 链解出角度；
4. `IgTextBoxEmitter` 的 `rotation: 0.0` 换成解出值，刷新 golden snapshot，新增非零旋转断言；
5. `OpenCADStudio` 侧零改动兑现（`text.rotation = rotation.to_degrees()` 已接），
   `examples/pid_probe` 实测 `rot ≠ 0` 收尾。

## 明确不做（本轮）

- 不动 `igTextBox` 解码器与 `IgTextBoxEmitter` 一行；
- 不开 `radsrvitem.dll` / `style.dll` 的 IDA 逆向（下一轮取证的入口，本轮时间盒外）；
- OCS 侧标签维持水平兜底。
