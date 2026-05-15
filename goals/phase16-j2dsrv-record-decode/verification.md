# Verification: Phase 16 PSM 0x0030 真实归属与 decoder 重写

## 验证命令矩阵

| 命令 | 目的 | 通过判据 | 证据存放点 |
|---|---|---|---|
| `select_instance(port=j2dsrv_port)` + `list_funcs` + `decompile(47FCC338 ClassFactory)` | IDA 拿真实类名 / Save / Load / Validate | RTTI 字符串 / vtable 函数地址记录 | `progress.jsonl` |
| `cargo run --release --example probe_garc2d_packed_bytes` | 复现 §10 触发证据 | exit 0；总 hit ≥ 95；不再有错位 dump | `progress.jsonl` |
| `cargo test --locked --lib parsers::sheet_records` | parser 单测（含重命名后） | 新 DTO 单测全过；移除/替换的旧测试与新名一致 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parser_panic_safety` | adversarial matrix | 新 decoder entry 不 panic | `progress.jsonl` |
| `cargo test --test parse_real_files <new_baseline_test> -- --nocapture` | cross-fixture | 4 fixtures 总 decoded 计数 ∈ [90, 98] | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files iglines_decoder_emits_decoded_iglines_with_provenance -- --nocapture` | Phase 14 igLine 不退化 | DWG-0201=24, DWG-0202=42, 工艺管道-1=243, A01=0 维持 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files igsymbols_decoder_emits_decoded_symbols_with_provenance -- --nocapture` | Phase 14 igSymbol 不退化 | 27 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files iglinestrings_decoder_emits_decoded_polylines_with_provenance -- --nocapture` | Phase 14 igLineString 不退化 | 119 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files igpoints_decoder_emits_decoded_points_with_provenance -- --nocapture` | Phase 14 igPoint 不退化 | 146 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files igtextboxes_decoder_emits_decoded_texts_with_provenance -- --nocapture` | Phase 14 igTextBox 不退化 | 142 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --locked -j 1 --test parse_real_files primitive_line_decoder_emits_decoded_lines_with_provenance -- --nocapture` | Phase 14 GLine2d 不退化 | 3 跨 fixture 维持 | `progress.jsonl` |
| `cargo test --test parse_real_files graphic_group_decoder_ratchets_fixture_counts_and_header_fields -- --nocapture` | Phase 15 GraphicGroup audit 不退化 | DWG-0201=135, DWG-0202=84, 工艺管道-1=125, A01=8 | `progress.jsonl` |
| `cargo build --locked --workspace --all-targets` | 全 workspace 编译 | exit 0，无 warning | `progress.jsonl` |
| `cargo test --locked --workspace --all-targets` | 全测试 | exit 0 | `progress.jsonl` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | lint gate | exit 0 | `progress.jsonl` |
| `cargo fmt --all -- --check` | formatting gate | exit 0 | `progress.jsonl` |
| `cargo rustdoc --lib --locked -- -W missing-docs` | missing_docs ratchet | warning 数 ≤ baseline | `progress.jsonl` |

## 手工 / 半手工核查

| 核查项 | 怎么做 | 通过判据 |
|---|---|---|
| 字段表是否克制 | 检查 new analysis 文档 | 不确定 tail 字段没有被命名为 "rotation" / "sweep" / "arc" 等几何概念，除非有 IDA 证据 |
| DTO 命名是否准确 | 看 Slice C 用户对话 | 与 IDA 反编译拿到的 RTTI 类名一致或经用户确认的合理映射 |
| `geometry.rs` 路径选择是否一致 | git diff `src/geometry.rs` | 要么 emit 新 variant（经用户拍板），要么 0x0030 record 不进 `PidGraphicEntity`（audit-only） |
| 重命名是否同 PR 完成 | git diff 所有受影响文件 | 没有 dangling "primitive_arc" 字符串残留 |
| audit-only tail 是否仍 raw | 检查 stable DTO 字段集 | tail 字段以 `Vec<u8>` / `&[u8]` 暴露，没有被人工解释成结构化字段 |
| Reference 链字段策略 | 检查 stable DTO 与 audit DTO 边界 | 已确认的 referenced PSM type code 可入 stable；未确认的 sub_kind / index 仅 audit |
| Phase 14 baselines 准确 | 看 `tests/parse_real_files.rs` 没有降低 assert 数 | 所有 Phase 14 decoder count assertion 维持原值或更严 |
| `tail+064 == 1.0` 现象记录 | 看 new analysis 文档 | 即使语义未定，也作为已观察规律记录 |

## Evidence JSONL 规则

每完成一个 acceptance item，在 `progress.jsonl` append 一条 JSON line，例如：

```json
{"type":"ida_decompile","timestamp":"2026-05-1?T...Z","ac":["AC1","AC2"],"command":"select_instance(port=...) + decompile(47FCC338)","exit":0,"artifact":"docs/analysis/2026-05-1?-j2dsrv-47FCC338-fields.md","summary":"class name = ?; Save addr = 0x...; Load addr = 0x...; Validate addr = 0x...; field count = ?"}
```

DTO 重命名证据：

```json
{"type":"rename","timestamp":"...","ac":["AC3"],"old":"SheetPrimitiveArcDecoded / decode_primitive_arcs","new":"...","files":["src/parsers/sheet_records.rs","src/model.rs","src/streams/cluster.rs","src/geometry.rs","src/schema.rs","tests/parse_real_files.rs","tests/parser_panic_safety.rs","CHANGELOG.md"],"user_acked":true}
```

Cross-fixture ratchet:

```json
{"type":"verification","timestamp":"...","ac":["AC6"],"command":"cargo test --test parse_real_files <new_baseline_test> -- --nocapture","exit":0,"artifact":"per-fixture decoded count: DWG-0201=N, DWG-0202=N, 工艺管道-1=N, A01=N; total=N"}
```

## 收口检查

merge 或声明完成前按顺序跑：

```powershell
cargo build --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
bash .github/scripts/check-missing-docs.sh    # 若失败用 cargo rustdoc 等价命令
```

任一 gate 失败：停止，记录 blocker，不继续扩大 scope。

Windows 本地 `bash` 问题与 Phase 15 同：用 `cargo rustdoc --lib --locked
-- -W missing-docs` 等价命令，手工核 baseline。

## 完成签名

最后 append：

```json
{"type":"goal_complete","timestamp":"...","decoded_type":"PSM 0x0030 (j2dsrv 47FCC338)","real_class_name":"<IDA 拿到的类名>","fixtures":4,"decoded_record_count":N,"phase14_baselines_preserved":true,"phase15_audit_preserved":true,"gates":"5/5 green"}
```

然后暂停等用户签收，不主动扩到 J2DSrv 其他 12 个 type code 或 0x0010
sub-record。
