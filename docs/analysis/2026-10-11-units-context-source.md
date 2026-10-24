# `unitsCtx`：三个因子是纸 / 设计 / 视图的换算，5/3 不是「单位码不同」

> 日期：2026-10-11
> 任务：t-325（U5）。计划 `OpenCADStudio/docs/plans/2026-10-11-dash-pitch-follows-a-view-scale.md` §1 ③、§6。
> 上游：U2 `docs/analysis/2026-10-11-dash-scale-writer.md` §2 / §6；U1 比例 1.67±0.03；U3 `docs/analysis/2026-10-11-dash-scale-field-candidates.md` §5（单位码）。
> 只读。没改 `src/`、golden、测试、计划文件；没跑 cargo；没 git。库只开 `D:\ida_scratch\units-ctx\` 下的临时副本。

## 0. 一句话

`unitsCtx` 不是带 RTTI 的类，是一块至少 60 字节、实际读到 `+128` 的 POD：`+0/+8/+16` 三个 f64 分别是**纸（11）/ 设计（12）/ 视图（13）**的因子。虚线 / 线宽 / 点标记走 `sub_1007B150`（像素模式开时**不用** `+0`，改走 `+40/+48` 再乘 `+16`）。文字字高走 `sub_1007B270`（**永远**用 `+0/+8/+16`，不看像素模式），纸单位再乘同一指针的 `+128`。三个因子和 `+128` 的**写入函数没追到**（断点在 §4）。「虚线单位码和线样式不同，所以因子比是 5/3」**否**。文字没被放大、虚线被放大，差在这条公式分叉，不是两套单位码。

## 1. 布局（两个库里同一段代码）

读者，两边伪代码一致：

| 库 | 函数 | 角色 |
|---|---|---|
| `style.dll` | `sub_1007B150` `0x1007B150`（thunk `sub_100030EE` `0x100030EE`） | 带像素模式的换算 |
| `style.dll` | `sub_1007B270` `0x1007B270`（thunk `sub_10001758`） | 只按单位码取因子，没有像素分支 |
| `imagdex.dex` | `sub_101F73F0` `0x101F73F0` | 与 `sub_1007B150` 同构 |
| `imagdex.dex` | `sub_101F74F0` `0x101F74F0` | 与 `sub_1007B270` 同构 |

```c
// sub_1007B150 / sub_101F73F0
if (u->pixelMode /* +24, dword, 非 0 */) {
  if (styleUnits == 11) {
    px = u->affineA /* +40 */ * len + u->affineB /* +48 */;
    if (isWidth) { px = max(1.0, px); if (u->maxPx /* +56, dword */) px = min((double)u->maxPx, px); }
    return u->fView /* +16 */ * px;
  }
  if (styleUnits == 12) return u->fDesign /* +8 */ * len;
  if (styleUnits == 13) return u->fView * len;
  return len;
}
if (styleUnits == 11) return u->fPaper /* +0 */ * len;
if (styleUnits == 12) return u->fDesign * len;
if (styleUnits == 13) return u->fView * len;
return len;   // 14（Drawing）以及其它码：不乘表内因子
```

| 偏移 | 类型 | 非像素模式 | 像素模式 |
|---|---|---|---|
| `+0` | f64 | 单位码 11 的因子 | **不读** |
| `+8` | f64 | 单位码 12 | 单位码 12，原样乘 |
| `+16` | f64 | 单位码 13 | 单位码 13；另外乘在单位码 11 的像素结果上 |
| `+24` | dword | 0 = 关 | 非 0 = 开 |
| `+40` | f64 | 不读 | 单位码 11：`px = a·len + b` 的 a |
| `+48` | f64 | 不读 | 同一仿射的 b |
| `+56` | dword | 不读 | 仅 `isWidth != 0`：夹到 `[1, maxPx]`；0 表示不设上限 |

`isWidth` 在线宽路径为 1，虚线段和点标记为 0（U2 §6，本轮未改这句）。

没有 `??_7` / RTTI 名字对得上这块内存。候选里的 `tagRenderDataInfo`（`control.dll` 线型预览）是另一份渲染描述，字段对不上这 7 个偏移。`CJRenderContext`（`render.dll`，`sub_1001E0E0` `0x1001E0E0`）是视图变换对象，见 §4，**不是**这块 POD。

## 2. 单位码的名字

`render.dll` 把样式单位码打印成文字。三处同一张表：`sub_10010040` `0x10010040`（文字框几何）、`sub_10057CB0` `0x10057CB0`（字符样式）、`sub_10058000` `0x10058000`（段落样式）。

| 值 | 字符串 | 与 `StyleUnitsConstants`（08-25 文档） |
|---|---|---|
| 0 | `Style units are undefined` | — |
| `0xB` = 11 | `(Paper)` | `igPaperStyleUnits` |
| `0xC` = 12 | `(Design)` | `igWorld` / `igDesign` |
| `0xD` = 13 | `(View)` | `igView` |
| `0xE` = 14 | `(Drawing)` | `igDrawing` |
| 其它 | `invalid value` | — |

所以三个因子是：

- `+0`：纸面单位 → 输出坐标
- `+8`：设计（世界）单位 → 输出坐标
- `+16`：视图单位 → 输出坐标

文件里的单位码不是直接写 11/12/13。U3：payload `+12` 的 u16 记作 `w`，`units = (w & 7) + 10`。

线几何在进 `RealRender2d` 之前就被标成纸单位。`imagdex.dex` `sub_101C4230` `0x101C4230`（以及同形的 `sub_101C3AC0` `0x101C3AC0`）调用：

```text
BuildRenderSolidLine(0, 11 /*0x0B*/, width_f64, color_dword)
```

`render.dll` `BuildRenderSolidLine_0` `0x10081270` 把这 11 写进图元 `+8`，宽度 f64 写进 `+16`：

```c
*p = 22; p[1] = 2; p[2] = unitCode; *(double *)(p + 16) = width; p[6] = color;
```

xref：`BuildRenderSolidLine` 导出 thunk `0x10003E40`，数据引用 `off_100A2218`。图元自己只携带「这段长度按单位码 11 解释」，乘因子是后面 `sub_1007B150` 的事。

## 3. 它怎么交到读者手上（构造点断在这里）

`style.dll` 入口：

```text
IJStyleRenderImp @ 100A687C 槽 11 = thunk 100024F0 → sub_1007ABA0
  → CStyleRdr2dBase 主表 100A68F4 槽 47 = sub_1007BE60
       第 4 个参数（this 之后第 3 个）= unitsCtx
       sub_10001758(单位码, unitsCtx) → sub_1007B270
