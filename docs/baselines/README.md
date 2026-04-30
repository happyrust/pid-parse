# byte-audit baselines

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
