# `JStyleTextPara` 全布局：对齐找到了，38% 的标签摆错位置

> 日期：2026-08-13
> 阶段：文字保真取证 **+ 水平对齐已接线**（见 §6）
> 证据等级：**native-reader**（`style.dll!sub_100337A0`）+ **厂商枚举定义**（`Interop.RAD2D.dll`）
> 产物：`examples/probe_text_para_layout`、`examples/probe_igtextbox_tail_kind`

## TL;DR

段落样式 `0x002D JStyleTextPara` 此前我们**只读它的 `+38`**（字符样式指针），其余一概
未解。拿到原生读序之后整条记录一次交代完，其中：

- **`+35` 是水平对齐**，取值 `{0 左, 1 居中, 2 右}`——映射来自**厂商自己的枚举定义**，
  不是猜的；
- 全语料 376 条里 **`{0: 232, 1: 136, 2: 8}`——144 条（38%）不是左对齐**，而
  `OpenCADStudio` 目前把每一条标签都按左对齐放在插入点上。

**这是位置错，不是字形错。** 一条居中的标签按左对齐画，会整体右移半个词宽，在密集的
P&ID 上会压到管线。优先级高于字体名。

## 一、完整布局（`style.dll!sub_100337A0`）

序列化器在基类块之后逐字段读：

```text
  sub_1000287E(...)        基类公共字段（payload +12..+26）
  +26  u32   （只有高半字有效）
  +30  u32
  +34  u8
  +35  u8    水平对齐        ← 接口有 get/put 对
  +36  u8    垂直对齐        ← 接口有 get/put 对
  +37  u8    （读后丢弃）
  +38  u32   字符样式指针     ← 已知锚点，corpus 237/237
  +42  f64  ⎫
  +50  f64  ⎪
  +58  f64  ⎬ 六个度量，接口 setter 拒收负值
  +66  f64  ⎪
  +74  f64  ⎪
  +82  f64  ⎭
  = 90 字节
```

**自证**：`payload == 90` 全语料 **376/376** 成立，读序把每个字节都交代了；而 `+38`
是我们早已在用的字符样式指针，落点分毫不差。

字段是怎么找到的：`IJStyleTextParaImp` 的 vtable（`0x100a1f38`）里那些 18–40 字节的
小函数是访问器，暴露出对象槽位（接口指针 = 对象基址 + 48）；再回头找哪个 DoIO 调用者
喂这些槽位，序列化器就是 `sub_100337A0`。**与定位 `JStyleTextChar` 同一手法。**

## 二、对齐的映射不是猜的

`+35`／`+36` 是接口上仅有的两个 u8 get/put 对——形状就是两个小枚举。但"三值枚举"不
等于"就是对齐"，所以去找了厂商的定义。`dlls/Interop.RAD2D.dll` 是 Intergraph 自己的
.NET interop 程序集，用反射读出：

```text
TextHorizontalJustificationConstants     TextVerticalJustificationConstants
  igHorizontalTextLeft   = 0               igVerticalTextBaseline = 0
  igHorizontalTextCenter = 1               igVerticalTextTop      = 1
  igHorizontalTextRight  = 2               igVerticalTextCap      = 2
  igHorizontalShapeLeft  = 3               ...（共 10 个）
  igHorizontalShapeCenter= 4
  igHorizontalShapeRight = 5
```

四重吻合：

1. `+35` 与 `+36` 是接口暴露的仅有两个 u8 枚举；
2. `+35` 实测**恰好**取 `{0,1,2}`——水平枚举的"文本"档，`3/4/5`（Shape 档）一条都没有；
3. `+36` 恒为 `0` = `igVerticalTextBaseline`，正是默认基线；
4. 读序里 `+35` 在 `+36` 之前，与接口 put 的声明顺序（先水平后垂直）一致。

## 三、六个度量与行距

| 偏移 | 全语料取值 | 读法 |
|---|---|---|
| `+42` / `+50` / `+58` / `+74` | 恒 `0.0` | 未使用的缩进/间距 |
| **`+66`** | `{0.0: 8, 1.0: 332, 1.5: 36}` | **行距倍数**（单倍 / 1.5 倍） |
| `+82` | `{0.0: 370, 0.000508: 6}` | `0.000508 m = 0.02″`，某种间距 |

`+66` 的 `1.0 / 1.5` 是行距的典型取值。单行标签用不上，多行才显现，暂不接线。

