# Phase 36：`.sym` 符号库几何解码

> 日期：2026-07-26
> 范围：证明 `.sym` 参考库文件与 `.pid` 同为一族容器，其 `Sheet*` 流是
> **确定性记录链**；解出线 / 圆 / 弧 / 多段线四类图元，并落地
> `src/symbol_library.rs`，使 `igSymbol2d` 放置可以画出真实图形而非标记。

## 结论

**`.sym` 与 `.pid` 同构，且比 `.pid` 更规整。** Phase 35-C 把每条
`igSymbol2d` 解析到了 `JSite<id>` 上的 `.sym` 库路径，但符号本体一直被
当作"外部不可达资源"。事实上：

1. `.sym` 是 CFB 复合文档，magic 与 `.pid` 一致，`PidParser::parse_package`
   直接可读，618 个库文件 **零解析失败**；
2. 其 `Sheet*` 流从 **offset 8** 起就是标准 6 字节 PSM 记录链
   （`u16 type_code` + `u32 bytes_to_follow`），可确定性遍历；
3. 四类记录承载可绘制几何，其余为连接点、文本、样式与结构。

全语料库验证（`examples/probe_sym_corpus_scan.rs`）：

| 指标 | 数值 |
|---|---:|
| `.sym` 文件 | 618 |
| 解析失败 | 0 |
| `Sheet*` 流 | 1134 |
| **记录链精确落在流末尾** | **1134 / 1134（100.0%）** |
| 提前终止 / 越界 | 0 / 0 |

100% 精确落地是这套读法的核心证据：链上任何一条记录的 `type_code` 或
`bytes_to_follow` 读错，末尾就对不齐。

## 记录链起点为何是 8 而不是 16

`parsers/cluster_header.rs` 的 16 字节 cluster header 布局为
`magic / record_count / stream_type / body_len / flags`。在 `.sym` 的
`Sheet*` 流上，**后 8 字节其实是第一条记录自己的 PSM envelope**，被
header 解析器误读为 `stream_type` + `body_len`。

`Circle.sym` 的 `/Sheet12`（57 字节）是最小反例：

```text
0000  44 f5 90 6c 01 00 00 00 | 59 00 2b 00 00 00 | a0 01
      ^ magic      ^ count=1    ^ type   ^ btf=43   ^ payload 起点
0010  00 00 0c 00 00 00 0e 00 00 00 00 00 09 00 00 00
0020  a7 0a 46 25 75 02 ba 3f  a8 c6 4b 37 89 41 c0 3f
0030  2d 73 ba 2c 26 36 6f 3f  00
```

算术自洽：`8（magic+count）+ 6（envelope）+ 43（payload）= 57` = 流长度。
被当成 `stream_type` 的 `0x0059` 正是 `igCircle2d`，被当成 `body_len` 的
43 正是该记录的 payload 长度。

## 类型码与几何语义

payload 前 18 字节为定位头（oid / parent_ref / 一个 u32 / u16 sub_type /
index），其后是 f64 序列。全库 27 个类型码中，四类携带可绘制几何：

| type | 名称 | 记录数 | payload | 几何 |
|---|---|---:|---|---|
| `0x0018` | `igLine2d` | 7696（43.1%） | 恒定 50 | 4×f64 = 起点、终点 |
| `0x0059` | `igCircle2d` | 510 | 恒定 43 | 3×f64 = 圆心、半径（+1 字节） |
| `0x0061` | **弧** | 230 | 恒定 59 | 5×f64 = 圆心、半径、起止角（+1 字节） |
| `0x0084` | `igLineString2d` | 31 | 变长 | `(btf-24)/16` 个顶点 |

### `0x0061` 判定为弧的证据

跨全库槽位统计（`examples/probe_sym_record_shapes.rs`）：

```text
+18  n=230  min=-0.066675  max= 0.166370   ← 圆心 x
+26  n=230  min=-0.015875  max= 0.246380   ← 圆心 y
+34  n=230  min= 0.000397  max= 0.068000   v>0: 100.0%   ← 半径
+42  n=230  min= 0.000000  max= 6.230829   |v|<=2pi: 100%  ← 起始角
+50  n=230  min= 0.000000  max= 6.283185   |v|<=2pi: 100%  ← 终止角
```

`+34` 全库 **100% 为正**且量级与半径一致；`+42`/`+50` 全部落在
`[0, 2π]`，且 `+50` 的最大值 `6.283185` 恰为 2π。

两角为**绝对起止角**而非"起始角 + 扫掠角"：`ElecTraceLine.sym` 的两条弧
分别是 `2.944 → 0.197` 与 `6.086 → 3.339` 弧度，数值不同但逆时针扫掠角
**同为 202.6°**、半径同为 `0.001619`——正是重复波浪符号应有的形状。按
起止角绘制后两弧无缝拼成 S 形；若按扫掠角解读则会画成互补弧。

