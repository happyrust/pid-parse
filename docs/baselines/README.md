# byte-audit baselines

本目录还存放 `publish-diff-accepted.json`（`--diff-against` 的已接受差异基线），见文末。

每份 `<slug>.byte-audit.json` 文件都是一份真实 `.pid` fixture 在 commit
时刻的 byte-audit 快照。`.github/scripts/check-byte-audit-baselines.sh`
在 CI 上把这些 baseline 与同名 fixture 当前的 byte-audit 报告做差分；
任何 `overall_coverage_ratio` 下降、已 traced stream `consumed_bytes`
下降、或已 traced stream 翻回 unregistered 都会 hard-fail。

## 命名约定（ASCII slug + sidecar）

baseline 文件名只用 ASCII slug，避免跨平台 / 跨 shell 的编码问题
（Windows NTFS UTF-16 / Linux ext4 byte sequence / macOS NFD vs NFC
normalize / git pathspec / CI shell escaping 等）。

- `<slug>.byte-audit.json` — baseline 数据本体。
- `<slug>.fixture.txt` — 一行文本，记录 fixture 在仓内的相对路径。
  允许非 ASCII（中文）文件路径。runner 会优先读 sidecar；如果
  sidecar 不存在，回退到 `test-file/<slug>.pid` 的旧约定，保持向后
  兼容。

## 当前 baseline

| slug | fixture path | total bytes | covered |
|---|---|---|---|
| `dwg-0201gp06-01` | `test-file/DWG-0201GP06-01.pid` | 223 KB | ~11% |
| `dwg-0202gp06-01` | `test-file/DWG-0202GP06-01.pid` | 206 KB | ~9% |
| `sample-cn-1` | `test-file/工艺管道及仪表流程-1.pid`（中文 fixture） | 211 KB | ~4% |

> covered 是 commit 时刻的 `overall_coverage_ratio`，仅供参考；任何
> 下降都会触发 CI hard-fail。

## 如何新增 / 更新一份 baseline

```bash
# 1. 选 ASCII slug
SLUG=my-new-fixture
FIXTURE=test-file/MyNewFixture.pid   # 允许中文 / 空格

# 2. 生成 baseline JSON（注意 PowerShell 5.x 的 UTF-16LE 陷阱，见下）
cargo run --locked --bin pid_inspect -- "$FIXTURE" --byte-audit --json \
    > "docs/baselines/$SLUG.byte-audit.json"

# 3. 写 sidecar
echo "$FIXTURE" > "docs/baselines/$SLUG.fixture.txt"

# 4. 跑 runner 确认 0 diffs
bash .github/scripts/check-byte-audit-baselines.sh
```

### PowerShell 5.x UTF-16LE 陷阱

Windows PowerShell 5.x 的 `>` 重定向默认输出 **UTF-16LE**，会让 baseline
JSON 在 Linux runner 上无法解析。务必：

```powershell
# 推荐：PowerShell 7+ 默认 UTF-8
cargo run --locked --bin pid_inspect -- $FIXTURE --byte-audit --json `
    | Out-File -Encoding utf8NoBOM "docs/baselines/$SLUG.byte-audit.json"
