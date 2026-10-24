# 虚线消费点乘的那个 `+24`：它是 `tagStyleOverride` 的「比例」位，P&ID 渲染链上没人把它写成 1 以外的值

> 日期：2026-10-11
> 范围：只读逆向（计划 `OpenCADStudio/docs/plans/2026-10-11-dash-pitch-follows-a-view-scale.md` U2；协同任务 t-192）
> 结论类型：**native-reader**——全部来自 IDA 9.2 批处理反编译，库只开临时副本（§8），原 `.i64` 哈希前后一致。
> 前置：计划 §1 ③（10-10 S2 的消费点）、`2026-10-10-an-override-repaints-width-colour-and-switches-the-dash-off.md`（`JStyleOverride` 的 `+30` 掩码与 `sub_1002A080` 合成例程）、`2026-08-07-jstyle-simple-dash-type-linetype.md`（`0x002F` 流尾部 f64）、`2026-09-14-viewfilterset-carries-the-layer-display-state.md`（`TopVFSet`）
> 库：`pid-parse/dlls/style.dll.i64`（主）、`j2dsrv.dll.i64`、`viewfil.dex.i64`、`docext.dex.i64`、`radsrvitem.dll.i64`；`D:\pid\RADInstallA~` 下 `render.dll`、`symbol.dex`、`imagdex.dex`、`control.dll`、`jeditdlg.dll` 临时新建库（原 `imagdex.dex.i64` 只分析了 1610 个函数，不够用）。

## 0. 一句话

**`+24` 来自：常量 1.0（P&ID 渲染链上没有别的写入者）。** 10-10 S2 看到的「画图上下文对象」就是 RAD 的 `tagStyleOverride`（88 字节，`+8` 是属性掩码，`+24` 是 bit2「比例」的 f64），它由 `render.dll` 的 `BuildFinalOverrideStructure` 把**视图过滤器的覆盖**（`viewfil` `JFilterStyleOverride`，只会带颜色 / 线宽 / 虚线三位）与**调用方传入的覆盖**合并后交给 `IJStyleRender::Render`，再沿样式链由每级的 vtable 槽 50「叠加自己」。能把 bit2 置位的只有四处：文件里 `JStyleOverride` 自己的 `+30` 掩码 bit2（语料 53 条全为 0）、`JStyleSimpleFill`（填充族，线不走）、三个「没设就填 1.0」的默认分支、以及 `render.dll` 导出的 `CreateOverrideStructure(…, 4, scale)`（本机只有 `control.dll` 的线型预览位图在用）。`j2dsrv` / `viewfil` / `docext` / `imagdex` / `symbol.dex` 全库搜不到任何 `[+8] |= 4` 或写 `+24` 的指令。所以 ` Line2` 那条发丝虚线在 SmartPlant 里乘的 `+24` 就是 1——**参考图上 1.7 倍的节距不是这个「view-scale」给的**。

顺带三件事都有了答案（§5–§7）：`JSymbol::Display` 的 `+456` 是 SmartFrame 纸张比例，走 `IJSymbol` 槽 13 改符号变换，和 `+24` 不是一个数；线宽路径另有一套「像素模式」规则（仿射 + 1 px 下限 + 上限），不经 `+24`；`JStyleSimpleDashType` 流尾部那个 f64（成员 `+112`）**在渲染路径上被用到**——它是「按线长调整虚线」算法里的端部比例，而这个调整算法本身是本轮找到的、唯一能只放大虚线不放大线宽的机制（§7.3，留给 U1/U3/U4 核）。

## 1. 对象身份：`tagStyleOverride`（88 字节）

10-10 S2 只说消费点读「画图上下文对象 `+8` bit2 与 `+24`」。本轮先把它认出来：