### 非几何类型

值恒为 0 或不含 f64，已确认不承载形状：`0x0019`（2598）、`0x0006`（821）、
`0x0085`（511）、`0x0082`（468）、`0x0015`（160）。`0x005E`（`igPoint2d`，
2488）是连接点，不应绘制。`0x004D`（`igTextBox`，1490）是符号内文本，
留待后续。

## 与 `.pid` 的方言差异

同族记录在两种容器里的校验字段不同，因此 `.sym` 需要独立解码路径而非
复用 `decode_iglines` 等扫描式解码器：

- `decode_iglines` 要求 payload `+8..12 == 12`（`IGLINE2D_REMAINING_HEADER`），
  而 `.sym` 该字段最常见取值为 **8**，其次为 5 / 164 / 89，故 `.sym` 的
  7696 条线在 `.pid` 侧解码器上全部被拒。

更根本的差异在策略：`.pid` 的 `Sheet*` 是异构流，解码器必须逐字节扫描并
用严格取值门禁防误报；`.sym` 的链可确定性遍历，链本身即为最强校验，无需
也不应套用那些门禁。

## 多 `Sheet` 重复本体

一个 `.sym` 常有多个 `Sheet*` 流承载**同一本体的副本**。
`2-Way Angle Globe Valve.sym` 的 `/Sheet63`（9 图元）是 `/Sheet6`
（11 图元）的子集，浮点值逐位相同。全部绘制会使每一笔重影，全库图元由
8467 降为 **5636**（-33%）。

`read_symbol_geometry` 因此**跨 sheet 去重、sheet 内保留**：单个 sheet 内
的重复笔画可能是符号自身有意为之。

## 形状验证

`examples/dump_symbol_geometry.rs` + `examples/plot_symbol.ps1` 将解出的
本体光栅化，与符号应有形状逐一比对：

- `2-Way Angle Globe Valve`（8 线 3 圆）：两个成 90° 的三角形 + 同心球阀
  体圆，标准角式截止阀符号；
- `ElecTraceLine`（8 线 2 弧）：两弧拼成 S 形波浪 + 8 条斜线标记；
- `Positive Displacement Pump`（66 线 2 圆 2 多段线）：跑道形外壳 + 双圆。

## 覆盖率

图纸引用 109 次放置、38 个不同符号。本地备份包
（`backup-test/*/RefData~4~681.zip` 等，共 1223 个不同路径）解析后：

| Fixture | 放置 | 可绘制 | 图元 |
|---|---:|---:|---:|
| DWG-0201GP06-01 | 20 | 20（100%） | 120 |
| DWG-0202GP06-01 | 23 | 23（100%） | 179 |
| 工艺管道及仪表流程-1 | 58 | 46（79%） | 552 |
| D06 | 6 | 6（100%） | 45 |
| publish A01 | 2 | 2（100%） | 8 |
| **合计** | **109** | **97（89.0%）** | **904** |

未命中的 3 个符号（12 次放置）均为 XaLNG 站点定制件
（`Piping\Piping OPC's\Xa.sym`、`Xa chu.sym`、
`Design\Annotation\Labels\Xa Item Note & Label.sym`），本地任何备份包中
都不存在——属**参考数据缺失**而非解码缺陷，OCS 侧对其回落圆标记。

## 落地

- `src/symbol_library.rs`：`SymbolPrimitive` / `SymbolGeometry` /
  `read_symbol_geometry` / `SymbolLibrary`（含 UNC → 库相对路径归一，
  缓存命中与未命中）。
- UNC 前缀按 `\Symbols\` 切分：同一符号在不同图纸中经由
  `\\WIN-SPID\...`、`\\SPID\...`、`\\MM-128\...` 等不同共享引用，其后半段
  才是各站点一致、也是本地库的目录布局。
- 探针：`probe_sym_container.rs`（字节级）、`probe_sym_corpus_scan.rs`
  （链完整性）、`probe_sym_record_shapes.rs`（类型语义）、
  `probe_sym_library_yield.rs`（验收）。

## 未决

- `0x004D`（`igTextBox`）符号内文本未解，符号上的固定标注暂不绘制；
- `0x0115`（252）、`0x0013`（`igBoundary2d`）、`0x004D` 等变长族未定语义；
- 40 个 `.sym` 解出空本体，推测为纯文本标签类符号，待 `igTextBox` 解出后
  复核；
- 弧的 bounds 以端点与圆心近似，未做象限展开（当前仅用于放置级取景）。
