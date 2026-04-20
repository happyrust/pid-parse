# 开发计划：layout.rs symbol hint P2 缺陷修复

> 起稿：2026-04-19  
> 背景：本次 code review（参见 `docs/porting-direction-analysis.md` 同日附带的审核结论）在 `4c1cb80 feat: infer pid symbol hints from jsites` 中识别出 3 条 P2 级问题，**无阻塞但影响精度与可移植性**。本计划聚焦这 3 条的修复，保持范围小、可复核。

## 目标

| 编号 | 问题 | 期望 |
|---|---|---|
| ~~P2-1~~ | ~~`infer_symbol_identity` 改写后失容 `file_stem()` 回退~~ | **已撤回**（见下方"审核自纠"） |
| P2-2 | `infer_semantic_from_symbol_hint` 硬编码英文关键词，本地化 symbol 库 → `representative_symbol_hints` 空 | 抽成 `SEMANTIC_KEYWORDS` 常量表，追加常见中文同义词 |
| P2-3 | `representative_symbol_hints` 的 tiebreaker（usage_count 后字典序）无注释、无单测 | 抽 `should_replace_candidate` 函数 + doc comment + 1 个单测 |

## 审核自纠（P2-1 撤回）

初审时以为 `4c1cb80` 删除 `Path::file_stem()` 回退会丢失细粒度 symbol_name（例如 `"BallValve"`），从而影响下游。实施 M1 后复读 `bounds_for_item`（`src/layout.rs:569-584`）发现：

```rust
let (half_w, half_h) = match semantic {
    "Pipeline" | "PIDPipeline" | "PipeRun" => (50.0, 8.0),
    ...
    "PipingComponent" | "PIDPipingComponent" | "PipingComp" => (18.0, 18.0),
    ...
    _ => (24.0, 16.0),  // fallback
};
```

match arm **只认 6 个语义 tag + 对应 "PID\*" 原值**，不认 `"BallValve"` 这种细粒度 stem。如果恢复 file_stem 回退让 symbol_name 变成 `"BallValve"`，反而会 **fall through 到默认 (24, 16)**，丢失 PipingComponent 本来能命中的 (18, 18)。4c1cb80 的"坍塌到语义 tag"是**正向**设计。

需要细粒度展示时，下游消费者（H7CAD pid_import / plant3d-web）应**从 symbol_path 现算 file_stem**，这是职责归属正确的地方，不应耦合进 `PidLayoutItem.symbol_name`。

**决策**：M1 撤销；实际范围只剩 M2 + M3。

**非目标**：
- 不改 `build_layout_model` 顶层结构
- 不改 `PidLayoutItem` / `PidLayoutModel` 字段
- 不引入新 crate 依赖
- 不动 `bounds_for_item` 的尺寸表（虽然是 P2-1 回归的下游受害者，修 P2-1 后自动恢复）

## 任务拆分与验收

### M1 — 恢复 file_stem 回退（P2-1）

**改动点**：`src/layout.rs` `infer_symbol_identity`

当前（4c1cb80）：
```rust
let symbol_name = symbol_name_for_type(object.item_type.as_str()).or_else(|| {
    direct_symbol_path
        .as_ref()
        .and_then(|path| infer_semantic_from_symbol_hint(None, path).map(|semantic| semantic.to_string()))
});
```

改为：
```rust
let symbol_name = symbol_name_for_type(object.item_type.as_str())
    .or_else(|| direct_symbol_path.as_ref().and_then(|path| {
        infer_semantic_from_symbol_hint(None, path).map(|s| s.to_string())
    }))
    .or_else(|| direct_symbol_path.as_ref().and_then(|path| {
        Path::new(path)
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
    }));
```

**测试**：在 `layout.rs` 的 `#[cfg(test)] mod tests` 添加：
- 给 PidObject 的 `extra` 直接塞一条 `"symbol_path" = r"\\srv\sym\Piping\Valves\BallValve.sym"`
- `symbol_name_for_type(item_type)` 返回 None（例如 item_type = "X-UNKNOWN"）
- 断言生成的 layout item.symbol_name == `Some("BallValve")`（不应是 `"PipingComponent"`）

**验收**：`cargo test --lib layout::tests::` 新增 1 测试通过。

### M2 — 关键词配置化 + 中文同义词（P2-2）

**改动点**：`src/layout.rs` 顶部新增 const + 重写 `infer_semantic_from_symbol_hint`

```rust
const SEMANTIC_KEYWORDS: &[(&str, &[&str])] = &[
    ("OffPageConnector", &["off-drawing", "off drawing", "opc", "接续符", "页间连接"]),
    ("Nozzle",          &["nozzle", "喷嘴", "管嘴"]),
    ("Instrument",      &["field mounted", "instrument", "system function", "dcs", "仪表", "现场仪表"]),
    ("Vessel",          &["vessel", "tank", "drum", "容器", "罐", "储罐"]),
    ("Note",            &["note", "annotation", "标注", "注释"]),
    ("PipingComponent", &["cap", "valve", "fitting", "reducer", "elbow", "tee", "flange",
                          "阀", "管件", "法兰", "弯头", "三通", "异径"]),
];

fn infer_semantic_from_symbol_hint(symbol_name: Option<&str>, symbol_path: &str) -> Option<&'static str> {
    let mut haystack = symbol_path.to_ascii_lowercase(); // 注意：中文 to_ascii_lowercase 是 no-op
    if let Some(name) = symbol_name {
        haystack.push(' ');
        haystack.push_str(&name.to_ascii_lowercase());
    } else if let Some(stem) = Path::new(symbol_path).file_stem() {
        haystack.push(' ');
        haystack.push_str(&stem.to_string_lossy().to_ascii_lowercase());
    }

    for (tag, keywords) in SEMANTIC_KEYWORDS {
        if keywords.iter().any(|k| haystack.contains(k)) {
            return Some(*tag);
        }
    }
    None
}
```

