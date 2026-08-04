# 线宽与颜色经 native reader 坐实：`0x002E` payload `+34` / `+50`

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**native-reader 级证据**——语料侧的两个字段与原生序列化逐字节吻合。
> 前置：`2026-08-04-jsl-simple-line-style-weight-colour.md`（语料侧）、
> `2026-08-04-jstyletextchar-native-reader-confirmed.md`（同一套路径）

## 1. 路径

与 `JStyleTextChar` 完全同构：

```text
CLSID 47FCC335 @0x1009DE18  →  ClassFactory<JStyleSimpleLine>
  └─ JStyleSimpleLine vtable @0x100A18F8
       └─ slot 17 (+68) = 0x10001893 → sub_1002DF50
            ├─ jengine_1076(stream, &xmmword_1009DE18, …)  取版本号
            ├─ version 2 → 本函数内联展开   ← 语料用的是这条
            └─ version 3 → (*this + 208)   即 vtable slot 52
```

`sub_1002DF50` 引用 `xmmword_1009DE18`，就是已逐字节核对过的 CLSID。

## 2. version-2 的字段序列

```text
sub_10002CFC(stream)              基类块
jengine_1075(stream, 4, this+60)
jengine_1075(stream, 8, this+80)   <-- 整条记录里唯一的 8 字节字段
jengine_1075(stream, 4, this+96)   <-- 带 -1 哨兵
jengine_1075(stream, 4, this+108)
jengine_1075(stream, 4, this+104)
jengine_1075(stream, 4, this+120)
jengine_1075(stream, 4, this+112)
```

`-1` 哨兵的处理与 `JStyleTextChar` 里那个同形：

```c
if ( *((_DWORD *)this + 24) == -1 && (*(_BYTE *)(stream + 24) & 1) != 0 )
    *v8 = 0;
```

## 3. 逐字节对上语料侧的结论

基类块长度记为 `B`，按上表累加：

```text
+B+0    4
+B+4    8   <-- f64
+B+12   4   （-1 哨兵）
+B+16   4
+B+20   4
+B+24   4
+B+28   4
```

语料侧实测**线宽 f64 在 `+34`** → **`B = 30`**。代回去：

| 偏移 | 大小 | 语料侧观察 | |
|---|---:|---|---|
| `+30` | 4 | | |
| **`+34`** | **8** | **线宽，值域是 ISO 128 系列** | **确认** |
| `+42` | 4 | 列分析里 `42..45` **不变** | 与 `-1` 哨兵字段相符 |
| `+46` | 4 | | |
| **`+50`** | **4** | **颜色，值域是 CAD 调色板** | **确认** |
| `+54` | 4 | 列分析里 `51, 52` 在变 | 与 `+50` 那个 u32 的高半相符 |
| `+58` | 4 | | |

**两个字段一次对上，且 `+34` 是整条记录唯一的 f64。**

## 4. 一处有意思的差异

`JStyleTextChar` 的语料记录走 **version 3**（基类块 `sub_10003814`，`B = 26`），
`JStyleSimpleLine` 的走 **version 2**（基类块 `sub_10002CFC`，`B = 30`）。
基类块大小不同是因为调的是不同的基类辅助函数——**不是矛盾**，
但说明「基类块」不是一个固定长度，按 version 与调用的 helper 而定。

## 5. 现状：三个保真度字段全部到 native-reader 级

| 字段 | 位置 | 证据等级 |
|---|---|---|
| 字高 | `0x002C +42` (f64, 米) | **native-reader** |
| 线宽 | `0x002E +34` (f64, 米) | **native-reader** |
| 颜色 | `0x002E +50` (u32, `[R,G,B,0]`) | **native-reader** |

## 6. 仍未做的

1. `0x0030` **JStyleOverride 的读取序**（slot 17 → `sub_10029DE0`）——
   「样式覆盖还是标注锚点」的质疑仍开着。
2. **基类块本身**（`sub_10002CFC` / `sub_10003814`）没读，所以样式 id 在 `+14`
   仍是语料观察。
3. **「几何 → 样式」的链路仍未打通**——知道颜色在哪，不等于知道哪条线用哪个颜色。
   这是接进 OCS 之前的最后一道关。

## 7. 复现

IDA 开 `dlls/style.dll.i64`，反编译 `0x1002DF50`。
