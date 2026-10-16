# igBoundary2d 的三种成员布局与填充语义（2026-10-16）

把五张参考图（W/A/B/LT/HE）的填充对到「ref 零缺失、ours-only 全可解释」之后，把沿途
敲定的 `igBoundary2d`（type `0x0013`）事实记在这里。解码入口：
`src/parsers/sheet_records.rs` 的 `decode_igboundary_payload`（图纸流）、
`decode_igboundary_template_payload`（`.igr` 链接模板）、
`decode_igsym_boundary_payload`（符号体缓存，loose 变体）。

## 1. 三种成员布局

| 布局 | 出现处 | 成员计数位 | 成员步长 | 成员前缀 |
|---|---|---|---|---|
| 紧凑 | 图纸 sheet、多数符号体 | `u32@+22` | 33B（tag + 4×f64） | 无 |
| 分隔 | 插头阀等符号体 | `u32@+18`（>1 时启用） | 48B = 35B 块 + 13B 分隔 | 每成员前 `04 01` |
| `.igr` 变体 | 链接模板 CPE-SQ-A0/A1 | 走扫到尾 | 变长 | `0x67` 线 33B / `0x68` 点列 7+16n / `0x74` 弧 58B |

- 分隔布局的 13B 分隔 = `f64` + `01 01 00 00 00`；逐成员重查 `04 01` 前缀，
  `+18` 数错了自己会暴露（`decode_igsym_boundary_payload:5820-5843`）。
- `+18 == 1` 时回落紧凑布局——DWG-0201GP06-01 有一条 `w18=1 w22=3`、`02 01`
  前缀的记录，仍按旧逻辑读，保住原描边计数。
- `.igr` 变体 `sub_type_word == 0x0000`：弧成员语法是
  `tag + 7×f64 [start_ang, end_ang, cx, cy, r, 0, 1] + flag`，按 ~0.25 rad 切弦
  落折线；尾固定 21B 且 `u32@+16 == 1` 校验。
- 图纸流最小记录放宽到 88B（`decode_igboundary_at:5373`）让短变体进得了扫描。

## 2. 源文件真存重复边界

`.pid` 的 `/Sheet6` 会存顶点与 fill 完全相同的第二条边界记录（A 图截止阀棋盘格：
oid 3033↔3066、3044↔3056），SmartPlant 出图只画一份。去重键 = 成员几何 + fill
身份，三处生效：

- `symbol_library.rs:941` `boundary_member_key`（体路径，`1013` 消费）；
- `geometry.rs:1712` 嵌入式缓存体同键去重；
- OCS 侧 `io/pid/styles.rs` `fill_dedupe_key`，`build.rs`/`template.rs` 各持一份
  已发射集合。

去重后 W/B 的 ours-only 填充计数归零——计数本身成了回归信号。

## 3. 同心圆成员 = 各自实心盘，不是复合环

插头阀 `Plug Valve Type 3.sym` 的边界跨两条记录引用三个同心圆成员
（r≈0.34/0.65/0.95 mm）。参考 PDF 三环全实心——SmartPlant 对每个圆成员独立填，
不是把外环当环带、内盘当洞。OCS 因此**每成员一个 HATCH 实体**（单 EXTERNAL
CircularArc path），不能放进同一 hatch 的多 path（渲染端按 even-odd 读会穿孔）。
回归测试：`io/pid/tests.rs` `concentric_boundary_circles_build_a_solid_hatch_each`。

链式线成员仍共用一个 ring path——两类成员语义不同，别合并。

## 4. 相关但已另档

- 图纸 `igArc2d` 直读 s→e 扫掠（`.igr` logo 弧实测），符号体弧 CW 约定不变：
  `2026-09-19-igarc2d-sweeps-clockwise-from-start-to-end.md` 已补更正段。
- fill 样式经 `(stream path, graphic oid)` join：`fill_styles_for_document`，
  `.igr` 根样式表 index=24 → rgb(0,0,255) 实色。
