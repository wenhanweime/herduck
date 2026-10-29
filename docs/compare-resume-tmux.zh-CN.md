# Herduck 与 `claude --resume`、tmux、Herdr 及其他 Agent 会话管理工具的区别

[English](compare-resume-tmux.md)

*作者本人写的。Herduck 还是早期 Alpha，所以这页尽量老实说清楚：每个工具擅长什么，Herduck 在哪些场景下
并不是合适的选择。关于其他工具的事实，来自它们各自的官方文档（文末有链接），核对时间 2026-10-28（"其他多 Agent 会话管理工具"一节为 2026-10-30）；
如果过时了，以官方文档为准。*

## 先说结论

| 你的问题是…… | 该用 |
| --- | --- |
| "把昨天那个 Claude Code 对话重新打开" | `claude --resume` / `claude --continue`，自带，不用装任何东西 |
| "关了窗口、断了 SSH，终端里的东西别死" | tmux，或者 Herdr 这类运行时 |
| "一眼看出哪个 Agent 在干活、哪个卡住了、哪个闲着" | Herdr（Herduck 继承了这部分） |
| "我同时用 Claude Code、Codex 等好几个 Agent，跨好几个目录，已经想不起每件事到底为了什么、做到哪、下一步是什么" | 这就是 Herduck 想补的空档 |

它们不在同一层，很多人可以同时用好几个。

## `claude --resume` / `claude --continue`

优点：

- `claude --continue` 载入当前目录最近一次对话；`claude --resume` 弹出选择器，或按 ID/名字恢复
  （先 `claude -n "my-work"` 起名，再 `claude --resume my-work`）。
- 能恢复该对话的完整历史，而且是官方自带，零安装、零额外组件。
- 较新的 Claude Code 版本还有后台会话（`claude agents`、`claude attach`）。

局限：

- 只管 Claude Code。Codex、OpenCode、Pi、Grok 各有自己的恢复方式。
- 恢复的是一段"对话"，不会跨对话记录"这件事的目标、被什么卡住、下一步是什么"。这些只能靠你脑子或笔记。
- 会话多、目录多的时候，找对那一个本身就是个检索问题。

**适合用它的情况**：你主要用一个 Agent，一次一个目录。这是最简单的办法，Herduck 不是要替代它；
Herduck 的"继续"动作，本质上也是用原来的 Agent 恢复原来的对话。

## tmux 等终端会话管理器

优点：

- 你断开、关终端、SSH 掉线之后，终端和里面的进程还活着。成熟、可脚本化、几乎哪里都有。
- 不挑 Agent：pane 里跑什么它都不管。

局限：

- 管的是终端，不是"事"。tmux 会话叫 `auth`，并不会告诉你目标、卡点、下一步，除非你自己维护。
- tmux 不知道 Agent 在干什么。pane 里的 Agent 是在工作、在等你批准还是闲着，要你自己去看。
- 重启机器后 pane 里的进程就没了。（tmux-resurrect 之类插件能恢复布局，但恢复不了一个正在运行的
  Agent 的内存状态。）

**适合用它的情况**：你要一个通用、极简、久经考验的"终端别死"方案，并且愿意自己维护工作层面的上下文。
Herduck 可以和它并用：Herduck 读的是磁盘上的 Agent 历史，不管你怎么启动的 Agent。

## Herdr

