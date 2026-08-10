# 填充色解开：`0x002A JStyleSimpleFill` payload `+30`

> 日期：2026-08-10
> 范围：`pid-parse`（解码）+ `OpenCADStudio`（消费/渲染）
> 结论类型：**native-reader 给字段与语义，corpus 给字节偏移与 join**——两种帧装，
> 各自可证伪，合起来把颜色钉死。
> 前置：`2026-08-10-fill-has-a-consumer-after-all.md`（填充有消费者）、
> `2026-08-04-jstylesimpleline-native-reader-confirmed.md`（线宽/颜色同一套路径）、
> `2026-08-04-style-dll-class-chain.md`（族身份四证 + CLSID → vtable 链）
> 工具：`examples/probe_fill_colour`

## 0. 一句话

`0x002A JStyleSimpleFill` 的实心填充色在 **payload `+30`**，是一枚 Win32 `COLORREF`
（`0x00BBGGRR`，字节序 `[R, G, B, 0]`），与 `0x002E` 线颜色**同一种编码**。
`0xFFFFFFFE`（-2）是「未设色」哨兵。两张真图上的流向箭头填充读出来都是 **`#0000FF` 蓝**，
此前一律画成图层默认白。

## 1. native reader：字段与语义

`0x002A` 的 CLSID 逐字节核对为 `{47FCC331-2D0F-11D0-A1FF-080036A1CF02}`
（`xmmword_1009DD50` = `31 c3 fc 47 …`），正是 type code → CLSID 表预测的 `47FCC331`；
对照 `0x002E` 在 `0x1009DE18` 读出 `47FCC335`，族身份此前已四证，这里再加一条序列化器
自证。

路径与 `JStyleSimpleLine` / `JStyleSimpleDashType` 完全同构：

```text
JStyleSimpleFill vtable @0x100A0D60
  └─ slot 17 (+68) = sub_10002E37 → sub_1001D610   （持久化 worker）
       ├─ jengine_1076 取版本号
       ├─ version 2 → 本函数内联展开   ← 语料用的是这条
       │    ├─ sub_10002CFC(stream)          基类块（与 JStyleSimpleLine 同一个 helper）
       │    ├─ jengine_1075(stream, 4, this+26)   → 对象 +104
       │    ├─ jengine_1075(stream, 4, this+28)   → 对象 +112
       │    └─ jengine_1075(stream, 4, this+30)   → 对象 +120
       └─ version 3 → (*this + 208)   即 vtable slot 52
```

版本 2 的类字段就是**三枚 u32**，落到对象 `+104` / `+112` / `+120`。逐一定性：

| 对象偏移 | 是什么 | 依据 |
|---|---|---|
| **`+104`** | **实心填充色** | `IJStyleSolidFillImp`（构造函数 `sub_1001C160` 装在主对象 `+0x4C`）的 get/put 对 `sub_1001C650`/`sub_1001CBA0` 各自读写 `this+28` = 主 `+104`；而 `JStyleSimpleLine` 的颜色也正是主 `+104`——同一基类成员槽。`IJStyleSimpleFillImp::put` 在清空时写 `-2` 到这里（`sub_1001CB00`），即「未设色」哨兵 |
| `+112` | 一个惰性解析的对象引用（不是颜色） | `sub_1001CC20` 以 `sub_1000448A(主+112, 主+108, …, IID {A99F1CA0-2DD4-11CF-…})` 处理，`+108` 是缓存指针、`+112` 是 id，与虚线引用同形 |
| `+120` | 第三枚 u32，语义未定 | get/put 对 `sub_1001C6B0`/`sub_1001CBF0` 读写主 `+120`；本轮不猜 |

**「实心填充」这个接口只暴露一枚颜色 get/put，而它指向 `+104`**——一枚平色填充就该只有
一个颜色，这条把 `+104` 认定为填充色，而非另两枚字段之一。

## 2. corpus：字节偏移

基类块 `sub_10002CFC` 与 `JStyleSimpleLine` 是同一个 helper，在 `.pid` 帧装里占 **30 字节**
（线样式侧已定 `B = 30`），所以第一枚类字段——填充色——落在 **payload `+30`**。
`examples/probe_fill_colour` 把每条 `0x002A` 的 `+30` 按 `COLORREF [R,G,B,0]` 解出来：

