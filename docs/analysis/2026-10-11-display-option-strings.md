# 显示选项字符串：没有一条接到 unitsCtx / 渲染器配置

**判定（P-D1 (b)）：封死。** 点名的七个库里，和「显示尺寸」有关的注册表路径、INI 键、自动化属性、对话框资源，没有任何一条被代码读进 `unitsCtx` 或渲染器配置。有代码 xref 的那些，写进的是翻译 INI 路径、图纸页偏好、调试开关或调试打印。

命中（下面总表的设置/路径/属性/资源行，不含 RTTI 与导入符号）：七库 28 条；另记 SmartSketch 偏好与 MicroStation 导入页 8 条，对话框资源 9 条。七库里代码 xref 非 0 的 14 条，没有一条的落点是 `unitsCtx`。

> 上游已查、本表纳入、不重复追调用链：U5 `2026-10-11-units-context-source.md` §5（`Write Polyline Width Threshold` 只在翻译选项目录 `0x10023DAC` / `0x10025C1C`，无代码 xref）；U2 `2026-10-11-dash-scale-writer.md` §4.4（`style` / `j2dsrv` / `render` / `viewfil` 里涉及 `+24` 的函数无注册表 / 选项字符串引用）。
> 库：`D:\ida_scratch\u7c\` 下的临时副本。`imagdex` / `j2dsrv` / `render` / `style` / `docext` 从 `D:\ida_scratch\units-ctx\` 拷来；`viewfil` / `radsrvitem` 从 `D:\ida_scratch\dash-scale\` 拷来（units-ctx 没有这两份）。原 i64 未打开。`skpref.dll` / `mstnopt.dll` 从 `D:\pid\RADInstallA~` 拷到 `u7c` 后新建临时库。地址均是这些副本里的 VA。

## 0. 做法

IDA 9.2 批处理（`ida.exe -A -S`，`idautils.Strings` + `XrefsTo`），脚本 `D:\ida_scratch\u7c\ida_xref.py`。代码 xref = `XrefsTo` 的 `frm` 落在函数内；数据 xref = 其余。正则覆盖 Line Width / LineWidth / Width / Linetype / Dash / Scale / Pixel / DPI / Zoom / Paper / Display Units / True Size / Actual Width / `Software\Intergraph` / Option / Threshold。`Read Default Width` 这类不含 “line” 的键用第二遍正则 `width|sheetscale|sheetpaper|paperunit|…` 补上。

没有现成 i64 的（`jeditdlg.dll`、`control.dll`、`sppidcommonen.dll`、`pidoptions.dll`、`llama.dll`、`skpref.dll`、`mstnopt.dll`）先做 PE 上的 ASCII / UTF-16 字节扫描。`skpref` / `mstnopt` 命中了带代码引用的键，才在 `u7c` 临时副本上建库反编译读者。不走 ida-bridge / idalib，不跑 cargo，不开 SmartPlant。

`D:\pid\RADInstallA~` 与同级 `RADInstallPatchA~` 下列过文件名：没有 `llama*` / `sppid*` / `pid*` 且名字里带 Render / Display / View 的二进制。`ctlDisplayBrowser.dll`（7 KB）关键字 0 条。SmartPlant 自家 DLL 在 `pid-parse/dlls/`（`llama.dll`、`pidoptions.dll`、`sppidcommonen.dll` 等），按字节扫，不建库。

`D:\pid\RADInstallA~\j2dsrv.dll`（186 880 字节）和 i64 对应的 `pid-parse/dlls/j2dsrv.dll`（189 440 字节）不是同一份构建。键名相同，VA 不同（安装目录里 `Write Polyline Width Threshold` 在 `0x10022618`，i64 在 `0x100243A0`）。下表用 i64 的地址。

## 1. 总表

「接到渲染配置」= 读者把值写进 `unitsCtx`（`+0/+8/+16` 因子或 `+24/+40/+48/+56` 像素模式）或 `render.dll` 的渲染配置对象。全部为否。

### 1.1 `j2dsrv.dll` — 翻译 INI 目录 `Options`

记录在 `.rdata`，步长 `0x10`：`+0` 节名、`+4` 键名、`+8` 缺省。第一份从 `0x10023CF8` 起，节名指针都是 `0x10024284` `"Options"`（该字符串数据 xref 92、代码 xref 0）。第二份整体平移到 `0x10025B68` 一带。缺省槽 `0x10023CF3` 是空串。

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref（两份目录的槽） | 接到渲染配置 |
|---|---|---|---|---|---|
| `Options` | `0x10024284` | INI 节名 | 0 | 92 | 否 |
| `Read Default Width` | `0x10024324` | INI 键 | 0 | 2：`0x10023D5C`、`0x10025BCC` | 否 |
| `Write Polyline Width Threshold` | `0x100243A0` | INI 键 | 0 | 2：`0x10023DAC`、`0x10025C1C` | 否 |
| `Read Widths` | `0x100248C4` | INI 键 | 0 | 2：`0x100240B8`、`0x10025F28` | 否 |
| `Write Widths` | `0x100248D4` | INI 键 | 0 | 2：`0x100240C8`、`0x10025F38` | 否 |
| `Metafile to Raster DPI resolution` | `0x10024630` | INI 键 | 0 | 2：`0x10023F0C`、`0x10025D7C` | 否 |
| `Read Linestyles` | `0x10024708` | INI 键 | 0 | 2：`0x10023F78`、`0x10025DE8` | 否 |
| `Write Linestyles` | `0x1002471C` | INI 键 | 0 | 2：`0x10023F88`、`0x10025DF8` | 否 |
| `Read Default Units` | `0x1002430C` | INI 键 | 0（字符串本身）；槽被两处代码引用，见 §2 | 2：`0x10023D4C`、`0x10025BBC` | 否 |
| `Write Default Units` | `0x100243C8` | INI 键 | 0 | 2 | 否 |
| `Sub Units Per Master Units` | `0x10024658` | INI 键 | 0 | 2 | 否 |
| `Pos Units Per Sub Units` | `0x10024678` | INI 键 | 0 | 2 | 否 |
| `Export All Graphics to PaperSpace` | `0x100246AC` | INI 键 | 0 | 2 | 否 |
| `Software\Intergraph\Applications\` | `0x1002B00C` | 注册表路径 | 2：`sub_1000E670`、`sub_1000E760` | 0 | 否 |
| `Software\Intergraph\Applications\%s\FileExtensions\%s\Translate\Ini` | `0x10026198` | 注册表路径 | 1：`sub_1000E760` | 0 | 否 |

同一目录里还有 Seed File、Template File、Bigfont Name 等约 40 个键，与显示尺寸无关，不逐条列入。`.text` 里对上述 Width / DPI / Linestyles 槽地址的立即数搜索为 0；唯一被 `.text` 引用的槽是 `Read Default Units`。

`trnopt.dll`、`mstnopt.dll` 里有同一套键名（字节扫描）。`mstnopt` 的 Width / Threshold 同样只有 `.rdata` 指针（各 4，代码 0）。`Ignore Sheet Scale` 例外，见 §1.8。

### 1.2 `docext.dex` — 图纸页偏好（自动化属性名，经 `SkPrefHelper`）

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref | 接到渲染配置 |
|---|---|---|---|---|---|
| `PaperUnits` | `0x1003F310` | 自动化属性 / 偏好键 | 2：`sub_100064E0`、`sub_10006AD0` | 0 | 否 |
| `PaperUnitsPrecision` | `0x1003F320` | 同上 | 2：`sub_10006610`、`sub_10006C50` | 0 | 否 |
| `SheetScaleOption` | `0x1003F38C` | 同上 | 2：`sub_100043B0`、`sub_10004D90` | 0 | 否 |
| `SheetSizeOption` | `0x1003F3A0` | 同上 | 2：同上 | 0 | 否 |
| `SheetWidth` | `0x1003F3C4` | 同上 | 2：同上 | 0 | 否 |
| `SheetPaperScale` | `0x1003F3D4` | 同上 | 2：同上 | 0 | 否 |
| `SheetScaleHasAlternateMeaning` | `0x1003F430` | 同上 | 2：同上 | 0 | 否 |

落点是 sheet 对象字段和偏好存储，不是 `unitsCtx`。U5 §4 已写明 `sub_100043B0` / `sub_100042F0` 拿的是单位类型 1 的图纸英寸（8.5、11）。本轮只补字符串 xref，不重开那条调用链。

### 1.3 `render.dll`

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref | 接到渲染配置 |
|---|---|---|---|---|---|
| `(Paper)` | `0x100913B4` | 调试/单位标签 | 3：`sub_10010040`、`sub_10057CB0`、`sub_10058000` | 0 | 否 |
| `Display Width: %.15g` | `0x100947A8` | 调试格式串 | 2：`sub_10056330`、`sub_10056610` | 0 | 否 |
| `Software\Intergraph\Applications\%s\PrefSets\Options-FileLocations` | `0x10091F30` | 注册表路径 | 2：`sub_10019490`、`sub_1001C5A0` | 0 | 否 |
| `Software\Intergraph\DEBUG` | `0x10092A4C` | 注册表路径 | 2：`sub_100220B0`、`sub_1002E4F0` | 0 | 否 |

同函数 `sub_10010040` 还打印 `m_widthVector`、`Width:`、`Minimum Width:`（`0x10091028` 起）。`GdipSetPenWidth` / `GdipSetPenDashArray` 是导入名，xref 0。没有 Line Width / DPI / True Size 键。

### 1.4 `style.dll`

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref | 接到渲染配置 |
|---|---|---|---|---|---|
| `Software\Intergraph\Applications\%s\Draft\Settings\NoLangIDValidate` | `0x100A1E70` | 注册表路径 | 0 | 1 | 否 |

Width 第二遍正则在 `style` 上命中 0。`Dash` 命中的是类型名 / RTTI（`JStyleDashType`、`JStyleSimpleDashType` 等）和 UTF-16 填充名 `Hatch Dash 1`…`Hatch Dash 18`，不是选项键。与 U2 §4.4 一致：涉及 `+24` 的函数不引用这些字符串。

### 1.5 `viewfil.dex`

设置类字符串 0 条。仅 3 个导入名，代码 xref 都是 0：`HGetDashStyle` `0x1002CC50`、两份 `HGetDashStyleName` `0x1002CC80` / `0x1002CCB2`。

### 1.6 `imagdex.dex`

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref | 接到渲染配置 |
|---|---|---|---|---|---|
| `Software\Intergraph\DEBUG` | `0x104AB20C` | 注册表路径 | 3：`sub_103C2F80`、`sub_103C7C00`、`sub_103C8810` | 2 | 否 |
| `software\intergraph\debug` | `0x104A6DB4` | 注册表路径 | 0 | 1 | 否 |

`CreateOverrideStructureWidthAndColor` `0x10557E7E` 是导入名，xref 0。没有 Width / DPI / Sheet 键。

### 1.7 `radsrvitem.dll`

| 字符串 | 地址 | 类型 | 代码 xref | 数据 xref | 接到渲染配置 |
|---|---|---|---|---|---|
| `SOFTWARE\Intergraph\Applications\` | `0x56660790` | 注册表路径 | 1：`sub_5646D810` | 0 | 否 |
| `SOFTWARE\Intergraph\SP3D\Core\Debug` | `0x56664FBC` | 注册表路径 | 2：`sub_56473790`、`sub_564738F0` | 4 | 否 |
| `Software\Intergraph\Applications\%s\Draft\PersistLogging` | `0x56665D98` | 注册表路径 | 1：`sub_5648A870` | 0 | 否 |
| `Software\Intergraph\Applications\%s\Draft\FileRepair` | `0x56666494` | 注册表路径 | 1：`sub_56490960` | 0 | 否 |
| `GTransf has an invalid scale` | `0x56665334` | 错误文本 | 1：`sub_56477CC0` | 0 | 否 |

`sub_56477CC0` 只在变换非法时把出参清 0 并打这条日志，不读选项。Width 正则另有一条几何错误串 `GRectangle2d: width vector is NULL`（`0x56668838`，`sub_5651B310`），同样不是设置。

### 1.8 七库之外、但是「应用设置」候选

这些字符串**不出现**在上面七个库里（ASCII 与 UTF-16 都搜过：`PixelDust`、`BorderLineWidth`、`Ignore Sheet Scale`、`Line Width`、`True Size`、`Actual Width`、`Width Display`、`Linetype Scale`、`Display Units`、`OptionDispSet`、`SP_BorderWidth`）。

| 库 | 字符串 | 地址 | 类型 | 代码引用 | 落点 |
|---|---|---|---|---|---|
| `skpref.dll` | `Options-View` | `0x1002EE58` | INI 节名 | 2：`SkPrefsView::SkPrefsView` `0x10021450` 等 | 拷进 `this+24`，作 `SkPrefHelper` 的节名 |
| `skpref.dll` | `PixelDustSlope` | `0x1002EE78` | INI 键 | 2：`GetPixelDustSlope` `0x10021610` | 写出参 `double*`；缺省用 `this+104` 或全局 `dbl_1004FE98` |
| `skpref.dll` | `PixelDustMaxWidth` | `0x1002EE8C` | INI 键 | 2：`GetPixelDustMaxWidth` `0x100215B0` | 写出参 `unsigned*`；缺省 `this+112` 或 `dword_1004FEA0` |
| `skpref.dll` | `BorderLineWidth` | `0x1002ED80` | INI 键 | 2：`SkPrefsTextBox::GetBorderLineWidth` `0x1001D390` | 写出参 `double*`；缺省常量 `0.00035` |
| `skpref.dll` | `BorderLineStyle` | `0x1002ED5C` | INI 键 | 2（`.text` `0x1001D31A`、`0x1001DC03`） | 文本框边线，与 `BorderLineWidth` 成对 |
| `skpref.dll` | `PaperSize` / `GraphPaper` | `0x1002E850` / `0x1002CDB8` | INI 键 | 各 2 | 打印/格纸偏好，未再追进渲染器 |
| `mstnopt.dll` | `Ignore Sheet Scale` | `0x1001A980` | INI 键 | 2：`sub_10006780`、`MstnImptOptSheetOpen_0` `0x10008B30` | 读进导入选项页 `this+61`；保存时 `WritePrivateProfileStringA("Options", …)` 写回 INI。缺省 1 |

`PixelDust*` 是视图定位的像素灰尘，`BorderLineWidth` 是文本框边宽。构造函数 `SkPrefsView::SkPrefsView` 把这两个值缓存进全局，仍然留在 `skpref` 里。七个渲染库不引用这些键名。

## 2. 有代码 xref 的读者，值写到哪

只列设置/路径，不列 RTTI。

| 读者 | 读什么 | 写到哪 |
|---|---|---|
| `j2dsrv` `sub_1000E760` `0x1000E760` | `HKLM\…\FileExtensions\%s\Translate\Ini`（以及 CLSID / ReadData / WriteData） | `Reg` 读出的路径拷进出参 `a3`（INI 文件名）。不碰 `unitsCtx` |
| `j2dsrv` `sub_1000E670` `0x1000E670` | 同上 CLSID 子键 | 只 `RegOpenKeyExA` 探键是否存在，读完即关。无尺寸值 |
| `j2dsrv` `sub_1000D2F0` `0x1000D2F0` | `GetPrivateProfileIntA("Options","Read Default Units",…)` | 非 0 时 `ConvertUnitToDbu(1, 1.0, this+105, …)`，`this` 是 `double*`，落在对象偏移 840。U5：这是翻译选项，不是显示用的 `unitsCtx` |
| `j2dsrv` `sub_10006D00` `0x10006D00` | 同一键（槽 `0x10023D4C`，指令 `0x10007361`） | 整数作为单位码交给翻译服务器接口槽 `+12`（同时传入 `1.0`）。不是 `unitsCtx` |
| `render` `sub_10019490` / `sub_1001C5A0` | `HKCU\…\PrefSets\Options-FileLocations` 下由调用方传入的值名 | `lstrcpyA` 到出参 `lpString1`。文件位置字符串，不是比例 |
| `render` `sub_100220B0` | `HKCU\Software\Intergraph\DEBUG` / `RENDER_TIMED_BLT_INTERVAL` | `JRender` 对象 `this+1353`（旁边 `+1352..+1358` 是计时缺省）。不是线宽 |
| `render` `sub_1002E4F0` | 同键 / `RENDER_RECORD_UPDATE_SECONDS` | `JRenderRec` `this+553`。记录刷新秒数 |
| `render` `sub_10056330` | 格式串 `Display Width: %.15g` | `fprintf` 文本行对象 `v4+56`。只打印，不读注册表 |
| `imagdex` `sub_103C2F80` | `DEBUG` / `JTEXT_NEW_SERVICES` | 静态 `Data`，文本服务开关 |
| `imagdex` `sub_103C7C00` | `DEBUG` / `JTEXT_DBG_LOGMODE` | 静态 `dword_10552250`，日志模式 |
| `imagdex` `sub_103C8810` | `DEBUG` / `JTEXT_DBG_LOGFILE` | 打开 `c:\temp\JText.log`。无尺寸 |
| `radsrvitem` `sub_5646D810` | `SOFTWARE\Intergraph\Applications\<app>.Application\TaggedTxtModules\…\InProcServer32` | 查询到的 DLL 路径交给 `sub_5646CB10`。COM 插件枚举 |
| `docext` `sub_100043B0` / `sub_10004D90` | `SheetScaleOption`、`SheetPaperScale`、`SheetWidth` 等 | `SkPrefHelper::Get/SetPref*`；对象字段 `+64`（比例选项）、`+80`（纸张选项）、`+88`（宽）、`+104/+112`（纸/模型比例，经单位类型 1 换算） |
| `docext` `sub_100064E0` 等 | `PaperUnits` / `PaperUnitsPrecision` | `Get/SetPrefLong`，图纸单位码 |
| `skpref` `GetPixelDustSlope` / `GetPixelDustMaxWidth` | `Options-View` 下两个键 | 出参；构造函数再缓存到 `dbl_1004FE98`、`dword_1004FEA0` |
| `skpref` `GetBorderLineWidth` | `BorderLineWidth` | 出参 `double*`，缺省 `0.00035` |
| `mstnopt` `sub_10006780` | `Ignore Sheet Scale`（缺省 1） | 导入选项页 `this+61`。`MstnImptOptSheetOpen_0` 用 `WritePrivateProfileStringA` 写回同一 INI |

`Read Default Width`、`Write/Read Widths`、`Write Polyline Width Threshold`、`Metafile to Raster DPI resolution`、`Read/Write Linestyles` 没有读者函数：字符串 xref 与槽地址的 `.text` 立即数都是 0。

## 3. 用户可见的显示选项

`jeditdlg.dll`：`.rsrc` 里长度 ≥ 6 的 UTF-16 只有 27 条，全是版本信息（`jEditDlg`、`Hexagon PPM`、`09.02.00.0102`）。没有 Line / Width / Scale / Dash。

`control.dll`：没有对话框字符串 “Line Width” / “线宽显示” / “线型比例”。命中的是 `CDashTypeBitmapGenerator` 的修饰名和 UTF-16 `Hatch Dash %d`、`FillDash`。U2 已写明线型预览位图走 `CreateOverrideStructure(…, 4, scale)`，那是预览，不是本轮要找的应用设置键。

`sppidcommonen.dll`（image base `0x547F0000`）对话框资源，UTF-16：

| 字符串 | 地址 | 旁边是什么 |
|---|---|---|
| `Line Color` / `Line Type` / `Line Width` | `0x547FA0E0` / `0x547FA0F6` / `0x547FA10A` | 紧挨 `Style`，后面是 `Colors`、`Widths`。属性/图例标签，不是注册表键 |
| `Custo&m scale:` / `Select sc&ale:` / `&Coincident (1:1)` / `Scale reference files` | `0x547F6CFA` / `0x547F6CDC` / `0x547F6CB8` / `0x547F6D64` | 后面是 `1/4 = 1'0"` 等图纸比例和 `A (8.5in x 11in)` 纸张。导出/参照比例对话框 |
| `Brea&k-away:` + `pixels`；`L&ocate:` + `pixels` | `0x547F6B70`、`0x547F6B8A`、`0x547F6B98`、`0x547F6BAA` | 定位容差，单位是像素 |
| `Zoom` | `0x547F8EE4` | 与 `Off`、`Pan` 并列，滚动模式 |
| `Dashed` | `0x547F4ED2` | 孤立标签 |
| `Options Manager` / `Options` | `0x547F4754` / `0x547F7186` | 对话框标题 |

