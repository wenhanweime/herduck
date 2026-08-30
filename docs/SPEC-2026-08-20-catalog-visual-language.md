# SPEC：Catalog 视觉语言与会话命名重构

> 状态：v0.1 · 待评审
> 日期：2026-08-20
> 关系：修订 `SPEC-2026-08-19-sidebar-interaction.md` 的 F2，并为
> `SPEC-catalog-hygiene-and-cluster-quality.md` 的命名层补一层展示契约。
> 依据：全部结论来自本轮直接读码与真实 Catalog（`~/.config/ork3-dev/projects/catalog.sqlite3`，
> 28 MB）查询，未使用框架惯例推断。

---

## 0. One-liner

Projects / Clusters 侧栏当前**只画了光标**、**只截了原文**、**只有一套灰**——用户看不出
"哪条是我正在用的"、看不懂会话在做什么、也分不清层级。本 SPEC 定义三件事的产品契约：
**状态可视化**、**会话命名**、**颜色语义**。

**Done when：**

1. 侧栏同时区分四种状态：光标选中 / 当前打开 / 运行中 / 普通，且切到 Terminal 模式后
   "当前打开"仍然可见。
2. 会话行显示 `【主题】任务` 形态的可读标题，而不是首条用户消息的前 96 字符原文。
3. 三层信息（Project / Session / 元数据）有明确的亮度阶梯，元数据不再和标题同色。
4. 侧栏内 `all / live / unclassified` 三个 filter chip 之间有间隙。

---

## 1. 实测诊断

### 1.1 状态可视化缺失（P0）

`src/ui/projects.rs:455`：

```rust
let selected =
    app.mode == crate::app::Mode::Navigate && app.projects.selected_row == absolute_idx;
if selected {
    frame.render_widget(
        Paragraph::new("").style(Style::default().bg(app.palette.surface0)),
        rect,
    );
}
```

三个缺陷，逐条对照 Sessions 侧栏（`src/ui/sidebar.rs:1112`）：

| | Sessions 侧栏 | Projects/Clusters 侧栏 |
|---|---|---|
| 光标选中 | `surface0` 背景 + `text` 前景 + BOLD | 仅 `surface0` 背景，文字样式不变 |
| 当前激活 | `is_active` → `surface_dim` 背景 + BOLD | **完全没有这个概念** |
| 拖拽中 | `surface1` | 不适用 |
| 离开 Navigate | 保留 `is_active` 高亮 | **高亮整条消失** |

即 Sessions 侧栏已经实现了 `selected / is_active / is_dragged` 三态，Projects 侧栏只实现了一态，
且这一态被 `app.mode == Navigate` 门死。用户点进一个 session 进入 Terminal 模式后，侧栏回到
完全无高亮状态——这正是用户报的"选中没有高亮，激活状态也没有对应的高亮状态"。

数据是有的：`ProjectSessionActivation::Live` 已携带 `workspace_id / pane_id /
runtime_generation`（`src/app/state.rs:851`），`AppState::is_active_pane`
（`src/app/state.rs:1805`）已存在，`session.live` 已在渲染路径可用
（`src/ui/projects.rs:496`）。缺的只是把这三者投影成行状态。

### 1.2 会话命名（P0）

ork3 的标题就是**首条足够长的用户消息的前 96 个字符**：

- `src/projects/adapters.rs:1348` `safe_title`：剥离注入前言 → 折叠空白 → `take(TITLE_MAX_CHARS)`，
  `TITLE_MAX_CHARS = 96`（`adapters.rs:1245`）。
- `src/projects/domain.rs:340` `fallback_title`：无标题时退化为 `"Codex session · a1b2c3d4"`。
- 没有主题抽取，没有摘要，没有语言感知截断。

真实 Catalog 前 30 条按时间倒序的标题实测：

```
看下这个看下这个 grok项目 分析目前项目的割裂的问题，然后制定下一阶段的研发规划和spec grok --resume 01a01579-2b40-7e01-86b0-730297b95b
你是本轮审阅席（Claude）。仓库 `/Users/pot/Workspace/ork3`。 任务：把 Grok 计划改成正式三轮审阅。证据已由上一轮 Claude 实测确认，**不要再大范
推荐顺序： 1. Chrome Canary（最合适） bundle ID 是 com.google.Chrome.canary，图标是黄色，Dock / Cmd+Tab 会变成两个 App。
New session - 2026-08-14T12:33:40.778Z
在吗
今天天气如何
```

四类可判定的失败：

