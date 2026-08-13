# 普查的「认领」是起点相等，不是区间覆盖：第二个静默洞收口

> 日期：2026-08-12
> 阶段：Phase 40 收尾（改普查判据，不改解码器）
> 上游：`2026-08-11-what-refuses-the-remaining-53.md` §5（发现这个洞的地方）、
> S1 `2026-08-10-the-silent-bucket-is-refusals-not-unknowns.md`
> 产物：`parsers/undecoded_census.rs` 的 `unclaimed_counts` 判据变更 +
> `tests/render_gap_census.rs` 棘轮重钉

## TL;DR

普查原来用**区间覆盖**判定一条链上记录是否已被认领
（`claimed.iter().any(|range| range.contains(&off))`）。链上记录互不重叠——一条的
结束就是下一条的开始——所以一个认领区间**盖住**另一条记录的起点而**不起始于**它，
只可能是扫描型家族的越界认领，绝不可能是对这条记录的解码。判据改为**起点相等**
（`range.start == off`）之后，被越界认领消音的被拒记录全部进了 S2 的警告通道：

| fixture | refused（旧） | refused（新） | 新进来的 |
|---|---:|---:|---|
| `DWG-0201GP06-01.pid` | 4 | **15** | `igTextBox` +10、`DependencyObject` +1 |
| `DWG-0202GP06-01.pid` | 5 | **15** | `igTextBox` +6、`igLineString2d` +4 |
| `工艺管道及仪表流程-1.pid` | 21 | **23** | `igTextBox` +2 |
| `D06.pid` | 0 | 0 | — |
| `A01.pid` | 18 | 18 | — |
| **合计** | **48** | **71** | **+23（graphic）** |

## 一、为什么是 +23 而不是 §5 说的 32

§5 的 32 是**按盖住方计数**的：`SubRecord0x0010` 15 条、`JStyleOverride` 12 条、
`AttributeFragment` 5 条。两笔账的差有两个来源：

1. `SubRecord0x0010` 与 `AttributeFragment` 是**同一个 `0x0010` 信封的两个注册表
   视图**（`model/sheet_families.rs` 的设计），同一条越界认领在那张表里被计了两次；
2. 被盖住的记录里有非 graphic 类型码（`0x0030` / `0x0010` 自己），它们进普查计数，
   但被原生图形谓词（`radsrvitem.dll!sub_56449950`）滤在警告与棘轮之外——这是
   S2 立下的警报口径，本次不动。

去重、过谓词之后剩下的 graphic 增量就是 +23。

## 二、改了什么

`parsers/undecoded_census.rs::unclaimed_counts`，一行判据：

```text
旧：claimed.iter().any(|range| range.contains(&off))
新：claimed.iter().any(|range| range.start == off)
```

正当性：普查走的是链校验贪心走查（候选头的推进位置必须是流尾或另一个合法头）。
在这个走查里，一条记录被某家族解码过，当且仅当该家族的 `decoded_ranges` 里有一条
**从这条记录的起点开始**的区间——链式家族的区间天然起始于记录起点
（`tests/sheet_family_wiring.rs` 的合成用例钉着 `start..start+56`），扫描型家族对
真记录的认领同样起始于记录头。反过来，「区间盖住 `off` 但不起始于 `off`」在不重叠
的记录链上没有合法解释，只能是扫描认领越过了自己记录的真实边界。

新增单元测试 `an_over_claim_covering_a_later_record_does_not_silence_it` 钉住这个
形状：前一条记录被合法认领、认领区间越界盖住后一条 `igTextBox` 的起点，后者必须
以 refused 计出。

## 三、这次看见了什么新东西

- **`DWG-0201` 有一条被拒的 `DependencyObject`（`0x00FA`）**。0x00FA 在原生图形
  谓词的「会画」集合里，所以它进了 graphic 警告。此前 136 条同类都解码，这一条
  被自家解码器拒收——病因未归因，挂在 refused 桶里等取证。
- 被消音的其余 22 条与 §5 的三个总体同族（`igTextBox` 内联长度 / `btf<68`、
  `igLineString2d` 退化），只是此前连「被拒」这个事实都说不出来。归因仍归
  §5 的总体分析管，本文不重复。

## 四、明确不做的

- **扫描型家族改链式认领**（治因）：`SubRecord0x0010` / `JStyleOverride` /
  `AttributeFragment` 仍按滑窗认领字节。2026-08-05 的零剩余实测给过链式化的证据，
  但它动的是 `sheet_records.rs` 的解码核心，登记为后续，不随本次报表修复夹带。
- 三个总体（A/B/C）的处置维持 §5 的建议排序，本次不动解码器一行。

## 五、下游影响

- `NormalizedPidGeometry::refused_graphic_records` 的计数变大（48 → 71），
  每条仍有 S2 的具名警告；`OpenCADStudio` 的 `.pid` 导入日志与命令行汇总行
  随之如实变大。几何实体、golden snapshot、`igLine2d` 计数棘轮全部不变
  （只改普查，不改解码）。
