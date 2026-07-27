# IDA: `ugeom2d1.dll` 曲线族流读取器 —— 弧角语义坐实 + 层级归属澄清

> 日期：2026-07-27
> 范围：用 idalib（headless）分析 `dlls/ugeom2d1.dll`，反编译其
> `IMG<族>ReadFromStream2d` 读取器，回答 Phase 36 遗留的弧角语义问题，并
> 判定 `ugeom2d1` 是否是 `.pid/.sym` PSM `ig*` 记录（`0x0059`/`0x0061` 等）
> 的解码层。结论：**弧角为绝对起止角（符号级坐实）**，但
> **`ugeom2d1` 是几何数学层，不是 `ig*` PSM 记录层**。
> 只读逆向，未改 parser/schema/model。

## 工具与产物

- `tools/idalib_curve_probe.py`：`open_database(run_auto_analysis=True)` 分析
  `ugeom2d1.dll`（1176 函数 / 2932 名字），生成并保存 `dlls/ugeom2d1.dll.i64`。
- `tools/idalib_curve_decompile.py`：复用 `.i64`（秒开）反编译目标读取器。
- `tools/idalib_igreader_scan.py`：在 `sppid/core/radsrvitem` 三个已存在 `.i64`
  中扫描 `ig*` 类名与 `*2d` 序列化动词。

环境沿用 `2026-07-27-pid-load-status-snapshot.md` §5：IDA Professional 9.2 +
`idapro`(idalib) 0.0.8，默认 Python 3.12 可直接 `import idapro`。

## 结论 1：弧 = 椭圆基类 + 绝对起止角（native reader 坐实）

`ugeom2d1.dll` 的每个几何族都有具名流读取器
`IMG<族>ReadFromStream2d`（`IMGLine/IMGArc/IMGEllipse/IMGLineString/IMGBspCurve/`
`IMGComplexString/IMGFitCurve/IMGRectangle/IMGBoundaries/IMGTypedCurve/...`）。
读取通过对象 vtable 偏移 `+12` 的顺序 read 回调 `(handle, dest, size, &got)` 逐段消费。

**`IMGEllipseReadFromStream2d` @ `0x10035c10`** 的流序（struct 偏移即消费顺序）：

| stream 偏移 | 字节 | 字段 |
|---|---|---|
| `+0` | 16 | `center`（2×f64 = x, y） |
| `+16` | 16 | `major_axis` 向量（2×f64） |
| `+32` | 8 | `minor/major ratio`（1×f64） |
| `+40` | 1 | 1 字节标志 |

合计 41 字节。字段名由 `IMElVal2dInfo` 与同库
`IMEllipseByCtrMajRatOr2d` / `IMEllipseGetMinorMajorRatio2d` 佐证。

**`IMGArcReadFromStream2d` @ `0x10028780`** 先调用
`IMGEllipseReadFromStream2d` 读上面 41 字节，再多读两段：

| stream 偏移（续） | 字节 | 字段 |
|---|---|---|
| ellipse 之后 +0 | 8 | `start_angle`（1×f64，struct `a2+48`） |
| ellipse 之后 +8 | 8 | `end_angle`（1×f64，struct `a2+56`） |

即 `GArc2d = GEllipse2d(41B) + start_angle(8B) + end_angle(8B) = 57B`。

**ASCII 调试读取器 `IMRead_GArc2d` @ `0x10068980` 把字段名写死**：它先扫到
`"GArc2d{"`，调用 `IMRead_GEllipse2d`，再分别按字面量 `"startAngle"` /
`"endAngle"` 读两个 `%lf`，最后扫 `"}GArc2d"` 收尾。**字段字面就叫
`startAngle` / `endAngle`，两者都是绝对角，不是「起始角 + 扫掠角」。**

这与 Phase 36（`2026-07-26-phase36-sym-symbol-library.md`）纯靠 `ElecTraceLine.sym`
两弧几何自洽推出的「绝对起止角」结论一致 —— 现在有了符号级证据，
弧角语义可从 `corpus-statistical` 升级。

