# 标注族（igDimension / igBalloon / igLeader）风险敞口

> 日期：2026-08-04
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（渲染）
> 结论类型：**风险评估 + 建议加告警，不建议现在写解码器**
> 证据来源：`radsrvitem.dll` IDA 反编译、
> `examples/probe_psm_type_code_histogram` 全语料实跑、`dlls/sppiddatamap.xml`。

## 1. 问题

`radsrvitem.dll!sub_56448F70` 的类表里有 9 个几何族在 6 个 fixture 里一次都没出现，
其中 `igDimension`(277) / `igBalloon`(279) / `igLeader`(280) 是真实 P&ID 高频使用的
标注族。语料里没有不等于格式里没有——需要判断这是不是一个「换批图纸就丢内容」的
静默风险。

## 2. 它们确实是图形族，不是约束

`radsrvitem.dll!sub_56449950` 是一个「这个 type code 是不是图形元素」的谓词，与类表
`sub_56448F70` 相邻且共用同一组常量。它返回 1 的集合是：

```
0x13 0x18 0x20 0x21 0x3D 0x4D 0x59 0x5D 0x5E 0x61 0x63
0x7B 0x7E 0x84 0xCE 0xFA 0xFF 277 279 280
```

它**排除**了全部 12 个 `*Relation2d` 约束族（`0x06 0x0F 0x15 0x17 0x19 0x40 0x69
0x6A 0x6B 0x77 0x82 0x85`）。

两点推论：

1. **277 / 279 / 280 在谓词内 → 是可绘制图形**，一旦出现在流里就该画。
2. 谓词还包含 `0xFA`（= GraphicGroup，已解码，语料 458 次）和 **`0xFF`（类表里没有
   名字、语料里 0 次）**——后者是同一类风险的第二个未知项。

## 3. igDimension 有完整的多级 reader

`sub_564BA320` 是 igDimension 的反序列化入口，链式调用五个子读取器：

```
sub_564BA320
  -> sub_564BB990   // 校验 *(WORD*)payload == 277，读 header + 符号学位域
  -> sub_564BB8B0
  -> sub_564BC3B0
  -> sub_564BBC40
  -> sub_564BB630
```

`sub_564BB990` 的位域读取值得单独记一笔。**注意它只是「IGDS 符号学长什么样」的样本，
不是本语料线宽/颜色缺口的入口**，理由见 §6：

```c
*(WORD*)(this+20)  = *(WORD*)(a2+18);
*(BYTE*)(this+25)  =  *(BYTE*)(a2+32)        & 0x0F;   // 4 位字段
*(BYTE*)(this+26)  = ((*(DWORD*)(a2+32) >> 4) & 0x03) + 1;  // 2 位字段 +1
// 其后 ~16 个来自 *(DWORD*)(a2+32) 的位标志展开到 this+32
*(BYTE*)(this+24)  = sub_564B82A0(*(WORD*)(a2+20));
```

payload `+32` 处那个 DWORD 是一个密集位域块，`&0x0F` 与 `(>>4)&3` 的形状与 IGDS
元素头的 style / weight 惯例吻合。但这些偏移是 igDimension 自己的，不是共享头部
（`sub_564B82A0` 只有这一个 caller，是 igDimension 专用的对齐码重映射表）。

## 4. 但 SmartPlant P&ID 不用这条路径

三条独立证据指向同一结论：

**（a）全语料 0 命中。** `probe_psm_type_code_histogram` 对每个 Sheet 流做链式校验
walk，统计**所有**通过校验的 type code，无过滤（每张图输出 `take(20)`，而实际族数
都远小于 20，没有截断）。6 个 fixture、1589 条 PSM 记录里，277 / 279 / 280 / 0xFF
各 **0 次**。

**（b）P&ID 的标注是用符号 + 文本框做的，而这条路已经全解码。** 语料里的标注对象
是 `.sym` 库里的符号放置：`Item Note & Label`、`Item Note & Label line1.5`、
`Off-Drawing`、`Drawing Description`、`Remarks`（工艺管道一张图就 35 次），配合
`igTextBox` 与 `JStyleOverride` 锚点。带引线的注释在 SmartPlant P&ID 里是一个
**符号**，不是 `igLeader`。

