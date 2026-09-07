# IDA：`imagdex.dex` 四族几何 `IJPersist::DoIO` —— 曲线族流布局升 native-reader

> 日期：2026-08-31
> 范围：用 idalib（headless）分析 `dlls/imagdex.dex`，沿 `pid-format-guide.md` §10
> 的入口路径反编译四个仅存在于 `JSite/PSMcluster0` 的几何族的持久化读取器
> （`JCircle2d 0x0059` / `JArc2d 0x0061` / `JRectangle2d 0x0020` / `JBspCurve2d 0x005D`），
> 把 Phase 36 停在 corpus 级的曲线族流布局升级到 **native-reader**。
> 只读逆向，未改 parser/schema/model。

## 结论速览

| 族 | type | payload | native 布局（信封 `+18` 之后） | 等级 |
|---|---|---:|---|---|
| Circle | `0x0059` | 43 | `center.x`(f64) `center.y`(f64) `radius`(f64) + 1B flag | **native-reader** |
| Arc | `0x0061` | 59 | `center.x` `center.y` `radius` `startAngle` `endAngle`（5×f64）+ 1B | **native-reader** |
| Rectangle | `0x0020` | 变长 | `u16` + `u32` + 5×f64 几何核心（+ 版本化 SmartSketch 关系数据）| **native-reader** |
| BspCurve | `0x005D` | 变长 | `u16`+`u32`+`u32 N`+N×(2×f64) poles+[N×f64 权重]+`u32 M`+M×f64 knots+f64+4×u8 | **native-reader** |

Circle / Arc 的 native 布局与 Phase 36 的 corpus 假设**逐字节吻合**（见 §6），
两条独立证据链互证，可从 `corpus-statistical` 升到 `native-reader`。
Rectangle（§8.4 原记「未解码」）与 BspCurve（语料仅 1 例、无法字节统计）本轮由原生读取器坐实。

## 1. 环境与产物

- IDA Professional 9.2 + `idapro`(idalib)，Python `C:\Users\dpc\AppData\Local\Programs\Python\Python312`。
- `imagdex.dex` 是 **32 位 DLL**（基址 `0x10000000`，`__stdcall`，4 字节指针），
  **带完整 MSVC 符号 + RTTI**（`JCircle2d` / `JArc2d` / `JRectangle2d` / `JBspCurve2d`、
  各族 `IJPersistImp`、`ClassFactory<…>`、`IGDSFactory*`、`H*Geometry2d` 创建器等）。
- 随库的解包库（`imagdex.dex.id0/id1/id2/nam/til`）只有 3 个已识别函数——上一轮打开后
  未跑完分析。本轮 `open_database(run_auto_analysis=True)` 跑满分析（45s，**1610 函数**）
  并 `save=True` 生成 `dlls/imagdex.dex.i64`（96 MB），后续脚本秒开。
- 动手前把解包库 5 个文件备份到 `dlls/_imagdex-idb-backup-20260831/`。

工具脚本（均只读，`run_auto_analysis=False` + `close save=False`）：

| 脚本 | 作用 |
|---|---|
| `tools/idalib_imagdex_build.py` | 首次完整分析 `imagdex.dex` → `.i64` |
| `tools/idalib_imagdex_geom_probe.py` | 定位四族 CLSID 常量、xref、工厂入口 |
| `tools/idalib_imagdex_dissect.py` | `.data` 类描述符 + 字符串（RTTI/字段名）+ 命名函数 |
| `tools/idalib_imagdex_persist.py` | 定位 `ExportedVTableJ<族>2dPersistImp` |
| `tools/idalib_imagdex_vtable.py` | 解 thunk 取 PersistImp vtable，dump slots |
| `tools/idalib_imagdex_findio.py` | 按 jengine/几何 IO 密度给 slot 打分 |
| `tools/idalib_imagdex_doio.py` | slot3 分派 + DoIO worker 反编译 |
| `tools/idalib_imagdex_geomread.py` | 当前版 DoIO + 几何读取子程序（逐字段）|

## 2. 入口路径（imagdex 版，对应 guide §10 的 style.dll 路径）