`IMGLineStringReadFromStream2d` @ `0x10036e00`：读 count(4B) + 2 个 1 字节标志，
再读 `16 * count` 字节 = 每顶点 16B（2×f64）。与 Phase 36 的
`(btf-24)/16` 顶点、每顶点 16 字节一致。

## 结论 2：`ugeom2d1` 是几何数学层，不是 `ig*` PSM 记录层

`ugeom2d1` 的弧按**椭圆式**存：`center + major_axis 向量 + 轴比`。
而 `.pid/.sym` 的 PSM 记录按 Phase 36 实测是**圆式**：

| | 圆心 | 半径 | 起/止角 | 尾 |
|---|---|---|---|---|
| `ugeom2d1` GArc2d（57B stream） | 2×f64 | major_axis 向量(2×f64) + ratio(f64) | 2×f64 | 1B flag 在中间 |
| `.sym` `0x0061`（payload 59） | 2×f64 | **标量 radius**(1×f64) | 2×f64 | +1B |
| `.sym` `0x0059` igCircle2d（payload 43） | 2×f64 | **标量 radius**(1×f64) | — | +1B |

布局不同（向量+比 vs 标量半径），故 **`ig*` 记录不是经
`ugeom2d1::IMG*ReadFromStream2d` 序列化的**。`ugeom2d1` 只回答了「弧角是绝对
起止角」这个语义，不能直接把 PSM `0x0059`/`0x0061` 升级为 `ida-proven`。

## 下一个 IDA target：`radsrvitem.dll` 的 `ig*` 读取器

`tools/idalib_igreader_scan.py` 在三个已分析 DLL 上扫描：`sppid.dll`（158 函数）
与 `core.dll`（34166 函数）**零** `ig*` 命中；**`radsrvitem.dll`（5374 函数）
命中全部 13 个族的类名字符串**：

```
0x5665f4e0 igArc2d          0x5665f508 igCircle2d      0x5665f4fc igLine2d
0x5665f514 igEllipse2d      0x5665f4e8 igEllipticalArc 0x5665f520 igLineString2d
0x5665f560 igBSplineCurve   0x5665f544 igPoint2d       0x5665f550 igRectangle2d
0x5665f574 igBoundary2d     0x5665f584 igSymbol2d      0x5665f070 igTextBox
0x5665f684 igPointOnRelation
```

`radsrvitem.dll` 正是历史上证过 `0x0089` 与 `PSMspacemap` handle 数学的同一
DLL（roadmap `ROADMAP-DA-0089` / `ROADMAP-PSMSPACEMAP`）。因此 `ig*` 记录读取器
的定位路径清晰：**对 `aIgcircle2d`/`aIgarc2d` 等字符串取 xref → 类注册/工厂/读
vtable → 消费 PSM payload 的 read 方法**，验证 `0x0059 = center(2)+radius(1)+1B`、
`0x0061 = center(2)+radius(1)+start+end+1B` 的确切位序。这是把曲线族 PSM 解码
从 `corpus-statistical` 升级到 `ida-proven` 的下一步，属独立切片。

## 证据等级与影响

- **可升级**：弧的绝对起止角语义（`ida`-级字段名 `startAngle`/`endAngle`）。
- **不改**：`0x0059`/`0x0061` 的 PSM 位序仍是 `corpus-statistical`（Phase 36 的
  100% 链落地 + 几何自洽），`ugeom2d1` 不构成其 native-reader 证据。
- **无 parser 改动**：本轮纯只读逆向；`SheetGeometry`/schema/model 不动。
- 渲染现状不受影响：Phase 36 已按 `.sym` 语料把线/圆/弧/多段线画出（89% 覆盖）。

## 结论 3：`radsrvitem.dll` 有权威的「类型码 → 类名」枚举

`tools/idalib_igreader_scan.py` 判定 `ig*` 类名字符串全在 `radsrvitem.dll`
（`sppid`/`core` 零命中）。`tools/idalib_radsrv_igxref.py` 对这些字符串取 xref，
命中唯一函数 `sub_56448F70 @ 0x56448f70`——它按记录首个 `u16 type_code`
（`*a1`）返回类名，是 native 代码里**权威的类型码枚举**（反编译逐条抄录）：