**设计注记**：
- 中文关键词原样留存（不 lowercase 也能匹配，中文无大小写）
- 顺序保留 `OffPageConnector → Nozzle → Instrument → Vessel → Note → PipingComponent` 与现行代码一致（`OPC` 出现在 `PipingComponent` 之前，避免 `"OPC valve"` 类 edge case 被归入 PipingComponent）
- 查找 O(tags × keywords) ≈ 6 × 6 = 36 次 contains/call，和原代码差不多

**测试**：新增 2 个单测：
1. `infer_semantic_from_chinese_path_maps_to_correct_tag`：对 `r"\\srv\sym\管件\球阀.sym"` 返回 `Some("PipingComponent")`
2. `infer_semantic_ordering_opc_before_piping`：对 `"OPC-valve.sym"` 返回 `Some("OffPageConnector")` 而非 `"PipingComponent"`

**验收**：新增 2 测试通过 + `representative_symbol_hints` 已有测试全绿。

### M3 — tiebreaker 显式化（P2-3）

**改动点**：`src/layout.rs` `representative_symbol_hints` 内部

提取函数：
```rust
/// 在 `representative_symbol_hints` 的 candidate 选择中决定是否替换既有代表。
///
/// 规则：
/// 1. 新候选的 usage_count 更大 → 替换
/// 2. usage_count 相同，新候选 symbol_path 字典序更小 → 替换
/// 3. 否则保留原来的
///
/// 规则 2 作为稳定 tiebreaker：保证同一份输入在不同运行/平台得到
/// 确定的代表 symbol_path，便于 diff 与回归测试。
fn should_replace_representative(
    existing_count: usize,
    existing_path: &str,
    candidate_count: usize,
    candidate_path: &str,
) -> bool {
    candidate_count > existing_count
        || (candidate_count == existing_count && candidate_path < existing_path)
}
```

**测试**：新增 1 单测：
- `should_replace_representative_covers_all_three_branches`：构造 (count=1, "A") + (count=2, "Z") + (count=2, "A") 三种 candidate，断言替换/保留语义

**验收**：新增 1 测试通过。

### M4 — fmt / test / changelog / commit

**步骤**：
1. `cargo fmt --all`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --lib layout` + `cargo test --all`
4. 追加 CHANGELOG.md 新条目（放在 v0.4.1 之后的 Unreleased 段）：
   ```
   ## [Unreleased]
   ### Fixed
   - layout: restore file_stem fallback for symbol_name detail (regressed in 4c1cb80)
   - layout: semantic keyword table supports Chinese synonyms for localized symbol libraries
   ### Changed
   - layout: extract `should_replace_representative` helper for explicit tiebreaker semantics
   ```
5. `git add -A && git commit -F .git/COMMIT_MSG_TMP.txt`
6. 同步面板

## 预计工时

| 步骤 | 估时 |
|---|---|
| 写本 plan | 已完成 |
| M1 file_stem 回退 + 测试 | 15 min |
| M2 配置化 + 中文 + 测试 | 30 min |
| M3 tiebreaker 抽函数 + 测试 | 15 min |
| M4 fmt/test/changelog/commit | 15 min |
| **合计** | **~1h 15min** |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| M1 恢复 file_stem 后某些 downstream 如果依赖"semantic 化"的 6 个 tag 会变回细粒度 | 检查：`grep -r "PipingComponent\|OffPageConnector"` 看其他 crate 是否有字符串级依赖。本地 grep 确认无（仅 layout.rs 内部）→ 低风险 |
| M2 中文关键词列表未能覆盖所有方言 | 作为 config 抽出后，未来新增只需 PR 加一行；本轮先加 6 种常见的 |
| M3 改函数导致已有 3 条 layout 测试漂移 | 保留函数调用不变，只是把 inline 表达式替换为函数调用；行为完全等价 |

## 回滚方案

所有改动仅在 `src/layout.rs` 一个文件。出问题 `git revert HEAD` 即可。无数据库迁移、无 public API break。

## 下一步（不在本 plan 范围内但排队）

- **P3 的 4 条**：file_stem 跨平台、每次 layout 重建 hints、diff.rs writeln.unwrap、测试 use 散落 —— 不紧急，待空期清理
- **WASM 移植方向确认**：`docs/porting-direction-analysis.md` 中列的 A/B/C/D/E/F 还等用户定方向
- **plant3d-web P1 修复**：此前另一轮 code review 发现的 `reviewCommentUpdate/Delete` 静默失败 2 条 P1 —— 不同仓库，单独排期