| 证据 | 地址 | 内容 |
|---|---|---|
| `HloadOverrideData_0` | `style.dll` `0x10062A30` | 按 `+8` 的位逐项拷贝：bit0→`+12`(u32) bit1→`+16`(f64) **bit2→`+24`(f64)** bit3→`+32`(f64) bit4→`+40`(u32) bit5→`+48`(f64) bit6→`+56`/`+58`(u16×2) bit7→`+60`(ptr) 0x100→`+64`(u32)，最后 `dst+8 |= src+8` |
| 导出名 | `?HGetOverrideStyle@@YAJPAUIUnknown@@PAUtagStyleOverride@@0PAK@Z` | 该结构的 C 名字就是 `tagStyleOverride` |
| `render.dll!CreateOverrideStructure_0` | `0x10081A60` | 变参构造器：`+0 = 4`（版本）、`+4 = units`、然后 `(flag, value)` 对，`case 4: +8 \|= 4; +24 = double` |
| `render.dll!MakeCurrentVersion_0` | `0x10081CF0` | 版本 1/2/3 → 4 的升格，字段偏移同上，确认 `+24` 自版本 1 起就有 |
| 消费点 | `sub_1003AAD0` `0x1003AB96`、`sub_1001A470` `0x1001A5D3`、`sub_1004A7B0` `0x1004A8F5`、`sub_1004B250` `0x1004B29A`、`sub_1004B460` `0x1004B530`、`sub_100274A0` `0x100274AC`、`sub_10031760` 第 501 行、`render.dll!sub_1003EC50` / `sub_10054450`（文字） | 全是 `test byte ptr [x+8], 4` 紧跟 `movsd xmm, [x+18h]` |

语义：`+8` 是「哪些属性已经定了」的掩码，`+24` 是**尺寸比例**——除虚线段长外，点标记尺寸（`sub_1004A7B0` / `sub_1004B250`）、文字字高（`style.dll` `sub_10031760`；`render.dll` `CTextBuilder` / `CTextDecompBuilder`）都乘它；线宽（`+16`）**不乘**（§6）。`render.dll` 里还有一层 104 字节的 `tagRenderStyleOverride`（RTTI `.?AUtagRenderStyleOverride@@`）= 16 字节头 + 这个 88 字节结构（`MergeOverrideStructures_0` 对两个入参都取 `+16` 再 `MakeCurrentVersion`）。

## 2. 它怎么到达消费点（调用链，`style.dll`）

```text
外部  IJStyleRender::Render(geom, out, unitsCtx, renderer, tagStyleOverride* ov, flag)      ← 接口 vtable 100A687C 槽 11 = sub_1007ABA0
  └─ CStyleRdr2dBase 主 vtable 槽 47 = sub_1007BE60                                          （所有 JStyleR2d 派生类共用）
       ├─ ov 非空：memcpy 88 字节到栈上 acc；为空：memset 0
       ├─ 槽 50 (+0xC8) this->Accumulate(acc, unitsCtx, unitFactor)                          ← 「把自己叠到 acc 上」
       └─ 槽 40 (+0xA0) this->Draw(geom, out, unitsCtx, renderer, acc, …)
            JStyleOverride 的槽 40 = sub_1002A370 → 槽 51 sub_1007BD10 → 基样式的 IJStyleRender 槽 11 (+44)，把 acc 当 ov 传下去
            JStyleSimpleLine 的槽 40 = sub_1002E940 → sub_1002E160：acc+60 有虚线对象时
                 → IJStyleDashTypeRender 槽 5 (+20)(dash, pts, out, unitsCtx, acc+40, acc, flag, colour, width)
                 → JStyleSimpleDashType 槽 36 sub_1001BEC0 → sub_1001A470（铺开）；点生成 sub_1003AAD0 读 *(a3+12) = acc
```

所以「谁写 `+8` bit2 / `+24`」只有两类地方：(A) 调用 `Render` 之前把 `ov` 准备好的人；(B) 链上某一级的槽 50。

## 3. 写入点清单（B：链上的槽 50，`style.dll`）

`call dword ptr [reg+0C8h]` 全库 12 处，样式族的实现逐个读过：