[Herdr](https://herdr.dev) 是 Herduck 派生自的项目，也是更成熟的那个（用户基数大得多、版本更新频繁、
有插件市场）。按它自己 README 的说法，它是"你的编码 Agent 住的运行时"：后台服务让终端在你断开或
SSH 掉线时继续跑；每个 pane 标记 working / blocked / idle；Agent 可以通过 CLI 和 socket API 操作它；
支持多台机器合在一个窗口里。它不包装、不替代你跑的 Agent。

Herdr 回答的是 **"我的 Agent 在哪、哪个需要我？"** Herduck 从同一个运行时底座出发，在上面加了一层，
想回答 **"它们在为哪件事出力、我能从哪接着干？"**

关系要说清楚：

- Herduck 是从 Herdr v0.7.4 派生的独立项目（见 [UPSTREAM.md](UPSTREAM.md)），自带可执行文件，不需要
  另装 Herdr。它不是 Herdr 官方发布，也不是 Herdr 插件。
- Herdr 在 v0.7.4 之后已经继续往前走了。如果你只需要运行时（断开重连、Agent 状态、多机），
  直接用 Herdr 本身，维护得更好，这方面功能也更多。

**适合用它的情况**：你要的只是一个好的运行时。

## Herduck 多了什么，代价是什么

在运行时之上多的：

- **Work**：把不同 Agent、不同目录的会话聚成"一件事"，带可编辑的目标、最多三个下一步、一条卡点备注，
  会话结束或重启后还在；自动更新不会覆盖你自己写的内容。
- **Sessions**：本机 Agent 对话历史的统一可搜索列表——Claude Code、Codex、Pi、OpenCode、Grok——
  包括从没在 Herduck 里启动过的会话。
- **继续（Continue with this）**：把后续指令发给原来的 Agent，复用匹配的在线会话或恢复原对话。
- 一套 CLI 和 JSON API（`herduck work list`、`herduck work plan get/update`），让 Agent 能读到人看到的
  同一份计划。

老实说的代价：

- 这是 **Alpha**，只支持 macOS 和 Linux。文中说的 Work 计划和进展概览需要源码编译（Rust 和 Zig 工具链）；
  已发布的 npm 包（`herduck@alpha`）版本较旧、功能较少（见[安装文档](installation.md)），会有毛刺。
- 自动 Work 聚类、会话命名、进展摘要**默认关闭**。要自己配模型来源（OpenCode、Pi、Codex、Hermes CLI
  或 OpenAI 兼容 API），启用后会把选中的对话内容发给你选的来源。不启用的话，你仍有会话列表和你自己写的计划。
- OpenCode 的历史会被索引，但还不能预览对话内容。
- 这是个新项目，用户很少（写这页时 GitHub 8 个 star）。别默认它在你的环境里已经被充分测过。
- Work 层的质量取决于你写的目标/卡点/下一步和聚类结果；它不会让 Agent 变聪明，也不会给它更大的上下文窗口。

## 其他多 Agent 会话管理工具

终端里管理多个编程 Agent 会话的开源工具还有不少。下面是对各项目自己 README 的简短概括，核对时间
2026-10-30，细节请看它们的页面。它们大多侧重**同时跑**很多 Agent（隔离的 worktree、状态、通知）。
Herduck 的侧重点不一样：跨 Agent、跨天记住**每件事为了什么、下一步是什么**。如果你的主要问题是并行执行，
下面某个工具可能更适合你。

| 工具 | 是什么（按它的 README） | 值得知道的 |
| --- | --- | --- |
| [Claude Squad](https://github.com/smtg-ai/claude-squad) | 管理多个终端 AI Agent（Claude Code、Codex、OpenCode、Amp）的终端应用 | 每个任务有独立的 git 工作区；任务可以后台跑；合并前先审改动。需要 tmux 和 `gh`。 |
| [Agent Deck](https://github.com/asheshgoplani/agent-deck) | AI 编程 Agent 的终端会话管理器：一个 TUI 管 Claude、Gemini、OpenCode、Codex 等 | 一个屏幕看所有会话状态；自带 MCP 和 skills 管理；会话分组。基于 tmux。支持 macOS、Linux、WSL。 |
| [CCManager](https://github.com/kbwo/ccmanager) | 在多个 git worktree 和项目里跑多个 AI 编程助手会话（Claude Code、Gemini CLI、Codex CLI、Cursor Agent、Copilot CLI、OpenCode 等）的命令行工具 | 不需要 tmux；忙 / 等待 / 空闲状态；重启后能恢复会话。用 npm 安装。 |
| [cmux](https://github.com/manaflow-ai/cmux) | 基于 Ghostty 的 macOS 终端，带竖排标签和给 AI 编程 Agent 的通知 | 是一个完整的终端应用，不是加在上面的一层；只支持 macOS。 |
| [Rove](https://github.com/Sma1lboy/rove) | "终端里的 Agent 复用器" | 在隔离的 git worktree 和分支里并行跑任务；断开后会话还在；可以脚本化。 |

Claude Code 自己现在也有 **agent view**（`claude agents`），能在一个屏幕里看所有后台 Claude Code 会话——
见上面的 Claude Code 部分，以及[同时开好几个 Claude Code 怎么不乱](manage-many-claude-code-sessions.zh-CN.md)。

## 快速选择

- 一个 Agent、一个目录、偶尔恢复 → `claude --resume`。
- 只要终端断线不死 → tmux。
- 想要懂 Agent 状态、专为编码 Agent 设计的运行时 → Herdr。
- 主要想让很多 Agent 并行跑、各自一个 worktree → 看看上面的[其他多 Agent 会话管理工具](#其他多-agent-会话管理工具)。
- Agent 多、目录多、一件事要做好几天、老是丢"这件事到底是什么"的线索 → 可以试试 Herduck，
  最好先在不重要的项目上试，坏了请告诉我：[提 issue](https://github.com/wenhanweime/herduck/issues)。

## 资料来源

- Claude Code CLI 参考（`--continue`、`--resume`、`--name`、`claude agents`、`claude attach`）：
  <https://code.claude.com/docs/en/cli-reference>
- Herdr README 与文档：<https://github.com/herdrdev/herdr> · <https://herdr.dev/docs/>
- tmux：<https://github.com/tmux/tmux/wiki> · tmux-resurrect：
  <https://github.com/tmux-plugins/tmux-resurrect>
- Herduck：[README](../README.zh-CN.md) · [UPSTREAM.md](UPSTREAM.md) ·
  [配置](configuration.md) · [项目概览](project-overview.md)
- 其他工具（README，核对时间 2026-10-30）：[Claude Squad](https://github.com/smtg-ai/claude-squad) ·
  [Agent Deck](https://github.com/asheshgoplani/agent-deck) · [CCManager](https://github.com/kbwo/ccmanager) ·
  [cmux](https://github.com/manaflow-ai/cmux) · [Rove](https://github.com/Sma1lboy/rove)
- 延伸阅读：[同时开好几个 Claude Code 怎么不乱](manage-many-claude-code-sessions.zh-CN.md) ·
  [Herduck 和 Herdr](herduck-and-herdr.zh-CN.md)