```
type code → psm_type_clsid.py → CLSID
  → CLSID 在 .data 类描述符 → 类主 vtable（ExportedVTableJ<族>2dPersistImp 返回）
    → vtable slot 3 = IJPersist 持久化入口
      → jengine_1076(ctx) 取版本
        → 版本匹配 → 当前版 DoIO worker；否则 case 1/2/3 → 旧版 worker
          → 逐字段 jengine_1075(ctx)，字段字节数在调用前经栈槽设定
```

- `ExportedVTableJ<族>2dPersistImp` 是 thunk（`JUMPOUT`），跳到 `mov eax, offset <vtable>; retn`。
  取到的 vtable：Circle `0x1052A460`、Arc `0x1052996C`、Rectangle `0x1052AD80`、BspCurve `0x1052A5FC`。
- vtable 前 3 槽是 `IUnknown`（QI/AddRef/Release），**slot 3** 是持久化入口，各族一致。
- `jengine_1076` / `jengine_1075` 与 `style.dll`（guide §10）同族：前者取版本，后者逐字段 IO。

## 3. `jengine_1075` 字段字节数语义

DoIO worker 里反复出现的形状（以 Circle 当前版 `sub_10153980` 为例）：

```c
savedregs = 2;  jengine_1075(a3);   // 读 u16
savedregs = 4;  jengine_1075(a3);   // 读 u32
... 几何子程序 ...
```

`savedregs`（`[ebp+0]`，即调用前压栈的参数）就是**该字段的字节数**：

| 值 | 字段 |
|---|---|
| 1 | u8 / bool |
| 2 | u16 |
| 4 | u32 |
| 8 | f64（double）|
| 16 | 16 字节（点对 / GUID）|

> BspCurve/Rectangle 当前版反编译出 `jengine_1075` 的完整签名
> **`jengine_1075(ctx, size, dest_ptr)`**：第 2 参是字节数、第 3 参是目标地址
> （如 `jengine_1075(a2, 8, obj+56)` = 读 8 字节到 `obj+56`）。Circle/Arc 处被反编译成
> `savedregs = N; jengine_1075(a3)` 只是没识别出后两个寄存器参数，语义完全一致。

## 4. Circle / Arc（native-reader 坐实）

**Circle 当前版**：`slot3 sub_10153C10` → 版本匹配 → `sub_10153980`（当前版 DoIO）：

```c
savedregs = 2; jengine_1075(a3);   // sub_type_word (u16)  = 信封 +12
savedregs = 4; jengine_1075(a3);   // index (u32)          = 信封 +14
v7 = sub_100125A8();               // 几何子程序（→ sub_101546E0）
```

几何子程序 `sub_101546E0`：

```c
savedregs = 8; jengine_1075(a2);   // center.x (f64)  payload +18
savedregs = 8; jengine_1075(a2);   // center.y (f64)  payload +26
savedregs = 8; jengine_1075(a2);   // radius   (f64)  payload +34
               jengine_1075(a2);   // 1B flag         payload +42
// 读毕写回对象：*(a3+16)=..，*(a3+24)=0，*(a3+32)=1.0
```

→ **Circle payload 43 = 信封 18 + [center.x, center.y, radius](3×f64=24) + flag(1)**。

**Arc 当前版**：`slot3 sub_10140CC0` → 版本匹配 → `sub_10140BD0`：

```c
savedregs = 2; jengine_1075(a3);   // sub_type_word (u16)
savedregs = 4; jengine_1075(a3);   // index (u32)
result = sub_1001177F();           // 几何子程序（→ sub_10143B60）
```

几何子程序 `sub_10143B60`：

```c
               jengine_1075(a2);   // center.x (f64)  payload +18
savedregs = 8; jengine_1075(a2);   // center.y (f64)  payload +26
savedregs = 8; jengine_1075(a2);   // radius   (f64)  payload +34
savedregs = 8; jengine_1075(a2);   // startAngle(f64) payload +42
savedregs = 8; jengine_1075(a2);   // endAngle (f64)  payload +50
               jengine_1075(a2);   // 1B flag         payload +58
```

→ **Arc payload 59 = 信封 18 + 5×f64(40) + flag(1)**。角为**绝对起止角**
（`startAngle` / `endAngle`），与 `2026-07-27-ugeom2d1-curve-readers-ida.md` 结论一致。

> 旧版 worker（Arc `sub_101441C0` / `sub_10144040`，case 1/2/3）读 `u16+u32+6×f64+u8`，
> 是历史格式；当前图纸走上面的当前版路径。旧版存在但四主 fixture 未命中。

