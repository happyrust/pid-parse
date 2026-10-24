# `0x0059 igCircle2d` Sheet 级提升：Phase 35-B 语义门已满足

> 日期：2026-10-16
> 前序：`2026-07-10-phase35a-0059-igcircle2d-layout-evidence.md`
> （当时结论 `layout-admitted; semantic proof required`，候选 A/B/C 不命名）。

## 结论

**A = center.x，B = center.y，C = radius（米），terminal byte 为 flag**。
语义证据来自真实图纸 W（`I井场 注采阀组工艺及自控流程图.pid`）：
`/Sheet6` 顶层携带 **17 条 `0x0059`**，解码后坐标全部落在页空间，
与参考 PDF 的仪表 balloon / 传感元件中心点一一对应：

| 记录数 | 半径 | 渲染对象 |
|---:|---:|---|
| 11 | 6.350 mm | 标准 ISA 仪表 balloon（12.7 mm 直径圆），含 PV 1001103 @ (451.4, 170.2) oid=453 |
| 6 | 1.397 mm | 菱形坡度标记内的传感元件中心点 |

17/17 圆心/半径与参考 PDF 矢量路径对齐 ≤ 0.2 mm —— 这构成 Phase
35-B 门要求的"controlled fixture diff"等价物：字节列到页面几何的
映射在真实图纸上被逐点验证。

## 改动

- `SHEET_RECORD_FAMILIES` 新增 `0x0059` 行（`decode_igcircles` →
  `SheetGeometry::decoded_igcircles`，DTO 复用
  `DecodedIgCircle2dRecord`——JSite 嵌套体与 Sheet 的记录布局相同）。
- `EMITTERS` 新增 `IgCircle2dEmitter`：每条记录发
  `PidGraphicKind::Circle`（`SheetRecordKind::PrimitiveCircle`，
  `confidence: Decoded`），携带 oid、`source_layer`、byte_range
  与样式 `index`（W 的 balloon 走 `index=22` → 0.13 mm 实线）。
- `0x0059` 移入 `DECODED_TYPE_CODES`——不再计入 refused/dropped
  census。

## 边界（保持）

- **嵌套 JSite 圆仍然是符号体局部几何**：坐标在体局部系，未经变换
  证明前不向页面发射（AGENTS.md 的 held-back 约定不变）。
- `.sym` 库 sheet 的圆走符号体管线，不进 `SheetGeometry`。

## 下游验证（OpenCADStudio export-W-v4.pdf）

- PV/MOV/全部 balloon 轮廓落地；矢量级 14/14（A/B/LT/HE）+ 17/17
  （W）圆对齐。
- "FIQ balloon 椭圆失真"旧疑点消除：参考图本来就是 12.7 mm 正圆，
  之前的椭圆观感来自缺失轮廓 + 邻近文字。
- 小圆（r=1.4）渲染为坡度菱形内的开口圆，与参考一致。

## 附带澄清

同轮排查证实 "Interlocks 三角内的波形差异" 不是几何缺陷：缓存体
是 3 条线（三角）+ 文本 "S"，两侧均正确渲染旋转 -90° 的 S 字形，
差异仅为字体度量级。