```

或在 Git Bash / WSL / cmd.exe 下用普通 `>`。

## fixture 私有性 / 公开性

- 当前 3 份 fixture（`DWG-0201GP06-01.pid` / `DWG-0202GP06-01.pid` /
  `工艺管道及仪表流程-1.pid`）已 commit 到本仓 `test-file/`，公开 CI
  可直接跑 baseline 比较。
- 未来若引入私有 fixture（不能 commit 到公开仓），把 fixture 放到
  `.gitignore` 排除路径下，但 baseline JSON 与 sidecar 仍 commit 到
  `docs/baselines/`。runner 在 fixture 缺失时会 soft-skip，不影响公开
  CI；私有 CI runner 持有 fixture 时会跑实际比较。
- baseline JSON 本身只含字节偏移数值与 SmartPlant 标准 stream 路径名
  （如 `/PSMroots`、`/\u0005SummaryInformation`），不含业务数据。

## 何时刷新 baseline

任何会**合法地**改变 byte-audit 输出的 PR 都需要在同一 PR 内刷新对应
baseline，例子：

- 新增 `_with_trace` parser → 多个 stream 从 unregistered 翻为 traced。
- 已 traced parser 扩展消费范围 → `consumed_bytes` 上升。
- byte-audit framework schema 演进 → JSON 字段变化。

刷新流程：
1. 在 PR 分支上重新跑 `cargo run ... --byte-audit --json` 覆盖 baseline。
2. PR description 里说明哪些 stream 状态翻转、为什么是合法 improvement
   而非 regression。
3. CI 自然通过（因为 baseline 与当前一致）。

不刷 baseline 的话，`check-byte-audit-baselines.sh` 会把 improvement 也
报为 diff（取决于 `compare_byte_audit_reports` 的 regression vs
improvement 分类），CI 行为视具体 case 而定。

## publish-diff 已接受差异（`publish-diff-accepted.json`）

`publish-diff-accepted.json` 是 `pid_publish_xml --diff-against` 的
**已接受差异**基线：每条差异逐项登记，`tests/publish_diff_accepted_baseline.rs`
读它并与实跑 stdout 逐条比对。**它是记录，不是豁免机制** —— A01 仍然按设计
退出 1；基线的作用是让「已登记」「已消失」「未登记」三种状态分得开。

### 三种状态

| 状态 | 含义 | 判定 |
|---|---|---|
| registered | 基线列了这一行，实跑也打印了 | 正常 |
| disappeared | 基线列了这一行，实跑**没有**打印 | **失败**：图形在基线之下变了 |
| unregistered | 实跑打印了一行，基线里没有 | **失败** |

**没有计数式豁免**：某个类目的总数恰好对上，不等于该类目被接受。任何一条
未登记差异都让门禁退出 1。

### 已登记的图形

- **A01** —— `test-file/export-test/publish-data/A01/A01.pid` 对
  `A01_Data.xml`（`--plant TEST02`）。退出 **1 按设计**。共 **27 项**：
  18 项生成侧（1 类判错、1 多发对象、3 处 `IDrawingRepresentation.GraphicOID`
  值不同、6 处参考里没有的值、7 条多发关系）+ 9 项仅参考侧（3 个 missing
  对象、6 条 missing 关系）。
  - 接口名是 `IDrawingRepresentation`，**不是** `IDrawingPresentation`。
  - 复现时**不要**加 `--out-dir`：那会往 stdout 多打三行 `meta ...`。
- **I井场** —— 对 `D:\work\cad\PID自研软件验证` 里的厂商参考，`.pid` 单张
  与 `--oracle-exp` + `--datamap` 两种模式。**路径敏感**：图名由**文件名**
  派生，所以断言写成**析取**，两种结果都合法，基线两种下都逐项可核：
  - 长中文名到达进程 → 退出 **0**、`different 0`、不打印任何差异行；
  - 8.3 短名 `IB031~1.PID` 到达进程 → 退出 **1**，且**恰好一处**差异：
    `value PIDDrawing 5633AF4121EE4F4D9FE78E90722CC050 IObject.Name: "IB031~1" reference "I井场 注采阀组工艺及自控流程图"`。
  - **不要**写成「必须出现」或「必须 clean」，两种写法都会 flaky。

### 不在这里登记的东西

- 整类豁免的属性类目在 `src/publish/diff_exclusions.rs`
  （`EXCLUDED_ATTRIBUTE_CLASSES`），以 `excused ...` 行打印。
- 整包的 `FluidSystem` 上界在 `src/publish/diff.rs` 的
  `FLUID_SYSTEM_REFERENCE_EMPTY_ALLOWANCE`；`whole_package.registered_exceptions`
  只**指向**它，不另立第二个界。
- `reference-only:` 与 `excused ...` 是账目行，不是差异，不登记。

### 如何核对 / 刷新

```powershell
# 逐项核对（基线文件里的 command 字段就是这条）
D:\Rust\target-pid-mission\debug\pid_publish_xml.exe `
  test-file\export-test\publish-data\A01\A01.pid `
  --diff-against test-file\export-test\publish-data\A01\A01_Data.xml --plant TEST02
