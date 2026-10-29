# 同时开好几个 Claude Code（和 Codex），怎么不乱、第二天怎么接着做

[English](manage-many-claude-code-sessions.md)

*作者本人写的。这页大部分方法不用装任何额外工具，Herduck 只在最后提一下。关于 Claude Code 的说法来自它的
官方文档（文末有链接），核对时间 2026-10-30。*

## 问题是什么

你开了一个 Claude Code 修 bug，又开一个写测试，再开一个 Codex 做重构，另一个目录里还有一个在查怪问题。
开到第五个终端，就记不清哪个窗口在干什么、哪个在等你回话；到第二天早上，更想不起每件事当初为了什么、
下一步该做什么。

这里其实是三个不同的问题：

1. **找回某个会话**（那段对话是哪一个？）
2. **看出现在谁在等你**（哪个在等你回答？）
3. **记住这件事本身**（目标是什么、卡在哪、下一步是什么？）

下面这些免费办法，前两个问题解决得不错；第三个一般只能靠你自己记。

## 1. 给每个会话起名字

不起名的会话，在列表里显示的是你第一句话，过几天很难认出来。起个名字：

```sh
claude -n auth-refactor          # 启动时起名
# 或者在会话里：
/rename auth-refactor
```

以后可以直接回去：

```sh
claude --resume auth-refactor    # 按名字打开
claude --resume                  # 从列表里挑
claude --continue                # 打开当前目录最近的一个
```

## 2. 一件事一个目录（git worktree）

两个会话同时改同一批文件，会互相打架。git worktree 就是同一个仓库在另一个目录里的副本，有自己的分支，
每个会话各改各的。Claude Code 可以帮你建：

```sh
claude --worktree feature-auth
```

在另一个终端换个名字再跑一次，就是第二个互不干扰的会话。

## 3. 在一个地方看谁在等你

较新的 Claude Code 有 **agent view**：运行 `claude agents`，一个屏幕里能看到所有后台 Claude Code 会话——
哪个在干活、哪个在等你输入、哪个做完了。正在跑的会话可以用 `/bg` 放到后台。

如果你习惯普通终端，tmux 可以在你关掉窗口或断开 SSH 后让会话继续跑（`tmux new -s shop` 新建，
`Ctrl-b d` 暂时离开，`tmux attach -t shop` 回来）。但 tmux 不知道 Agent 在干嘛，还是得一个个窗口去看。

## 4. 记"这件事"，而不只是"这个会话"

这一步上面的工具都不会替你做。一个会话只是一段对话；一件事常常横跨好几段对话、好几个 Agent、好几天。
不管用什么工具，都有用的一个习惯：每件事留一张小纸条，只写四行——

```text
目标：做到什么样算完
进展：现在到哪了
卡点：被什么挡住了（没有就空着）
下一步：接下来一到三步
```

——每天收工时更新一下。项目目录里放个文本文件就够了。

## Herduck 在这里的位置

如果你只用 Claude Code、只在一两个目录里干活，上面这些多半就够了。

Herduck 适合"纸条记不过来"的情况：好几个 Agent（Claude Code、Codex、OpenCode、Pi……）、好几个目录、
一件事要做好几天。它做的事（以 [README](../README.zh-CN.md) 为准）：

- **Work 视图**：每件事保存一个可编辑的目标、最多三条下一步、一条卡点备注。会话结束、服务重启都还在，
  自动更新不会覆盖你自己写的内容。来自不同 Agent、不同目录的对话可以归到同一件事下面。
- **Sessions 视图**：把本机 Claude Code、Codex、Pi、OpenCode、Grok 的历史对话放进一个能搜索的列表，
  包括不是在 Herduck 里启动的。可以先预览、不启动任何东西；支持的话可以用原来的 Agent 接着聊。
- **Agents 视图**：多个 Agent 终端并排，能看出在干活 / 等输入 / 空闲，通知可以自己配置。关掉界面，
  后台服务会让它们继续跑；再运行 `herduck` 就能接回来。
- **Continue with this（接着做）**：把选中的下一步发给原来的 Agent，能复用正在运行的会话，
  或者恢复原来那段对话。

试之前要知道的：

- 它还是 **Alpha**，只支持 macOS 和 Linux。Work 相关功能需要从源码编译；npm 上的版本比较旧
  （见[安装文档](installation.md)）。
- 自动归类成 Work、会话命名、进展描述**默认关闭**，需要你自己配置模型来源（OpenCode、Pi、Codex、Hermes CLI
  或任意 OpenAI 兼容接口）。打开后，选中的对话内容会发给那个来源。不打开的话，Agents、Sessions、Projects
  视图照样能用，读取本机历史不会调用模型。
- 它不会帮你建 git worktree，这部分继续用 `claude --worktree` 或 `git worktree`。

和 `claude --resume`、tmux、Herdr 以及其他会话管理工具的对比：[compare-resume-tmux.zh-CN.md](compare-resume-tmux.zh-CN.md)。

## 资料来源

- Claude Code：[管理会话](https://code.claude.com/docs/zh-CN/sessions) ·
  [Worktrees](https://code.claude.com/docs/en/worktrees) ·
  [agent view](https://code.claude.com/docs/zh-CN/agent-view)
- tmux：<https://github.com/tmux/tmux/wiki>
- Herduck：[README](../README.zh-CN.md) · [项目概览](project-overview.md) · [配置](configuration.md)
