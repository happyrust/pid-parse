# 开发计划：Phase 9o — Writer API ergonomics patches

> 起稿：2026-04-21
> 背景：Phase 9k → 9n 四轮 Writer 内部能力补齐之后，现在该回头看
> "从 consumer 侧调用 pid-parse 的手感"。当前 4 个显著的样板折扣：
>   1. `PidPackage` 创建必须写 `PidParser::new().parse_package(&path)`，
>      两步；每个 consumer 都抄一遍。
>   2. `PidPackage` 无法从 `Vec<u8>` 字节流创建，必须先落盘再 parse；
>      H7CAD 侧如果从 HTTP/压缩包/嵌入资源拿到 `.pid` 二进制会很绕。
>   3. `PidWriter::write_to` 只接受 `&Path`，没法直接拿到输出字节 —
>      用户必须落盘再读回，多一次磁盘来回。
>   4. `WritePlan` 的 JSON round-trip 要 consumer 自己 `serde_json::to_string`
>      / `serde_json::from_str` 外加 error-type 转换。
> 本 Phase 加一组**additive**的便利 API 把上面四个缺口补齐，不破坏
> 任何已有签名。
> 目标：v0.5.3 patch ship。

## 动机

- 下游集成（H7CAD / CLI / 其他 binding）重复抄相同样板代码，违反
  "零学习曲线 API"原则。
- 所有改动都是纯加法（new methods），不改任何已有签名，低风险。
- 为 v0.6.0 做准备：本期 ergonomic 后，下期就能考虑 "单一入口" +
  文档升级。

## 非目标

- 不做 async / streaming API（所有 `.pid` 都是 CFB，天然内存级，
  不需要 async）
- 不改 PidParser / PidWriter struct 本身的字段签名
- 不引入新依赖（复用 `std::io::Cursor` + 已有 `cfb` / `serde_json`）
- 不覆盖 error type surface —— 新 helper 全部返回 `PidError`

## 范围

| 文件 | 改动类型 | 行数估计 |
|---|---|---|
| `src/api.rs` | `PidPackage::from_path` / `from_bytes` 便捷构造器 | +60 |
| `src/writer/plan.rs` | `WritePlan::from_json` / `to_json` / `to_json_pretty` | +50 |
| `src/writer/mod.rs` | `PidWriter::write_to_bytes` | +30 |
| `src/writer/cfb_write.rs` | internal `write_package_to_writer<F: Read+Write+Seek>` 泛型化 | +30 |
| `src/lib.rs` | 重新 re-export 新方法（已有 re-export 覆盖 `PidPackage` / `WritePlan`，只是 doc / prelude 更新） | ±5 |
| `tests/writer_roundtrip.rs` | +2 end-to-end 测试：bytes 路径 + plan JSON round-trip | +80 |
| `src/api.rs` / `src/writer/plan.rs` tests mod | +5 unit | +100 |
| `docs/writer-quickstart.md` | "0. 前提" 段或新 0.5 节："Consumer 入门模板" | +40 |
| `CHANGELOG.md` | `[Unreleased]` → `[0.5.3] - 2026-04-21` | +30 |
| `Cargo.toml` | version 0.5.2 → 0.5.3 | ±1 |
| **本 plan** | | +本文件 |

合计 ~430 行，**零** 已有签名破坏。

## 关键设计决策

### A. `PidPackage::from_bytes` 的 Parser 实例复用

目前 `parse_package` 只接受 `&Path`。字节版要把 `&[u8]` 喂给 `cfb::open`
（也是泛型 `CompoundFile::open<F>`）。

实现思路：

```rust
impl PidPackage {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PidError> {
        use std::io::Cursor;
        let cursor = Cursor::new(bytes);
        let parser = PidParser::new();
        parser.parse_package_from_reader(cursor)  // 新内部方法
    }
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, PidError> {
        PidParser::new().parse_package(path.as_ref())
    }
}
```

`parse_package_from_reader` 需要 `impl Read + Seek`。复用 `parse_package`
的内部实现，改成 generic over reader。

### B. `PidWriter::write_to_bytes` 实现

```rust
impl PidWriter {
    pub fn write_to_bytes(package: &PidPackage, plan: &WritePlan) -> Result<Vec<u8>, PidError> {
        let mut working = package.clone();
        metadata_write::apply_metadata_updates(&mut working, &plan.metadata_updates)?;
        for repl in &plan.stream_replacements { ... }
        for patch in &plan.sheet_patches { ... }
        cfb_write::write_package_to_writer(&working, Cursor::new(Vec::new()))
            .map(|cursor| cursor.into_inner())
    }
}
```

`cfb_write::write_package_to_writer<F: Read + Write + Seek>(pkg, writer: F) -> Result<F>` —
泛型化既有 `write_package`。`write_package` 变成对 `write_package_to_writer`
with `File::create(path)` 的 thin wrapper。

### C. `WritePlan::from_json` / `to_json` 错误包装

```rust
impl WritePlan {
    pub fn from_json(s: &str) -> Result<Self, PidError> {
        serde_json::from_str(s).map_err(|e| PidError::ParseFailure {
            context: "WritePlan JSON".into(),
            message: e.to_string(),
        })
    }
    pub fn to_json(&self) -> Result<String, PidError> {
        serde_json::to_string(self).map_err(|e| PidError::ParseFailure {
            context: "WritePlan serialization".into(),
            message: e.to_string(),
        })
    }
    pub fn to_json_pretty(&self) -> Result<String, PidError> {
        serde_json::to_string_pretty(self).map_err(...)
    }
}
```