- **F-A 重复词与噪音入标题**："看下这个看下这个"、残留的 `grok --resume <uuid>`。
- **F-B 助手输出被当成用户意图**："推荐顺序： 1. Chrome Canary（最合适）…" 是 assistant 文本。
- **F-C 无信息标题占位**："在吗" / "今天天气如何" / `New session - <ISO>`。
- **F-D 硬截断**：96 字符在中文下约等于 96 字，侧栏宽度通常 26–40 列，实际只看得到开头的
  寒暄，看不到任务本身。

对照 `paseo-multica`（`src/semantic/envelopes.mjs`）已解决同一问题，其可移植的四条规则：

1. `cleanIntent`（`envelopes.mjs:27`）：剥离 `Last login:`、shell 提示行、XML 块、
   `claude --resume <uuid>`、UUID、"继续/接着完成"前缀。
2. `isLowSignalIntent`（`:43`）：ping 类、"照原样回复"类、纯 resume、通用任务模板、
   去标点后有效字符 < 3 的，一律不作为标题候选。
3. `selectIntentCandidates`（`:80`）：**全会话**用户轮次去重后按 `informationScore` 排序取前 N，
   再按原始顺序还原——所以标题来自"最有信息量的那一轮"，不是"第一轮"。
   ork3 当前只扫前 4 轮（`TITLE_SCAN_TURNS = 4`），最多 40 轮（`MAX_TITLE_SCAN_TURNS`），
   且只要前 4 轮里出现 ≥12 字符的就停止（`adapters.rs:1336`）——"看下这个"这类正好卡在阈值上。
4. `fallbackTitle`（`:120`）：输出 `【{inferSubject}】{task}`，主题从 `【】`、
   产品名 token、`X项目/系统/模块` 模式、目录名依次推断；task 取首句并截到 58 字符。

ork3 已经有 LLM 语义层（`src/projects/semantic.rs`），且 `semantic_assignments` 里已有
可用的主题（实测 top-3：`agent 会话管理` 54、`paseo-multica 聚类与开发` 51、
`staro 项目迁移开发` 35），**但这些主题只用于 Clusters 分组，没有回流到 Session 行的标题**。
这是本 SPEC 最大的一次性收益：主题已经算出来了，只是没显示。

### 1.3 颜色与层级（P1）

`src/app/state.rs:106` 的 `Palette` 有 16 个 slot，Projects 侧栏只用到 6 个，且用法有三处冲突：

- **Session 标题用 `subtext0`（166,173,200），Project 名用 `text`（205,214,244）** —— 差 39 级亮度，
  在多数终端上肉眼几乎无差别，两层树看起来是平的。
- **元数据 `· codex` 用 `overlay0`(108,112,134)，Project 的会话计数也用 `overlay0`** ——
  不同语义同色。
- **`Thin` 行和 `ScanStatus` 行都用 `surface_dim`(30,30,46)** —— 这是**背景色**，
  在 `panel_bg`(24,24,37) 上前景对比度约 1.1:1，实际不可读。这是明确的可访问性缺陷，
  不是审美问题。
- Session 的 live/history 只靠 `● / ○` 加 `green / overlay0` 区分；backend
  （codex/claude/grok/pi/opencode）**完全没有颜色编码**，五个后端在视觉上不可区分。

### 1.4 Tab 间隙（P1，前一轮的遗留）

`SPEC-2026-08-19` F2 只修了顶层三个视图 tab：`src/ui/projects.rs:282` 引入 `tab_gap`，
测试 `projects.rs:651` 断言不相邻——**这部分已生效**。

但**同一函数下方的 filter chip 没有修**（`projects.rs:303`）：

```rust
let first_width = content.width.min(5);
let second_width = content.width.saturating_sub(first_width).min(6);
let filter_tabs = [
    Rect::new(content.x, controls_y, first_width, 1),
    Rect::new(content.x + first_width, controls_y, second_width, 1),  // ← 无间隙
```

`all`/`live`/`unclassified` 三个 chip 逐列相接，且选中态用 `surface1` 背景块
（`projects.rs:414`），相邻色块直接粘连成一条。

工作区 tab bar（`src/ui/tabs.rs:58`）确实有 `width + 1` 的 1 列间隙，但间隙填的是
`panel_bg`(24,24,37)，非激活 tab 背景是 `surface0`(49,50,68)——25 级亮度差，
在低对比度显示器上等同于没有间隙。

---

## 2. 产品定义

### 2.1 三层信息模型（不变）

保持 `Project → Session` 两层树（PRD Non-goal 不引入第三层）。但**每行承载三个信息层**，
必须有明确的视觉权重顺序：

