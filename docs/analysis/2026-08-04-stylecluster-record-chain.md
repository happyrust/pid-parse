# `/StyleCluster` 的记录结构已解出

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**结构已解**（六个流全部逐字节走通，零剩余）
> 前置：`2026-08-04-stylecluster-unblocked.md`、`2026-08-04-psm-type-code-registry.md`
> 工具：`examples/probe_stylecluster_records`

## 1. 流的整体结构

`StyleCluster` 是 `u32 magic + u32 count` 之后接一串 PSM 记录，
envelope 与 Sheet 流同构（`u16 type_word + u32 bytes_to_follow`）：

```text
  +0  u32  magic 0x6C90F544
  +4  u32  count
  +8       记录链开始
```

**验证：六个流全部走通，终点逐字节等于流长。**

| 流 | 字节 | 记录数 | 终点 |
|---|---:|---:|---|
| DWG-0201 `/StyleCluster` | 17097 | 135 | `0x42C9` = 流长 |
| DWG-0201 `/JSite329\StyleCluster` | 16944 | 101 | `0x4230` = 流长 |
| DWG-0201 `/JSite396\StyleCluster` | 2269 | 11 | `0x08DD` = 流长 |
| 工艺管道 `/StyleCluster` | 10127 | 84 | `0x278F` = 流长 |
| 工艺管道 `/JSite6963\StyleCluster` | 1877 | 8 | `0x0755` = 流长 |
| 工艺管道 `/JSite7559\StyleCluster` | 8375 | 46 | `0x20B7` = 流长 |

零剩余、零重叠。这不是启发式切分，是结构本身。

## 2. 第一条记录：`0x005A` JSL Style Librarian

链上第一条恒为 `0x005A`（= **JSL Style Librarian**，style.dll），内含**样式类型目录**：

```text
  +0x1A  u16  目录条目数（实测 13）
  +0x1C       目录本体
```

条目形状（实测）：首条是 `[GUID(16)][u32 index][u32 0]`（24 字节），
其后为 `[提供者 GUID(16)][样式类型 GUID(16)][u32 index][u32 0]`（40 字节）。
1 + 6×2 = 13 个 GUID，与声明的条目数**精确吻合**。

登记的样式类型（经 `tools/clsid_registry.py` 定名）：
JSL Dash Style、JSL Text Char Style、JSL SmartFrame Style，以及
`606FE420/421/423/424` 四个注册表未收录的同族类型。
其后的区段还登记了 JSL Pattern Style、JSL Segmented Style、
JSL Point Symbol Style、JSL LinePointGenerator Style、JSL 3D Style。

## 3. 样式实例记录 —— 两个缺口的确切位置

链上其余记录就是**样式实例**，type code 连成一段 `0x002A..0x0030`，
全部是 `style.dll` 的类，CLSID 也连号（`47FCC331` … `47FCC338`）：

| type | 类名 | DWG-0201 `/StyleCluster` | `/JSite329` | 工艺 `/StyleCluster` |
|---|---|---:|---:|---:|
| `0x002A` | JSL Simple Fill Style | 3 | 3 | 3 |
| `0x002B` | JSL Hatch Fill Style | 1 | 1 | 1 |
| **`0x002C`** | **JSL Text Character Style** | **48** | **35** | **21** |
| `0x002D` | JSL Text Paragraph Style | 42 | 38 | 14 |
| **`0x002E`** | **JSL Simple Line Style** | **20** | **8** | **19** |
| **`0x002F`** | **JSL Simple Dash Type Style** | 2 | 1 | — |
| `0x0030` | JSL Override Style | 4 | 11 | 2 |
| `0x0116` | JDimParameters Object | — | 2 | — |

**结论：**

- **字高在 `0x002C` JSL Text Character Style**，DWG-0201 根流有 48 条实例。
- **线宽/颜色在 `0x002E` JSL Simple Line Style**，20 条。
- **线型在 `0x002F` JSL Simple Dash Type Style**。

三者都在同一条记录链上，都是 `style.dll` 的类，**且与 pid-parse 已经解码的
`0x0030 JStyleOverride` 是 CLSID 连号的兄弟类**（`47FCC333` / `47FCC335` /
`47FCC336` vs `47FCC338`）。Phase 16 证 `JStyleOverride` 的那套 native-reader 方法
可以直接迁移过来。

## 4. 与 Phase 35-D 的 join

Phase 35-D 证明 `igTextBox` trailer 末 4 字节是稳定的样式 id（56 / 64 / 21）。
现在知道 `0x002C` 有 35–48 条实例，**id 空间与实例数量级相符**。

⚠ join 仍未坐实：需要先解出 `0x002C` 实例记录的字段布局，找到实例自身的 id 字段，
再证明 `igTextBox` 的 id 指向它。**不要**在此之前按数量吻合下结论。

## 5. 下一步

1. 解 `0x002C JSL Text Character Style` 的记录布局（btf 分布 → 分桶 → 列分析），
   同一套已经在 GraphicGroup 上用过的方法。
2. 用 `style.dll` 做 native reader 取证：类已定名、CLSID 已知、
   与 `JStyleOverride` 同族，IDA 入口清楚。
3. 坐实 id join，然后 promotion，配 fixture ratchet 与 byte-audit。
