# 开发计划：Phase 9k — Ship v0.4.2 + P3 cleanups

> 起稿：2026-04-21
> 背景：`3a2ecde`（`feat(writer): --apply-plan ...`）已完整实施 Phase "9i+" 候选里的 `--apply-plan` 条目，`CHANGELOG.md` 的 `[Unreleased]` 段对应的代码 / 测试 / 文档就绪（`cargo test` 260 green 已本地复核）。本地领先 `origin/main` 两个 commit（`48135a8` layout 语义关键词 + `3a2ecde` apply-plan）均未 push、未 tag。
> `phase8-9h-summary.md` / `2026-04-19-layout-symbol-hint-p2-fixes.md` 列出的 P3 cleanups 4 条仍未动。Phase 9i（v0.3.11）已把 `cargo fmt` 清理 + CI `cargo fmt --all -- --check` 的 hard-fail 做完，这条无需重做。
> 本 Phase 做的事：**把 apply-plan 合法地 ship 成 v0.4.2**（bump + CHANGELOG rename + commit + tag），**再消化 P3 cleanups 中的 3 条低风险项**（延后 representative_symbol_hints 缓存到下一轮）。

## 动机

- 代码已完工但未 ship：本地 2 个 commit 未 push、Cargo.toml `version = "0.4.1"` 未 bump，违背 Phase 9i 起"CI 是最权威的质量通道"的原则（`cargo fmt --check` 和 `cargo clippy -D warnings` 在本地可能漂移，必须先本地确认再依赖 CI）
- P3 cleanups 是 v0.4.1 / v0.4.2 两轮新功能后的自然"卫生窗口"（Phase 9d 方法论）：连续做功能后停下来扫地一次，把技术债水位抬回清零
- 本机 nightly toolchain 目前 `cargo clippy` / `cargo fmt` binary 存在但 DLL 缺失（孤立 rustup 安装残留），本 Phase 顺手修掉，保证下次本地跑 CI 前置检查不再卡

## 非目标

- 不做 `representative_symbol_hints` 缓存（需要给 `PidDocument` 加 `OnceCell<_>` 字段，触及 public API 且当前调用频率低，留待下一个性能 Phase）
- 不做 PSMclustertable 逆向 / SummaryInformation 回写（各自专门大 Phase）
- 不做 base64 / cfb / quick-xml 等依赖 bump（锁定版本策略，单独 PR）
- 不改 CI workflow（Phase 9i 已完成 `-D warnings` + `cargo fmt --check`，本 Phase 只验证其继续通过）

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `Cargo.toml` | version `0.4.1` → `0.4.2` | ±1 |
| `CHANGELOG.md` | `[Unreleased]` → `[0.4.2] - 2026-04-21`，前置空 `[Unreleased]` 段 | ±5 |
| `src/inspect/diff.rs` | 11 处 `writeln!(...).unwrap()` 改为 `String::push_str` / `write_fmt` pattern | ~25 |
| `src/layout.rs` | `file_stem()` 前加路径分隔符归一化 helper | ~10 |
| `tests/writer_real_files.rs` | 2 条函数内 `use` 提升到文件顶部 | ±4 |
| `tests/writer_validate_cli.rs` | 1 条函数内 `use` 提升到文件顶部 | ±2 |
| **本计划文件** | 起草本 plan | +本文件 |

合计 ~50 行代码改动 + 1 个 ship commit。完全不碰 lib public API，不改 Writer 层，不触 H7CAD。

## 关键决策

### A. `writeln!(&mut String, ...).unwrap()` 如何替代

11 处位于 `src/inspect/diff.rs`，例如：

```rust
writeln!(out, "=== Package Diff ===").unwrap();
writeln!(out, "\n--- Modified Streams ---").unwrap();
```

**事实**：`String` 的 `fmt::Write` impl 永不 fail，所以这些 `.unwrap()` 理论上是死代码路径。但 `clippy::unwrap_used` 和 code review 常年会标记，读者每次都需要"验证它不会 panic"才能安心。

**方案选择**：
- 候选 1（直接 `write!` 去 `.unwrap()` 改 `let _ = ...`）—— 治标不治本，读者还是要疑惑
- 候选 2（`out.push_str("...\n")` 对纯字符串 + `out.push_str(&format!(...))` 对变量）—— 语义更直观，没有 `write!` macro 的复杂性
- 候选 3（自定义 `macro_rules! pushln { ($buf:expr, $($arg:tt)*) => { $buf.push_str(&format!($($arg)*)); $buf.push('\n'); } }`）—— 封装感好但引入新 macro

**选候选 2**：P3 应该最小侵入，push_str 是 stdlib 最基础 API，阅读零成本。例如：

```rust
out.push_str("=== Package Diff ===\n");
out.push_str(&format!("\nFirst mismatch offset: 0x{:x}\n", m.first_mismatch_offset));
```

### B. `file_stem()` 跨平台归一化的范围

`src/layout.rs:582` 当前只用于构造语义 haystack。问题路径形如 `\\srv\sym\管件\球阀.sym`：
- Windows：`Path::new(..).file_stem()` = `Some("球阀")` ✓
- Linux：没有 `\\` 分隔符概念，整个字符串被视作一个文件名，`file_stem()` = `Some("\\\\srv\\sym\\管件\\球阀")`

