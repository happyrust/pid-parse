# 视图侧：`dispCtx+404` 谁建谁配，像素模式开没开

> 日期：2026-10-11
> 任务：t-519（U7b）。上游：U5 `docs/analysis/2026-10-11-units-context-source.md`，U2 `docs/analysis/2026-10-11-dash-scale-writer.md`。
> 只读。没改 `src/`、golden、测试、计划。没跑 cargo。库只开 `D:\ida_scratch\u7b\` 下的临时副本。`pid-parse/dlls/` 下的 `.i64` 没有作为 IDA 输入。

## 0. ③④⑤（交付摘要）

③ **像素模式 `unitsCtx+24`：关（0）。** U7a：`unitsCtx` 在 `CJRenderContext+896`，由 `CJTransformationServices::sub_10061EE0` 填；`+24` 从服务对象 `+40` 拷来。构造 `sub_1005FD30` 的 `0x100607F3` 把 `[eax+28h]` 写成 0。唯一写成 1 的是服务对象槽 32，`sub_10063110` 的 `0x1006312F` `mov dword ptr [ecx+28h], 1`（thunk `0x10002F6D`）。`render.dll` 里打到这块对象的 `call [reg+80h]` 只有一处：`IJRenderViewSetup` 槽 19 `sub_10020AA0` 的 `0x10020ACF`。该 thunk `0x10003765` 的引用只有虚表 `0x100925C8`。`imagdex` / `j2dsrv` 对 `{0693921A}` 的全部调用都不进槽 19；`radsrvitem` / `docext` 没有这个 IID。

④ **模式关，虚线走 `f0·len`，不走 `a/b`。** `a/b/maxPx` 仍会被 `sub_10061EE0` 写进缓冲，读者在 `+24==0` 时不读。`GetDeviceCaps` 没有 `LOGPIXELSX/Y`，`a/b` 不是 DPI。`ctx+128` 默认 1.0：槽 45（`0x1000105A` → `sub_10061490`，置 `+0x710`）同样只有虚表引用，管理器包装不调它，`sub_10061EE0` 走 else，把 `+128` 写成 1.0。`f0` 由 `sub_10061EE0` 按空间轴长填，不是立即数 1.67。

⑤ **1.67 不在 `a/b`。** 它也不是 `ctx+128`（那是 1）。剩下的位置是 `f0` 相对截图坐标的值，而 `f0` 不是写死的 1.67。固定像素（DPI、与缩放无关）不成立。随不随缩放：轴长公式在 U7a，本侧没有第二条比例；**不能**把 1.67 判成一个不随视图变的常数。和 U6 的张力见 §6：模式关且 `+128=1` 时，纸单位文字和虚线共用 `f0`，解释不了「文字≈1、虚线≈1.67」。

## 1. `dispCtx+404` 的构造：四库不 CoCreate

`dispCtx+404` 上的接口是 `JRenderManager2` 对 IID `{06939214-FC58-11CF-A193-080036BABA02}` 做 QI 返回的 `CRenderGeometry`（嵌在管理器 `+100`）。这张 QI 表在 `render.dll` `sub_1001D5F0` `0x1001D5F0`（`_DWORD* this`，返回值的字节偏移 = 下标 × 4）：

| IID | `render.dll` 常量 | 返回 | 身份 |
|---|---|---|---|
| `{06939214-…}` | `unk_1008FF88` `0x1008FF88` | `this+25` = `+100` | `CRenderGeometry`，即 `dispCtx+404` |
| `{0693921A-…}` | `unk_10090398` `0x10090398` | `this+5` = `+20` | `IJRenderViewSetupImp` |
| `{06939218-…}` | `unk_1008FB64` `0x1008FB64` | `this+7` = `+28` | `IJRenderSpaceSetupImp` |
| `{06939217-…}` | `unk_1008FC04` `0x1008FC04` | `this+8` = `+32` | 管理器 `+32` 的嵌入接口（构造里 `sub_1000199C(this+32)`） |
| `{06939219-…}` | `unk_100902F8` `0x100902F8` | `this+4` = `+16` | `IJRenderInitializeImp` |

类厂 `ClassFactory<JRenderManager2>` 在 `render.dll`。`pid-parse/dlls` 里含 `{06939214}` 的是 `imagdex.dex`、`j2dsrv.dll`、`radsrvitem.dll`、`style.dll`、`viewfil.dex`、`shlyhp.dll`。含视图设置 IID `{0693921A}` 的只有 `imagdex.dex` 和 `j2dsrv.dll`。`docext.dex` 两个都没有。`{06939219}`（Initialize）在这批客户端里一份都没有。

对位移 `0x194`（404）作**目标操作数**的 `mov` / `movsd` / `lea` / `movq` / `movss`：

| 库 | 函数数 | 命中 |
|---|---|---|
| `imagdex.dex` | 42257 | 0 |
| `j2dsrv.dll` | 984 | 0 |
| `radsrvitem.dll` | 5374 | 0 |
| `docext.dex` | 2354 | 2 个函数，都是 QI 前保存、QI 后写回原指针 |

`docext.dex`：

- `sub_1000D900` `0x1000D900`，数据 xref `0x1003F830`。`0x1000DBCC` 把 `[edi+194h]` 清 0，`0x1000DBD9` 写回 QI 之前保住的原指针。
- `sub_1000E530` `0x1000E530`，数据 xref `0x1003F82C`。同样是 `0x1000E7A8` / `0x1000E7B5`。

`j2dsrv.dll` 里 `{06939214}` 的字节在 `0x100234B0`，代码引用 0，指向该地址的指针 0。旁边的 `{0693921A}` 在 `unk_1002326C` `0x1002326C`，只有一处引用（§2）。`JCoCreateInstance`（`0x10011534`）的调用在 `sub_100046B0` / `sub_10009490` / `sub_1000C530` / `sub_10011B80`，实参是别的 CLSID（`unk_10023708`、`xmmword_10023744` 等），没有 `{06939214}`。

所以：**渲染器不是这四个库 CoCreate 出来再塞进 `+404` 的。** `dispCtx` 进 `RealRender2d` 时 `+404` 已经有值。谁创建管理器，断在这四个库外面（`render.dll` 自己的类厂；客户端不引用 Initialize IID）。

## 2. 视图 → 渲染器：实际打到的槽

调用形状都是：`(*(dispCtx+404))->QI(iid, &iface)`，然后 `iface->slot`。槽号 = 调用位移 / 4。`IJRenderViewSetup` 的表在 `render.dll` `0x1009257C`。

### 2.1 `imagdex.dex`：只读，不设像素模式

IID `{0693921A}` 在 `unk_1049CA08` `0x1049CA08`。五处引用：

| 函数 | 引用点 | 调用方 xref | 接着调的槽 |
|---|---|---|---|
| `sub_10311290` `0x10311290` | `0x103113BC` | `sub_10007153` `0x10007153` | 先 QI `{06939217}`（`unk_1049B11C` `0x1049B11C`）槽 3（`+0x0C`，实参 `&out, 2, 3`），再对视图设置接口调槽 20（`+0x50`，三个输出） |
| `sub_1031C460` `0x1031C460` | `0x1031C55A` | `sub_100036FC` `0x100036FC` | 同上：`{06939217}` 槽 3，视图设置槽 20 |
| `sub_1031A6A0` `0x1031A6A0` | `0x1031AECF` | `sub_1000A06A` `0x1000A06A` | QI `{06939217}` 后调导入 `GetTransformScale2d(iface, 0, 3, &out)`，再槽 10（`+0x28`）读三个 f64；然后视图设置槽 28（`+0x70`）读一个 f64，拿去做剔除：`v68 = 该值 / 槽10的第二个 f64` |
| `sub_103D4750` `0x103D4750` | `0x103D4A87` | `sub_1000D747` `0x1000D747` | 槽 7（`+0x1C`）：回调 `sub_1000BF55`，实参 `1, 3` |
| `sub_103D54E0` `0x103D54E0` | `0x103D571A` | `sub_10010145` `0x10010145` | 槽 6（`+0x18`）：同一回调，实参 `a1+1160, 1, 3` |

`dispCtx+412` 另 QI `{B8423DA0-030A-11CF-81DA-08003601C61B}`（`unk_1049C5E4`）。那是 VFS，不是渲染器配置（U2 §4.2）。

这五处都没有调下面三个写槽。写槽的实现在 `render.dll`，视图四库的调用图里没有它们：

| 槽 | 实现 | 做什么 |
|---|---|---|
| 25 `+0x64` | `sub_10020B60` `0x10020B60` | `*(a1+5476) = f64`。`a1` 是嵌在管理器 `+20` 的接口，所以落点是管理器 `+5496`。然后调 `*(a1-8)` 的槽 5，实参 `128`。该槽在主表 `0x100926F4` 上是 `nullsub_12`，**不会**把脏位打上 |
| 27 `+0x6C` | `sub_10020B30` `0x10020B30` | 同样，f64 落到管理器 `+5504`，通知同样是空函数 |
| 28 `+0x70` | `sub_100207C0` `0x100207C0` | 读管理器 `+5504`。这是 `sub_1031A6A0` 唯一打到的比例槽，是读 |
| 20 `+0x50` | `sub_10020790` `0x10020790` | 转到管理器 `+5528` 那块对象的槽 33（vtable `+132`），三个输出。不是 `unitsCtx` 的 `a/b` |
| 18 `+0x48` | `sub_10020750` `0x10020750` | 读一个 dword。接口 `+5324` = 管理器 `+5344`，构造时写成 `GetSysColor(5)` |

构造 `sub_1001CF10` `0x1001CF10` 把 `xmmword_10092780` `0x10092780` 抄进管理器 `+5496`。这 16 字节是 `0.5`（`0x3FE0000000000000`）和 `3.0`（`0x4008000000000000`）。

脏位路径在 `sub_1001DAD0` `0x1001DAD0`。`0x1001DDCD` `test byte ptr [esi+18h], 80h`：`esi` 是 `CJRenderContext`（`+24` 的 bit7）。成立时 `0x1001DDD3` 把管理器 `[edi+1578h]`（`+5496`，就是那个 `0.5`）放进栈，`mov ecx, esi`，`0x1001DDE5` `call sub_10001C3A`。thunk 到 `sub_1001EEC0` `0x1001EEC0`，一句：`this[157] = a2`，`this` 是 `_QWORD*`，即上下文 `+1256`。U5 §4 已说明 `CJRenderContext` 不是 `unitsCtx`：这里多一个字段，仍然对不齐 `+0/+8/+16/+24/+40/+48`。

### 2.2 `j2dsrv.dll`：视图设置只读背景色

`sub_10008B60` `0x10008B60`，代码 xref `sub_100013B6` `0x100013B6`。`0x10008C6F` `push offset unk_1002326C`（`{0693921A}`），QI 之后 `0x10008C8C` `call [ecx+48h]`，即槽 18。失败则 `GetSysColor(5)`（`0x10008CB4`）。和 §2.1 槽 18 的构造初值是同一条。没有比例、没有 DPI、没有像素模式。

`sub_10012E20` `0x10012E20`（多处数据 xref，例如 `0x100265D0`）只是 `RealRender2d(a3, 局部描述, a4, 0)`。`a3` 是调用方传来的 `dispCtx`，本函数不填 `+404`。

`sub_1000AE30` `0x1000AE30` 仍是 U2 的结论：Get `IJFilterStyleOverride` 后拷到视图对象 `this-1144..this-1064`，不 Set，也不碰渲染器。

`sub_1000F3A0` `0x1000F3A0` 把对象 `+1144` 起的 6 个 qword 拷给调用方。那是视图变换的读出口，不是 `unitsCtx`。

### 2.3 `radsrvitem.dll`：空间接口上传变换，仍然不写 `+24`

没有 `{0693921A}`。`{06939214}` 在 `dword_56664C84` `0x56664C84`，唯一代码引用 `sub_56512000` `0x56512000` 的 `0x565121C0`（调用方 `sub_5650DE80` `0x5650DE80`）。那是 QI 这个 IID 去画，和 `imagdex` `sub_101F90B0` 同一形状，不是 CoCreate。

`{06939218}` 在 `dword_56661310` `0x56661310`。引用：

| 函数 | 引用点 | 调用方 |
|---|---|---|
| `sub_5649CF40` `0x5649CF40` | `0x5649D590` | （空间接口再交给 `sub_56481550`） |
| `sub_5649E5B0` `0x5649E5B0` | `0x5649EFBF` | `sub_56472140` `0x564721BA` |
| `sub_5649F460` `0x5649F460` | `0x5649F557` | `sub_56471F00` `0x56471F7C` |
| `sub_5649FA10` `0x5649FA10` | `0x5649FBD8` | `sub_56472140` `0x564721D4` |

`sub_5649F460` 在 QI 之前会把 `dispCtx+436` 临时或上 `0x40`，结束再清掉。然后：

1. `sub_5649F690` `0x5649F690` 把空间接口再 QI 成 `{E976776E-CAC9-11D3-B074-08003693E102}`（`dword_56661130` `0x56661130`），槽 5（`+0x14`）把一块 OWORD 读进输出。
2. `sub_56481550` `0x56481550`（xref `0x5649F63E`、`0x5649D61C`）对同一 IID 再 QI。槽 5（`+0x14`）收一个 `_WORD*`；槽 3（`+0x0C`，`0x5649…` 调用点在该函数 `call [vtable+0Ch]`）收 `_WORD*`、一份 200 字节的对象（`qmemcpy` 长度 `0xC8`）和一块 48 字节缓冲。

48 字节是 6 个 qword 的尺寸，和 `j2dsrv` 视图对象 `+1144` 那组变换同宽。这是**变换传递**，参数里没有像素模式开关，也没有 `maxPx`。`{06939217}`（`dword_56661140`）在 `sub_5649E5B0` 里另有一次 QI，后续是画图参数，不是 `unitsCtx` 填写。

### 2.4 `docext.dex`

没有这两个 IID。`+404` 的两次写入只是 §1 的保存/恢复。

## 3. 像素模式与 1.67

U5 §1：像素模式非 0 时，单位码 11 的长度是 `fView * (a·len + b)`，不用 `f0`；线宽再夹 `[1, maxPx]`。模式为 0 时用 `f0·len`。

本轮能排除的：

- 视图四库不把 `a/b/maxPx/+24` 算出来。
- `render.dll` 不用 `LOGPIXELSX/Y`。`sub_100790E0` `0x100790E0` 用 HORZSIZE/HORZRES 做图元文件回放的窗口缩放，条件是对象类型 4 或 12，不是 P&ID 屏幕虚线。`sub_100762F0` `0x100762F0` 的索引 2 和 36 是设备和裁剪能力。`sub_10084DD0` `0x10084DD0` 的索引 38 是调色板位。

§0 是 U7a 对齐之后的判定，取代上面这三句「未定」。对齐过程在 §6。

`j2dsrv` 的 `Software\Intergraph\Applications\…\Translate\Ini`（`sub_1000E760` `0x1000E760`、`sub_1000E670` `0x1000E670`）和 `"Write Polyline Width Threshold"`（`0x100243A0`，仍只有数据引用 `0x10023DAC`、`0x10025C1C`）都没有接到渲染器配置。U5 §5 对这根字符串的结论不用改。

## 4. 复现

临时副本（本轮打开的就是这些，源库未开）：

```text
D:\ida_scratch\u7b\imagdex.dex.i64    ← units-ctx\imagdex\imagdex.dex.i64
D:\ida_scratch\u7b\j2dsrv.i64         ← units-ctx\j2dsrv\j2dsrv.i64
D:\ida_scratch\u7b\docext.i64         ← units-ctx\docext\docext.i64
D:\ida_scratch\u7b\radsrvitem.i64     ← dash-scale\radsrvitem.i64
D:\ida_scratch\u7b\render.dll.i64     ← units-ctx\render\render.dll.i64
```

`render.dll.i64` 只用来给槽号和 `GetDeviceCaps` 对实现。和 U7a 不是同一个文件。

```powershell
$env:DS_OUT = "D:\ida_scratch\u7b\imagdex-stores.txt"
$env:DS_DISPS = "194"
& "D:\IDA Professional 9.2\ida.exe" -A -LD:\ida_scratch\u7b\imagdex-stores.log -SD:\ida_scratch\u7b\stores.py D:\ida_scratch\u7b\imagdex.dex.i64
```

`stores.py`：`idc.batch(1)` + `ida_auto.auto_wait()`，扫每个函数里目标操作数为位移 `0x194` 的 `mov`/`movsd`/`lea`/`movq`/`movss`。IID 用 `iid_users.py`（`bin_search` + `XrefsTo` + Hex-Rays）。视图设置表用 `D:\ida_scratch\dash-scale\vt.py`，`DS_VTDUMP=IJRenderViewSetupImp@JRenderManager2`。

第三方核对顺序：

1. `imagdex` `0x103113BC` 的 `push offset unk_1049CA08`，以及 `0x1049CA08` 的 16 字节是 `{0693921A-…}`。
2. `j2dsrv` `0x10008C6F` / `0x10008C8C`（QI 后 `call [ecx+48h]`）。
3. `radsrvitem` `0x5649EFBF` 的 `push offset dword_56661310`，以及 `sub_56481550` 里 `call [reg+0Ch]` / `call [reg+14h]`。
4. `render` `0x10092780` 的 16 字节、`0x1001DDD3`–`0x1001DDE5`、`sub_1001EEC0` 的 `this[157] = a2`。
5. `render` 里 `GetDeviceCaps` 的每个 `push imm`（本轮落在 `D:\ida_scratch\u7b\render-getcaps.txt`），确认没有 `58h` / `5Ah`。

## 5. 验证

| 项 | 结果 |
|---|---|
| 四库 `+404` 目标写入 | 已跑。imagdex 42257 函数 / j2dsrv 984 / radsrvitem 5374 命中 0；docext 2 个函数是保存/恢复 |
| IID 与槽号 | 已跑，地址见 §1–§2 |
| `GetDeviceCaps` 索引 | 已跑，无 88/90 |
| `cargo` / 构建 | NOT RUN。任务禁止 |
| `pid-parse/dlls/*.i64` | 未作为 IDA 输入 |
| `src` / golden / 测试 / 计划 | 未改 |

## 6. U7a 对齐：槽 32 没人调，所以 `+24` 保持 0

U7a：`unitsCtx = CJRenderContext+896`，填写函数是 `CJTransformationServices::sub_10061EE0`（服务对象槽 35，`+0x8C`）。`CJRenderContext::sub_1001E750` `0x1001E750` 用 `*(this+8)` 的槽 35，把 `this+896` 当输出缓冲传进去。

`sub_10061EE0` 在 `qmemcpy` 之后显式写四个字段（`v49` / `v42` 是输出缓冲，`v4` 是服务对象，`double*` 下标 ×8，`DWORD*` 下标 ×4）：

```c
qmemcpy(v49, v4 + 6, 0x88);
v49[5] = ...;                              // unitsCtx+40  仿射 a
v42[6] = v4[3];                            // unitsCtx+48  仿射 b
*((_DWORD *)v42 + 14) = *((_DWORD *)v4 + 8);   // unitsCtx+56  maxPx
*((_DWORD *)v42 + 6) = *((_DWORD *)v4 + 10);   // unitsCtx+24  像素模式 = 服务对象 +40
```

对应指令（U7a 指出，和 `sub_10061EE0` 的存储对得上）：`0x10062625` `mov [esi+18h], eax` 把像素模式写进 `CJRenderContext+896` 的 `+24`；`0x1006260D` `movsd [esi+28h], xmm0` 把仿射 `a` 写进 `+40`（上下文 `+936`），源是服务对象 `+0x10`。管理器 `+5496` 的 `0.5` 进的是上下文 `+1256`，不是这里的 `+40`。

服务对象 `+40`（`[ecx+28h]`）的写入：

| 地址 | 指令 | 值 |
|---|---|---|
| `sub_1005FD30` `0x100607F3` | `mov dword ptr [eax+28h], 0` | 构造，关 |
| `sub_10063110` `0x1006312F` | `mov dword ptr [ecx+28h], 1` | 槽 32，开 |

槽 32 的 thunk `0x10002F6D` 只有两处数据引用：`0x10095554`、`0x10095724`（两份虚表的 `+0x80`）。`sub_10063110` 的代码引用只有这个 thunk。管理器 `+5528`（`0x1598`；视图设置接口上的位移是 `0x1584`）上，`call [reg+80h]` 只有 `sub_10020AA0` `0x10020ACF`。它是 `IJRenderViewSetup` 槽 19，thunk `0x10003765`，数据引用只有 `0x100925C8`（表 `0x1009257C` + `0x4C`）。

§2 的五处 `imagdex` 调用和 `j2dsrv` `sub_10008B60` 都不进槽 19。所以拷进 `unitsCtx+24` 的是 0。

`ctx+128` 同理。`sub_10061EE0`：服务对象 `+0x710`（`*((_DWORD *)v4 + 452)`）非 0 才用轴长，否则 `v42[16] = 1.0`。置这个标志的是槽 45 `sub_10061490`，thunk `0x1000105A`，引用只有虚表 `0x10095588`、`0x10095758`。和 `0x1584` / `0x1598` 同函数的 `call [reg+0B4h]` 为 0。标志保持构造时的 0（`0x10060725`），`+128` 保持 1.0。

因此在 `render.dll` 和这四个库里，纸单位虚线的长度因子是 `f0`，纸单位文字是 `f0 × 1`。U6 量到文字约 1、虚线约 1.67。这条路径给不出这个分裂：模式开才会让虚线改走 `a/b` 而文字仍用 `f0`，但模式开的那个槽没人调。`pid-parse/dlls` 里持有 `{0693921A}` 的只有 `imagdex.dex` 和 `j2dsrv.dll`，都已经点名。1.67 若仍在 `f0` 里，它是轴长算出来的数，不是立即数，也不是 DPI。
