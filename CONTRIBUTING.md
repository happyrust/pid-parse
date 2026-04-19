# 贡献指南

感谢对 `pid-parse` 的关注。本文记录**经本周期 CI 绿化实战验证过的流程要求**，照着做能一次 push 通过 CI。

## 本地 push 前的 3 件事

```bash
cargo fmt --all               # 自 v0.3.11 起 CI 对 fmt drift 硬失败
cargo clippy --all-targets -- -D warnings  # v0.3.6 起 0 warnings
cargo test                    # 本地有 test-file/ 时跑 172; CI 环境下自动跳过真实样本测试
```

三件都通过才值得 push。`.github/workflows/ci.yml` 做的就是这三项。

## 版本升级（bump Cargo.toml 的 `version`）

**必须同步 Cargo.lock**，否则 CI 在 `--locked` 下因 version 不一致 fail（Phase 9i 实战血泪）：

```bash
# 编辑 Cargo.toml 把 version = "x.y.z" 改成新版本
cargo build                   # 自动更新 Cargo.lock 里 pid-parse 的 version
git add Cargo.toml Cargo.lock
git commit -m "..."
```

## 真实样本测试

真实 `.pid` 文件（SmartPlant 导出）放在 `test-file/` 目录，已加入 `.gitignore`。测试里**必须优雅跳过** fixture 缺失情况：

```rust
let fixture = "test-file/DWG-0201GP06-01.pid";
if !std::path::Path::new(fixture).exists() {
    eprintln!("skipping: fixture {} not found", fixture);
    return;
}
// ... 正常测试逻辑 ...
```

已有模板：`tests/writer_real_files.rs`、`tests/parse_real_files.rs::parse_test_file`（返回 `Option<PidDocument>`）、`tests/unit_parsers.rs::sheet_stream_reuses_cluster_header`。

## Probe / Decode 分层原则

- **Decoded**：字节级结构明确解开的逻辑，通过`confidence = "decoded"` 或没有 confidence 字段（强保证）
- **Heuristic**：启发式探测（如 0x89 marker scan），带 `confidence = "heuristic"`，下游消费方自行判断可用性
- Probe 结果放 `ProbeSummary` 类型（`body_start_offset / marker_count / bytes_scanned`），在报告里显式标 `[PROBE]` 或 `[EXPERIMENTAL/heuristic]`

## Writer 层约定

- 不写语义未知的二进制（SheetPatch 是例外，但要设 `experimental = true`）
- 修改内容走 `WritePlan`，不直接 mutate `PidPackage.streams`（除非你在实现新的 set_* helper）
- 能力矩阵见 [`docs/writer-clsid-and-timestamps.md`](./docs/writer-clsid-and-timestamps.md)
- 完整使用示例见 [`docs/writer-quickstart.md`](./docs/writer-quickstart.md)

## 逆向工作的 cross-check 要求

为 `/Unknown` 流写 parser 时，至少做 2 个独立对齐点再合并：
- 多样本差异对比
- 与已解出的冗余路径交叉（如 DocVersion2 ↔ DocVersion3）
- 与已知外部规范对齐（如 CLSID ↔ Microsoft COM registry）

单一样本 + 无外部对齐的 heuristic 解码应标 `confidence = "heuristic"`。Phase 9f 的 DocVersion2 逆向成功就是"四个独立对齐点"的教科书示例，详见 [`docs/phase8-9h-summary.md`](./docs/phase8-9h-summary.md) 的"方法论沉淀"一节。

## 提交信息（Conventional Commits + 中文 body）

参考既有 commit（`git log --oneline | head -20`）：

- `feat: v0.x.y 功能简述 (Phase Nx)`
- `fix(tests): 具体问题`
- `docs: 简述`
- `style: cargo fmt 类纯格式改动`

body 可以用中文，但类型（`feat` / `fix` / `docs` / `style` / `refactor` / `chore`）保持英文。

## 相关文档

- [`README.md`](./README.md) — 功能概览 + 典型命令
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — 分层架构 + mermaid 图 + 演进路线
- [`CHANGELOG.md`](./CHANGELOG.md) — 完整变更历史（中文）
- [`docs/writer-quickstart.md`](./docs/writer-quickstart.md) — Writer 链路使用入门
- [`docs/writer-clsid-and-timestamps.md`](./docs/writer-clsid-and-timestamps.md) — 容器保真能力矩阵
- [`docs/phase8-9h-summary.md`](./docs/phase8-9h-summary.md) — Writer 建设周期汇总（方法论 + 决策）