| 类（主 vtable 槽 50） | 函数 | 对 bit2 / `+24` 做什么 | 来源 |
|---|---|---|---|
| `JStyleOverride` (`100A1580`+0xC8) | `sub_1002A080` `0x1002A080` | `if ((acc+8 & 4)==0) acc+24 = this+112`；末尾 `acc+8 \|= this+96` | **文件**：`0x0030` 记录 `+30` 掩码 bit2、`+50` f64（10-10 文档 §2.1）。语料 53 条掩码 ∈ {0x02,0x03,0x82,0x80,0x40000100}，**bit2 全 0，`+50` 全 0** → 不生效 |
| `JStyleSimpleLine` (`100A18F8`) | `sub_1002E340` `0x1002E340` | **不碰** bit2 / `+24`；写 bit0 颜色 `this+96`、bit1 宽 `this+80`、bit4 `acc+40 = this+108 & 1`、bit7 虚线 `this+100`、`+68/+72/+76` ← `this+108` 位域 | — |
| `JStyleLineSolid` (`100A2734`) | `sub_1003EB70` `0x1003EB70` | 只写颜色 `this+88`、宽 `this+80`，`acc+8 \|= 3` | — |
| `JStyleSimpleFill` (`100A0D60`) | `sub_1001D700` `0x1001D700` | `if (acc+8 & 4) acc+24 *= this+96 else {acc+8 \|= 4; acc+24 = this+96}` | 填充样式自己的比例成员；**线几何不经此类** |
| `JStyleHatchFill` (`100A0F8C`) | `sub_10022360` `0x10022360` | 没设则 `acc+8 \|= 4; acc+24 = 1.0` | 常量 1 |
| `JStyleSymbolFill` (`100A2A38`) | `sub_10044600` `0x10044600` | 没设则 `acc+8 \|= 4; acc+24 = 1.0` | 常量 1 |
| `JStyleDashType` / `JStyleLinePointGen` / `CStyleRdr2dBase` | `sub_1007BD70` `0x1007BD70` | `return 0` | — |

其余 5 处 `+0xC8` 调用者（`sub_100545C0` `sub_10056FE0` `sub_10057320` `sub_1005B210` `sub_10067A30`）是 `JStyleLib` 一类对象的别的槽 50，与 `tagStyleOverride` 无关；`sub_10031760` / `sub_10031A10` / `sub_100284E0` 是文字与 prerender 路径，只读不写 bit2。

全库指令扫描（`or/mov [reg+8], imm` 含 4 且同函数内有 `[reg+18h]` 的 f64 存取）另外只命中 `sub_10016AE0`（`JStyleSimpleDashType::IJStyleSimpleDashTypeImp` 槽 8，写的是虚线对象自己的 `+8`/`+24`，不是 `tagStyleOverride`）和若干构造函数。

## 4. 写入点清单（A：`Render` 之前准备 `ov` 的人）

### 4.1 入口在 `render.dll`，不在 `style.dll`

`style.dll`、`j2dsrv.dll`、`imagdex.dex`、`symbol.dex`、`smrtfrm.dex`、`olesite.dll` 等都导入 `render.dll!RealRender2d`。`RealRender2d_0`（`0x10081720`）：

```c
v6 = BuildFinalOverrideStructure(dispCtx, callerOv, &v9);         // 0x100819A0
(*(dispCtx+404))->slot3(geom, out, v6 /* 最终 ov */, 0);           // 送进样式的 Render
```

`BuildFinalOverrideStructure_0`：若显示上下文 `dispCtx+424` 的 IID == `{AC93CBE0-E7C8-11CE-81D3-08003601C61B}` 且 `dispCtx+420` 非空，则 `(*(dispCtx+420))->slot3(&v13)` 取出一份 104 字节结构，再 `MergeOverrideStructures(v13, callerOv)`：**先取 `v13` 里已定的位，再补 `callerOv` 里它没定的位**。否则直接用 `callerOv`。

### 4.2 `dispCtx+420` 是 `IJFilterStyleOverride`——视图过滤器那份覆盖