**现状**：因为 `SEMANTIC_KEYWORDS` 里同时有英文和中文关键词，两边都会命中 `"球阀"`，**行为实际等价**。但：
1. 字符串里多出的 `\\srv\\sym\\管件` 段会造成语义污染，`\path` 片段若包含 `valve` 英文会无差别被命中
2. Linux CI 上跑中文 fixture 时 log 里看到 escape 字符串会困惑 reader

**方案**：在 `infer_semantic_from_symbol_hint` 里，对 `symbol_path` 做 `.replace('\\', "/")` 后再喂 `Path::new`。保持 haystack 原串末尾追加原始 path（不变），只让 `file_stem()` 看到归一化过的路径。

```rust
let normalized = symbol_path.replace('\\', "/");
if let Some(stem) = Path::new(&normalized).file_stem() {
    haystack.push(' ');
    haystack.push_str(&stem.to_string_lossy().to_ascii_lowercase());
}
```

**回归保障**：`layout::tests::infer_semantic_maps_chinese_symbol_path_to_piping_component` 覆盖 Windows 风格反斜杠路径命中 `piping`，本修改必须让此测试继续绿。

### C. tests use 散落

3 条全部是函数内 `use`，提升到文件顶部。这样改的唯一风险：顶部 import 可能与其他测试函数里的 import 冲突（`Read` trait 被多个函数需要但只有一个函数声明）。实际冲突概率 = 0（所有 3 个 use 的 item 都是独立 trait/type），确认一遍即可。

### D. ship 版本号选 0.4.2

v0.4.1 已于 `d6ddeb2` tag（Phase 8c layout-first 模型 + ergonomic API）。本次 apply-plan 是 minor feat，按 SemVer minor 应 0.5.0，但因为整个 `0.4.x` 系列被规划为 "Writer 层渐进演化 + layout-first 稳定"（见 v0.4.0 release note 的滚动宣告），保持 0.4.x patch 节奏到稳定后 major bump。

**决议**：**v0.4.2**，与 v0.4.1 的 patch 关系正当（仅 CLI 新增 `--apply-plan`，无破坏性改动；`#[serde(default)]` 是向后兼容 superset）。

## 实施步骤

### W1 — 修本机 clippy/rustfmt 环境（前置，~5 min）

```powershell
# 确认 D:\Rust\.cargo\bin 下孤立的 cargo-clippy.exe / cargo-fmt.exe / clippy-driver.exe / rustfmt.exe 全部删除
Get-ChildItem D:\Rust\.cargo\bin | Where-Object { $_.Name -match '^(cargo-clippy|cargo-fmt|clippy-driver|rustfmt)\.exe$' } | Remove-Item
# 然后用 rustup 重装到当前 nightly 工具链
rustup component add clippy rustfmt
# 复核
cargo clippy --version
cargo fmt --version
```

验收：两条 `--version` 能输出且退出码 0。

### W2 — 本地 CI 等价检查（~10 min）

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

验收：三条命令退出码 0、260 tests pass。

**如果 fmt --check 显示漂移**：说明 3a2ecde 的 commit 里某些新增代码没过 fmt（Phase 9i 后理论上不应出现，但仍以事实为准）→ `cargo fmt --all` 后重提交 / amend。这步骤一定要先于 ship commit。

### W3 — P3-3 diff.rs writeln! unwrap 收敛（~15 min）

改 11 处，每处 push_str 或 push_str(&format!(...))。改完跑：

```powershell
cargo test inspect::diff  # 直接打的单元测试（若有）
cargo test --test parse_real_files diff  # 集成测试里涉及 diff 渲染的
cargo test --test writer_roundtrip
cargo test --test writer_real_files
```

验收：全部通过，diff 输出字节级等价（`diff.rs` 没有任何测试断言输出字符串的话，手工跑一次 `pid_inspect --diff a.pid b.pid` 对比肉眼读）。

### W4 — P3-1 file_stem 跨平台归一化（~10 min）

1. 改 `src/layout.rs` `infer_semantic_from_symbol_hint` 里 `Path::new(symbol_path).file_stem()` 前加 `.replace('\\', "/")`
2. 新增单测 `infer_semantic_normalizes_backslash_path_across_platforms`：构造路径 `\\srv\sym\valve.sym` + `"/srv/sym/valve.sym"`，断言两种都命中 `PipingComponent`
3. 跑 `cargo test layout`

### W5 — P3-4 tests use 提升（~5 min）

- `tests/writer_validate_cli.rs`：`use std::io::Read as _;` → 文件顶部
- `tests/writer_real_files.rs`：`use pid_parse::writer::xml_edit::replace_simple_tag_text;` + `use pid_parse::{MetadataUpdates, WritePlan};` → 文件顶部

若其他 `use` 已 import 了 `MetadataUpdates` / `WritePlan`，避免重复。

### W6 — 再次 local CI 等价检查 + ship commit（~10 min）