## 5. Rectangle（native-reader）

`slot3 sub_10166D30` 是对版本 1..5 的**纯 switch**（无「当前版短路」分支），
最高版 **case 5 = `sub_1016A7C0`** 是当前格式：

```c
jengine_1075(a2, 2, this+88);   // sub_type_word (u16)
jengine_1075(a2, 4, this+72);   // u32
sub_100016F4(a2, this+32);      // → sub_10151860：几何核心
```

几何核心 `sub_10151860` 连读 **5×f64**（`this+32 … this+64`）。其后按 flag 读一段
**版本化的 SmartSketch 关系数据**（`jengine_1004` 建关系表、`u32` 计数、`l%d` 命名的
逐条关系），属约束/参数化信息，不是绘制几何。Clone `sub_10166E10` 的内存布局
（`+16/+32/+48` 三个 16 字节、`+64` 8 字节）与 5×f64 几何核心相符；bbox 子程序
`sub_10143A00` 用 `ffloor`/`fceil` 求四角，说明矩形带旋转（5 个 f64 ≈ 原点 + 轴向/尺寸 + 角）。
Rectangle 之前在 guide §8.4 记为「未解码（Phase 34-B 负结论）」，本轮**核心几何 native 坐实**；
5 个 f64 的逐个语义建议用一张真实图纸的 3 个 Rectangle 实例做 fixture ratchet 确认。

## 5bis. BspCurve（native-reader，变长）

`slot3 sub_10157800` → 版本匹配 → 当前版 `sub_10156FB0`。字段读序：

```c
jengine_1075(a2, 2, obj+92);            // sub_type_word (u16)
jengine_1075(a2, 4, obj+84);            // u32（容量/标志）
jengine_1075(a2, 4, obj+56);            // u32  pole_count = N
for i in 0..N:                          // 控制点，每点 2×f64
    jengine_1075(a2, 8, poles + 16*i);      // pole.x
    jengine_1075(a2, 8, poles + 16*i + 8);  // pole.y
jengine_1075(a2, 4, obj+64);            // u32  weight_flag/count
if weight != 0:                         // 有理 NURBS：逐控制点一个权重
    for i in 0..N:
        jengine_1075(a2, 8, weights + 8*i);
jengine_1075(a2, 4, obj+60);            // u32  knot_count = M
for i in 0..M:
    jengine_1075(a2, 8, knots + 8*i);       // knot[i]
jengine_1075(a2, 8, obj+48);            // f64（标量：参数域/端参）
jengine_1075(a2, 1, obj+72);            // u8 flag
jengine_1075(a2, 1, obj+73);            // u8 flag
jengine_1075(a2, 1, obj+74);            // u8 flag
jengine_1075(a2, 1, obj+75);            // u8 flag
```

→ 变长布局：`u16 + u32 + u32 N + N×(2×f64) poles + u32 + [N×f64 权重] + u32 M + M×f64 knots + f64 + 4×u8`。
控制点循环见 `sub_10156FB0` 的 `do…while(i < pole_count)`，权重/结点循环紧随其后
（`sub_102DA280` 是配套的运行时重建器，非流读取器）。这是曲线族里唯一的变长记录，
语料仅 1 个实例，此前无法用字节统计解码，本轮由原生读取器坐实。

## 6. 与 corpus 对照（互证）

| 族 | corpus（Phase 36）| native（本轮）| 一致 |
|---|---|---|---|
| Circle 0x0059 | center(2f64)+radius(f64)+1B，payload 43 | 同 | ✓ |
| Arc 0x0061 | center(2f64)+radius(f64)+start+end(f64)+1B，payload 59 | 同 | ✓ |

两条独立证据链（真实图纸字节统计 × 原生读取器反编译）逐字节吻合，
故 Circle / Arc 的流布局从 `corpus-statistical` 升到 **`native-reader`**。

## 7. 待续

- **四族流布局均已 native-reader 坐实**（Circle / Arc / Rectangle / BspCurve）。
  剩余为语义细化：Rectangle 5 个 f64 的逐个含义、BspCurve `obj+48` 标量与 4 个 u8 flag 的语义，
  建议各用一张真实图纸的实例做 fixture ratchet。
- 回写 `pid-format-guide.md` §5 / §8.4（把四族布局并入正文），据此设计四族读取 API。
