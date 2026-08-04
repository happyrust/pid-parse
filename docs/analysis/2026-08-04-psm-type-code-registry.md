# PSM `type_code → CLSID → 类名` 表已定位

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**已坐实**（多锚点求解 + 七个独立已知项交叉验证）
> 工具：`tools/psm_type_clsid.py`、`tools/clsid_registry.py`

## 1. 表在哪

`radsrvitem.dll!dword_5667B068`（文件偏移 `0x23A468`），**每项 20 字节**：
16 字节 GUID + 4 字节未解释，**按 PSM type code 索引**。

类名来自 `jutil.dll` 的 RAD CLSID 注册表（96 字节条目：GUID 16 + 模块 16 + 友好名）。
本机在 `D:\pid\RADInstallA~\jutil.dll`，`dlls/` 里没有。

## 2. 为什么这次可信（上一次不可信）

**上一次的错法**：用单个控制项 `0x0030` 反推基址、再用同一个控制项验证。这是循环
论证——基址就是从它推的。当时还猜错了步长（4 而不是 20），错了五倍，而「验证」照样
通过。发现问题靠的是查一个独立已知项：`0x004D` 应该是文本框，结果查出
`JSL Convert Hole Processor Solid Fill Style`。

**这次的做法**：用**三个互相独立**的已知 `(type code, 类)` 对**联立求解** `(base, stride)`：

| type code | 类 | 出处 |
|---|---|---|
| `0x0013` | `Boundary2d Object` | radsrvitem 自己的 `ig*` 类表 `sub_56448F70` |
| `0x0030` | `JStyleOverride` | Phase 16，经 jutil 注册表 |
| `0x003D` | `SmartFrame2d Object` | radsrvitem 自己的 `ig*` 类表 |

三个锚点只有**唯一解** `(base=0x23A468, stride=20)`。

**再交叉验证**：解出来之后，五个 pid-parse 已独立解码的族全部对上：

| type code | pid-parse | RAD 类名 |
|---|---|---|
| `0x0018` | igLine2d | **Line Object** |
| `0x004D` | igTextBox | **Text Object** |
| `0x005E` | igPoint2d | **Point Object** |
| `0x0084` | igLineString2d | **LineString Object** |
| `0x00CE` | igSymbol2d | **JSymbol**（`symbol.dex`） |

七个独立已知项全中，不存在循环。

## 3. 全表（本项目关心的部分）

| type code | CLSID | 模块 | 类名 |
|---|---|---|---|
| `0x0006` | `104D3C70-FB1E-11CD-812F-080036C6FD01` | imagdex.dex | OnElement Constraint |
| `0x0007` | `13584FA0-41DF-11CE-BB8E-08003601BB4E` | docext.dex | JSheetSetup Object |
| `0x0010` | `1D1928C0-0000-0000-C000-000000000046` | imagdex.dex | JDim Object ※ |
| `0x0013` | `1EB2FA20-D1AD-11CE-A9B0-08003601B487` | imagdex.dex | Boundary2d Object |
| `0x0018` | `2D4E13C0-D3D1-11CD-8AEA-08003601B44A` | imagdex.dex | Line Object |
| `0x0020` | `3643B830-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | Rectangle Object |
| `0x0021` | `3750D460-90EB-11CE-976E-08003601E26D` | imagdex.dex | ComplexString Object |
| `0x0030` | `47FCC338-2D0F-11D0-A1FF-080036A1CF02` | style.dll | JSL Override Style |
| `0x003D` | `5B552E30-7C2D-11CE-A80E-08003601DADA` | smrtfrm.dex | SmartFrame2d Object |
| `0x004D` | `777A6860-3C8F-11B9-C000-4ECAE2741999` | imagdex.dex | Text Object |
| `0x0059` | `902AD280-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | Circle Object |
| `0x005A` | `9196D9D1-E94A-11CF-8094-080036CE6C02` | **style.dll** | **JSL Style Librarian** |
| `0x005D` | `94834300-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | BspCurve Object |
| `0x005E` | `98F8BC10-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | Point Object |
| `0x0061` | `9D650A00-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | Arc Object |
| `0x0063` | `A3494010-D3E1-11CD-8AEA-08003601B44A` | imagdex.dex | Ellipse Object |
| `0x0077` | `CA3B3C60-0D7D-11CE-812F-080036C6FD01` | imagdex.dex | **Fix Constraint** |
| `0x007B` | `DA02A6D0-C991-11CD-B02F-08003601BE3A` | imagdex.dex | Group implementation |
| `0x007E` | `DE359E40-1278-11CE-976E-08003601E26D` | imagdex.dex | Elliptical Arc Object |
| `0x0084` | `F875B4A0-D97A-11CD-8AEA-08003601B44A` | imagdex.dex | LineString Object |
| `0x0085` | `FAFFE580-0259-11CE-AD7A-0800365FFA01` | imagdex.dex | Vertical Constraint |
| `0x00CE` | `719C2A5E-B6B5-11CE-B656-080036D72102` | symbol.dex | JSymbol |
| **`0x00FA`** | `24D10655-0917-11D1-BC33-08003609D002` | imagdex.dex | **Dependency Object** |
| **`0x00FF`** | `DD249E56-2A0C-11D1-BC36-080036BACB02` | imagdex.dex | **Graphics Bag** |

※ `0x0010` 与 `0x0115` 查出同一个 GUID，且该 GUID 带 `C000-...-46` 这种 OLE 尾缀。
**这一项存疑**，需要单独核；不要当结论用。

## 4. 四个直接影响

**（1）`0x00FA` 不是 GraphicGroup，是 Dependency Object。**
它是全语料出现最多的族（458 次），pid-parse 现有的
`SheetGraphicGroupDecoded` / `PSM_TYPE_CODE_GRAPHIC_GROUP` /
`decode_graphic_groups` 命名基于一个未经证实的假设，应当改名。它带两个 OID 引用
（payload `+22` / `+34`）也就顺理成章了——**那是一条依赖关系的两端**。

连带后果：`2026-08-04-graphicgroup-tail-property-block.md` 第 3–5 节把尾部四个小整数
词读作「每对象显示属性」的假设**大概率不成立**，更可能是依赖元数据。该文已加修正头。

**（2）`0x005A` = "JSL Style Librarian"（style.dll）。**
字高的阻塞点从此有了确切身份：不是一个匿名 cluster，而是 style.dll 里的样式库对象。
线宽/颜色的候选在排除几何图元与 `0x00FA` 之后，**也收敛到这里**——两个缺口很可能
共用一个答案。

**（3）`0x00FF` = "Graphics Bag"。**
此前只知道它通过图形谓词、语料 0 命中、类表无名。现在有名字了，可以纳入
`2026-08-04-annotation-families-risk.md` 的告警清单并写清楚它是什么。

**（4）标注族有了 RAD 侧名字**：`0x0115` = JDim Object、`0x0117` = JBalloon Object、
`0x0118` = JLeader Object。与 `ig*` 类表的 igDimension / igBalloon / igLeader 对应。
（这三项落在 `0x0010` 存疑区的同一 GUID 家族里，可信度低于第 2 节那七项，
写解码器前需另行核实。）

## 5. 复现

```powershell
python tools/psm_type_clsid.py            # 默认关心的那批
python tools/psm_type_clsid.py 0x5A 0xFA  # 指定 type code
python tools/clsid_registry.py --grep "Constraint"
python tools/clsid_registry.py --self-test
```

`psm_type_clsid.py` 每次运行都重新联立求解 `(base, stride)`，锚点对不上就直接报错，
不会静默给出错误答案。