`sppid.dll` 有命令名 `ToolsOptions.ToolsOpts`（UTF-16，`.text`）。打开的是上面这组资源，不是一条写进 `unitsCtx` 的键。

`pidoptions.dll` / `llama.dll` / `smartplantpid.exe` 里和 “display” 沾边的名字是数据库属性，不是屏幕比例：

- `OptionDispSet` / `OptionDispSetFolder`：与 `OptionSymbology`、`OptionAutoGap`、`OptionHeatTrace` 列在一起。
- `SP_BorderDashTypeName`、`SP_BorderWidth`、`SP_BorderColor`、`SP_IsBorderVisible`：图框边线。
- `llama.dll` 的注册表只有 `Software\Intergraph\Applications\SmartPlantPID.Application\Version` 和 `SPPIDAutomation.Application\Version`。

这些字符串不在七个渲染库里。没有为 `llama.dll`（7.4 MB）建库。

## 4. 判定

**P-D1 (b)「来自应用设置」封死。**

理由：能改参考图上虚线节距 / 线宽显示放大的那条路，必须有一个选项字符串被读进 `unitsCtx` 或渲染器配置。七个库里不存在这样的字符串。

- 尺寸相关的 INI 键全在 `j2dsrv` 的 ImagView **翻译**目录里，而且 Width / DPI / Linestyles 连代码引用都没有。目录里唯一被代码读的尺寸相邻键是 `Read Default Units`，写到翻译对象偏移 840 或翻译服务器的单位码参数。
- 真正有代码 xref 的「比例」是 `docext` 的图纸页偏好（`SheetScaleOption`、`SheetPaperScale`）和 `mstnopt` 的 `Ignore Sheet Scale`。前者落在 sheet 字段，单位类型 1；后者落在 MicroStation 导入选项页再写回 INI。都不是 `unitsCtx` 的因子，也不是 `tagStyleOverride+24`。
- `skpref` 的 `PixelDust*` / `BorderLineWidth` 有读者，但键名不进入七个渲染库，读者把值留在偏好对象和出参里。
- 用户能在界面上看到的 “Line Width” / “Line Type” / “Custom scale” 是 `sppidcommonen.dll` 的对话框资源。七个库不引用这些资源字符串。
- 与 U2 §4.4、U5 §5 一致，本轮把当时没点名的 `Read Default Width`、`Read/Write Widths`、`PixelDust*`、`Ignore Sheet Scale`、对话框 “Line Width” 补进同一结论。

