# Phase 40：渲染缺口重普查（静默拒收 → 具名警告 → 排序）

> 日期：2026-08-10
> 范围：`pid-parse`（普查 + 诊断）+ `OpenCADStudio`（报告）两仓。
> 前序：Phase 39 三项全部收口，且**三项原病因判断全部不成立**。
> 本计划的第一原则由此而来：**缺口清单必须重新测量，不得沿用。**

## 0. 现状（2026-08-10 实测）

### 0.1 两仓状态

| 仓 | HEAD（本 phase 开始时） | 工作树 |
|---|---|---|
| `pid-parse` | `1240440 feat(style): decode the JStyleSimpleFill colour at payload +30` | 干净（仅 CRLF 噪声） |
| `OpenCADStudio` | `4faa9475 feat(io): fill the flow arrowheads in the blue the drawing states` | 干净（仅 CRLF 噪声 + 未跟踪 `teach/`） |

### 0.2 先排除一个旧假设

Sheet 流几何**早已饱和**。跨图未解码类型码只剩三个：`0x0013`（Phase 39 S6 起已发）、
`0x003D`（已用作页幅）、`0x0020`（4 条，Phase 34-B 负结论）。`0x3FE6` 全语料 0 条命中
——S5 的撤回确实生效。

**沿用 Phase 39 的缺口清单会直接走错方向**：那张单子上的三项都已结案，而真正的缺口
从来没被数过。

## 1. 重普查的结果

`examples/probe_phase40_render_gap_census` 把每条链上记录归入唯一一个桶：

| 桶 | 含义 | 全语料 | 谁在说话 |
|---|---|---:|---|
| claimed | 解码器收了 | 1511 | 正常上屏 |
| **REJECTED** | **解码器存在，拒了这些字节** | **141** | **无人** |
| warned | 无解码器，原生谓词说该画 | 5 | 具名警告（Phase 38 S2） |
| quiet | 无解码器，谓词说不画 | 2 | 按设计静默 |

**被报告的只占丢失量的 3.4%。** 静默桶：`0x0018` 88 条（占全部线 21%）、`0x004D`
45 条（17%）、`0x0084` 8 条。

根因是一行判据：`undecoded_type_code_census` 测的是**类型码**，而一条被拒记录的
类型码恰恰是本 crate 解码的那些之一，于是它同时掉出解码输出和普查。函数注释把这
叫「另一种诊断」，**而没有任何地方发出那种诊断**。

### 证伪（先做，再下结论）

把 141 个被拒偏移去对 `sheet_record_starts` 严格记录链——**当初撤回 `0x3FE6` 幽灵
家族用的同一把尺子**。**141/141 在链上，零条脱链**；探针直调发行版 `decode_igline_at`
**零分歧**。这次尺子指向相反方向：它们是真记录。

### 病因（对 88 条线）

被拒与被收的 `0x0018` 声明长度**完全一样（50 字节）**；四道校验逐条复算，
**88/88 只栽在 `remaining_header != 12`**。两个总体：A01 的 80 条是 `8`，
DWG-0202 `/Sheet6615` 的 8 条是 `6996`（且 `type_flags` 也不同）。
**这是第二种帧装，不是未知形状。**

集中度：A01 `/JSite204/Sheet6` 拒 98/111（88%）；DWG-0202 `/Sheet6615` 拒 4 警 1，
整个流一个像素不画。

全部证据见 `docs/analysis/2026-08-10-the-silent-bucket-is-refusals-not-unknowns.md`。

## 2. 排序结论

先把静默变成警告，**再**谈解码。理由不是「便宜」，是**顺序本身就是病因**：这个洞在
两个相邻 phase 眼皮底下藏了下来，正因为没人在喊。解码期间的每一份报告若仍旧漏报
141 条，是在用不诚实的基线做排序。

## 3. 执行序列

```text
S1  [pid-parse] 四分账重普查 + 证伪 + 规则归因            ✅ 2026-08-10 完成
 -> S2  [两仓] 静默拒收接上具名警告                       ✅ 2026-08-10 完成
 -> S4  [pid-parse] 先证「JSite 内的线该不该画在图纸上」   ✅ 2026-08-10 完成，四读数全肯定
 -> S3  [pid-parse] 45 条 text / 8 条 linestring 的拒收归因   ⬜ 未开始
 -> S5  [两仓] 解 igLine2d 的第二种帧装                    ⬜ 未开始
```

### S1：四分账重普查 ✅ 完成

产物 `examples/probe_phase40_render_gap_census`：一次链走查，四个桶，外加两项判据
——严格链成员资格（证伪）与逐规则复算（归因）。结论见 §1。

### S2：静默拒收接上具名警告 ✅ 完成

**只把静默变成警告，不动任何解码规则。** 拒收本身是对的——解码器去猜一个未知帧装
会画出虚构的线；静默才是错的。

