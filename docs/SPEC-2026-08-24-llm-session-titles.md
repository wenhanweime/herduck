# SPEC：LLM 会话标题生成（Session Title Generation）

> 状态：v0.1 · 待评审
> 日期：2026-08-24
> 依据方案：`/Users/pot/Workspace/agent-session-title-generation/PLAN.md`（641 行，2026-08-24）
> 关系：**替换** `src/ui/session_label.rs` 承担的"从原始标题猜主题"职责；
> 该文件降级为纯展示清洗（截断、宽度、去重），不再负责语义。
> 全部数字来自 2026-08-24 对真实 Catalog 与本机 transcript 的直接查询。

---

## 0. One-liner

会话标题改为**LLM 生成、规则兜底**的独立模块：Catalog 存 `session_key → title` 的持久配对，
后台分批生成，格式 `【具体对象】具体任务`。

**Done when：**

1. 侧栏标题不再是"首条最长用户消息的前 96 字符"。
2. 粘贴进来的旧对话、`继续`、`在吗` 不再成为标题主体。
3. 模型不可用时仍有可读标题，且**不标注**它来自规则（owner 决定）。
4. 标题与 `session_key` 成对持久化，重启后不重复调用。

---

## 1. 为什么现在必须换掉启发式

### 1.1 触发本轮的实证

Owner 截图中的标题：`claude 推荐顺序： 1. Chrome Can…`

追查原始 JSONL（`~/.claude/projects/-Users-pot-Workspace/770046bc-…jsonl`），该轮次
**在文件里确实是 `type:"user"`** —— owner 把上一个 agent 的回答粘贴进来作为开场。
所以这不是解析错误，是**选取策略错误**：

| 轮次 | 长度 | 内容 |
|---|---|---|
| 0 | **167** | `推荐顺序：1. Chrome Canary（最合适）…`（粘贴的旧对话） |
| 1 | 142 | `看下ork3这个项目，他的projects 抓取和cluster聚类…` |
| 2 | 61 | `把诊断和计划以及建议写 spec 到项目文件夹…` |

`TitlePicker`（`adapters.rs:1297`）**只比长度**：167 > 142，于是选中粘贴内容。
真实意图在轮次 1，且含 `ork3` / `projects` / `cluster` 三个专有名词。

### 1.2 启发式已连续三轮打补丁失败

`session_label.rs` 上已叠加：剥 `--resume`、折叠相邻重复、剥角色设定句、相邻行去重。
每一轮都由新的真实数据推翻。根本原因是**长度和位置无法表达"哪句是主题"**。

PLAN.md §D3 的高信息评分直接命中：正向信号是产品名/仓库名/模块名/文件名/错误码/交付物，
负向信号是"继续/恢复/看一下/处理一下"。按此评分，轮次 1 完胜轮次 0。

### 1.3 可复用的既有设施（实测确认）

`src/projects/semantic.rs` 已有完整的 CLI-LLM 调用链，本模块**复用而不重写**：

- 模型池 + 配额轮换（`BackendSpec` / `BatchError::QuotaExceeded`）
- 超时轮询、输出字节上限（`MAX_OUTPUT_BYTES`）
- **opencode 沙箱隔离**：`XDG_DATA_HOME` 指向临时目录，防止调用自身在 `opencode.db`
  落新会话又被下轮扫描捡回（`SPEC-catalog-hygiene` R2 已治理过的自污染）
- `visit_json_lines` 的扫描边界（2 MiB 行 / 8 MiB 前缀）

后端可用性实测（2026-08-24）：

| 后端 | 路径 | 一次性调用 | 实测 |
|---|---|---|---|
| opencode | `~/.opencode/bin/opencode` | `run --model M P` | 已在 semantic 使用 |
| pi | `/opt/homebrew/bin/pi` | `-p --model M -nt -ns -np -nc --no-session P` | 已在 semantic 使用 |
| hermes | `~/.local/bin/hermes` | `-z P --ignore-rules` | **本轮新增**，实测返回 `{"ok":1}` 无噪音 |

