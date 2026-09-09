# SPEC：会话标识、行高亮与预览排版三修

> 状态：v0.1 · 待评审
> 日期：2026-08-22
> 关系：修订 `SPEC-2026-08-20-catalog-visual-language.md` 的 F1/F2（高亮）与 F3/F4（命名），
> 并为新增的只读预览补排版契约。
> 依据：全部数字来自 2026-08-22 对真实 Catalog（1,301 条交互式会话）与本机真实 transcript
> 的直接查询与解析探针，非估计。

---

## 0. One-liner

上一轮交付后仍有三处失效：**前标不区分会话**、**点击后高亮被文本覆盖**、**预览正文被工具
噪音淹没**。本 SPEC 定位三者根因并给出修复契约。

**Done when：**

1. 同一 Project 下的会话，行文本彼此可区分——不再出现连续多行完全相同的前标+开头。
2. 点击会话后，整行背景高亮在文字所在的 cell 上依然可见（对齐 herdr Sessions 侧栏）。
3. 预览正文按真实对话顺序呈现人类可读的往返，不再被 tool_result 与注入块占满。

---

## 1. 实测诊断

### 1.1 问题一：前标不区分会话（P0）

**现象**（`herduck` 项目下按时间倒序的真实数据）：

```
herduck | herduck catalog hygiene | 你是本轮审阅席（Claude）。仓库 `/Users/example/Workspace…
herduck | herduck catalog hygiene | 你是本轮 **审阅席**（Claude）。仓库 `/Users/example/Work…
herduck | herduck catalog hygiene | 规划席。仓库 /Users/example/Workspace/herduck。 用户：Pro…
herduck | herduck catalog hygiene | 你是规划/审阅席。仓库 /Users/example/Workspace/herduck。 阅…
```

**双重重复**，两个原因叠加：

**R1 · topic 的粒度等于 Project，不等于 Session。**
`semantic_assignments` 的设计目的是**分组**（Clusters 视图），一个 topic 天然覆盖几十条会话：
实测 `闲聊问候与回复测试` 63 条、`agent 会话管理` 54 条、`sample-project 聚类与开发` 51 条。
把分组标签当成每行前标，等于在同一组内重复打印组名 N 次——**信息量为零**，还挤占了本就
稀缺的侧栏宽度。这是我上一轮的设计错误：我把"已经算好的 topic"当成收益，却没检查它的基数。

**R2 · 标题取"最早的实质轮次"，而派工类会话的开头是固定模板。**
`TitlePicker`（`adapters.rs:1297`）在前 4 轮里选最长的一条。多 Agent 派工会话都以
"你是本轮审阅席（Claude）。仓库 …"开头，真正的任务在模板之后。实测共享前缀分布：

| 条数 | 共享的标题开头 |
|---|---|
| 89 | `New session - 2026-04-10T…` |
| 82 | `hi` |
| 44 | `在吗` |
| 29 | `# AGENTS.md instructions <INSTRUCTIONS>…` |
| 24 | `只回复两个字：收到` |
| 22 | `Grok session · 1451a5b5` |
| 21 | `用中文回复一个简短标题（不超过 12 个字…` |

即 1,301 条交互式会话里，**至少 311 条（24%）的标题开头与其他会话雷同**。

### 1.2 问题二：点击后高亮不可见（P0）

根因在 `src/ui/projects.rs`，两行代码的顺序：

```rust
if cursor || open {
    frame.render_widget(Paragraph::new("").style(Style::default().bg(background)), rect);
}
// …
frame.render_widget(Paragraph::new(line), rect);   // ← 无 style，重置整个 rect 的背景
```

第二个 `Paragraph` **没有携带背景色**。ratatui 渲染 Paragraph 时会用其 style 填充整个 rect，
所以先画的高亮被逐 cell 覆盖。只有竖条 `▎` 幸存——因为它在文本之后才写。

对照 herdr Sessions 侧栏（`src/ui/sidebar.rs:1116`）的做法：**先逐 cell 写背景，且每个 span
的 style 都从 `highlighted` 派生**，文本不会清掉背景。

**我的测试为何没抓到**：`cursor_and_open_rows_use_distinct_backgrounds` 用
`row.iter().find_map(|cell| cell.bg)` 取"该行第一个有背景的 cell"——竖条那一列恰好满足，
于是断言通过。它从未检查**文字所在 cell** 的背景。这是第四次同类假测试。

### 1.3 问题三：预览正文错乱（P0）

解析探针（真实 claude transcript，40 条消息）显示：

```
[0] User:      帮我给pi 配置 cc switch 中的 http://…
[1] Assistant: 我先找一下 pi 和 CC Switch 的配置位置。
[2] Assistant: 找到了两边。先看 cc-switch 里那个端点的配置…
[3] Assistant: pi 已经有一个指向该端点的 CliProxy provider…
… 其余 36 条全是 Assistant
```

**40 条里只有 1 条 User。** 逐后端核查：

**R3 · Claude 把 tool_result 记为 `type:"user"`。**
实测该文件前 8 条 `type:"user"` 记录里，7 条的 content 是 `[{type:"tool_result"}]`。这些是
工具输出，不是人说的话。当前 `read_claude` 无差别接受 `type=="user"`，于是真正的用户轮次
被工具噪音稀释；又因 `first_text` 在 `tool_result` 里找不到 `text` 键而返回 `None`、该轮被
丢弃，最终呈现为"用户只说了一句话，助手自言自语 39 次"。