- IID `{AC93CBE0-…}` 在 `viewfil.dex` 由 `JFilterStyleOverride::ImpIJFilterStyleOverride` 的 QI（`sub_100031D0`）实现；槽 3 `sub_10003180` 返回 `this+16`（那 104 字节结构的地址），槽 4 `sub_100032E0` 是 Set（拷入参的 `+16..+103`，强制 `+16 = 4` 版本，`this+80` 非空时 `+24 |= 0x80`）。
- `JTopVFS` 构造（`sub_10005B50` `0x10005B50`）`JCoCreateInstance(CLSID_JFilterStyleOverride)` 存到 `this+96`；`IManageVFS` 槽 7 `sub_10005390`（`0x10005390`，`radsrvitem` 的 `JSite` 显示 `sub_56463FD0` / `sub_56464450` / `sub_56464860` 经 `dispCtx+412` 调它取每层的显示态）把 `this+104`（图纸级覆盖）与该层过滤器 `+16` 的覆盖用 `sub_10014160` 合并后 Set 进去、作为 `*a7` 交出，调用方塞进 `dispCtx+420/+424`。
- **合并例程 `sub_10014160`（`0x10014160`）只认 bit0 颜色、bit1 线宽、bit7 虚线三位**；`JTopVFS` 的持久化 `sub_10006AA0`（`0x10006AA0`，IJPersist 版本 2，经 `jengine_1075` 逐字段 DoIO）也只序列化这三位（`+8` 掩码 → bit0 `+12` → bit1 `+16`+`+4` → bit7 的 CLSID + 对象）。**bit2 / `+24` 既不读文件也不合并**。`GetTopVFS_0`（`0x10003530`）按名字 `L"TopVFSet"` 从文档取对象——这就是 09-14 文档的 `ViewFilterSet` 记录；它在 `.pid` 里**没有**比例字段可供 U3 找。
- `j2dsrv.dll` 唯一引用该 IID 的 `sub_1000AE30`（`0x1000AE30`）只 Get 后拷到视图对象（`this-1144..-1064`），不 Set。`docext.dex` `sub_10011E20`（`0x10011E20`）同样只 Get。全部含该 IID 的二进制（`imagdex` `j2dsrv` `radsrvitem` `style` `viewfil` `control` `ddacad` `docext` `gtype2dtoxmlwriter` `iemstn` `jeditdlg` `render` `smrtfrm` `stylecnv` `symbol` `tgmstn` `xmltoradgtype2dreader`）里，SmartPlant 自己的 `llama` / `sppid*` / `pid*` 一个都不在。

### 4.3 `callerOv`：谁会带比例来

`render.dll!CreateOverrideStructure`（能写 bit2 的唯一公开 API）的导入方只有 `imagdex.dex`（实为 `CreateOverrideStructureWidthAndColor`，`0x1043FF8E`，颜色 + 宽，掩码 1 / 0x101 / |2，**无 bit2**）、`control.dll`（`CDashTypeBitmapGenerator::GenerateBitmaps` `0x10022560`，线型下拉框预览位图，只给颜色；预览的缩放走 `CalculateScale` 另一个参数）、`skcmdspt.dll`（未开库；SmartSketch 命令支持，不在显示路径）。

新建库全库扫 `[reg+8]` 写入含 bit2 + 同函数 `[reg+18h]` f64：`imagdex.dex`（42 257 函数）命中 0 处 `tagStyleOverride` 写入（`sub_101D9FA0` 的 `mov [edi+8], 4` 是另一个结构的枚举字段；三处 `test [eax+8],4` 读者 `sub_1003ABA0` / `sub_100520E0` / `sub_100901E0` 是别的数组元素）；`symbol.dex`（10 717 函数）、`j2dsrv.dll`、`viewfil.dex`、`control.dll`、`jeditdlg.dll` 都是 0。`imagdex` 调 `RealRender2d` 的 53 个几何 `Display` 函数因此只能把 `callerOv` 传成 NULL 或颜色 / 宽覆盖。

### 4.4 归类

| 来源类别 | 结论 |
|---|---|
| 文件字段 | 只有 `JStyleOverride` `0x0030` 的 `+30` bit2 / `+50` f64 能进来；0201 的 ` Line2` 覆盖 9（掩码 `0x82`）没有置位 → 1.0。`TopVFSet` 不持久化比例 |
| 视图缩放 | 没有：`+24` 的链上没有任何一处读 zoom / 变换；缩放只进 `unitsCtx`（§6），对虚线段和线宽**同乘** |
| 应用设置 | 没有：`style` / `j2dsrv` / `render` / `viewfil` 里涉及 `+24` 的函数无注册表 / 选项字符串引用（`Software\Intergraph\…` 的几处 xref 都是文件扩展 / 调试开关） |
| 常量 | **是**：没人设时各级默认 1.0（`JStyleHatchFill` / `JStyleSymbolFill` / `JStyleDashType` 槽 50，`sub_1003AAD0` 的 else 分支 `0x3FF0000000000000`） |