| code | class | | code | class |
|---|---|---|---|---|
| `0x06` | igPointOnRelation2d | | `0x63` | **igEllipse2d** |
| `0x0F` | igParallelRelation2d | | `0x69` | igSymmetricRelation2d |
| `0x13` | igBoundary2d | | `0x6A` | igEqualRelation2d |
| `0x15` | igPerpendicularRelation2d | | `0x6B` | igColinearRelation2d |
| `0x17` | _TangentRelation2d | | `0x77` | igFixRelation2d |
| `0x18` | **igLine2d** | | `0x7B` | igGroup |
| `0x19` | igKeyPointRelation2d | | `0x7E` | **igEllipticalArc2d** |
| `0x20` | igRectangle2d | | `0x82` | igHorizontalRelation2d |
| `0x21` | igComplexString2d | | `0x84` | **igLineString2d** |
| `0x3D` | igSmartFrame2d | | `0x85` | igTangentRelation2d |
| `0x40` | igConcentricRelation2d | | `0xCE` | igSymbol2d |
| `0x4D` | igTextBox | | `0x115` | igDimension |
| `0x59` | **igCircle2d** | | `0x116` | igBalloon |
| `0x5D` | **igBSplineCurve2d** | | `0x117` | igLeader |
| `0x5E` | igPoint2d | | | |
| `0x61` | **igArc2d** | | | |

（粗体 = Phase 36 已解出可绘制几何的族。）

**这张表把快照 §3 与 Phase 36 的多处开放项在 identity 层一次性坐实：**

- 快照「未定」`0x06`/`0x7B`/`0x85` → `igPointOnRelation2d` / `igGroup` /
  `igTangentRelation2d`。
- Phase 36 判为「非几何（值恒 0 / 无 f64）」的 `0x19`/`0x06`/`0x85`/`0x82`/`0x15`
  **全部是 Relation（几何约束）记录** —— 难怪不带可绘制几何，与「不发几何」的处置
  完全自洽。`0x5E igPoint2d`（Phase 36 判为连接点、不绘制）亦确认。
- roadmap `ROADMAP-SMARTFRAME-003D`、`ROADMAP-RECTANGLE-0020` 的类名身份坐实
  （`0x3D igSmartFrame2d`、`0x20 igRectangle2d`）；但**仅 identity**，payload
  语义与页面变换判定不变（仍 `IdentifiedOnly`/`Unavailable`）。
- `0x115=igDimension`、`0x116=igBalloon`、`0x117=igLeader` 首次落名。

## per-class payload 读取器在 COM 式接口分发之后（独立切片）

`sub_56448F70` 只是类型→名（用于日志/错误串），不是 payload 读取器。追踪：

- 唯一调用者 `sub_564462F0 @ 0x564462f0`：按 `type_code` 取类名后建对象、
  遍历关系、emit `"RELEATIONS"` 记录——**记录处理入口**。
- `sub_564468B0 @ 0x564468b0`：`igTextBox`（`*a2 != 77` = `0x4D` 门禁）读取器，
  按 `a2[12]∈{1,2,3}` 选变体、读**长度前缀宽字符串**为文本，emit `"TEXT"`。
  → 这是 gap #2「符号内文本 / `igTextBox` 内容」的直接线索。
- `sub_564623D0` 等一打函数带 `{0x18,0x20,0x59,0x5D,0x61,0x63,0x7E,0x84}` 的
  「是否几何族」谓词——确认哪些是可绘制族，但非 payload 解码器。

真正的 per-class 二进制读取器经由对象系统的接口 vtable 分发
（`(*(...+160))(...)` COM 式调用），需顺着对象实例化深挖，属独立切片。
因此 `0x59`/`0x61` 的 **payload 位序仍维持 `corpus-statistical`**（Phase 36），
本轮把它们的**类名身份**升级到 `ida-proven`，位序未升级。