## 四、顺带：`igTextBox +20` 是尾块格式选择子

原生 `igTextBox` Load（`radsrvitem.dll!sub_56498C00`）在 `Src[13]`（record 字节 26 =
**payload +20**）上分支，取值只允许 `1` 或 `2`，其余返回 `E_INVALIDARG`：

```c
if ( Src[13] == 1 )      apply(kind=1, (char*)Src + body_len + 28);
else if ( Src[13] == 2 ) apply(kind=2, (char*)Src + body_len + 28);
else                     return -2147024809;
```

`(char*)Src + body_len + 28` 折算成 payload 就是 `+22 + body_len`——**正是解码器开始
读放置尾块的位置**。所以 `+20` 选择的是**那 36 字节怎么读**。指南里"style-tail tag
语义未解"这条就此关闭。

实测交叉表：

```
 shape  tail kind    records   accepted
     1          1         32         32
     2          1        191        191
     2          2         24         24
     3          1         13         13
```

**24 条 kind 2**，且这 24 条正是此前记为"尾巴 68/76 字节、像是挂了东西"的那批
（15 + 9 = 24）。所谓"尾巴变长"的真身是**另一种尾块格式**。

**但今天没有正确性 bug。** 三条依据：

1. 这 24 条全部通过解码器的单位向量校验（`accepted 24/24`）——该校验要求尾块开头四个
   double 构成合法插入点 + 单位方向，布局若不同必然读出垃圾并失败；
2. 原生两个分支调**同一个**辅助函数、传**同一个**数据指针，只差 kind 选择子，说明
   放置信息在两种 kind 里位置相同；
3. 因此 kind 2 只是在放置块之后多挂 32 或 40 字节。

那多出来的 32/40 字节仍未解——要读那个样式应用辅助函数才能知道。

## 五、形状 3 的 A/B 不是 doubles，是格式化 run

原生 case 3 里，`A + B` 个条目被**按 8 字节步长**遍历，每个条目读一个 u16 选择子
（`1` 或 `2`）和一个值，分别派给两个不同的 vtable 槽（+68 / +72）——与 case 2 用
`(count, value)` 调 +68 是同一对调用。

所以形状 3 是**带格式化 run 的富文本**：`A` 个一种 run、`B` 个另一种。我们目前把整条
文本当单一样式渲染。这解释了"形状 3 的 A/B doubles 语义未知"，也说明它不是几何数据，
不影响位置。

## 六、接线结果

水平对齐已端到端接通。

**`pid-parse`**：新增 `TextAlignment{Left, Center, Right}` 与
`TEXT_PARA_HORIZONTAL_ALIGNMENT_OFFSET`，`ResolvedTextHeight` 多带一个
`alignment` 字段，走**已有的**两跳链，不新建索引表。有两处刻意的取舍：

- 它取自**段落**记录而非字符样式——对齐属于 run 不属于字形。所以一跳形状（文字记录
  直接命名 `JStyleTextChar`）下为 `None`，而不是编一个默认值：那会与"确实声明了左
  对齐"无法区分。本语料没有一跳形状。
- **拒收 `Shape` 三档（3/4/5）**。它们要求一个我们没有的外框；把它当文本对齐读，会
  把 run 自信地放到错处，比不动更糟。

**`OpenCADStudio`**：`apply_text_alignment` 与既有的 `apply_text_height` /
`apply_text_colour` 同一作用域（只管图纸自己在 `PID-TEXT` 上的文字），并在设完枚举后
调用**既有的** `sync_text_alignment_point`。这一步是关键：TEXT 实体只有在"左对齐 +
基线"时插入点才是 run 原点，否则原点是 `alignment_point`。**只设枚举不播种对齐点，
标签不是偏半个词，而是直接掉到原点。**

**回归测试**锁住两侧：语料侧 `{center: 60, left: 79, right: 16}`，
`DWG-0201GP06-01` 侧 `{center: 18, left: 28, right: 2}`，且断言"非左对齐必有对齐点、
左对齐必无"。注意 `left` 这一档不能当测量值读——样式解析失败的记录会保留实体默认值
落进这一档，与"确实声明左对齐"无法区分；该 sheet 恰有一条这样的记录（就是同时保留
字高与颜色回退的那条，三者同走一条 join）。

暂不接线的：行距 `+66`（单行标签用不上）、垂直对齐 `+36`（恒为基线）、kind 2 的
额外字节、形状 3 的格式化 run。