没做：不追 `SkPrefHelper` 的存储介质（注册表还是 INI 文件）最终文件名；不把 `skpref` 全局 `dbl_1004FE98` 的跨 DLL 调用方再打开一遍（七库内没有这个键，也没有这几个导出名的字符串引用）。那是「别的模块用数值调用」，不是「应用设置字符串接到渲染配置」。

## 5. 验证

- IDA 批处理七个副本各两遍（宽正则 + width 正则），退出码 0，CSV 在 `D:\ida_scratch\u7c\`。反编译：`j2d-decomp.txt`、`j2d-units2.txt`、`render-decomp.txt`、`render-decomp2.txt`、`imagdex-decomp.txt`、`imagdex-decomp2.txt`、`rs-decomp.txt`、`skpref-decomp.txt`、`mstn-decomp.txt`。
- 原 i64 未打开，尺寸与开跑前一致：`units-ctx` 的 `docext.i64` 1 018 076、`imagdex.dex.i64` 95 335 798、`j2dsrv.i64` 578 604、`render.dll.i64` 14 014 966、`style_pp.i64` 15 572 307；`dash-scale` 的 `viewfil.i64` 575 234、`radsrvitem.i64` 37 098 098。`pid-parse/dlls/*.i64` 未打开。
- `NOT RUN`：cargo / 测试 / 构建。只读，未改 `src`、golden、计划文件。未启动 SmartPlant。