## 探针
| 脚本 | 作用 |
|---|---|
| `tools/idalib_curve_probe.py` | 分析 `ugeom2d1.dll` → `.i64`，导出曲线族/序列化符号 |
| `tools/idalib_curve_decompile.py` | 反编译 `IMG*ReadFromStream2d` + `IMRead_GArc2d` |
| `tools/idalib_igreader_scan.py` | 在 `sppid/core/radsrvitem` 定位 `ig*` 类名载体 |
| `tools/idalib_radsrv_igxref.py` | `ig*` 字符串 xref → 类型码枚举 `sub_56448F70` |
| `tools/idalib_radsrv_factory.py` | 找记录处理入口与「几何族」谓词 |

均只读，不改 parser/schema/model；复用已存在 `.i64`（`ugeom2d1` 首次生成）。

## 结论 4：radsrvitem 把 PSM 记录当不透明 RAD 对象 —— payload 位序封顶在 corpus-statistical

沿 `sub_564623D0` 的几何分支追到底（`tools/idalib_radsrv_reader.py`）：

- **`sub_564851F0 @ 0x564851f0` 不是 payload 读取器，是图元计数分类器**：按
  `type_code & 0x7FFF` 给出图元数 —— `0x18 Line=3`、`0x59 Circle=2`、
  `0x61 Arc=4`、`0x5E Point=1`、`0x63 Ellipse=6`/`0x7E=6`、`0x20 Rectangle=8`；
  `0x13/0x5D/0x78/0x84` 转子处理器。`sub_564623D0` 用它汇总一组记录的图元总数，
  属**枚举/渲染层**，不解析坐标字节。
- **`sub_564459D0 @ 0x564459d0` 是通用导出路径**：建 RAD 对象后 emit
  `"RADObjectProperties"` 与 `"RAD_OBJECT_TYPE"=<类名>`（读 `record+6` 的 oid，
  与 PSM 信封 `type(2)+btf(4)` 后接 oid 一致）。这正是 roadmap 记载的
  `radsrvitem` 经 `RAD_OBJECT_TYPE` 泛型导出边界，**不逐字段解释几何**。

**含义**：`radsrvitem.dll` 把 PSM 记录当**不透明 RAD 对象**处理（计数 + 属性
emit），不存在「从 payload 偏移读 `center`/`radius`/`angle`」的可读字段读取器 ——
真正解释坐标的是 RAD2D 图形引擎（经接口 vtable，非本 DLL 的具名函数）。因此
**`0x59`/`0x61` 的 payload 字节位序在本证据源封顶于 `corpus-statistical`**
（Phase 36 的 100% 链落地 + 几何自洽已是恰当上限），不再强推 ida 位序。

## 结论 5（附带）：igTextBox 记录布局坐实 —— gap #2 的抓手

`sub_564468B0`（igTextBox，`*a2 != 0x4D` 门禁）+ 三个定位器
`sub_56449240/56447710/56447730` 一致给出布局：`word[12] ∈ {1,2,3}` 是变体选择，
文本从 `word[14]`（字节 28）起是 **`u16 count` + `count` 个宽字符（UTF-16）**。
这是 gap #2「符号内文本 / `igTextBox` 内容」可直接落地的读取形状，属独立切片。

## style.dll 现状（文字样式线的硬阻塞）

`ROADMAP-STYLECLUSTER` 要的 `/StyleCluster 0x005A` 读取器最可能在 `style.dll`
（Phase 30 `docs/analysis/2026-06-12-phase30-style-dll-jstyleoverride-ida.md`
证过其 `IOContext::DoIO` 持久化模式：`sub_1000F030` = `JStyleOverride`、64B payload）。
**但本机既无 `dlls/style.dll`，也未装 SmartPlant/Intergraph**（探测
`C:\Program Files*`、常见 SPID 根均无）。故快照 §5「idalib 打通即可取
`0x005A` reader」需修正：idalib 就绪，但 `style.dll` 需先从原始安装/网络共享
（`\\WIN-SPID\...` 一类）取回并放进 `dlls/`，才能生成 `.i64` 推进字高/字体。
