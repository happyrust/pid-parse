# `/StyleCluster` 不再是阻塞点

> 日期：2026-08-04
> 范围：`pid-parse`
> 结论类型：**阻塞解除**（承载体已定名、语料齐备、native reader 在本机）
> 前置：`2026-08-04-psm-type-code-registry.md`

## 1. 原来的阻塞是什么

字高是 07-27 快照排第一的缺口，判定是：

> 字高不在 `igTextBox` 内；样式表在 `/StyleCluster 0x005A`，需要 `style.dll`
> native reader，**本机未装 SmartPlant**。

线宽/颜色则是「完全未解码，需要独立取证 phase」。

## 2. 这个判定的三个前提现在全部不成立

**（一）`0x005A` 有名字了。** PSM 表给出
`0x005A = {9196D9D1-E94A-11CF-8094-080036CE6C02}` → `style.dll` /
**"JSL Style Librarian"**。不是匿名 cluster，是 style.dll 里的样式库对象。

**（二）`style.dll` 就在本机。** `D:\pid\RADInstallA~\style.dll`（816 KB）。
同目录还有 `imagdex.dex`、`symbol.dex`、`smrtfrm.dex`、`stylecnv.dex`、
`docext.dex`、`sfstyl.dex`，以及 Phase 16 用过的 `jutil.dll`。
**「本机未装 SmartPlant」这条限制已经过时**——RAD 运行时整套都在。

**（三）承载体已定名，且六张图全有。** `probe_phase29_stylecluster_prefix` 早就
统计出 StyleCluster 里有 13 个 GUID 在 6 个 fixture 上全命中，但当时没有名字。
拿 `tools/clsid_registry.py` 一查，其中三个正是要找的东西：

| GUID | 模块 | 名字 |
|---|---|---|
| `606FE422-0025-11D0-A1E1-080036A1CF02` | style.dll | **JSL Text Char Style Type/Workbench** |
| `93ADC030-0CB6-11D0-B29B-08003622D702` | style.dll | **JSL Dash Style Type/Workbench** |
| `606FE425-0025-11D0-A1E1-080036A1CF02` | style.dll | JSL SmartFrame Style Type/Workbench |

**字高（Text Char Style）与线型（Dash Style）的承载体在同一个流里，且六张图全有。**
两个缺口共用一个答案，这一点在排除法阶段就推测过，现在得到证实。

## 3. 流的开头是一张样式类型目录

`examples/probe_stylecluster_records`（本次新增）实测，`/StyleCluster` 开头是
`GUID(16) + u32` 的目录，登记的是 JSL 样式**类型**工作台
（`606FE420` … `606FE425` 连号），每个后面跟一个小整数：

```text
  +0x1C  93ADC030…  (JSL Dash Style)         → 0
  +0x40  606FE420…                            → 1
  +0x68  606FE421…                            → 3
  +0x94  606FE422…  (JSL Text Char Style)     → 0
  +0xB8  606FE423…                            → 5
  +0xE4  606FE424…                            → 6
  +0x10C 606FE425…  (JSL SmartFrame Style)    → 0
```

这是**类型目录**，不是样式实例本身；实例应在其后的主体区。

DWG-0201 有**三个** StyleCluster 流：`/StyleCluster`、`/JSite329\StyleCluster`
（16944 字节）、`/JSite396\StyleCluster`（2269 字节）。两个 JSite 流的目录头逐比特
相同，差别在主体大小——与「JSite 是符号/标签的嵌套文档」这一既有认识一致。

## 4. 与 Phase 35-D 的接口

Phase 35-D 证明 `igTextBox` 的 12 字节 trailer 末 4 字节是**跨图纸稳定的样式 id**
（`56` → 中文、`64` → 管道号、`21` → 常规标注）。

实测：**这三个 id 作为 u32 都出现在 `/JSite329\StyleCluster` 里**
（`{21: 2, 56: 2, 64: 6}`）。

⚠ **这还不是证据**：探针按 2 字节步长扫全流，小整数偶然命中很常见，而且目录头里
本来就有 `0/1/3/5/6` 这类小计数。要坐实这条 join，必须证明这些 id 出现在
**样式实例记录的固定字段位置**上，而不是流里随便某处。

## 5. 建议的下一步

1. **解 `/StyleCluster` 主体的记录结构**：目录之后按什么切分、实例记录长什么样。
   语料侧先做，`probe_stylecluster_records` 是起点。
2. **用 `style.dll` 做 native reader 取证**：`0x005A` = JSL Style Librarian 已定名，
   IDA 里从 `style.dll` 的 RTTI / CLSID 入口找 `JStyleTextChar` 的序列化读取序，
   与 Phase 16 证 `JStyleOverride` 是同一套方法。
3. 拿到字段位置后再验证第 4 节那条 join，然后才谈 promotion。

**不要**在 join 未坐实前按语料统计给字高编值——07-27 快照第 5 节 B 方案的
corpus-statistical 上限在这里已经没有必要了，因为 native reader 现在可达。
