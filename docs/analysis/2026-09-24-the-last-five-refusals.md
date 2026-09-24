# 最后五条拒收：四条是零长折线，一条是解码器自己编的上限（2026-09-24）

> 承接 OpenCADStudio 计划 `docs/plans/2026-09-24-pid-import-status-and-next-steps.md` H2（N-D11：只定性不改码）。
> 产物：`examples/probe_the_last_refusals.rs`——按 `parsers::undecoded_census` 的链走法找出没有解码记录起于其上的
> `0x0084` / `0x00FA` 链记录，逐条重放该族的校验规则，报第一条不过的，再列同图同族被接受的记录长什么样。
> 上游：`2026-08-11-what-refuses-the-remaining-53.md`（工艺八条零长折线判为正确拒收）、`tests/render_gap_census.rs`。

## 一句话

`render_gap_census` 在两张 DWG 上还点名的五条拒收，**四条是对的，一条是规则太紧**。DWG-0202 `/Sheet6` 的四条 `igLineString2d`
与工艺那八条是同一个总体——两个顶点重合的零长折线，form 1 / **scope 3** / index 1，而且全在原图关闭的 `HiddenObjects` 层上，
拒收不丢任何看得见的笔画。DWG-0201 `/Sheet6` 那一条 `DependencyObject` 是被解码器的 `group_kind_word ∈ 1..=16` 拒的，
可 `+14` 其实是**成员数**，这是一个 22 个成员的组，字节账与被接受的记录同一条公式；该族不出几何，所以同样不丢笔画，
但它让 census 与 OCS 导入摘要多报了一条「没画」。

## 一、四条零长折线（DWG-0202 `/Sheet6`）

| 位置 | oid | parent | 层（`+8`） | 两个顶点 |
|---|---:|---:|---|---|
| `0x000A09` | 520 | 526 | 17 `HiddenObjects` | (0.170947, 0.220177) ×2 |
| `0x000C21` | 539 | 545 | 17 `HiddenObjects` | (0.156514, 0.220177) ×2 |
| `0x00455F` | 4433 | 4444 | 17 `HiddenObjects` | (0.129299, 0.220177) ×2 |
| `0x0047AF` | 4446 | 4450 | 17 `HiddenObjects` | (0.142106, 0.220177) ×2 |

- 四条都是 `btf` 56（两个顶点）、form 1、scope 3、index 1；重放下来第一条不过的规则都是「所有顶点相同」，前面八条全过。
- 同图接受的 28 条折线 scope 只有 1 和 2（`(1,2,2)` 25 条、`(1,1,4)` 2 条、`(1,1,3)` 1 条）；工艺被拒的八条同样是 form 1 / scope 3 /
  index 1 / 两点重合（在 oid 12 的 `Labels` 层）。**scope 3 的两点重合在语料上是一个总体，12 / 12**，没有一条 scope 3 被接受。
- 0202 这四条所在的 oid 17 是 `HiddenObjects`，导入时按文件的显示位关着：即便放行，也是关闭层上看不见的零长线。
- **裁定：正确拒收**，与 08-11 对工艺八条的判断一致。

## 二、一个 22 成员的依赖对象（DWG-0201 `/Sheet6`）

- `0x00322A`，oid 781，parent 6，`+8..14` 全零——前三条规则都过；**`+14` = 22**，不过「`group_kind_word` ∈ 1..=16」。
- **`+14` 是成员数**。同图被接受的 135 条里，按 `+14` 分的最短长度恰是 `36 + 8·k`：`k` = 1 → 44、2 → 52、4 → 68；这条 212 = 36 + 8·22。
  字节也是同一个形：16 字节头（oid / parent / 六个零 / `k`），22 个 `(u32 成员 oid, u16 1)`（`0x02BB`、`0x027F`、`0x02BD` …… `0x01B1`），
  再 22 个 `u16 1`，末尾 20 字节属性块（`2026-08-04-graphicgroup-tail-property-block.md` 说的那种自描述块）——16 + 6k + 2k + 20 = 36 + 8k。
  上限 16 是按当时语料里见过的最大值编的，不是格式的约束。
- 影响：`DependencyObject` 在族注册表里 `emits_geometry: false`（审计用，放行了也不画），所以拒收不丢笔画；但 census 按原生图形谓词
  把 `0x00FA` 算作图形类，于是 OCS 导入摘要说 0201「1 source records not drawn」，而那一条本来就不会画。
- **裁定：规则过紧，是解码器的小缺口。** 修法是把 `1..=16` 换成结构自洽的校验（`k ≥ 1` 且 `36 + 8·k ≤ btf`，愿意的话再核成员表逐条是
  `(oid, 1)`）；放行后 census 0201 1 → 0、OCS 摘要里 0201 的「没画」1 → 0，几何不变。按 N-D11 另开小单，本次不改。

## 三、随之改的

- `tests/render_gap_census.rs` 的 `EXPECTED` 注释写上裁定（数字不动）。

## 四、复现

```powershell
cargo run --example probe_the_last_refusals
```