| fixture / cluster | 0x002A id | `+30` | 判读 |
|---|---:|---|---|
| DWG-0201 | 3 | `0xFFFFFFFE` | 未设色（-2，画图层默认） |
| DWG-0201 | 13 | `0x00000000` | `#000000` 黑（在线调色板内） |
| DWG-0201 | 34 | `0x00FF0000` | `#0000FF` 蓝（在线调色板内） |
| DWG-0202 | 3 | `0xFFFFFFFE` | 未设色 |
| **DWG-0202** | **20** | **`0x00FF0000`** | **`#0000FF` 蓝 ← 被边界引用** |
| DWG-0202 | 26 | `0x00000000` | `#000000` 黑 |
| 工艺管道-1 | 3 | `0xFFFFFFFE` | 未设色 |
| 工艺管道-1 | 13 | `0x00000000` | `#000000` 黑 |
| **工艺管道-1** | **23** | **`0x00FF0000`** | **`#0000FF` 蓝 ← 被边界引用** |
| D06 / A01 | 3 | `0xFFFFFFFE` | 未设色 |
| D06 / A01 | 16 | `0x00000000` | `#000000` 黑 |

三条硬判据：

1. **哨兵对得上 DLL。** 每个文档都定义一条 id 3 模板填充，`+30` 恰是 DLL 里
   `IJStyleSimpleFillImp::put` 清空时写的 `-2`。没有一条别的偏移会在全语料齐刷刷给出这个值。
2. **8/8 非哨兵值全在本文件自己的线调色板内**，且高字节一律为 `0x00`（合法 `COLORREF`）。
   偏移错一位就不会这么干净——这正是当年定线颜色 `+50` 用的同一种可证伪判据。
3. **被引用的两条填充都是蓝。** DWG-0202 id 20、工艺管道-1 id 23——就是那 15 个流向箭头
   （5 + 10）实际填的色。

## 3. 端到端：渲染器现在拿到蓝

`ResolvedFill` 多带一个 `colour: Option<u32>`（`rgb()` 拆成 `[R,G,B]`）。跑
`fill_styles_for_file`（OCS 调的公共 API）：

| fixture | 边界填充数 | 带色 | 颜色 |
|---|---:|---:|---|
| DWG-0202 | 5 | 5 | 全 `#0000FF` |
| 工艺管道-1 | 10 | 10 | 全 `#0000FF` |

**15/15 箭头带色，全蓝。** OCS `build_fill` 把这枚颜色上到实心 `HATCH` 上（`Color::from_rgb`），
只在填充声明了颜色时覆盖图层默认——一条哈希填充或未设色的填充仍取图层默认（`BYLAYER`），
因为「没声明颜色」本来就该画图层色。

## 4. 两种帧装（为什么 native 与 corpus 的偏移不必相等）

DLL 内存流里 worker 读三枚 u32 到对象 `+104/+112/+120`；`.pid` 帧装里颜色落在 `+30`。
和 `0x002E`（native 的字段间距 vs 语料的 `+34/+50`）、`0x002F`（DLL 计数 u32 vs `.pid`
u16）一样：**native reader 给的是字段清单、顺序与语义**（哪枚是颜色、哪枚是引用），
**语料给的是这份帧装在 `.pid` 里的字节偏移**。两者各自可证伪，合起来钉死。

## 5. 本轮没有做、也不该顺手做的

- **没有解 `+112` 的引用指向何物**（IID `{A99F1CA0-…}`），也没有解 `+120` 的语义。
  它们不是颜色，与「把箭头填成蓝」无关；要解是独立一刀，手法同 `+104`。
- **没有解 `0x002B JStyleHatchFill`。** 全语料没有一条边界引用图案填充，`resolve_fill`
  对它返回 `colour = None`（画图层色）。没有消费者就不解——同 Phase 39 的 Stop 规则。
- **没有解释填充记录尾部那枚 `f64 = 1.0`**（payload `+38..+45`）。像不透明度，但 version-2
  worker 并不读它，语料也全是 1.0，没有可对照的变化，本轮不猜。

## 6. 复现

```powershell
cargo run --example probe_fill_colour          # §2 每条 0x002A 的 +30、§3 端到端
cargo test --locked --lib style_link           # 颜色 / 哨兵 / 黑色三条断言随 style_link 测试跑
```

IDA 开 `dlls/style.dll.i64`，反编译 `0x1001D610`（fill 的持久化 worker）核对 §1；
`sub_1001C160` 看三张接口 vtable 的安装偏移（主 `+0x4C` / `+0x50`）。
