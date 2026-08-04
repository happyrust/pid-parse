# `style.dll` native reader 复核：部分完成

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**部分完成**——类清单得到独立佐证，序列化读取序**未找到**。
> 前置：`2026-08-04-jstyleoverride-points-at-geometry.md`

## 1. 做到的（一）：`style.dll` 已就位

`D:\pid\RADInstallA~\style.dll`（816 KB）已复制到 `dlls/style.dll` 并在 IDA 里建库
（`dlls/style.dll.i64`，imagebase `0x10000000`，auto-analysis 完成）。下次可直接开。

## 2. 做到的（二）：RTTI 独立佐证了类清单

从 PE 里抽 RTTI type descriptor，得到 61 个类。与样式相关的：

```
JStyleBase        JStyleR2d          JStyleDescriptor   JStyleLib
JStyleOverride    JStyleSimpleLine   JStyleSimpleFill   JStyleHatchFill
JStyleSimpleDashType   JStyleDashType     JStyleTextChar     JStyleTextPara
JStylePointSymbol      JStyleLinePointGen JStyleLineSolid    JStyleLineTerminator
JStyleBitmap      JStyleFillType     JStyleSymbolFill   JStyleMultiplexer
JStyleUnits       JStyleWorkshop     JStyleOLG          JStyleSimpleMultiplexer
```

**这是一条完全独立的证据链。** 此前的 type code → 类名映射是走
「radsrvitem CLSID 表 → jutil 注册表」得到的；RTTI 是从 style.dll 自己的编译产物里
读出来的，两条路互不依赖，**给出同一份类清单**：

| type code | CLSID 路线得到的名字 | RTTI 里的类 |
|---|---|---|
| `0x002A` | JSL Simple Fill Style | `JStyleSimpleFill` |
| `0x002B` | JSL Hatch Fill Style | `JStyleHatchFill` |
| `0x002C` | JSL Text Character Style | `JStyleTextChar` |
| `0x002D` | JSL Text Paragraph Style | `JStyleTextPara` |
| `0x002E` | JSL Simple Line Style | `JStyleSimpleLine` |
| `0x002F` | JSL Simple Dash Type Style | `JStyleSimpleDashType` |
| `0x0030` | JSL Override Style | `JStyleOverride` |

七个逐一对上。**`0x002A..0x0030` 的族身份可以当结论用了。**

RTTI 里还有 `JStyleR2d` 与 `JStyleBase`，与 Phase 16 记录的继承链
`JStyleOverride → JStyleR2d → JStyleBase` 一致。

## 3. 一处必须记下的更正

Phase 16 的注释写：

> the IDA Version-3 schema (`style.dll!sub_1000F030`) writes the payload as
> `4 × u32 + 4 × f64 + 3 × u32 + 2 × u16`

**这个地址在本机这份 `style.dll` 里不对。** `0x1000F030` 落在 `sub_1000F020` 内部，
那是一个引用计数释放函数（`if (Block[1] == 1) { ...; return 0; } else --Block[1];`），
与序列化无关。

说明 Phase 16 用的是**另一个 build** 的 style.dll（`D:\pid\` 下同时有
`RADInstallA~` 与 `RADInstallPatchA~` 两份）。**裸地址不可跨 build 复用**，
引用时必须连带 build 标识，或改用 RTTI / CLSID 这类稳定锚点定位。

## 4. 没做到的：序列化读取序

试过三条路，都没在合理时间内走通：

1. **RTTI → vtable**：type descriptor 的 xref 指向 Class Hierarchy Descriptor 与
   Base Class Array，没能顺到 Complete Object Locator 再到 vtable；按 COL 地址
   反查 4 字节指针也无命中。
2. **Phase 16 的地址**：见第 3 节，build 不符。
3. **CLSID 表**：`style.dll` 文件偏移 `0x9CD50..0x9CF60` 有一张 20 字节条目的 GUID
   数组（GUID 16 + u32 0），`47FCC331`…`47FCC339` 连号在内，但**只有 GUID、
   没有工厂函数指针**，到不了类实现。

## 5. 下次从哪继续

- IDB 已建好，直接开 `dlls/style.dll.i64`。
- 建议入口：从 `DllGetClassObject` 导出函数往下走，那是 COM 类工厂的唯一入口，
  必然能到每个 CLSID 对应的构造与 vtable；比 RTTI 反走更直接。
- 目标仍是三个问题：`0x002C` 的 `+42` 是否确为字高、`0x0030` 覆盖值的位置、
  以及 `JStyleOverride` 到底是样式覆盖还是标注锚点
  （`2026-08-04-jstyleoverride-points-at-geometry.md` §5）。

## 6. 现状小结

语料侧的字段定位（字高 `0x002C +42`、线宽 `0x002E +34`、颜色 `0x002E +50`、
override 的几何指针 `0x0030 +50`）**证据等级仍是 corpus-statistical**，
本轮没有把它提到 native-reader。族身份则已经是双路独立佐证，可以当结论。
