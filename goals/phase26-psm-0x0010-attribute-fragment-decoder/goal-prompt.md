# Goal Prompt: Phase 26 PSM 0x0010 Attribute Fragment Decoder

本目录的 goal package 用于启动 Phase 26。准备执行时,把下面 `/goal`
段落粘给执行 agent:

```text
/goal 开始 Phase 26:把 PSM 0x0010 从 Phase 18 的 audit-only raw sub-record 升级为 typed attribute-fragment decoder,提取 SmartPlant 承载在 0x0010 里的工程属性文本(plant tags / line numbers / nominal sizes / drawing refs / annotation text)。本 phase 是 additive typed-audit:与现有 raw decode_sub_records_0x0010(582)并存、不替换,audit-only(不 emit PidGraphicKind),不改 Phase 14–25 任何 baseline。

用 goals/phase26-psm-0x0010-attribute-fragment-decoder/ 作为 durable source of truth:

- 读 brief.md:目标、背景、上下文、约束、非目标、Ask Before、Done Means(AC1–AC9)
- 跟 plan.md:solution overview、DTO/API、保守解码逻辑、Slice A–H
- 跑 verification.md:unit + cross-fixture ratchet + 582 baseline 断言 + 5 道 gate
- 遇 blockers.md 的 Stop-And-Ask 条件立即暂停、写 progress.jsonl、等用户

必读证据:docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md(结构 marker(4)+aux(8)+[u16 len+UTF-16LE] + 跨 5 fixture 验证 + H2 修正 + Phase 19 谜题解释)。

执行顺序:

1. Slice A:新增 examples/probe_attribute_fragment.rs,从 Sheet 流(非全文件扫描)提取 0x0010,按结构解 string,dump 每 fixture 可提取 string 数 + 样本 + 单/多 string 直方图。append progress slice_complete。
2. Slice B(gate):若 dlls/radsrvitem.dll.i64 可在 IDA 打开,analyze_function 追 0x0010 Read 路径确认多 string tail 推进规则 + aux 语义;否则走保守路径(只解第 1 个 string),记 decision。
3. Slice C:src/parsers/sheet_records.rs 新增 SheetAttributeFragmentDecoded + DecodedAttributeString + decode_attribute_fragments + decode_attribute_fragment_at,8–12 unit tests(canonical ODOIL020150 MM / CJK 设计温度 / 单string / 多string(或保守) / 截断 len / 非法 UTF16 回退 / panic)。
4. Slice D:src/model.rs 新增 DecodedAttributeFragment + From impl + SheetGeometry::decoded_attribute_fragments(#[serde(default)]);src/schema.rs needle ratchet。
5. Slice E:src/streams/cluster.rs pipeline 填充;同步 src/cfb/reader.rs + src/geometry.rs fixture init;tests/parse_real_files.rs 新增 attribute_fragment_cross_fixture ratchet(per-fixture string count >= N,N 由 Slice A 定)+ 显式断言 decoded_sub_records_0x0010 == 582 不变。
6. Slice F:tests/parser_panic_safety.rs 加新 decoder adversarial matrix。
7. Slice G:更新 docs/analysis(结构 + 提取统计 + 与 raw decoder 关系 + samples)。
8. Slice H:5 道 pre-commit gate + Phase 14–25 回归确认。

每 Slice append progress.jsonl slice_complete(命令 / 输出摘要 / AC 编号)。

不要做:
- 不 emit PidGraphicKind(promotion 是后续 phase)。
- 不替换/删除现有 decode_sub_records_0x0010 或改 582 baseline。
- 不改 Phase 14–25 任何 stable DTO 既有字段 / collection。
- 不跨 fixture 坐标对齐。
- 不接入新 fixture。
- 多 string tail 在 IDA 未确认前不猜测,走保守单 string 子集。
- 不 commit / push,除非用户明确授权。

完成时(full path,多 string tail 已 IDA 确认)append goal_complete(见 verification.md);保守路径 append partial_complete。然后暂停等用户签收。Promotion 需单独后续 phase。
```

## 启动检查清单

- [ ] `brief.md` / `plan.md` / `verification.md` / `blockers.md` 已读
- [ ] `progress.jsonl` 含 initial scaffold entry
- [ ] 已读 `docs/analysis/2026-05-31-psm-0x0010-ida-recheck-plan.md`(本 phase 证据基础)
- [ ] 已确认 working tree clean(除新增五件套 + progress 外)
- [ ] 已确认 5 道 pre-commit gate 当前全绿(baseline)
- [ ] 已确认 `decoded_sub_records_0x0010 == 582`(raw baseline 起点)
- [ ] 首个执行动作是 Slice A Sheet-stream probe,不是直接写 DTO
- [ ] (建议)Plannotator gate 审批 brief 后再启动 /goal
