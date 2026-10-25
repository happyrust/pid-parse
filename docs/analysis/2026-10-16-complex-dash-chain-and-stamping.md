# Complex dash：样式链、哨兵段与盖章规则（2026-10-16）

W 图液压管线（`Hydraulic`/`Hydraulic Line`）在参考 PDF 上沿管打 `└`/`⊢` 形 tick。
整条链从 style 记录解到 group 笔画已通，对照 ref 135/135 tick 全中。
解码：`src/parsers/sheet_records.rs` `read_complex_dash_pattern`；
解析链：`src/style_link.rs` `DocumentStyleTable::complex_dash_named`；
盖章：OCS `src/io/pid/build.rs` `stamp_complex_dash`。

## 1. 样式链四跳

```
JStyleSimpleLine (28)  -> dash: JStyleSimpleDashType (27) "Hydraulic Line"
                       pattern [-7.62 mm, <sentinel>] + component 26
0x0034 component (26)  -> +74 -> JStylePointSymbol (25) "Hydraulic"
JStylePointSymbol (25) -> Group(0x201D) 两笔 └ glyph（米制坐标）
```

- `0x0034` 是新族（无 librarian 名），入 STYLE_FAMILY 表于
  `style_link.rs:170`，常量 `PSM_TYPE_CODE_JSTYLE_COMPLEX_DASH_COMPONENT`。
- 段尾 `-DBL_MAX`（`0xFFEFFFFFFFFFFFFF`）哨兵标记符号槽位：盘上**没有**记录
  存 `-1`，哨兵只活在内存语义里（`style_link.rs:597-604` 注释）。
- `period = 7.62 mm` 从 dash 段长来；glyph 本身是米，独立于 sheet 单位，
  盖章时 `projection.mm` 换算（×1000）。

## 2. 盖章规则（对照 ref 逐 tick 标定）

- `cells = round(len / 7.62)` 等分，cell **中点**盖章；
- `cells <= 1` 的短段/stub 在**段终点**盖一枚（ref 在 3.8 mm stub 末端的
  stamp 落在端点，不是钳到中点）；
- 位置用**有向**切向推进，glyph 帧用**无向归一化**角度——反向管线
  （149.8→117.1）ref 的 tick 方向与正向完全一致，glyph 不随行进翻转；
- 断链即拒 stamp：缺 component、缺 point symbol、glyph 为空都不画
  （单测覆盖）。

## 3. Review 标记不进 PDF

接点 `igPoint2d` 解到 `lsWarning` marker（斜线 + 箭头 glyph），是屏幕态
review 覆盖层——ref PDF 全图无一条。OCS 把它们放进 authored 层
`ConsistencyChecks`（源图层原名），层标 `is_plottable = false`：
屏幕显示、plot 跳过。该层共 945 Point + 238 marker 线，整层抑制与
SmartPlant 语义一致（`io/pid/build.rs` authored-layer 创建点）。

## 4. 验证状态

- pid-parse：3 个 complex-dash 单测（端到端 / 断链拒 stamp / 错帧拒绝），
  全量 1221 绿；
- OCS：`io::pid` 65 绿；
- 视觉：W 导出 ref 对照 135/135 stem 全中、ours-only 0；
  x>200 的远端 ref-only stems 实为阀门内部件误检，非液压 tick。
