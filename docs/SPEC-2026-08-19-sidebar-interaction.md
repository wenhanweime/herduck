# SPEC：Projects/Clusters 侧栏交互三修

> 状态：v0.1 · 已实施
> 日期：2026-08-19
> 规划：Claude 席预定，CLI 两次 `429 Service Unavailable`，本文件按 Grok 已核实代码落盘。

## 0. One-liner

Projects / Clusters 侧栏要能点进会话、能拖宽、顶栏三个英文 Tab 彼此分开。

**Done when：**

1. 在 Projects 或 Clusters 视图，点一条可见会话行：live 聚焦对应 pane 并进 Terminal；历史进入 `ProjectHistory`。
2. 在 Projects / Clusters 视图拖侧栏最右一列 `│`，宽度与 Sessions 视图一样变化并持久化。
3. 顶栏三个词之间至少有一列空隙，不再连成 `sessionsprojectsclusters`。

## 1. Context

用户在 2026-08-19 报了三件事。代码核对：

- 拖宽：`mouse.rs` 在 `is_project_browser()` 分支末尾无条件 `return None`，吃掉 `on_sidebar_divider`。
- Tab：`render_sidebar_tabs` 三等分、0 间隙、`"sessions"|"projects"|"clusters"` 在宽 26 时正好顶满。
- 点击：Activate 路径存在，但 Thin/Automation 无 action；缺少「鼠标点 session 行」测试。本轮不 resume（PRD Non-goal）。

## 2. Features

### F1. project browser 不吞侧栏拖宽 · P0

`in_sidebar && is_project_browser()` 在 `return None` 前处理 divider，与 Sessions 共用 `DragTarget::SidebarDivider`。

### F2. 顶栏 Tab 分开 · P0

Title Case：`Sessions` / `Projects` / `Clusters`。三个 hit rect 之间留 1 列。窄宽度可截断，不可粘连。

### F3. 会话行可点 · P0

- 点 `Session` 行发出 `Activate`（已有语义）。
- 点 `Thin` 展开该项目的短会话，使它们变成可点的 Session 行。
- 补鼠标测试：Projects 视图点 session 行进入 live focus 或 ProjectHistory。

### F4. resume 历史会话 · P2（不做）

## 3. Boundaries

- Always：拖宽与 Sessions 同一套 clamp/持久化；历史点击不写 PTY。
- Never：本轮不 spawn/resume Agent。

## 4. 验收

```bash
cd /Users/pot/Workspace/ork3
cargo test --locked --bin ork3 -- dragging_sidebar_divider clicking_project sidebar_tabs
```