```powershell
cargo fmt --all -- --check  # 确认所有 P3 改动都过 fmt
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

**预期计数**：184 + 28 + 18 + 8 + 9 + 13 + 1（W4 新增）= 261 tests pass。

然后 bump + ship：

```powershell
# 1. Cargo.toml: 0.4.1 → 0.4.2
# 2. CHANGELOG.md:
#    [Unreleased] → [0.4.2] - 2026-04-21
#    在其上新插入一个空的
#      ## [Unreleased]
#      （占位，下一轮用）
# 3. P3 cleanups 追加到同一个 [0.4.2] 段的 ### Changed / ### Tests 两节
cargo test --all-targets   # 再复核一次
git add -A
git commit -m "chore(release): v0.4.2 — ship --apply-plan + P3 cleanups (Phase 9k)"
git tag v0.4.2
```

### W7 — push 决策点

**不直接 push**，在计划验收时把下面的问题留给用户：

- 本地 ship 完毕后，是否 `git push origin main --tags`？
- 若仓库策略是 PR-first，改为 `git push origin HEAD:refs/heads/release/v0.4.2` + `gh pr create`？

本 Phase 在用户确认 push 策略之前停止。

## 预计工时

| 步骤 | 估时 |
|---|---|
| 本 plan 起草 | 已完成 |
| W1 toolchain 修复 | 5 min |
| W2 首轮本地 CI 等价 | 10 min |
| W3 diff.rs unwrap | 15 min |
| W4 file_stem 归一化 + 新 test | 10 min |
| W5 tests use 提升 | 5 min |
| W6 ship commit + tag | 10 min |
| W7 push 决策 | 待用户 |
| **合计（W1-W6）** | **~55 min** |

## 验证清单

- [ ] `cargo clippy --version` / `cargo fmt --version` 退出 0（环境修复）
- [ ] `cargo fmt --all -- --check` 退出 0
- [ ] `cargo clippy --all-targets -- -D warnings` 退出 0
- [ ] `cargo test --all-targets` 全绿、test count = 261（+1 from W4 新测试）
- [ ] `src/inspect/diff.rs` grep `writeln!.*unwrap` 命中数 = 0
- [ ] `src/layout.rs` 新增单元测试命中 `PipingComponent` for 反斜杠 + 正斜杠两种路径
- [ ] tests 目录下 `^\s+use \w` grep 命中数 = 0（函数内 `use` 全部提升）
- [ ] `Cargo.toml` version = "0.4.2"
- [ ] `CHANGELOG.md` 含 `## [0.4.2] - 2026-04-21` 段并列出所有 apply-plan + P3 改动
- [ ] `git tag --list v0.4.2` 有输出
- [ ] `git log --oneline origin/main..HEAD` 有 3 个 commit（48135a8, 3a2ecde, + 本次 ship commit）

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `rustup component add clippy rustfmt` 因孤立 exe 冲突再次失败 | W1 先 `Remove-Item` 明确清理残留后再装；若仍冲突，考虑 `rustup self update` 或切 stable toolchain 本地跑 lint（CI 本身走 stable）|
| `cargo fmt --all -- --check` 显示 3a2ecde 有漂移 | 先跑 `cargo fmt --all`，diff 一眼确认仅是 whitespace，`git commit --amend` 到 3a2ecde 或加单独 `style(apply-plan): fmt` fix 提交 |
| P3-3 `push_str` 改动改错文案（少空格 / 少换行） | 前后对比同一份 fixture 的 diff 输出；若无现成测试，手动 invoke 一次 `pid_inspect --diff a.pid b.pid` 对照 HEAD 版和改后版的 stdout |
| W4 新测试与既有 `infer_semantic_maps_chinese_symbol_path_to_piping_component` 冗余 | 保留两个：中文 fixture 侧重符号库中文化、新测试侧重路径分隔符归一化。不同维度。 |
| 合入 PR 后 CI fail（本地漏测的配置） | 本计划在 W2 / W6 两次本地 CI 等价检查 + P3 改完整各跑一次子集，覆盖度高，预期 CI 首战绿 |
| 0.4.2 SemVer 争议（是否该 0.5.0） | plan A 决策：CLI 新增 + serde field default → backward-compatible superset，patch 合理。留在 `0.4.x` 系列 |

## 回滚

本 Phase 所有改动集中在 6 个文件 + 1 个 ship commit + 1 个 tag：
- 代码回滚：`git reset --hard 3a2ecde`
- tag 回滚：`git tag -d v0.4.2`
- 远端未 push，回滚无对外后果

## Next 候选（跟进排队）

以下**本 Phase 不做**但已备好路径：

- **P3-2 representative_symbol_hints 缓存**：触及 `PidDocument` 字段，可与"layout 大规模性能 pass" 合并成 Phase 10a
- **PSMclustertable per-record 字段精确映射**（1-2 hr，中风险）：需先确认是否有 `DocVersion3`-level 文本冗余作为交叉验证锚点
- **SummaryInformation property-set 回写**（4-8 hr，中风险）：独立大 Phase
- **v0.4.x → 0.5.0 roadmap 定义**：等 0.4.x 稳定几轮后整理
