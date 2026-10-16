# `unitsCtx` 的写入方在 `CJRenderContext+896`，由 `CJTransformationServices` 用矩阵轴长填

> 日期：2026-10-11
> 任务：t-518（U7a）。从 U5 的断点 `CRenderGeometry+8` 往下追 `unitsCtx` 八个字段的写入。
> 上游：U5 `docs/analysis/2026-10-11-units-context-source.md` §1 / §3 / §4 / §6；U2 `docs/analysis/2026-10-11-dash-scale-writer.md` §2 / §4.1。
> 只读。没改 `src/`、golden、测试、计划文件；没跑 cargo；没 git。库只开 `D:\ida_scratch\u7a\render\render.dll.i64`（从 `D:\ida_scratch\units-ctx\render\` 拷出的副本）。没开 `pid-parse/dlls/` 下的 i64，也没开 imagdex / j2dsrv。

## 0. 一句话

屏幕这条链上，`IJStyleRender::Render` 的第 3 个参数是 `CJRenderContext+896` 起的 0x88 字节块。它由 `CJTransformationServices::sub_10061EE0`（`0x10061EE0`）填：`+0/+8/+16` 三个 f64 分别是纸（空间 2）/ 设计（空间 3 或 4）/ 视图（空间 1）的矩阵轴长再乘该空间的单位栈顶。`+24` 像素模式在构造时写成 0，本链不把它改成 1。`+128` 在标志为 0 时写成常量 1.0。这条写入链**算不出**常数 1.67。

## 1. `CRenderGeometry+8` 是谁

`JRenderManager2::QueryInterface`（`sub_1001D5F0` `0x1001D5F0`，数据 `unk_1008FF88` = IID `{06939214-FC58-11CF-A193-080036BABA02}`）对上后返回 `this+25`（`_DWORD*`，字节偏移 100 = `0x64`）。那就是嵌在管理器 `+100` 的 `CRenderGeometry`。管理器构造 `sub_1001CF10` `0x1001CF10` 在 `+52/+68/+84/+100` 各放一个，虚表 `??_7CRenderGeometry@@6B@` = `0x100927F8`。

`+8` 上先后有两个类。

### 1.1 初始化写成 `CTransformProxy`

`sub_1001D8F0` `0x1001D8F0`（`JRenderManager2` 主表槽 4，thunk `0x10003AA3`）在 `0x1001D9F4`：

```text
lea eax, [edx+1494h]    ; 0x1001D9E2  CTransformProxy，嵌在管理器 +0x1494
lea ecx, [edx+0F60h]    ; 0x1001D9E8  第 4 份 CJRenderContext，管理器 +0xF60
push ecx                ; a4
push eax                ; a3
push edx                ; a2 = 管理器
lea ecx, [edx+64h]      ; this = CRenderGeometry（+0x64）
call sub_1000489F       ; 0x1001D9F4  → sub_1001F330
```

`sub_1001F330` `0x1001F330`（唯一代码 xref 是上面这个 thunk）：

```c
*(this + 1) = a2;   // 几何 +4  = 管理器
*(this + 2) = a3;   // 几何 +8  = CTransformProxy*
*(this + 3) = a4;   // 几何 +12 = CJRenderContext*
```

`CTransformProxy` 虚表 `??_7CTransformProxy@@6B@` = `0x1009289C`。构造 `sub_1001F4D0` `0x1001F4D0`（thunk `sub_100029B9`，管理器 `+5208/+5228/+5248/+5268` 四个 20 字节对象；`0x1494` = 5268 是第四个，配几何 `+100`）。槽 0 = `sub_1001F870` `0x1001F870`。

同一函数后半 `0x1001DA5A` 再调 `sub_1001F790` `0x1001F790`（thunk `sub_10003661`），把代理自己的字段设成：`+4` 管理器，`+8` 几何，`+12` = `&管理器+0x14AC`（`CModule2d*` 的槽），`+16` = 那份 `CJRenderContext`。

`CRenderGeometry` 槽 3（`sub_1001F410` `0x1001F410`）转调的就是 `*(几何+8)` 的槽 0。初始化之后这一跳的目标是 `CTransformProxy::sub_1001F870`。

### 1.2 第一次走槽 0 时改成 `CModule2d`

`sub_1001F870` 在 `0x1001F888` 取 `代理+8`（几何），`0x1001F88B` 压入 `*(代理+12)`（`CModule2d*`），`0x1001F88D` `call sub_1000188E` → `sub_1001F4A0` `0x1001F4A0`：

```c
*(this + 2) = a2;   // 几何 +8 = CModule2d*
```

`CModule2d` 虚表 `??_7CModule2d@@6B@` = `0x10092C20`。对象在 `sub_10024D30` `0x10024D30`（thunk `sub_10004016`）里 `operator new(0x348)` + 构造 `sub_100246F0` `0x100246F0`，由初始化 `sub_1001FE50` `0x1001FE50`（`IJRenderInitialize` 的 this = 管理器+16）写到 `a1+5276` = 管理器 `+5292` = `+0x14AC`。同类写点还有 `sub_1001F970` / `sub_10020100` / `sub_1001FB20`。

因此：

| 时刻 | `几何+8` 的类 | 虚表 | 谁写的 | xref |
|---|---|---|---|---|
| `sub_1001D8F0` | `CTransformProxy` | `0x1009289C` | `sub_1001F330`，调用点 `0x1001D9F4` | `lea` `0x1001D9E2` |
| 第一次 `sub_1001F870` | `CModule2d` | `0x10092C20` | `sub_1001F4A0`，调用点 `0x1001F88D` | 值来自管理器 `+0x14AC` |

`sub_1001F870` 在改写之前已经做完两件事：`sub_10002C6B` → `sub_1001DAD0` `0x1001DAD0`（按脏旗标刷新上下文），然后 `CModule2d` 槽 10 `sub_10026290` `0x10026290`（`call [eax+28h]`，`0x1001F89C`）把上下文指针收进模块 `+124`。之后 `jmp [vtable]` 进 `CModule2d` 槽 0 `sub_10025650` `0x10025650`。改写之后，后续 `RealRender2d` 的槽 3 直接进 `CModule2d`，不再走代理。

## 2. `unitsCtx` 指针从哪来

`CModule2d` 槽 14 `sub_10025780` `0x10025780` 把样式对象的槽 11（`call [reg+2Ch]`，`0x10025ECB` 与 `0x10025FA0`）叫成：

```c
style->slot11(&v74, v60, context + 896, unitCode, ..., module + 296);
```

`this` 之后第 3 个参数是 `*(module+124)+896`。`module+124` 就是 `sub_10026290` 存进去的 `CJRenderContext*`。所以 **`unitsCtx` = `CJRenderContext + 896`（`0x380`）**。不是 `CJRenderContext` 自己，也不是 U5 对过的 `+0x3D8` 那组绝对偏移。

这块内存由上下文槽 2 `sub_1001E750` `0x1001E750` 交给服务对象去填：

```c
(*(context+8))->slot35(context+8, context+896, *(context+16), *(context+12));
```

`context+8` 在 `sub_1001D8F0` 的 `0x1001D908` 被设成管理器 `+0x1598`（5528）上的 `CJTransformationServices`（虚表 `0x100954D4`，构造 `sub_1005DF00` `0x1005DF00`）。槽 35 = `sub_10061EE0` `0x10061EE0`。`context+12` 构造时写成 2，`context+16` 构造时写成 0（`sub_1001E0E0` `0x1001E0E0`）。

`sub_1001E750` 的调用方是 `sub_1001DAD0`：上下文脏字 `+24` 的 bit `0x20` 置位时调虚表槽 2。代理槽 0 每次进来都先 `sub_10002C6B` → `sub_1001DAD0`。

## 3. 八个字段的写入

`sub_10061EE0` 先在服务对象上算好，再 `rep movsd`（`0x100625FE`，`ecx = 0x22`，136 字节）从服务对象 `+0x30` 拷到 `a2`（= `context+896`），然后用 `esi` = `a2` 改写几个字段。轴长都是 `gsumath1_162` 返回的向量做 `sqrt(x²+y²+z²)`（`mulsd` / `addsd` / `libm_sse2_sqrt_precise`）。

空间号和 U5 的单位码对得上：同一函数后半用 `units = context+12` 再算一套不乘栈的轴长，放在块内 `+80/+88/+96`（上下文 `+976/+984/+992`）。`sub_10025780` 用这套做容差比较，把 `+976` 配单位码 11、`+984` 配 12、`+992` 配 13。`+0/+8/+16` 用的是同一组空间号，只是 `units` 参数换成 `context+16`，并且多乘了单位栈。

| 字段 | 类型 | 写入指令 | 值 |
|---|---|---|---|
| `+0` 纸 f0 | f64 | 服务对象 `+0x30` ← `0x10062133` `movsd [edi+30h], xmm0`；经 `0x100625FE` 落入 `unitsCtx+0` | 空间 **2** 的轴长 × 栈。栈指针 `服务+0x494`，下标 `服务+0x4A4`，乘法 `0x1006211F` `mulsd xmm0, [eax+ecx*8]`。矩阵由槽 22 `sub_10062930` `0x10062930` 按 `push 2`（`0x1006211D`）取出 |
| `+8` 设计 | f64 | `0x100620AA` `movsd [edi+38h], xmm0`（服务 `+0x38`）→ `unitsCtx+8` | 空间 **3，或 `context+12==4` 时为 4**。`v45 = (a4==4)+3`。栈指针 `服务+0x4AC`，下标 `服务+0x4BC`，乘法 `0x1006208F` |
| `+16` 视图 | f64 | `0x100621C0` `movsd [edi+40h], xmm0`（服务 `+0x40`）→ `unitsCtx+16` | 空间 **1**（`push 1` 在 `0x1006211D`）。乘数是 `sub_10001B09(服务+0x478)` 返回的那个 double，`mulsd` 在 `0x100621A4` |
| `+24` 像素模式 | dword | `0x10062625` `mov [esi+18h], eax`，`eax = [edi+28h]` | 见 §5。源是服务对象 `+0x28` |
| `+40` 仿射 a | f64 | `0x1006260D` `movsd [esi+28h], xmm0`，源 `[edi+10h]` | 服务 `+0x10`。构造路径 `sub_1005FD30` `0x100607FA` 写入；置像素模式的 `sub_10063110` 用入参 a2 覆盖 |
| `+48` 仿射 b | f64 | `0x10062617` `movsd [esi+30h], xmm0`，源 `[edi+18h]` | 服务 `+0x18`。构造 `0x100607FF`；`sub_10063110` 用入参 a3 覆盖 |
| `+56` maxPx | dword | `0x1006261F` `mov [esi+38h], eax`，`eax = [edi+20h]` | 服务 `+0x20`。构造 `0x10060804` 写 0；`sub_10063110` 把入参 a4 写到这里 |
| `+128` 文字再乘 | f64 | `服务+0x710==0` 时 `0x100626E6` `movsd [esi+80h], xmm0`，`xmm0 = qword_10090A38` | 常量 **1.0**。标志非 0 时改走 `0x10062664`：`sqrt(m[0]²+m[1]²)`，矩阵是 `sub_10061490` 拷进服务对象 `+0x718` 的 0x80 字节。见 §4 |

槽 22 `sub_10062930` 本身不计算，只按 `(space, units)` 跳进服务对象里的一张函数指针表（`this + 5*space + units + 304`）。矩阵是更早推进这张表的人留下的。本函数没有 `GetDeviceCaps`，也没有 25.4 / 96 / 72 / 5/3 常量。

`0x100927E0` 是参考向量的退路 `(1.0, 0.0)`：当前矩阵轴长接近 0 时用它代替从矩阵对角抽出的向量。`0x10091FE0` 是 `(0.0, 1.0)`，只参与 `+128` 那条分支的符号，不参与三个因子。

## 4. f0 和 `+128` 各一句

- **f0（`+0`）** = `CJTransformationServices` 空间 2 的当前矩阵把参考向量变完之后的长度，再乘该空间单位栈顶（指针 `服务+0x494`，下标 `服务+0x4A4`）。长度在 `0x1006210B`，乘法在 `0x1006211F`，落点 `0x10062133`。
- **`+128`** = 服务对象 `+0x710` 为 0 时就是常量 1.0（`0x100626E6`，`qword_10090A38`）；为 1 时是 `sub_10061490` 拷进来的那块矩阵前两个分量的长度（`0x10062635`–`0x10062664`）。构造 `sub_1005FD30` 在 `0x10060725` 把 `+0x710` 写成 0。

## 5. 像素模式 `+24` 在屏幕路径上的值

填充函数不计算它，只拷贝。构造 `sub_1005FD30` `0x100607F3` 把服务对象 `+0x28` 写成 **0**，`0x10062625` 再拷进 `unitsCtx+24`。

写成 1 的指令只有一处：`sub_10063110` `0x1006312F` `mov dword ptr [ecx+28h], 1`，同时把仿射 a/b 和 maxPx 设成入参。它是 `CJTransformationServices` 虚表槽 32（thunk `0x10002F6D`，槽地址 `0x10095554`）。`render.dll` 里没有直接 `call` 这个 thunk，`RealRender2d` → 代理 → `CModule2d` → `sub_10061EE0` 也不经过它。清回 0 的是槽 34 `sub_100619A0` `0x100619A0`（`*(this+10)=0`）。

所以：**这条显示链本身不把 `+24` 设成非 0。** U7b 从视图侧核完：没人虚调槽 32。`render.dll` 里 `call [reg+80h]` 打到管理器 `+5528` 的只有 `IJRenderViewSetup` 槽 19 `sub_10020AA0` 的 `0x10020ACF`；imagdex / j2dsrv 对 `{0693921A}` 的 QI 不进槽 19。槽 45（`+128` 的另一条）同样只有虚表引用。屏幕路径上 `+24` 保持构造值 **0**，`+128` 保持 **1.0**。这组默认值和 U6（文字约 1、虚线约 1.67）对不上：像素模式关且 `+128` 为 1 时，纸单位文字和虚线共用 f0。这个分叉在已打开的库里还没有写入点。

`+0x710`（决定 `+128` 是 1 还是另一块矩阵的轴长）同样：构造写 0（`0x10060725`），写成 1 的是槽 45 `sub_10061490` `0x10061490`，清回 0 的是槽 46 `sub_10061C00`。显示链本身不置位。

## 6. 写入链能不能算出 1.67

**不能。**

断在 `0x10062133`（f0 的 `movsd`）以及它前面的 `0x1006211F`（`mulsd`）。右值只有两项：空间 2 的矩阵轴长，和单位栈顶那个 double。函数里没有 5/3，没有 1.67，没有 25.4 / 96 / 72，也没有 `GetDeviceCaps`。栈顶和矩阵都是别的调用事先塞进 `CJTransformationServices` 的，这里只做长度再乘一次。1.67 若出现，只能是那两项里已经带着，不是这条指令算出来的。

默认旗标下也拼不出 U6 那种「虚线 1.67、文字约 1」的分叉：像素模式是 0 时虚线走 f0（U5 §6），`+128` 是 1 时文字也是 f0×1。要拆开这两者，得先有人调槽 32（像素模式=1，虚线改走 `+40/+48` 与 `+16`）或槽 45（`+128` 改成另一块矩阵的轴长）。这两次调用都不在本链上。

## 7. 复现

```text
D:\ida_scratch\u7a\render\render.dll.i64
    ← 拷自 D:\ida_scratch\units-ctx\render\render.dll.i64