# 消费这份基线的测试
cargo test --locked -p pid-parse --test publish_diff_accepted_baseline
```

刷新只在**合法**改变输出时做，且必须在同一个改动里同时更新基线与
`tests/publish_diff_accepted_baseline.rs` 里的计数断言；diff 里要能看出
哪一项是新增的已接受差异、为什么。**不要**为了让门禁变绿而往基线里加条目。

## 出图记账基线（`export-ledger-*.json`）

`pid-plot` 的出图记账（`.kiro/specs/pdf-export-ledger/`）给每一页记一本账：
页上的每条图形记录、实体、页面图元、笔画段，要么上了页，要么归到一个有名字的
丢弃原因并计数。两份基线把这个账逐页钉住：

| 文件 | 语料 | 页数 | 门禁 |
|---|---|---:|---|
| `export-ledger-fixtures.json` | 仓库 `test-file/` 的 7 张夹具 + `test-file/symbols` + `test-file/Ref/Symbols` + `test-file/backup-test/*_p/RefData~*.zip` | 7 | `pid-plot/tests/export_ledger_fixtures.rs`（**CI 与本机都真跑**） |
| `export-ledger-backup.json` | 备份包语料 `backup-plant-data/**/*.pid`（53 张），模板与符号来自包内 `RefData~*.zip`，字型来自 `PID_PLOT_FONTS` | 53 | `pid-plot/tests/export_plant_backup.rs` 的账本段（无备份时软跳过） |

页键是**相对语料根的路径**（`/` 分隔，可含中文），文件头记 `schema`、
`corpus`、`CorpusDescriptor`（符号来源、模板来源、字型键名）与 `fonts_measured`。
基线只含原因码、计数、流路径、类型码（`0xNNNN`）、oid、字节范围、模板文件名与
页路径 —— **不含图面文字内容，也不含本机绝对路径**（门禁会扫描仓库根、
`CARGO_MANIFEST_DIR`、用户主目录与盘符路径）。

### 比对规则

比对的单位是**计数项**（页、账域、层、原因或标记、键），不是总数。

| 档 | 数量 | 规则 |
|---|---:|---|
| 可疑（`Suspect_Drop`） | 22 | **只许降**：升高即失败并列出页、原因码、基线值、当前值与多出的来源；降低也失败，并提示把基线降到新值（与 `missing_docs` 棘轮同一约定） |
| 有意（`Intentional_Drop`） | 21 | **精确钉住**：任何变化（升或降）都失败（`run_gate` 里用精确比较，不是「只许降」） |

另外：

- **段总数与已写数不比。** 它们照写进基线供人读，但一条弧展平成几段取决于
  `kurbo` 的浮点算术，跨平台可能在末位不同（设计偏差 I4）。
- **上页数、部分上页数与交叉计数**（按种类 / 置信度 / 结局）精确钉住。
- **字体部分分开处理。** 只有字型不同时只跳过两个字体标记
  （`text_condensed`、`text_clipped_at_floor`）；符号或模板部分不同时跳过整页
  计数比对，打印以 `export-ledger:` 开头的醒目段落（逐项差异 + 应设的变量），
  但未归因与核对照常执行，未归因大于 0 仍失败。
- **未归因必须为 0。** 任何一页任何一层（解析、排版、符号体、写出、段）的
  `unattributed` 大于 0 即失败。

### 两道门禁的调用命令

```powershell
# 夹具门禁：7 张夹具，无字体需求，CI 与本机都真跑
cargo test --locked -p pid-plot --test export_ledger_fixtures

# 备份门禁：53 张备份，需要备份语料与字型
$env:PID_PARSE_SELF_CHECK_DIR = 'D:\work\plant-code\cad\pid-test-data'
$env:PID_PLOT_FONTS = 'D:\work\cad\PID自研软件验证\fonts'
cargo test --locked -p pid-plot --test export_plant_backup
```

`export_plant_backup` 的账本段复用同一次运行的 `PaperModel` 与写出结果，
不重复排版、不重复写出。备份目录不在时整个测试照旧软跳过。

### 重写方法与拒绝条件

设 `UPDATE_EXPORT_LEDGER=1` 重写对应基线：

```powershell
$env:UPDATE_EXPORT_LEDGER = '1'
cargo test --locked -p pid-plot --test export_ledger_fixtures     # 写 7 页
# 或（备份语料下）
cargo test --locked -p pid-plot --test export_plant_backup         # 写 53 页
Remove-Item Env:UPDATE_EXPORT_LEDGER
```

重写**只许可疑项不升**（Property 11）：若重写会让某页某可疑项高于原基线，
拒绝写入并失败，列出这些计数，文件保持原样。以下情形同样拒绝：

- 基线文件读不出、解析失败或 `schema` 不同 → 提示删除后重建；
- 语料描述的符号或模板部分与旧基线不同；
- 有失败页、未归因大于 0 或核对不符。

重写两次逐字节相同（`serde_json::to_string_pretty` + 末尾换行 + `std::fs::write`，
映射一律 `BTreeMap`，不含浮点数）。

**不要为了让门禁变绿而放宽规则。** 可疑项降低是改进，但降基线必须是一次显式的
`UPDATE_EXPORT_LEDGER=1` 重写，好让降幅留在 git 历史里。