```
L1 主标识   Project 名 / Session 标题        —— 最亮，用户扫视时读这一层
L2 状态     live / active / 光标 / 分级       —— 用颜色和背景，不占字符宽度
L3 元数据   backend、计数、时间               —— 最暗，只在需要时读
```

当前实现把 L1 和 L3 挤在同一亮度带内，L2 几乎不存在。

### 2.2 行状态机（新）

每个 Session 行在任一时刻处于以下状态的组合：

| 状态 | 判定 | 视觉 |
|---|---|---|
| `Cursor` | `Navigate` 模式且 `selected_row == idx` | `surface1` 背景 + `text` 前景 + BOLD |
| `Open` | 该 session 的 pane 是当前 tab 的 focused pane | `surface0` 背景 + 左沿 `▎` accent 竖条 |
| `Running` | `session.live` 且其 pane agent 状态为运行中 | `● ` 用 `yellow` |
| `Live` | `session.live` 且非运行中 | `● ` 用 `green` |
| `History` | 非 live | `○ ` 用 `overlay0` |
| `Thin` | 短会话折叠行 | `overlay0`（不再用 `surface_dim`） |

关键契约：

- `Open` **不受 `app.mode` 影响**。进入 Terminal 模式后左沿竖条和 `surface0` 背景保留。
  这是修复用户报的"激活状态没有高亮"的核心。
- `Cursor` 与 `Open` 可同时成立：此时背景取 `surface1`（Cursor 优先），竖条保留。
  参照 Sessions 侧栏 `highlighted = selected || is_active || is_dragged` 的既有语义
  （`sidebar.rs:1114`），但把两者的视觉分开而不是共用一个 `highlighted` 布尔。
- 竖条用行首 1 列，不额外消耗标题宽度（当前 Session 行首已有 2 个空格缩进
  `"  ● "`，把第 1 列换成竖条即可，宽度零成本）。

### 2.3 命名契约（新）

Session 显示名按以下优先级取第一个可用值：

1. **语义标题** `【{topic}】{task}` —— `topic` 来自该 session 已有的
   `semantic_assignments.topic_label`（已存在，直接 join）；`task` 来自清洗后的最高信息量
   用户轮次首句。
2. **清洗标题** —— 无语义分配时，用清洗后的最佳用户轮次，不带 `【】`。
3. **后端占位** —— `fallback_title` 现状保留（`Codex session · a1b2c3d4`）。

清洗规则（移植 `paseo-multica`，在 `src/projects/` 内以 Rust 重新实现，不引入 JS 依赖）：

- 剥离：`<tag>...</tag>` 注入块（现有 `strip_injected_preamble` 已做）、
  `(claude|grok|codex) --resume <uuid>`、裸 UUID、shell 提示行、`Last login:`、
  前导"继续/接着（完成|处理|做）"。
- 低信息量过滤：去标点后有效字符 < 3；命中 ping 集合（`在吗`/`hi`/`hello`/`test`）；
  匹配 `New session - <ISO8601>`；通用任务模板。命中者**不作为标题候选**，继续向后找。
- 候选选择：扫描窗口从 `TITLE_SCAN_TURNS = 4` 提升到全部用户轮次（受
  `MAX_TITLE_SCAN_TURNS` 与既有字节预算约束），按信息量打分取最优，而非取最早。
- 截断：`task` 取首句（`。！？!?` 边界），上限 **48 个显示宽度单位**（不是 96 字符），
  按 `display_width` 而非 `chars().count()` 计算——中文占 2 列，当前按字符计算会溢出侧栏两倍。
- 去重：`看下这个看下这个` 这类相邻重复片段折叠一次。

**边界**：不为命名新增 LLM 调用。语义标题只消费 `semantic_assignments` 已有结果；
清洗标题纯离线、确定性、可测。这与 PRD 的"P0 分类必须离线、确定性、可解释"一致。

### 2.4 颜色语义（新）

在 `Palette` 现有 16 slot 内定义 Catalog 的用色表，**不新增 slot、不新增主题**：

| 用途 | 颜色 | 理由 |
|---|---|---|
| Project 名 | `text` + BOLD | L1 最高 |
| Session 标题（语义） | `text` | L1，与 Project 靠 BOLD 与缩进区分，而非靠亮度 |
| Session 标题（清洗/占位） | `subtext0` | 降级信号，用户能看出"这条没被识别" |
| `【topic】` 前缀 | `mauve` | 与任务文本分色，扫视时可按主题成组 |
| backend 标签 | 每后端固定色：codex=`blue` / claude=`peach` / grok=`teal` / pi=`mauve` / opencode=`yellow` | 五后端可区分，这是当前完全缺失的一维 |
| 计数与时间 | `overlay0` | L3 |
| Thin / ScanStatus | `overlay0` | **从 `surface_dim` 改出**，修复 1.1:1 对比度 |
| 分隔线 / 未激活 chip | `overlay0`（前景）/ `surface_dim`（背景） | 背景色只做背景 |