```

`sub_1007BE60` 的代码 xref 只在 `style.dll` 内部（槽 47 的各派生表，U2 §2）。它不分配这块 POD。

`imagdex.dex` 上文字 / 标注路径也是**接着传**，不填：

| 函数 | 地址 | 说明 |
|---|---|---|
| `sub_101F4AC0` | `0x101F4AC0` | thunk `0x100057C7`。参数 3 原样传给 `sub_101F90B0` / 内层 `Render`（`[vtable+2Ch]`，6 个 push） |
| `sub_100909F0` | `0x100909F0` | 调用方参数 4 → 上面的参数 3 |
| `sub_100B0220` | `0x100B0220` | 同上，单位码实参写死 `11` |
| `sub_1034C780` | `0x1034C780` | `CDimStyle::DimIJStyleRenderImp` 槽 11（thunk `0x1000BB1D` @ `104A7C9C`） |
| `sub_10350690` | `0x10350690` | `CAnnotStyle::AnnotIJStyleRenderImp` 槽 11（thunk `0x1001021C` @ `104A7D04`） |

线几何走另一条，仍然没有填这三个因子：

```text
imagdex Display（55 处 call RealRender2d，例 0x101C42DF）
  → render.dll RealRender2d_0 0x10081720
       renderer = *(dispCtx + 404)
       renderer->slot3(geom, out, ov, 0)
  → 该接口是 JRenderManager2::QI 对 IID {06939214-FC58-11CF-A193-080036BABA02}
     返回的 CRenderGeometry（嵌入在管理器 +100）
     槽 3 = thunk 0x10003BB6 → sub_1001F410 0x1001F410
       再转调 *(geometry+8) 的槽 0