这样 consumer 不用自己 care `serde_json::Error` 的 variant。

### D. 签名：`from_path` vs `parse`

取 `from_path` 而非 `parse` 名字。理由：
- `from_bytes` / `from_path` 配对，对称
- `PidParser` 已经有 `parse_package`；加 `PidPackage::parse` 会冲突
- `from_path` 在 Rust 生态惯例是"从某个 input 来源构造"的标准动词
  （`std::fs::read_to_string` / `PathBuf::from` / ...）

### E. 不引入 `PidPackage::write_to` 镜像

表面对称好像需要：
```rust
impl PidPackage {
    pub fn write_to(&self, plan: &WritePlan, path: &Path) -> Result<()> { ... }
}
```

但 `WritePlan` 是声明式独立输入，与 package 逻辑上不绑定。保持 
`PidWriter::write_to(pkg, plan, path)` 的三参形式更清晰（哪个是 source
/ 哪个是 mutation / 哪个是 destination 一目了然）。

不做 shortcut。

## 实施步骤

### W1 — 泛型化 write_package → write_package_to_writer

- `src/writer/cfb_write.rs`: 新 pub fn `write_package_to_writer<F: Read + Write + Seek>(package, writer: F) -> Result<F, PidError>`
- 既有 `write_package(pkg, path)` 改为 thin wrapper：`write_package_to_writer(pkg, File::create(path)?).map(|_| ())`
- 验收：既有 passthrough test 继续绿

### W2 — PidWriter::write_to_bytes

- `src/writer/mod.rs` 新 pub fn `write_to_bytes(pkg, plan) -> Result<Vec<u8>>`
- 单测：构造一个 plan → `write_to_bytes` → 字节长度 > 0 + parse 回来 streams 一致

### W3 — PidPackage::from_path / from_bytes

- 需要确认 `PidParser::parse_package` 内部是否已泛型。如果不是，**本 Phase 不拆 parser 泛型化**（scope creep），改为：
  - `from_path` 直接包装既有 `PidParser::new().parse_package(path)`
  - `from_bytes` 先写到 tempfile 再 parse（**临时 hack**）

折中：我一开始预想的是"parse_package_from_reader"优雅版本，但看代码体量这可能是半个重构。先用 tempfile 把 `from_bytes` 兜住，文档里说明未来可能升级到纯内存版本。

### W4 — WritePlan::from_json / to_json / to_json_pretty

- `src/writer/plan.rs` 加 3 个 impl 方法 + 错误包装
- 3 个单测

### W5 — 集成测试 + docs + ship

- `tests/writer_roundtrip.rs` 加 2 个 end-to-end：
  - `write_to_bytes_produces_bytes_parseable_by_from_bytes`
  - `write_plan_json_roundtrip_preserves_every_field`
- `docs/writer-quickstart.md` 新加"Consumer 入门模板"段，展示 5 行代码的 read-edit-write
- `CHANGELOG.md` 加 0.5.3 段
- `Cargo.toml` 0.5.2 → 0.5.3
- commit + tag v0.5.3

## 预计工时

| 步骤 | 估时 |
|---|---|
| W1 cfb_write 泛型化 | 30 min |
| W2 write_to_bytes | 20 min |
| W3 from_path / from_bytes | 30 min（tempfile 版）/ 90 min（parser 泛型化版）|
| W4 WritePlan json helpers | 30 min |
| W5 tests + docs + ship | 40 min |
| **合计** | **~2.5 hr**（走 tempfile），**~4 hr**（走 parser 泛型化）|

## 验证清单

- [ ] 所有既有 `cargo test` 全绿（零回归）
- [ ] test count 287 → 294+（+7）
- [ ] `cargo fmt --check` / `cargo clippy -D warnings` 双零
- [ ] `PidPackage::from_bytes(&bytes)?.parsed.drawing_meta` 能正常工作
- [ ] `PidWriter::write_to_bytes(&pkg, &plan)?` 返回字节长度 > 基础 CFB header (512B)
- [ ] `WritePlan::default().to_json()?` 不报错
- [ ] `WritePlan::from_json("{}")?` 得到 `WritePlan::default()` 等价结构
- [ ] `Cargo.toml` version = "0.5.3"
- [ ] `git tag --list v0.5.3` 有输出

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| `PidParser::parse_package` 泛型化触及内部多个文件，超出 patch scope | W3 走 tempfile 兜底（`std::env::temp_dir` + 随机文件名 + 写 + 解析 + cleanup），未来 Phase 10a 再做 parser reader 泛型化 |
| tempfile 在并发时文件名冲突 | 用 `std::process::id()` + nanos 作前缀，与 writer_validate_cli 已有 fixture 逻辑一致 |
| `serde_json` 错误消息过于冗长，包装后变得更难读 | 只包 context + e.to_string()，保留原始 detail。已有 PidError::ParseFailure 是标准渠道 |
| `write_to_bytes` 对大 `.pid`（几 MB）内存翻倍 | 本期不优化；大文件 consumer 应该用 `write_to(path)` 走磁盘路径 |

## 回滚

所有改动都是 additive，revert commit 即可。没有任何既有 public API
签名变更。

## Next 候选（跟进）

- **Phase 10a**：PidParser 的内部泛型化（让 `from_bytes` 真正走内存），
  同时加 layout 性能基准
- **Phase 9p**：`PidPackage::write_to_bytes` / `write_to_path` 作为镜像
  （如果决定改动 C 节决策）
- **Phase 9q**：`PidWriter::write_to_file` 的 alias，对称 `write_to_bytes`
  （是否需要另议）