> 断点声明：`skcmdspt.dll`、`ddacad.dll`（35 MB）、`smrtfrm.dex` 没开库。它们要么不在 P&ID 的常规显示路径（命令 / DGN 转换），要么只 Get；若日后要封死「应用设置」这条，开 `skcmdspt.dll` 看 `CreateOverrideStructure` 的实参即可。

## 5. `+456` 与 `+24` 不是同一个值

`symbol.dex` `JSymbol::IJGraphicImp::Display`（`sub_100510E0`，新建库地址与 S2 一致）：

```c
v62 = *(double *)(dispCtx + 456);
if ( (symbolFlags & 0x10000) && !this[47] && |v62 - 1| > eps ) {
    log(L"PS Apply a SF scale", …, 523);
    QI(this, {20ED936E-B6B9-11CE-B656-080036D72102}, &v78);
    v78->slot13(v62);                      // +52
}
```

- `{20ED936E-…}` 是符号自己的接口（`JSetRotation2DOfSymbol`、`JApplyParentScaleToSymbol` `0x10013980`、`HSetSymbolSwitches` 等 27 处都 QI 它）；槽 13 是 `JApplyParentScaleToSymbol` 里从父 SmartFrame 取到比例（`unk_100DE3C0` 接口槽 29）后调用的「SetScale」——**改的是符号的几何变换**（日志里的 SF = SmartFrame，"Dissolving symbol in SF document"），不是样式结构。
- `dispCtx+456` 由显示上下文携带（同一结构里 `+404` 渲染器、`+412/+416` `IManageVFS`、`+420/+424` `IJFilterStyleOverride`、`+436` 标志）；`style.dll` 全库没有任何函数读偏移 `0x1C8`，`tagStyleOverride` 也没有从它拷贝的路径。
- 结论：**不同源**。`+456` 是 SmartFrame 纸张比例 → 符号变换；`+24` 是样式尺寸比例 → 虚线 / 点标记 / 字高。P&ID 图纸里 sheet 不嵌 SmartFrame，`+456` 应为 1，`Display` 里那段也不会跑。

## 6. 线宽路径：另一套规则，在 `unitsCtx` 里，不经 `+24`

线宽从 `acc+16`（或 `this+80`）出来后只过一道单位换算 `sub_1007B150`（thunk `sub_100030EE`，`0x1007B150`）：

```c
double convert(double len, int styleUnits, UnitsCtx* u, int isWidth)
{
  if ( u->pixelMode /* +24 */ ) {
    if ( styleUnits == 11 ) {
      px = u->scale /* +40 */ * len + u->offset /* +48 */;
      if ( isWidth ) { px = max(1.0, px); if ( u->maxPx /* +56 */ ) px = min((double)u->maxPx, px); }
      return u->f2 /* +16 */ * px;
    }
    return (styleUnits == 12 ? u->f1 /* +8 */ : styleUnits == 13 ? u->f2 : 1.0) * len;
  }
  return (styleUnits == 11 ? u->f0 : styleUnits == 12 ? u->f1 : styleUnits == 13 ? u->f2 : 1.0) * len;
}
```

- `isWidth` 在线宽处为 1（`sub_1003EF60` `0x1003EF60` 与 `sub_1002E160` `0x1002E160`），在虚线段（`sub_1003AAD0`）与点标记处为 0。所以**线宽独有**：像素模式下 `px = a·w + b`，再夹到 `[1 px, maxPx]`；虚线段只有仿射、没有夹。
- `unitsCtx` 的 `+24/+40/+48/+56` 由 `Render` 的调用方（视图）填，`style.dll` 只读。这是 U1 量到「0.13 mm 格线也有 1–2 px、主管线 3 px」该对照的规则：**下限 1 px + 仿射偏移**，而不是一个乘在 style 宽度上的比例。`j2dsrv` 自动化属性表里有 `"Write Polyline Width Threshold"`（`0x100243A0`，仅数据表引用），像是 `maxPx` 一类设置的名字，本轮没有追它的赋值。
- `sub_1004B460`（`0x1004B460`，`JStyleSymbolFill`）把 `acc+24` 缓存进 `this+184` 并打脏，但线宽缓存 `this+176` 用的是 `convert(1.0, units, ctx, 0)`，仍不乘 `+24`。
- 结论：**线宽路径没有别的「比例」入口；有的是像素模式的最小 1 px / 最大 `maxPx` 分支**。P-D2 若量到线宽放大，优先按这套规则解释。

