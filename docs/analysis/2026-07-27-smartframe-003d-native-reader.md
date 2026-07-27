# `0x003D igSmartFrame2d`：native reader 定性 + 页框取证

> 日期：2026-07-27
> 范围：用 `radsrvitem.dll` 的 native reader 给 `0x003D` 命名有界结构字段，
> 并用 6 个 fixture 交叉验证。纯只读，未改 parser / schema / model /
> byte-audit，**未动 `PidPageTransform`**。

## 结论

**`igSmartFrame2d` 是 OLE 容器框，而图纸边框正是以「链接的 OLE 对象」这种
形式存在的。** 两句话都对：前者是机制，后者是它在 P&ID 里的用途。

`radsrvitem.dll sub_564464D0` 是权威分类器，它直接按记录字节判定：

```c
if ( *(WORD *)a2 != 61 ) return E_INVALIDARG;            // 61 == 0x003D
v6 = *(DWORD *)(a2 + 32);
if ( (*(DWORD *)(a2 + 20) & 0x8000) == 0 )               // "_Empty SmartFrame2d"      igOLENone
else if ( !(v6 & 0x40) && !(v6 & 0x20000) )              // "_Embedded SmartFrame2d"   igOLEEmbedded
else if ( v6 & 0x20000 )                                 // "_Locally Linked ..."      igOLELinked
else                                                     // 外部链接                    igOLELinked
```

类型字在 `a2 + 0` 被读取，而 PSM envelope 的类型字就在记录 `+0`，所以 `a2`
指向**记录起点**，两个标志字分别在记录 `+20` 与 `+32`（即载荷 `+14` / `+26`）。

## 交叉验证：12 条记录、3 个取值、零随机

`examples/probe_smartframe_variant.rs` 把上面的判定跑在 6 个 fixture 上：

| 记录 `+20` | 记录 `+32` | 判定 | 条数 |
|---|---|---|---:|
| `0x3c808001` / `0x5c808011` | `0x20e90040` | Linked | 10 |
| `0x5c808011` | `0x20e90004` | Embedded | 1 |
| `0x28c28011` | `0x00430000` | LocallyLinked | 1 |

**两个标志字在整个语料里各自只有 3 个取值**，不是随机位——这是「reader 读的
就是这些盘上字节」的判据。分类还与载荷完全自洽：11 条 Linked/Embedded 全带
ISO A 系页幅（长宽比 `+148` 恒 ≈ 0.7072 = 1/√2），唯一那条 LocallyLinked 在
嵌套 `/JSite204\Sheet6` 里、尺寸退化为 `1e-6`、长宽比 1.0 —— 不是页面。

这同时解释了 Phase 34-C 的困惑：载荷里为什么有页幅标量却不构成页面变换 ——
那是**被框住的对象自身的尺寸**。

## 页框：载荷 `+76/+84` 是页宽页高，原点是 (0,0)

| fixture | 页幅（`+76`×`+84`，mm） | 内容范围 x | 内容范围 y | 落在页内 |
|---|---|---|---|---|
| DWG-0201 | 594.3 × 420.3（A2） | 33.98 … 583.19 | 6.95 … 402.02 | 是 |
| DWG-0202 | 593.7 × 419.6（A2） | **-25.64** … 583.70 | 5.27 … 409.87 | x 左侧越界 25.6mm |
| 工艺管道-1 | 841.0 × 594.0（**A1**） | **-8.03** … 826.86 | 13.34 … 551.31 | x 左侧越界 8.0mm |
| D06 | 594.3 × 420.6（A2） | 75.11 … 377.43 | 48.23 … 285.46 | 是 |
| A01 | 594.0 × 420.0（A2） | **0.00** … 548.25 | **0.00** … 380.27 | 是，且恰好从原点起 |

三条互相独立的证据指向同一个结论：

1. **x_max / y_max 无一例外略小于页宽 / 页高**，五张图都是；
2. **A01 的内容恰好从 (0,0) 开始**；
3. 把 `(0,0)–(w,h)` 的矩形叠到 DWG-0201 上出图，边框把整张图不多不少地
   框住：右上说明表、右下标题栏、中间工艺主线，边距自然
   （`examples/plot_symbol.ps1`，见复现段）。

工艺管道-1 是 A1 而其余是 A2，与它实体数最多、幅面最大的事实相符。

**未解释处**：DWG-0202 与工艺管道-1 的 x 最小值为负（-25.6mm / -8.0mm）。
落在页左外侧，量级远小于页宽，但尚未定性——可能是页外注记，也可能是那批
已知的离群实体。在弄清之前，原点仍应视为**强证据而非定论**。

## 门禁状态

- `ROADMAP-SMARTFRAME-003D` 要求「native reader 或受控 fixture 证据才能命名
  有界结构字段」。本轮**已满足**：三态判定 + 两个标志字的位含义 + 记录偏移，
  均由 native reader 给出并经全语料交叉验证。
- `ROADMAP-PAGE-TRANSFORM` 要求坐标空间 / 单位 / 方向 / 原点 / 比例 / 边界 /
  溯源**全部**证明。本轮补齐了单位（米）、边界（`0,0`–`w,h`）、溯源
  （`0x003D` 载荷 `+76/+84`，reader 已定性），方向与原点为强证据但仍有上面
  那处未解释的负 x。**本轮不改 `PidPageTransform`**，维持 `Unavailable`。

## 复现

```
python tools/idalib_smartframe_scan.py
python tools/idalib_smartframe_decompile.py
cargo run --release --example probe_smartframe_variant
```
