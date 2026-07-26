# Phase 35-D：`igTextBox` 字高不在记录内，尾部是样式 id

> 日期：2026-07-26
> 范围：确定 `igTextBox` 的字高（text height）与旋转究竟存在哪里。
> 结论为「记录内无字高」+「尾部 4 字节是跨图纸稳定的样式 id」，
> 未落地解码器改动。

## 结论

1. **字高不在 `igTextBox` 内**。对 146 条记录做全量偏移扫描（head 32
   字节 + tail 36 字节，f64/f32 各按 2 字节步进），**没有任何偏移**能在
   全部记录上落进字高域（0.5–20mm）。此前两轮探针一直在找「记录指向
   样式」的引用，方向对，但缺这一步排除。
2. **12 字节 trailer 的末 4 字节（大端 u32）是样式 id**，前 8 字节恒为
   零。该 id 跨三张互不相关的图纸稳定，且按语义分类。
3. **该 id 不指向 `JStyleOverride`**。既不匹配其 `oid`（1/7、1/7、1/5），
   也不出现在它的任何 u32 列中（DWG-0201 1/7，工艺管道 2/5，命中的都是
   `id=1` / `id=21` 这类小值巧合）。
4. 样式表应是 `/StyleCluster` 流（DWG-0201 17 097 B，工艺管道 10 127 B），
   但 Phase 29 已判定其 prefix 不具备做 parser 的证据
   （`2026-06-08-phase29-stylecluster-prefix-characterization.md`），
   且 42 字节槽区容量放不下 `id=72`。**本轮到此为止**。

## 样式 id 的跨图纸一致性

`examples/probe_igtextbox_style_id.rs`，取 trailer `[8..12]` 作大端 u32：

| id | DWG-0201 | DWG-0202 | 工艺管道及仪表流程-1 |
|---:|---|---|---|
| 1 | `" * "` ×18 | `"H=1600mm "` ×8 | `"E-5801"` ×1 |
| 5 | `"DWG-0201GP06-01"` ×1 | `" "` ×4 | — |
| 21 | `"LIA"` ×17 | `"A3-FA060201"` ×22 | `"N     "` ×24 |
| 56 | `"污水外运"` ×3 | `"量液孔"` ×4 | `"SCV气化器"` ×12 |
| 64 | `"WW-100- -0101-1."` ×4 | `"OD-50-03-0301-1."` ×1 | — |
| 72 | `"  A3-06D01"` ×1 | — | `"0"` ×4 |

`id=56` 在三张图上**全部**对应中文文本，`id=21` 在三张图上都是出现最多
的常规标注，`id=64` 都是管道号。这不是巧合分布，是样式/字体分类。

## 记录内典型字节（DWG-0201 record[1]，text=`"LIA"`）

```text
head[0..32]
  36 00 00 00   oid = 54
  42 00 00 00   parent_ref = 66
  0c 00 00 00   remaining_header = 12（全 fixture 恒定）
  10 00         sub_type_word = 16（恒定）
  0f 00 00 00   index = 15
  02 00 01 00 03 00 01 00 10 00 00 00
                @22 = 3 = text_length（重复），@26 常为 index±1
  03 00         text_length = 3

tail[0..24]    3 × f64 = insertion.x, insertion.y, 1.0（已解码）
trailer[24..36]
  00 00 00 00 00 00 00 00   恒零
  00 00 00 15               大端 u32 = 21 = 样式 id
```

## 后续可走的路

- **需要 IDA 证据**：`/StyleCluster` 流类型 `0x005A` 的 reader/writer，
  解出槽布局后 id 才能解析成字高/字体。这与 Phase 29 已提的 IDA target
  request 是同一个，合并推进即可。
- **不建议**用 `JStyleOverride` 的位置最近邻配对补字高：距离在
  0.002–0.16 间跳动（`probe_igtextbox_style_pairing.rs`），不可靠。
- 在解出真值之前，消费方（如 OpenCADStudio 的 `.pid` 导入）继续用 ISO
  3098 的 2.5mm 默认值；实测该默认值在 A1/A2 图幅上不产生可见重叠，
  可读性瓶颈不在字高。

## 探针

| 探针 | 作用 |
|---|---|
| `examples/probe_igtextbox_field_sweep.rs` | 全量偏移扫描，证明记录内无字高 |
| `examples/probe_igtextbox_style_id.rs` | trailer id 提取 + 与 `JStyleOverride` oid 比对 |
| `examples/probe_style_table_hunt.rs` | 流清单 + id 对样式全 u32 列的比对 |

均为只读，不改 parser、schema、model。
