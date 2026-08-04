# `style.dll`：类链已打通，读取序未解

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**部分完成**——CLSID → 工厂 → 类 → vtable 全链打通且带 C++ 符号；序列化读取序**未解**。
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

## 4. 做到的（三）：CLSID → 工厂 → 类 → vtable 全链打通

走 RTTI 反推 vtable 没走通（type descriptor 的 xref 只到 Class Hierarchy Descriptor
与 Base Class Array）。**换成从 `DllGetClassObject` 正着走，一次就通。**

`DllGetClassObject_0`（`0x1000F2D0`，2037 字节）是一长串 CLSID 比对，每命中一个就调
对应的工厂函数。`.rdata` 的 **文件偏移 → VA 差值恒为 `0x10001000`**，所以之前在文件
里定位到的 CLSID 直接对上工厂分支：

| type code | CLSID | 常量 VA | 工厂 |
|---|---|---|---|
| `0x002C` | `47FCC333` | `0x1009DDB4` | `sub_10003382` → `sub_10009120` |
| `0x002E` | `47FCC335` | `0x1009DE18` | `sub_10001212` |
| `0x0030` | `47FCC338` | `0x1009DEF4` | `sub_1000286A` |

三个 CLSID 常量已逐字节核对。工厂体证实了身份：

```c
_DWORD *__thiscall sub_10009120(_DWORD *this, int a2) {
  *(this + 3) = a2;
  *this = &ClassFactory<JStyleTextChar>::`vftable';   // <-- 类名在符号里
  ...
}
```

**这份 DLL 带完整 C++ 符号**，vtable 全部有名字：

| 符号 | VA |
|---|---|
| `??_7JStyleTextChar@@6B@` | `0x100A1D1C` |
| `??_7IJStyleTextCharImp@JStyleTextChar@@6B@` | `0x100A1C4C` |
| `??_7JStyleSimpleLine@@6B@` | `0x100A18F8` |
| `??_7IJStyleSimpleLineImp@JStyleSimpleLine@@6B@` | `0x100A1854` |
| `??_7JStyleOverride@@6B@` | `0x100A1580` |
| `??_7IJStyleOverrideImp@JStyleOverride@@6B@` | `0x100A14A8` |

读出的 vtable 前若干槽，`JStyleTextChar` 与 `JStyleOverride` **共享一段相同的函数
指针**（`0x10003738` `0x1000370B` `0x10003F12` … `0x10003071` `0x1000196F`
`0x100033E6` `0x1000306C` `0x1000196A` `0x100033E1`），那是 `JStyleBase` / `JStyleR2d`
的基类虚函数——**持久化的 Load/Save 就在这批共用槽里**。

## 5. 下次从哪继续（入口已经很窄了）

1. 逐个反编译 `JStyleTextChar` vtable（`0x100A1D1C`）与 `JStyleOverride` vtable
   （`0x100A1580`）**共用**的那几个槽，找出读流的那一个。
2. 拿到 Load 之后，逐字段核对语料侧的结论：`0x002C +42` 是否字高、
   `0x002E +34/+50` 是否线宽与颜色、`0x0030` 覆盖值在哪。
3. 一并回答 `JStyleOverride` 到底是样式覆盖还是标注锚点
   （`2026-08-04-jstyleoverride-points-at-geometry.md` §5）。

## 6. 现状小结

- **族身份**：CLSID 表 / jutil 注册表 / RTTI / 类工厂**四条路互证**，可当结论。
- **字段定位**（字高 `0x002C +42`、线宽 `0x002E +34`、颜色 `0x002E +50`、
  override 几何指针 `0x0030 +50`）：**仍是 corpus-statistical**，本轮没提到
  native-reader——但入口已经收窄到「反编译两张 vtable 的共用槽」这一步。