---

## 2. Owner 已确认的四项决定

| # | 决定 | 取值 |
|---|---|---|
| D1 | 标题格式 | `【具体对象】具体任务` |
| D2 | 是否另存摘要 | 不存，标题自身可理解 |
| D3 | 兜底是否标注 | **不标注**。全部优先模型；模型失败也不单独标记规则生成 |
| D4 | 后端优先级 | **opencode → pi → hermes** |
| D5 | 历史回填 | 后台慢慢整理，`session_key → title` 单独存储 |

**D1 与前几轮的关系**：第 F1 轮移除的是行内 `【topic】`，那是**几十个会话共享的分组名**
（实测三大 topic 覆盖 63/54/51 条），重复无信息量。本轮的 `【对象】` 是**每会话独有的具体对象**，
不会重复。两者不冲突。

**D3 的后果需明确记录**：不标注意味着用户无法区分"模型确认的标题"和"规则猜的标题"。
Owner 明确接受此取舍。作为补偿，`title_source` 仍写入数据库以供诊断与后续重试，
只是**不在 UI 呈现**。

---

## 3. 模块边界

新增 `src/projects/title.rs`。**不放 `src/ui/`**：标题是 server 拥有的共享事实
（持久化 + API 暴露），按 CLAUDE.md 的 runtime/client 边界属服务端。

`src/ui/session_label.rs` 职责收缩为纯展示：按显示宽度截断、相邻行去重。
**删除**其语义猜测逻辑（`strip_role_assignment`、`is_low_signal` 的标题选取用途）。

### 3.1 数据模型

`sessions` 表新增（迁移 v6）：

```sql
generated_title      TEXT,          -- 模型产出，通过校验
custom_title         TEXT,          -- 用户手动重命名，优先级最高
title_source         TEXT NOT NULL DEFAULT 'heuristic',
                                    -- 'model' | 'heuristic' | 'manual'
title_status         TEXT NOT NULL DEFAULT 'pending',
                                    -- 'pending' | 'running' | 'done' | 'failed'
title_error          TEXT,
title_backend        TEXT,
title_model          TEXT,
title_generated_at   INTEGER,
title_input_fingerprint TEXT,       -- 相同输入不重复调用
title_schema_version INTEGER NOT NULL DEFAULT 1
```

`title` 列保持不变（原始观测标题，仍是合并契约的一部分），**不被本模块覆写**。
展示优先级：`custom_title` → `generated_title` → 规则兜底（由 `title` 派生）。

### 3.2 输入 envelope（PLAN §D3）

发给模型的**不是** transcript，是清洗后的 envelope：

```json
{
  "id": 1,
  "provider": "claude",
  "folder": "ork3",
  "intents": ["高信息用户意图 1", "…最多 5 条"],
  "outcome": "最近一条 agent 结论，限长"
}
```

复用本轮已实现的 `transcript::read_transcript` + `preview_excerpt`：
它已经做了 tool_result 剔除、wrapper 剥离、agent 轮次合并、结论取末段。

**意图筛选**（PLAN §D3 消息选择规则）：
1. 只取真实 user 文本轮次；
2. 剔除低信息量（`继续` / `在吗` / `好的` / 去标点后 < 3 字符）；
3. 按高信息评分排序取前 5，再按原顺序还原；
4. 单会话输入总量上限 2,500 字符。

**评分**（PLAN §D3）：路径/扩展名、错误码、`CamelCase` 或 `kebab-case` 标识符、
文件名加分；纯动词短语、问候、UUID 减分。

### 3.3 Prompt 与批量契约

沿用 semantic 的编号映射（防幻觉 key）：

```
给下面每个编码会话起一个标题。
格式必须是【具体对象】具体任务，对象 2-24 字，任务 6-42 字。
对象用产品名/仓库名/模块名/文件名，禁止用 Workspace/任务/会话/项目/Agent。
没有明确证据不要写"已完成""已修复"。
只输出 JSON：{"items":[{"id":1,"title":"【ork3】修复侧栏高亮"}]}，不要解释。
```