## 7. `JStyleSimpleDashType` 流尾部的 f64（成员 `+112`）在渲染路径上**被用到**

### 7.1 装载

版本 2 装载器 `sub_1001BC60`（`0x1001BC60`，由 `sub_1001BB50` 分发；版本 3 `sub_1001BE40` 多读一个 u32 到 `+124`）：

```text
jengine_1075(s, 4, this+92)      未命名
jengine_1075(s, 4, this+96)      N
jengine_1075(s, 8N, this+100)    图案 f64[N]
jengine_1075(s, 4, this+104)     标志：低 4 位 = 模式；bit4 = 端部翻倍
jengine_1075(s, 8, this+112)     ← 08-07 文档里的「比例 / 总长」
jengine_1075(s, 4, bit4 副本)
装载后：(flags & 0x7F00000F)==0 → 模式填 8；flags ≥ 0x80000000 → flags = 1
```

### 7.2 消费：按线长调整虚线的两个拟合器

`sub_1001A470`（`0x1001A470`，铺开）里：

```c
mode = this->flags(+104) & 0xF;  if (mode == 8) mode = (a5 /* = acc+40 */ != 0) ? 2 : 0;
ovScale = (acc && acc+8 & 4 && acc+24 != 0) ? acc+24 : 1.0;                     // 本文的 +24
if (mode && this->negSum(+144) > 0) {
    if (mode == 1)  sub_10017140(this, …, L, unitsCtx);                             // 只拉伸「空」把线长凑满；读 this+112
    else {          sub_10017A50(&fit, flags&1, firstSeg, this+112, negSum, posSum, L);   // 0x10017A50
                    sub_10016D50(this, …, ovScale * fit /* 负段 */, ovScale /* 正段 */, unitsCtx); }
} else              sub_10016D50(this, …, ovScale, ovScale, unitsCtx);          // 模式 0：图案 × ovScale，不拟合
```

`sub_10017A50` 的 `a4` 就是 `this+112`，记为 `r`。对全负图案（0201 的 `-3.5, -1.75`：`posSum = 0`，`negSum = S = 5.25`，`first = -3.5`）：

```text
n   = floor( (|f|·(1 − 2r) + L + 0.25·S) / S )
fit = L / ( n·S − |f| + 2·|f|·r )                      // first < 0 的分支
```

每一段（划和空都是负数）都乘 `ovScale·fit`。`mode == 1` 的 `sub_10017140`（`0x10017140`）也读 `this+112`（`*((double*)this+14)`），只重算负段。

### 7.3 这意味着什么（留给 U1 / U3 / U4，本文不下判）

- **开关来自线样式，不来自虚线样式**：`a5 = acc+40`，由 `JStyleSimpleLine` 槽 50 写成 `this+108 & 1`（`0x002E` 记录里 `this+108` 那个 u32，08-04 文档 §2 的第 4 个字段），或被 `JStyleOverride` 的 bit4（`+74` u32 → `this+128`，语料全 0）压过。`.pid` 的 `0x002F` 记录若按 08-07 文档「装到图案即截止」，`+104`/`+112` 要么在 08-07 没解的 `+26..+47` 头里、要么走默认（模式 8、`r` 取对象初值），U3 可以直接把 0201 两族虚线样式的 `0x002E +46/+50` 与 `0x002F` 头部倒出来对。
- 这是本轮**唯一**找到的「只动虚线段、不动线宽、随线长变」的放大机制。它预测：同一图案在不同长度的线上节距不同，且划 / 空等比；对 25.4 mm 的 ` Line2` 拿 `r = 0 … 0.5` 代入上式只得 `fit ≈ 0.97 … 1.12`，**凑不出 1.7**——所以要么 `r` / 模式不是默认值、要么 `L` 不是 25.4（例如拟合在别的单位或整条折线上做）、要么 1.7 另有来源。U1 量 `Electric`（1.905 / 1.27）那一族、U3 倒出 `+104/+112` 之后才能判。
- 与 `+24` 的关系：两者相乘（`ovScale·fit`），互不替代。