**R4 · Grok 每个用户轮次都裹在 `<user_query>` 里，且混有 `<user_info>` / `<system-reminder>`。**
实测前 5 条 user 轮次：3 条是纯 `<user_info>`/`<system-reminder>` 机器块，2 条是
`<user_query>\n真正的问题\n</user_query>`。上一轮我加的 `WRAPPER_TAGS` 删掉了整块
`<system-reminder>`，但**没有解包 `<user_query>`**——它的内容恰恰是唯一该保留的部分。

**R5 · Codex 多段 content 只取第一段。**
`first_text` 返回第一个带 `text` 的元素。实测一条真实用户轮次有 2 段 `input_text`：
`[0]` 是 AGENTS.md 注入、`[1]` 是 `<environment_context>`。真实请求若在第 3 段就会被丢弃。

**R6 · 预览按 `\n` 逐行 push 成 `Line`，同时又给 Paragraph 加了 `Wrap`。**
（`ui/projects.rs` 的 `render_project_history`）长行由 Wrap 折行后，行号与滚动量不再对应，
`history_scroll` 的翻页会跳过内容——这是"排版错乱"的第二层。

---

## 2. 修复契约

### F1 · 行文本按「组内唯一」构造 · P0

**topic 前标只在它能区分会话时显示。** 具体规则：

- Directories 视图（Projects）：**不显示 topic 前标**。同一目录下的会话共享 topic 是常态，
  前标只会重复。
- Topics 视图（Clusters）：**同样不显示**——整棵子树就在该 topic 下，父节点已经写了组名。
- 结论：`【topic】` 前标从行文本中**移除**。topic 已由树的父节点承担分组表达，
  这是上一轮的冗余设计。

**标题改为"最能区分本会话的那一段"**：

- 在既有 `strip_injected_preamble` 之外，新增**派工模板前缀剥离**：以
  `你是本轮…席`、`你是规划/审阅席`、`规划席。`、`仓库 <path>。`、`任务：` 等模板开头时，
  跳到模板之后的第一句实质内容。
- 剥离后若仍与**同一 Project 内已渲染的上一行**前 12 个显示宽度单位相同，则继续向后取下一句，
  最多尝试 3 次。这是渲染期的去重，不改持久化标题。

### F2 · 高亮必须覆盖文字所在 cell · P0

对齐 herdr：**行 style 从行状态派生，并传给 Paragraph**，而不是先画背景再被覆盖。

```rust
let row_style = match (cursor, open) {
    (true, _) => Style::default().bg(p.surface1),
    (false, true) => Style::default().bg(p.surface0),
    _ => Style::default(),
};
frame.render_widget(Paragraph::new(line).style(row_style), rect);
```

`Paragraph::style` 作为底色，span 自身的 fg 仍生效；背景不再被重置。竖条继续在文本之后写。

**测试契约（关键）**：断言必须取**文字所在 cell** 的背景，不能取"该行任意一个有背景的 cell"。
取一个已知含字形的列（如行首缩进之后的第 4 列）。

### F3 · 预览只呈现人类对话 · P0

| 后端 | 规则 |
|---|---|
| claude | `type=="user"` 且 content **不含** `tool_result` 段才算用户轮次；assistant 只取 `text` 段，跳过 `thinking` / `tool_use` |
| codex | 拼接**全部** `input_text` 段而非只取首段，再走注入剥离 |
| grok | 解包 `<user_query>…</user_query>` 取其内容；纯 `<user_info>` / `<system-reminder>` 轮次丢弃 |
| pi | 现状正确，保持 |

新增 `first_text` 的替代 `all_text(value, kinds)`：按段类型白名单拼接，不再"取第一个"。

### F4 · 预览排版 · P1

- 去掉 `Wrap`，改为**渲染前按可用宽度自行折行**，使 `Line` 数量与 `history_scroll` 一一对应。
- 用户与助手用不同缩进（助手缩进 2 列），使往返在视觉上可分。
- 单条消息超过 40 行时折叠尾部并标注剩余行数，避免一条长输出占满整屏。

---

## 3. Boundaries

- **Always**：预览只读，任何路径不得写 PTY、不得 spawn agent（`r` 键除外，且需显式确认）。
- **Always**：标题清洗是纯函数，无 IO，可单测；不改 Catalog 持久化标题。
- **Never**：不因排版需要而修改 `IndexedSessionSummary` 的持久化语义。
- **Never**：不引入新的 Palette slot。

---

## 4. 验收

| 测试 | 断言 |
|---|---|
| `highlight_survives_row_text_render` | 取文字所在 cell，背景 == surface1/surface0 |
| `cursor_and_open_backgrounds_differ_on_text_cells` | 同上，且两者不等 |
| `sibling_sessions_render_distinguishable_text` | 同一 Project 连续两行前 12 列不相同 |
| `dispatch_template_prefix_is_stripped` | `你是本轮审阅席（Claude）。仓库 X。任务：改 Y` → 标题含 `改 Y` |
| `claude_tool_results_are_not_user_turns` | 含 tool_result 的 user 记录不进消息列表 |
| `codex_multi_segment_content_is_joined` | 3 段 input_text 的真实请求不丢失 |
| `grok_user_query_is_unwrapped` | `<user_query>X</user_query>` → `X` |
| `preview_line_count_matches_scroll_unit` | 折行后行数 == 渲染 Line 数 |

真实文件复验：用 `PROBE_FILES` 探针对四个后端各跑一次，人工确认前 12 条是可读往返。

---

## 5. 实施顺序

1. **F2**（高亮）— 单文件、低风险，先让点击可见。
2. **F3**（预览内容）— 触及四个解析器，用真实文件探针验证。
3. **F1**（命名去重）— 触及标题渲染与去重策略，需先补 characterization 测试。
4. **F4**（排版）— 折行与滚动对齐。