| 仓 | 改动 |
|---|---|
| `pid-parse` | `undecoded_census` 新增 `refused_record_census` + `RefusedRecordCount`；两个普查抽出共用的 `unclaimed_counts`，从结构上保证它们**划分**未认领记录 |
| `pid-parse` | `SheetGeometry.refused_records`、`NormalizedPidGeometry.refused_graphic_records` |
| `pid-parse` | `tests/render_gap_census.rs`：语料棘轮（拒收 4/9/21/0/98）+ 具名不变式 + 两种措辞不得混同 |
| `OpenCADStudio` | `report_import` 把被拒收的图形记录按名报出，措辞与「无解码器」分开 |

措辞刻意分开，因为两者要求相反的工作：**无解码器 → 写一个；被拒收 → 重新丈量已有
那个的规则。**

### S4：先证「JSite 内的线该不该画」 ✅ 完成（四读数全部肯定）

`examples/probe_phase40_jsite_sheet_is_page_content` 取四条互相独立、任何一条都能
单独否掉结论的读数：

| # | 读数 | 结果 |
|---|---|---|
| 1 | 这条流已经是页面内容来源吗 | **是。** `/JSite204/Sheet6` 今天已有 7 条记录被接受并画在页面上 |
| 2 | 一个坐标系还是两个 | **一个，页面的。** 被拒线张成 `0.0000..594.0000` × `0.0000..420.0000 mm`；同流已接受记录落在框内 |
| 3 | JSite204 是符号定义吗 | **不是。** `symbol_path=None`；同文件 JSite1229 / JSite206 才是真符号，且不含 Sheet 流 |
| 4 | 会不会重复绘制 | **不会。** 顶层仅有的两条 `igSymbol2d` 指向 JSite1229 与 JSite206，无一指向 JSite204 |

**意外收获**：80/80 全部轴对齐，最长恰为 594.0mm（页宽），四角逐位落在页幅角上。
**这 80 条是 A01 的图框边框与网格**——它在 OCS 里只画 27 个实体、看着像空图，
就是因为边框全在被拒记录里。

这条同时把 S5 的风险降一档：坐标确实在标准偏移上（否则不会落成精确页面矩形）。
但**不据此放宽任何校验**——`remaining_header` 的语义仍要原生序列化器来定。

### S3：text / linestring 的拒收归因 ⬜

45 条 text 占静默桶近三分之一，且分布在四张图上（不只 A01），可能比线更普遍。手法
同 S1 的规则复算：逐条复算 `igTextBox` / `igLineString2d` 的校验链，看是哪一道在拒、
分几个总体。**先归因，再决定做不做。**

### S5：解 `igLine2d` 的第二种帧装 ⬜

前置 S4 已过：那 80 条属于图纸，且是 A01 的图框。手法同 `0x002A`/`0x002E`/`0x002F`
已验证四次的路子：`radsrvitem` 的 `igLine2d` 序列化器定字段与语义（`remaining_header`
到底是什么、`8` 与 `6996` 是不是两个版本），语料定字节偏移。

**仍不得凭「读出来合理」放宽 `remaining_header != 12`。** S4 证明的是这批记录该画、
且坐标在标准偏移上，不是证明校验该松——放宽会同时放进任何一个碰巧 50 字节的东西。
正确的落地是让解码器**认识**第二种帧装，而不是不再检查。

DWG-0202 `/Sheet6615` 那 8 条在顶层流、`type_flags` 也不同，是**另一个总体**，
需要自己的归属取证，不能搭 S4 的便车。

## 4. Gate 命令（沿用两仓门禁）

`pid-parse`：

```powershell
cargo build  --locked --workspace --all-targets
cargo test   --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo rustdoc --lib --locked -- -W missing-docs
```

`OpenCADStudio`：

```powershell
cargo check --tests --examples
cargo test --locked --test pid_import
cargo fmt --all -- --check
cargo clippy --locked --all-targets 2>&1 | Select-String "io.pid.rs|pid_import.rs"
$env:PID_SYMBOL_LIBRARY = "..\pid-parse\test-file\symbols-full"
cargo run --release --example pid_probe -- <六张 .pid>
```

## 5. Stop-And-Challenge

1. 想按标准偏移把第二种帧装的坐标读出来上屏，**而没有先由原生序列化器证明字段
   位置**。「读出来合理」是弱判据，Phase 39 三次判错都是这个理由。
2. 想在没证明「JSite 内的线属于图纸」之前先解 A01 那 80 条。
3. 想放宽 `remaining_header != 12` 这类校验来「多收一点」。拒收是对的，
   放宽等于画虚构内容。
4. 想给 `0x0020` / `0x007B` 这些无 fixture 支撑的族硬写解码器。
5. 任何 parser promotion 缺字节区间、fixture ratchet、panic-safety 或
   byte-audit movement。

## 6. 明确不做（本 phase）

| 项 | 为什么 |
|---|---|
| 放宽任何现有校验规则 | 拒收是对的；本 phase 只让它可见 |
| `0x002A` 的 `+112` / `+120` | 不是颜色，当前无消费者（Phase 39 §6 已记） |
| `0x002B` 图案填充 | 全语料无边界引用它 |
| 把 `0x3FE6` 从 `DECODED_TYPE_CODES` 摘掉 | 解码器仍在（只是从不接受）；留着不影响分账，全语料 0 条命中 |
| OCS clippy 存量债 | 是仓主的机械清理决策，不塞进显示收口 |