## 8. 复现

库只开临时副本：`D:\ida_scratch\dash-scale\{style_pp.i64 ← pid-parse/dlls/style.dll.i64, j2dsrv.i64, viewfil.i64, docext.i64, radsrvitem.i64}`；`render.dll` / `symbol.dex` / `imagdex.dex` / `control.dll` / `jeditdlg.dll` 从 `D:\pid\RADInstallA~` 拷到同目录下子目录新建库。原 `pid-parse/dlls/style.dll.i64` SHA-256 `FA928FFB…65CE8`、`_ida-probe-plant10-2026-08-05/style.dll.i64` `D101635C…750C7`，前后一致；`dlls/` 下六个 `.i64` 的修改时间未变。

批处理（不走 ida-bridge）：

```powershell
& "D:\IDA Professional 9.2\ida.exe" -A -Llog.txt -SD:\ida_scratch\dash-scale\decomp.py D:\ida_scratch\dash-scale\style_pp.i64
# decomp.py：idc.batch(1); ida_auto.auto_wait(); ida_hexrays.init_hexrays_plugin(); 对 $env:DS_TARGETS 的每个地址 decompile 并列 XrefsTo，落 $env:DS_OUT；
# 同目录 tree.py（调用树 + vtable 槽定位）、vt.py（??_7 名字 → 槽号 / 整表 dump）、callslot.py（call [reg+disp] 全库扫）、scan.py（[+8]&4 / [+18h] 指令扫 + 名字 / 字符串）、qi.py（函数引用的 16 字节 GUID）。
```

第三方核对顺序：`style_pp.i64` 反编译 `0x10062A30`（结构身份）→ `0x1007BE60`（Render 入口）→ `0x1002A080` / `0x1002E340` / `0x1001D700` / `0x10022360` / `0x10044600`（槽 50）→ `0x1007B150`（单位换算）→ `0x1001A470` / `0x10017A50` / `0x1001BC60`（拟合与 `+112`）；`render.dll` 新建库 `0x10081720` / `0x100819A0` / `0x10081A60` / `0x10081F10`；`viewfil.i64` `0x10005B50` / `0x10005390` / `0x10014160` / `0x10006AA0` / `0x100032E0`；`symbol.dex` 新建库 `0x100510E0` / `0x10013980`。GUID 落盘搜索：`E0 CB 93 AC C8 E7 CE 11 81 D3 08 00 36 01 C6 1B`（IID_IJFilterStyleOverride）。

## 9. 等级与未做

| 结论 | 等级 |
|---|---|
| 消费点读的对象 = `tagStyleOverride`，bit2 / `+24` = 尺寸比例 | native-reader（§1，7 处消费点 + 2 个 `render.dll` 构造器） |
| P&ID 线几何的 `+24` = 1.0；文件里唯一入口是 `0x0030 +30` bit2 / `+50`，语料为 0 | native-reader + corpus（10-10 文档 §3） |
| 视图过滤器覆盖只带颜色 / 宽 / 虚线，不持久化比例 | native-reader（§4.2） |
| `+456` 是 SmartFrame 比例，改符号变换，与 `+24` 无关 | native-reader（§5） |
| 线宽：像素模式仿射 + 1 px 下限 + `maxPx` 上限；无别的比例 | native-reader（§6）；`unitsCtx` 的赋值方（`j2dsrv`）未追 |
| `+112` 在模式 1 / 2 的拟合里被用 | native-reader（§7） |
| 1.7 倍节距的真正来源 | **未定**：`+24` 排除；§7.3 的拟合是候选，默认参数算不出 1.7 |

未做：`skcmdspt.dll` / `ddacad.dll` / `smrtfrm.dex` 没开库；`j2dsrv` 里 `unitsCtx`（`+24/+40/+48/+56`）的赋值没追；没有跑 cargo、没动 `src/`、golden、计划文件。