```

`CRenderGeometry` 的构造 `sub_1001F0F0` `0x1001F0F0`（管理器构造 `sub_1001CF10` 里 `lea ecx, [ebx+64h]; call sub_10001343`）把 `+4/+8/+12` 清零。`sub_1001F410` 依赖 `+8` 上的委托。本轮在 `render.dll` 里按「管理器 +0x6C（= 嵌入对象 +8）」扫存储，命中的是别的对象（GDI 折线缓冲一类），**没有**对上这块委托。三个 f64 的写入也就停在这一跳的前面。

`j2dsrv.dll` 的 `JBServer` 视图对象不是这块 POD。`sub_1000F3A0` `0x1000F3A0` 拷的是 `this+1144` 起的 6 个 qword（变换），布局对不上。

## 4. 三个因子的来源归类

| 因子 | 归类 | 证据 / 断点 |
|---|---|---|
| `+0` 纸 | **未定到写入指令** | 见下。消费侧已经能说：线宽和虚线在语料上都取它（§6） |
| `+8` 设计 | **未定到写入指令** | 读者只在单位码 12 时用。0201 的线 / 虚线不是 12（U3 §5）。0202 有 4 条**文字**样式是 12，不参与虚线段 |
| `+16` 视图 | **未定到写入指令** | 单位码 13，以及像素模式下再乘一次。语料线 / 虚线不是 13 |

断点（为什么停在这里）：

1. `style.dll`、`imagdex.dex` 全库扫「同一基址上 qword 存 `d, d+8, d+16`，且 `d+24` 不是又一个 qword、而是 dword，并带 `d+40` 或 `d+48`」。`style.dll` 的命中是虚线消费点 `sub_1003AAD0` 写输出，不是填 ctx。`imagdex.dex` 里偏移 `+0x3D8/+0x3E0/+0x3E8/+0x3F0` 的写入属于另一份大选项块：`sub_101A34D0` `0x101A34D0` 的属性号 44 / 43 / 76 / 78（`0x101A3EEF` / `0x101A3EDA` / `0x101A3F2E` / `0x101A3F66`），值从调用方原样拷入；`sub_1006A320` `0x1006A320` 用常量做缺省（`+984 = 0.1` 等）。那一块的 `+0x3F0` 是一个字节旗标，不是这里的 dword `pixelMode`。
2. `render.dll` 对绝对位移 `0x3D8/0x3E0/0x3E8/0x3F0` 没有 f64/dword 因子存储。`CJRenderContext::sub_1001E980` `0x1001E980` 把「矩阵 `this+416` 变换一个向量之后的长度」（`gsumath1_162` + `sqrt`）写进 `this+1032`（`0x408`）。这是视图比例的一个缓存，调用链是脏旗标 `+24` 的 bit `0x100` → `sub_1001DAD0` `0x1001DAD0` → 虚表槽 4。**不能**把它说成 `unitsCtx+48`：同一对象上没有配对的 `+0x3D8/+0x3E0/+0x3E8` 写入，对不齐 §1 的布局。
3. `j2dsrv.dll` `sub_1000D2F0` `0x1000D2F0`（数据 xref `0x10023BD4`、`0x10025B50`）在 INI `Options` / `Read Default Units` 非 0 时调用 `ConvertUnitToDbu(1, 1.0, this+105, …)`，`this` 是 `double*`，落点是对象字节偏移 840。这是翻译选项，不是显示用的 `unitsCtx`。
4. `docext.dex` 导入 `GetDefaultUnits` / `ConvertUnitToDbu`，调用点（`sub_100043B0`、`sub_10005D00`、`sub_100042F0`）拿的是单位类型 1 的图纸英寸尺寸（8.5、11），不是 11/12/13 这三档样式单位。

因此三个因子**不能**归到「文件字段 / 注册表键 / 某个已点名的常量」。它们由视图在调用 `Render` 之前填好；填的那条指令跨过 `CRenderGeometry+8` 的委托之后就断了。

## 5. 像素模式 `+24/+40/+48/+56`，以及 “Write Polyline Width Threshold”

公式见 §1，等级是 native-reader（两个库各一份）。**谁把这四个字段写进这份 POD，同样断在 §4**：没有找到那条存储。

`j2dsrv.dll` `0x100243A0` 的 `"Write Polyline Width Threshold"` **不是** `+56` 的赋值来源，至少在已打开的库里不是：

- 仅有的引用是数据：`0x10023DAC`、`0x10025C1C`。
- 这两处坐在 ImagView 翻译选项目录里。相邻键是 `Read Default Units`、`Write Default Units`、`Metafile to Raster DPI resolution`、`Sub Units Per Master Units`、`Pos Units Per Sub Units`，节名是 `Options`（`0x10024284`）。记录步长 0x10：节名、键名、缺省值。没有函数指针。
- 没有任何代码 xref 把读到的值写入 `unitsCtx+56`。

所以 `+56` 仍是「像素模式下线宽上限」这个**读者侧**的含义；和这根翻译选项字符串之间没有接上的调用链。

## 6. 文字和虚线差在哪一步

U6：文字字高比例 0.92–1.00，虚线 1.67、线宽 1.80±0.08、点标记 1.54–1.75。指挥官原先把 `sub_1000414C` 说成「从矩阵 a3 算缩放」。反汇编对不上。

`sub_1000414C` `0x1000414C` 是 thunk，落到 `sub_1007B250` `0x1007B250`：

```c
return sub_10001758(*(this + 15), a2);   // = sub_1007B270(this->单位码, ctx)
```

`sub_10031760` `0x10031760` 在 `0x10031845`：

```text
mov  ecx, esi                  ; this = 文字样式
push eax                       ; ctx
call sub_1000414C              ; ST0 = 按 this+0x3C 的单位码取 +0 / +8 / +16
cmp  dword ptr [esi+3Ch], 0Bh  ; +0x3C = 60，0xB = 纸
fstp [ebp+var_98]
jnz  not_paper
movsd xmm0, qword ptr [edx+80h] ; ctx+128
mulsd xmm0, [ebp+var_98]        ; 纸：因子 × ctx+128
```

`esi+0x3C` 与 U2 的 `this+60` 是同一个单位码。ctx 为空时这段函数在栈上铺一份恒等：`xmmword_100A1430` = 两个 1.0，`dbl_100A0C48` = 1.0，写到局部的 `+0` 一带和相当于 `+128` 的槽。

所以纸单位文字字高用的缩放是：

```text
textScale = f0 × (ctx+128) × ovScale
```

`ovScale` 仍是 U2 的 `tagStyleOverride+24`（没置位就是 1）。`sub_1007B270` **不读** `+24` 像素旗标。

虚线段（单位码 11）走 `sub_1007B150`，不乘 `ctx+128`：

| 像素模式 `+24` | 虚线 / 点标记的长度因子 | 文字（纸） |
|---|---|---|
| 关 | `f0`（`+0`） | `f0 × (ctx+128)` |
| 开 | `fView × (affineA × len + affineB)`，**不用 f0**；线宽再夹 `[1, maxPx]` | 仍是 `f0 × (ctx+128)` |

点标记 `sub_1004A7B0` `0x1004A7B0` 在 `0x1004AA..` 附近：`sub_100030EE(1.0, this+60, ctx, 0)`，再拿 `ctx+64` 送进 `gsumath1_384`。它和虚线一样进 `sub_1007B150`，不走文字那条 `+128`。

因此「和文字用的世界→设备变换差在哪」可以收成一句，不用再比 f1/f2：

- 像素模式**关**：虚线/文字 = `1 / (ctx+128)`。要得到 1.67，`ctx+128` 得约 0.60，放大全在 `f0`。
- 像素模式**开**：虚线根本不读 `f0`，1.67 在 `+40/+48` 与 `+16` 的乘积里；文字仍用 `f0×(ctx+128)`，所以可以停在 1 附近。

两条都还差写入方。`imagdex.dex` 和 `render.dll` 里没有函数对同一基址既 qword-存 `+0` 又 qword-存 `+0x80`（`pair80.py`，命中 0）。恒等模板只在 `style.dll` 自己的栈上（`sub_100305E0` `0x100305E0`、`sub_10031760` 的 ctx 为空分支）。视图把活的 ctx 传进来之前在哪写 `+0` 和 `+128`，断点仍是 §4 的 `CRenderGeometry+8`。

## 7. 「单位码不同 → 因子比 5/3」：否

要解释 U1 的 1.67，这条假设需要：虚线图案的单位码和线宽的单位码不同，且对应因子之比是 5/3。

两边都会进 §1 的换算。U3 §5 已经把五张图的 payload `+12` 倒完：

- `0x002F` 虚线 14/14，`(w & 7) + 10 = 11`
- `0x002E` 线样式全部是 11（0201 的 29 条也是）
- 0201 上真正虚的两族（线样式 9→虚线 8，线样式 79→虚线 78）单位码都是 11

单位码相同 ⇒ `sub_1007B150` 为虚线段和线宽选择**同一个**因子（非像素模式是 `+0`；像素模式是「仿射之后再乘 `+16`」，仍然是同一份 ctx、同一个单位码分支）。比值是 1，不是 5/3。没有任何文件字段能把这个比值设成 5/3，因为比值不由字段决定，由两个单位码是否相同决定，而它们相同。

1.67 若仍是 5/3，它只能是**纸因子 `+0` 自己**（或像素模式下 `affineA` 与 `+16` 的乘积）相对截图坐标的值，不是两个单位码的因子比。这个数不在 `.pid` 里（U3 §8）。本轮没有把 `+0` 的计算公式追出来，所以**不能**把 1.67 证成或否成「纸因子 = 5/3」；只能否掉「两个单位码之比」。

## 8. 复现

临时副本（打开过，原库未开）：

```text
D:\ida_scratch\units-ctx\style\style_pp.i64          ← dash-scale\style_pp.i64
D:\ida_scratch\units-ctx\render\render.dll.i64       ← dash-scale\render_fresh
D:\ida_scratch\units-ctx\imagdex\imagdex.dex.i64     ← dash-scale\imagdex_fresh
D:\ida_scratch\units-ctx\j2dsrv\j2dsrv.i64
D:\ida_scratch\units-ctx\docext\docext.i64
```

`pid-parse/dlls/` 下的 `.i64` 没有作为 IDA 输入打开。批处理不走 ida-bridge：

```powershell
$env:DS_OUT = "...\out.txt"
$env:DS_TARGETS = "1007B150,1007B270,1007BE60"
& "D:\IDA Professional 9.2\ida.exe" -A -Llog.txt -SD:\ida_scratch\dash-scale\decomp.py D:\ida_scratch\units-ctx\style\style_pp.i64
```

第三方核对顺序：

1. `style` `0x1007B150`、`0x1007B270`、`0x1007BE60`（布局与参数位置）。
2. `render` `0x10010040` 里 `Units: 0x%x` 的 switch（11/12/13/14 的名字）；`0x10081270`（图元上的单位码）。
3. `imagdex` `0x101F73F0`、`0x101F74F0`（同一布局的第二份）；`0x101C42A4` 的 `push 0Bh`。
4. `j2dsrv` 字符串 `0x100243A0` 的数据 xref `0x10023DAC`（选项目录，不是 `+56`）。

## 9. 验证

- IDA 9.2 批处理 `-A`，脚本 `D:\ida_scratch\units-ctx\hunt2.py`、`dash-scale\decomp.py`、`qi.py` 以及同目录的 `follow.py` / `window.py` / `dispstore.py`。各次进程退出码 0。`imagdex` 上 `parse_binpat_str` 有一条 SWIG 泄漏警告，输出文件仍写出。
- 没跑 cargo，没构建，没 git，没改计划 / golden / `src`。
- 三个因子和 `ctx+128` 的写入点：**NOT FOUND**（§4 / §6），不是「验证未跑」。
- 文字公式用 `0x10031845` 的指令核对过：`mov ecx, esi; push ctx; call sub_1000414C`，随后 `cmp [esi+3Ch], 0Bh` 和 `mulsd xmm0, [edx+80h]`。不是只看 Hex-Rays。
- 5/3 的判定用的是本轮读者加上 U3 的单位码表、U6 的文字比例，没有重扫五张 `.pid`。