**（c）`sppiddatamap.xml` 里没有这三个图形类。** 全文搜索只有数据模型对象
`Balloon instr`（一种仪表符号）、`Design Dimension` / `Installation Dimension` /
`Dimension`（属性名），没有任何图形类注册。

**根因：`radsrvitem.dll` 是 Intergraph 通用的 RAD 2D 绘图服务器，被多个产品共用。
它的类表描述的是 RAD2D 的全部能力，不是 SmartPlant P&ID 的实际写出集合。** 那 12 个
参数化约束族就是最好的旁证——通用 2D 草图器的约束系统，P&ID 全语料只碰到 1 次
（A01 里 1 条 `igPointOnRelation2d`）。

## 5. 风险定级与建议动作

| 维度 | 判断 |
|---|---|
| 会不会静默丢内容 | **会**。当前未知 type code 走 `ProbeOnly` + `PidGraphicKind::Unknown`，OCS 的 `build_entities` 直接返回空，日志只汇总成一句「N 条 inferred/probe-only 被隐藏或丢弃」，不说是什么 |
| 出现概率 | **低**。P&ID 的标注机制是符号 + 文本框，已全解码 |
| 后果 | 尺寸标注/气泡/引线整类消失，且读图人不会察觉 |
| 现在能不能写解码器 | **不能**。手上没有任何含这些族的 fixture，写了无法验证，违反 parser promotion 需要 fixture ratchet + byte-audit 的门禁 |

**建议做的是告警，不是解码器。** 具体：

1. 在 `pid-parse` 侧把未知 type code 按**原生谓词 `sub_56449950` 的图形集合**分成
   两类：图形类未知（`277 279 280 0xFF` 以及任何将来出现的图形码）与非图形未知
   （`*Relation2d` 一族）。
2. 只对**图形类未知**推一条独立的、点名的 warning（「本图含 N 条 igDimension 记录，
   当前无解码器，这些标注不会出现在图上」），非图形类的保持安静。
3. `OpenCADStudio` 的 `report_import` 把这条 warning 单独打出来，而不是并进
   「inferred or probe-only」那句汇总。

这样一来，第一张真的带标注族的客户图纸会**自己把 fixture 送上门**，那时再按正常
取证流程写解码器，`sub_564BA320` 的 reader 链就是现成的入口。

## 6. 顺带否掉一条路：符号学不在几何图元里

追这条线索时顺手核了一遍图元的字节账，结论是**线宽/颜色/线型不可能藏在几何记录里**：

| 族 | payload | 已读字段 | 余量 |
|---|---:|---|---:|
| `igLine2d` 0x18 | 50 | `oid`4 + `parent_ref`4 + `remaining_header`4 + `sub_type_word`2 + `index`4 + 4×f64 32 | **0** |
| `igPoint2d` 0x5E | 34 | `oid`4 + `parent_ref`4 + `remaining_header`4 + `sub_type_word`2 + `index`4 + 2×f64 16 | **0** |

两个族都是**定长且字节全额入账**，没有任何空位放符号学。所以「再多解几个 igLine2d
字段就能拿到线宽」这条路是**死路**，不必再试。

真正的候选只剩两个：

1. **`0x00FA GraphicGroup`**——全语料 458 次，是**出现次数最多的族**，却只解了
   header、tail 保持 raw。变长（实测 44 / 52 / 54 / 66 / 68 / 98 / 104 / 110 / 116 /
   122 / 154 / 164 / 170 / 200 字节），payload 里带几何 OID 引用，`parent_ref` 恒为 6，
   `sub_type_word` 随图纸复杂度增长（工艺管道那张到 0x1C65），更像每对象 id 而不是
   判别码。A01 的 dump 里 tail 尾部有 `FF FF FF FF` / `FD FF FF FF` 这类哨兵值。
   **每对象显示属性最可能就在这里。**
2. **`/StyleCluster 0x005A`**——与字高同一个阻塞点。

`GraphicGroup` 是「对象 ↔ 图元」的关联记录，比单纯的符号学目标更大，值得单独开一个
取证 phase，而不是当作线宽缺口的附带产物。

## 7. 顺带记下的一个未知项

**`0xFF`**：在图形谓词 `sub_56449950` 内，但类表 `sub_56448F70` 里没有名字，语料里
0 次。与标注族同级的未知图形码，应一并纳入 §5 的告警。
