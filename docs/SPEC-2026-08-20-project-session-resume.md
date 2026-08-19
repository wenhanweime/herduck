# SPEC：Projects/Clusters 点击后续上会话

> 状态：v0.1 · 已实施
> 日期：2026-08-20
> 关系：替换 `SPEC-2026-08-19-sidebar-interaction.md` F4（当时标 P2 不做）。
> 用户主诉：点 session 无法切进去、无法继续对话。

## 0. One-liner

点 Projects / Clusters 里的会话，应回到正在跑的 pane，或新开 tab 用官方 resume 命令继续同一段对话。

**Done when：**

1. live 映射仍有效 → 聚焦该 pane，进入 Terminal，可立即打字。
2. 非 live、且 backend 有 resume 计划（codex/claude/pi/opencode/grok/…）→ 在会话 cwd 新开 tab 执行 resume argv，进入 Terminal。
3. 无法 resume（缺 native ref、未知 backend）→ 仍打开只读 `ProjectHistory`，不瞎 spawn。
4. 不向已有 PTY 注入按键；resume 是新进程。

## 1. 根因（已读码）

上一轮把点击接到 `Activate`，历史分支只做 `Mode::ProjectHistory`（元数据卡片：「No process was created and no resume command was run」）。

`IndexedSessionSummary` **没有** `ref_value`。Catalog 有 `sessions.ref_value`，分页 SQL 没选出。因此即使用户点了，也拼不出 `codex resume <id>` / `grok --resume <id>`。

`agent_resume::plan` 已有官方 argv，但缺 `herdr:grok`。Grok CLI 是 `grok --resume <SESSION_ID>`。

## 2. Features

### F1. 快照带 native ref · P0

`IndexedSessionSummary` 增加 `ref_value`（serde default `""`）。`SESSIONS_PAGE_SQL` 与 topic 分页都选出 `s.ref_value`。更新 API schema 产物。

### F2. 点击 resume · P0

`activate_project_session`：

1. live 且 pane 仍在 → focus（已有）。
2. 否则用 backend + ref_kind + ref_value 调 `agent_resume::plan("herdr:{backend}", …)`。
3. 有 plan → 当前 workspace 用 `create_tab_argv_command` 新 tab（无 workspace 则新建）；设 `PersistedAgentSession`；focus + Terminal。
4. 无 plan / spawn 失败 → `open_project_history`。

补 `herdr:grok` → `grok --resume <id>`。

### F3. 测试 · P0

- catalog 分页带出 ref_value。
- `plan` 覆盖 grok。
- 历史点击无 ref/无 plan 仍进 ProjectHistory（不 spawn）。
- 有 plan 时 `activate` **不**停在 ProjectHistory（可用假 argv 如 `true` 在测试里替换，或测 helper 返回 plan 后走 spawn 路径的状态：mode=Terminal、tab 数 +1）。测试环境不要真起 `codex`。

## 3. Boundaries

- Always：live 优先；resume 新 tab 新进程；失败回只读历史。
- Never：把 transcript 当输入塞进别人的 pane；本轮不做 fork。

## 4. 验收

```bash
cd /Users/pot/Workspace/ork3
cargo test --locked --bin ork3 -- \
  clicking_project_session_row_opens_history \
  project_session_resume \
  grok_resume \
  generated_protocol_schema_artifact_is_current
cargo clippy --locked --all-targets -- -D warnings
```
