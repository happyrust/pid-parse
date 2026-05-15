# Verification: Phase 15 PSM 0x00FA GraphicGroup Records

## 验证命令矩阵

| 命令 | 目的 | 通过判据 | 证据存放点 |
|---|---|---|---|
| `cargo run --release --example probe_psm_0x00fa_shape` | 收集 `0x00FA` cross-fixture layout 证据 | exit 0；输出每个 fixture 的 hit count、size distribution、sample payload dump | `progress.jsonl` |
| `cargo test --locked --lib parsers::sheet_records` | parser unit tests | 新增 GraphicGroup tests 全过；无 panic / unwrap 失败 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | adversarial byte matrix | 新 decoder entry 在 truncation/random corpus 下不 panic | `progress.jsonl` |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | cross-fixture integration | 4 fixtures 输出 decoded groups；parser count 与 `SheetGeometry::decoded_graphic_groups` collection count 一致 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files iglines_decoder_emits_decoded_iglines_with_provenance -- --nocapture` | Phase 14 line decoder regression | existing decoded igLine2d count 不退化 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files igsymbols_decoder_emits_decoded_symbols_with_provenance -- --nocapture` | Phase 14 symbol decoder regression | existing decoded symbol count 不退化 | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | 全 workspace 编译 | exit 0，无 warning | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 全测试 | exit 0，所有 crates/tests ok | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint gate | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | formatting gate | exit 0 | `progress.jsonl` |
| `bash .github/scripts/check-missing-docs.sh` | missing_docs ratchet | exit 0，baseline 未上升 | `progress.jsonl` |

## 本轮实际结果

| Gate | Result |
|---|---|
| `cargo run --release --example probe_psm_0x00fa_shape` | PASS |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | PASS; counts DWG-0201=135, DWG-0202=84, 工艺管道及仪表流程-1=125, A01=8 |
| `cargo test --test parser_panic_safety -- --nocapture` | PASS |
| `cargo test --lib graphic_group -- --nocapture` | PASS |
| `cargo build --locked --workspace --all-targets` | PASS |
| `cargo test --locked --workspace --all-targets` | PASS |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | PASS after doc-markdown / `is_multiple_of` cleanup |
| `cargo fmt --all -- --check` | PASS |
| missing-docs ratchet | PASS via equivalent `cargo rustdoc --lib --locked -- -W missing-docs`; WindowsApps `bash.exe` failed locally with `REGDB_E_CLASSNOTREG`, manual count was `current=0`, `baseline=0` |

## 手工 / 半手工核查

| 核查项 | 怎么做 | 通过判据 |
|---|---|---|
| Layout 文档是否克制 | 检查 analysis 文档 | 不确定 tail 没有被命名为 style/color/layer |
| DTO 字段是否稳定 | 检查 `src/model.rs` / schema diff | stable 字段只含已验证 header / raw payload；child list 有证据才暴露 |
| Byte range 是否 bounded | 查 integration test failure message 或 debug output | 每条 decoded group 的 range 在 source stream 内，且长度等于 header + `bytes_to_follow` |
| False positive 风险 | 用 wrong type / random bytes tests | decoder 不在 random bytes 中产生高数量假阳性 |
| Phase 14 不退化 | 跑 Phase 14 representative tests | decoded primitive counts 和 provenance guard 仍通过 |

## Evidence JSONL 规则

每完成一个 acceptance item，在 `progress.jsonl` append 一条 JSON line，例如：

```json
{"type":"verification","timestamp":"2026-05-14T...Z","ac":["AC6"],"command":"cargo test --locked -j 1 --test parse_real_files graphic_groups_decoder_emits_decoded_groups_with_provenance -- --nocapture","exit":0,"artifact":"decoded_group_records=... fixtures=..."}
```

Layout / probe evidence 使用：

```json
{"type":"layout_evidence","timestamp":"2026-05-14T...Z","ac":["AC1","AC2"],"artifact":"docs/analysis/2026-05-14-psm-0x00fa-graphic-group-layout.md","summary":"0x00FA header stable; tail retained raw pending child-list validation"}
```

## 收口检查

merge 或声明完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh
```

任一 gate 失败：停止，记录 blocker，不继续扩大 scope。

Local note: on this Windows machine, `bash` resolves to
`C:\Users\dpc\AppData\Local\Microsoft\WindowsApps\bash.exe` and cannot run
the shell wrapper. Use the underlying rustdoc command from the script to
verify missing-docs until Bash/WSL is repaired.