硬约束：**任何前景色不得取 `surface_dim` 或 `panel_bg`**。加一条测试断言把这条钉死。

### 2.5 间隙

- filter chip 之间 1 列间隙，规则与 `sidebar_tabs` 的 `tab_gap` 一致（宽度不足时优先保证
  chip 可读，间隙先降为 0，但不得让两个带背景的 chip 相邻）。
- 工作区 tab bar 的间隙列改用 `surface_dim` 填充，与 `surface0` 的 tab 体形成可见边界，
  而不是与 `panel_bg` 同化。

---

## 3. Scope

### In（P0）

- F1 行状态机：`Open` 状态与 accent 竖条，脱离 `Mode::Navigate` 门。
- F2 `Cursor` 与 `Open` 视觉分离（`surface1` vs `surface0`）。
- F3 语义标题回流：Session 行显示 `【topic】task`。
- F4 标题清洗与按显示宽度截断。
- F5 filter chip 间隙。
- F6 前景色禁用背景 slot（含 Thin/ScanStatus 改色）。

### In（P1）

- F7 backend 颜色编码。
- F8 tab bar 间隙填充色。
- F9 `Running` 与 `Live` 分色。

### Non-goals

- 不引入新主题/新 Palette slot，不做主题切换 UI。
- 不为命名新增 LLM 调用或新的扫描 pass。
- 不改变 Session 身份、排序、分页、归属逻辑。
- 不引入第三层树。
- 不改 Sessions（Spaces/Agents）侧栏的既有视觉，本轮只对齐 Projects/Clusters 到它的水准。

---

## 4. Boundaries

- **Always**：`Open` 状态只从 server 拥有的 runtime mapping 推导（`workspace_id` + `pane_id` +
  `runtime_generation` 三者齐全），generation 不匹配时视为非 Open——与
  `SPEC-2026-08-20-project-session-resume.md` 的只读回退保持一致。
- **Always**：标题清洗是纯函数，输入是候选文本，无 IO，可单测。
- **Never**：不把颜色、行状态、竖条这类展示事实写进 server 状态或 JSON API。按 CLAUDE.md
  的 runtime/client 边界，这些是 TUI presentation state。
- **Never**：历史 session 的任何渲染路径写 PTY。

---

## 5. 验收

```bash
cd /Users/pot/Workspace/ork3
just check
```

专项断言（新增测试名）：

| 测试 | 断言 |
|---|---|
| `open_session_row_stays_highlighted_outside_navigate_mode` | 切到 `Mode::Terminal` 后，Open 行仍有 `surface0` 背景与 accent 竖条 |
| `cursor_and_open_rows_use_distinct_backgrounds` | 两者背景色不相等 |
| `stale_runtime_generation_is_not_open` | generation 不匹配 → 无 Open 视觉 |
| `semantic_topic_prefixes_session_title` | 有 `topic_label` 的 session 行渲染出 `【…】` |
| `low_signal_turns_are_skipped_for_title` | `在吗` / `New session - <ISO>` 不成为标题 |
| `resume_command_and_uuid_are_stripped_from_title` | `grok --resume <uuid>` 不出现在标题 |
| `title_truncates_by_display_width_not_char_count` | 全中文标题渲染宽度 ≤ 48 列 |
| `filter_chips_do_not_abut` | `filter_tabs[1].x > filter_tabs[0].right()` |
| `no_foreground_uses_background_palette_slot` | 遍历 Projects 渲染产生的 buffer，断言无 cell 前景 == `surface_dim` 或 `panel_bg` |

---

## 6. 实施顺序

1. **F5 + F6**（纯渲染，无状态）—— 最小改动，先让侧栏可读。
2. **F1 + F2**（行状态机）—— 需要把 runtime mapping 投影进 `project_tree_rows`。
3. **F3 + F4**（命名）—— 触及 `adapters.rs` 标题扫描与 catalog join，风险最高，单独一轮。
4. **F7–F9**（P1 打磨）。

按 CLAUDE.md，第 3 步触及持久化标题与 adapter 契约，属 refactor-risk，需先命名受保护行为
并补 characterization 测试再动。