### 3.4 服务端校验（PLAN §D6，全部必须）

1. JSON 可解析；
2. 每个输入 id 恰好一次，无未知 id；
3. 去 ANSI/C0/C1、Markdown fence、首尾引号、`<think>…</think>`；
4. 折叠空白为单行；
5. 匹配 `^【[^】]{2,24}】.{6,60}$`；
6. 对象不在禁用集（`Workspace`/`任务`/`会话`/`项目`/`Agent`/`session`）；
7. 无未经证据支持的完成词；
8. UTF-8 安全截断，硬上限 72 字符；
9. **单批部分失败不覆盖已有成功标题**。

校验失败的条目标 `title_status='failed'` 并记 `title_error`，保留原标题，可重试。

### 3.5 规则兜底（PLAN §D6 Fallback）

按优先级：`custom_title` → 合法 `generated_title` → **确定性实体前置**
（从 folder/仓库名取对象 + 最高分意图取任务）→ 原始 `title`。

兜底必须可读，**不得虚构完成状态**。按 D3，兜底标题在 UI 中与模型标题外观一致。

### 3.6 调度

- 复用 semantic 的 `startup_grace` / `idle_backfill` 节奏，避免与聚类抢后端。
- `batch_size` 起始 20（标题输出比聚类长，先保守）。
- 已有 `title_input_fingerprint` 且未变化的会话不重复调用。
- 全量回填 1,301 条交互式会话，后台分批。

---

## 4. Boundaries

- **Always**：只读 provider 历史目录；标题只写 ork3 Catalog。
- **Always**：prompt 中不含 transcript 全文、代码 diff、工具输出、凭证。
- **Always**：opencode 后端调用必须走 `XDG_DATA_HOME` 沙箱，否则自污染复发。
- **Never**：不改 provider 原始会话文件。
- **Never**：不因标题生成阻塞侧栏浏览或 pane 输入。
- **Never**：不覆盖 `custom_title`。

---

## 5. 验收

| 测试 | 断言 |
|---|---|
| `pasted_reply_is_not_chosen_as_intent` | 轮次 0 为粘贴回答时，envelope 的首个 intent 是轮次 1 |
| `low_signal_intents_are_excluded` | `继续`/`在吗` 不进 envelope |
| `title_validator_rejects_banned_subject` | `【Workspace】…` 被拒 |
| `title_validator_rejects_think_block` | 含 `<think>` 被拒 |
| `title_validator_enforces_entity_prefix` | 无 `【】` 被拒 |
| `partial_batch_failure_keeps_existing_titles` | 一条失败不影响同批其他条 |
| `unchanged_fingerprint_skips_the_model` | 第二次不发起调用 |
| `custom_title_is_never_overwritten` | 自动流程不覆盖手动标题 |
| `fallback_is_readable_without_a_model` | 模型全失败时标题非空且含对象 |
| `opencode_title_backend_uses_a_disposable_data_root` | 调用不在真实 `opencode.db` 落会话 |
| `migration_v5_to_v6_adds_title_columns` | 迁移幂等，旧数据不丢 |

真实文件验收：对 owner 截图那条会话跑一次，标题应形如 `【ork3】Projects 抓取与聚类问题排查`，
而非 `推荐顺序…`。

---

## 6. 实施顺序

1. **M1 迁移 + 数据模型**（v6 列、展示优先级、UI 读 `generated_title`）
2. **M2 envelope 抽取**（意图筛选 + 高信息评分，纯函数、可单测）
3. **M3 校验器**（纯函数，全部 9 条规则）
4. **M4 后端调用**（复用 semantic 基础设施；新增 hermes argv）
5. **M5 调度 + 回填**（fingerprint、批次、idle 节奏）
6. **M6 收缩 `session_label.rs`**（删除语义猜测，只留宽度与去重）

M2/M3 是纯函数，先做且必须先证明测试会失败（见项目记忆：渲染/逻辑测试须经破坏验证）。
