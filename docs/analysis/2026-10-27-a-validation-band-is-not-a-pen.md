# 一条 10 mm 的笔宽：那是 `BandAid` 校验带，不是笔（2026-10-27）

上一轮整包出图时，`Q井场` 的笔宽直方图里有一条谁都解释不了的读数：

```text
pens [0.130:180 0.180:844 0.350:2423 0.700:271 10.000:9 default:636]
```

53 张图里最宽的正经笔是 0.70 mm（个别模板体内 0.8），而这一张有 **9 条 10.000 mm**。
一条 10 mm 的线画在 594 mm 高的纸上，等于把一段 3.8 × 7.6 mm 的阀门本体涂成黑块。本轮把这
9 条查到底：它们不是解码错，而是**图纸自己的样式表里的一条校验标记**，被当成了笔。

## 一、落点：一个放置，一条样式，一个实体

`probe_which_style_paints_a_wide_pen`（本轮新增）按笔宽列出图纸的样式表：

```text
Q井场 注采阀组工艺及自控流程图: 3974 entities, 2746 resolved line styles
   10.000 mm  style   92  BandAid                      rgb [255, 167, 119]  entities 1
    0.700 mm  style   63  Primary Piping - New         rgb [128, 128, 0]  entities 280
    0.350 mm  style   64  Secondary Piping - New       rgb [128, 128, 0]  entities 51
    0.350 mm  style    1  Normal                       rgb [0, 0, 0]  entities 539
    …
```

- 宽笔只有 `style 92` 一条，**只画 1 个实体**。
- 那个实体是 `2 Way Globe Type 3.sym` 的一个放置：`Sheet6` oid 5620，插入点
  (269.11, 362.92) mm，旋转 90°；解析后的样式名就叫 `BandAid`，色 `[255,167,119]`（鲑色）。
- 同一个样式表里还有 `style 61 "lsOk"`（0.1 mm，黑）与 `style 81 "lsWarning"`（0.1 mm，蓝）——
  这正是 `src/style_link.rs` 早就记过的那批状态样式（`ls…` / `ps…` 前缀），
  也就是 `SmartPlant` 的校验引擎写给工程师看、而不是写给绘图机看的那几条。

## 二、不是解错了 `.sym`

第一嫌疑是符号体自己带 10 mm。`dump_symbol_geometry` 直接否掉：那份 `.sym` 的 11 条线全是
`@000000:13`，即 **0.13 mm**。

```text
$ cargo run --example dump_symbol_geometry -- …/2 Way Globe Type 3.sym
line,-0.000851694680083717,-0.00042534711932492984,-0.003809999999999999,-0.0019049999999999998,@000000:13
…（11 条，全部 @000000:13）
```

于是 10 mm 只可能来自放置（placement）那一侧，而 `pid-plot/src/model.rs` 的 `body_stroke`
规则确实是「放置的样式整体重绘符号体：颜色与笔宽都盖过体自己的」。这条规则对普通样式是对的
（`Equipment - New` 0.35 mm 等），对一条**校验标记**就错了：它把一个 0.13 mm 的阀门轮廓
改写成 10 mm。

## 三、为什么纸面上确实错了

`write.rs` 的 `pen_width_mm` 把任何声明宽度压到三支笔上（0.169 / 0.339 / 0.678 mm），所以
10 mm 落到最粗那支 —— 看起来「只是粗了 4 倍」，而不是黑块。但同一张纸上其余阀门都是
0.13 mm → 0.169 mm 的细线，**只有这一个阀门被画成粗线**。这是肉眼可见的偏差，也正是
`BandAid` 不该出现在墨里的原因：屏幕上它是一条蒙在被标记对象上的带子，纸上是对象的轮廓。

## 四、修法：一条「是不是笔」的判据

`pid-plot/src/write.rs` 里加在笔表旁边，而不是散在模型里：

```rust
/// The widest width a drafting pen declares in this corpus, in millimetres.
pub const WIDEST_PEN_MM: f64 = 1.0;

/// Whether a declared width is one a plotter pen can carry.
pub fn carries_a_pen(declared_mm: f64) -> bool { … }
```

理由：这套语料的 ISO 128 笔阶梯最高 0.70 mm，`pen_width_mm` 已把它压到最粗笔；超过 1 mm 的
宽度就不是笔了——`BandAid` 的 10 mm 是那条带子的屏幕宽度，比最粗的正当笔宽 14 倍。1.0 mm 留了
0.7 的余量，同时离 10 mm 足够远。

`body_stroke` 只改**宽度**这一项：放置的宽度要经 `carries_a_pen` 才生效，否则体自己的笔宽
（该 `.sym` 的 0.13 mm）说算；颜色仍随放置（单色笔表反正把它着成黑）。宽度的例外写在函数头
注释里，名字与那条带子一起记下来。

## 五、读数

| | 改前 | 改后 |
|---|---|---|
| `Q井场` 笔宽 | `0.130:180 … 10.000:9` | `0.130:189 …`（10.000 消失，9 条回到 0.13） |
| `Q井场` PDF | 672876 B | 672868 B |
| 整包 53 张 | — | 47 张挂框 / 63 放置 / 63 有名字缺口，读数不变 |

新闸门断言（`pid-plot/tests/export_plant_backup.rs`）：**任何图纸自己的墨都不会用比最宽笔更宽的
宽度**（`!primitive.furniture` 范围内的 `stroke.width_mm <= WIDEST_PEN_MM`）。挂框页的
「家具」不在范围内——模板的笔是模板自己的。

## 六、复现

```text
# 谁在画宽笔（按图纸列样式表，宽到细）
cargo run --example probe_which_style_paints_a_wide_pen -- <Q井场.pid>

# 符号体自己的笔宽（证明 10 mm 不是体里的）
cargo run --example dump_symbol_geometry -- "…/Ref/Symbols/Instrumentation/In-Line/Valves/2 Way Common/2 Way Globe Type 3.sym"

# 判据与规则的单元测试
cargo test -p pid-plot --lib carries_a_pen
cargo test -p pid-plot --lib a_placement_pen

# 整包闸门（含新断言）
cargo test -p pid-plot --test export_plant_backup -- --nocapture
```

## 七、边界

- 修的是**符号体上的笔宽**，不是「把带标记的对象丢掉」：对象照画，只是用它自己的笔。
- `BandAid` 的颜色仍进模型（`rgb [255,167,119]`），因为对象的样式确实写着它；单色笔表把它着成
  黑，所以纸面无差别。若哪一轮要「校验标记一律不上纸」，那是另一个问题，本节没答。
- 判据是「比最宽笔宽就是标记」，不是「名字叫 `BandAid` 就是标记」：名字会随项目/语言变，宽度
  阶梯不会。53 张里唯一越过 1 mm 的样式就是这一条，而每条正经笔宽都 ≤0.70 mm。