```

`pid-parse/dlls/` 下 11 个 `.i64` 的修改时间与打开前一致（未作为 IDA 输入），例如 `style.dll.i64` `2026-08-07 15:41:34.936`，`imagdex.dex.i64` `2026-08-31 02:59:10.939`。批处理不走 ida-bridge：

```powershell
& "D:\IDA Professional 9.2\ida.exe" -A -LD:\ida_scratch\u7a\pass1-ida.log -SD:\ida_scratch\u7a\pass1.py D:\ida_scratch\u7a\render\render.dll.i64
```

第三方核对顺序：

1. `0x1001D5F0`：IID `{06939214-…}` 返回 `this+25`（几何 `+100`）。
2. `0x1001D9E2`–`0x1001D9F4`：`几何+8` = `CTransformProxy`（`+0x1494`），`几何+12` = 上下文（`+0xF60`）。虚表 `0x1009289C` 槽 0 → `0x1001F870`。
3. `0x1001F888`–`0x1001F88D`：第一次槽 0 把 `几何+8` 改成 `CModule2d`。虚表 `0x10092C20`。
4. `0x10025780` 里 `call [reg+2Ch]`（`0x10025ECB`）：第 3 个参数 = `context+896`。
5. `0x10061EE0`：`0x10062133` / `0x100620AA` / `0x100621C0` 三个 `movsd`，`0x100625FE` 的 `rep movsd`，以及 `0x1006260D` / `0x10062617` / `0x1006261F` / `0x10062625` / `0x100626E6`。
6. `0x100607F3` 写像素模式 0；`0x1006312F` 是唯一写 1 的指令。

## 8. 验证

IDA 9.2 批处理反编译加反汇编，上面每个地址都在 `D:\ida_scratch\u7a\render\render.dll.i64` 里对过指令。SmartPlant 实机显示 **NOT RUN**（本任务不跑程序、不跑 cargo）。单位栈顶在一次真实视图里的具体数没有读到，只追到指针和下标。

## 9. 和 U7b 对过的三件事

U7b（`docs/analysis/2026-10-11-units-context-writer-view.md`）从视图侧回来说：`imagdex` / `j2dsrv` / `docext` / `radsrvitem` 不 CoCreate `JRenderManager2`，也不写 `CRenderGeometry+8`；`dispCtx+404` 进 `RealRender2d` 之前已经是这份几何。这和 §1 一致：`+8` 的两笔写入都在 `render.dll` 的管理器初始化 / 代理槽 0 里，不在那四个库。

管理器 `+5496` 的 f64（构造初值 0.5）在上下文脏字 bit7 时进上下文 `+1256`。那不是 `unitsCtx+40`。`unitsCtx` 从 `+896` 起，`+40` 是 `+936`，源是服务对象 `+0x10`，指令 `0x1006260D`。`+1256` 是另一格（`sub_1001E6E0` 按 qword 下标 157 读它）。

`GetDeviceCaps` 没有 `LOGPIXELSX/Y`（88/90）。§6 的断点不变：f0 的乘法 `0x1006211F` 不读 DPI。`unitsCtx+24` 不再断在委托之后，写入点就是 §5 的 `0x10062625`。U7b 随后确认槽 32 没有虚调用，屏幕路径上的值就是构造时的 0。
