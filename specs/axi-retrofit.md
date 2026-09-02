# AXI retrofit spec: herdr

This document audits the installed `herdr` command-line interface against the ten AXI (Agent eXperience Interface) principles and records what would have to change to meet the bar.
All ten principles are mandatory for agent-invoked tooling, so there is no tiering here and no per-principle exemption on the grounds that Herdr is primarily an interactive terminal application.
This is a specification only; it changes no code, and the retrofit itself is tracked by a separate follow-on ticket.

Audited binary: `/usr/local/bin/herdr`, version `herdr 0.8.0`, protocol `19`.
Every verdict below cites a command that was actually run against an isolated named Herdr session and the output that came back.
The reference implementations used as the shape to copy are `gh-axi` and `tasks-axi`, which already meet the bar.

## 1. Scorecard

### Principle 1: Token-efficient output

Verdict: gap.

Every machine-readable command emits raw JSON rather than TOON.

```
$ herdr pane list --session <lab>
{"id":"cli:pane:list","result":{"panes":[{"agent_status":"unknown","cwd":"/tmp","focused":true,"foreground_cwd":"/tmp","pane_id":"w1:p1","revision":1,"scroll":{"max_offset_from_bottom":393,"offset_from_bottom":0,"viewport_rows":23},"tab_id":"w1:t1","terminal_id":"term_65a86bc35b6d81","terminal_title":"root@work:/tmp","terminal_title_stripped":"root@work:/tmp","workspace_id":"w1"}],"type":"pane_list"}}
```

The envelope keys `id`, `result`, and `type` are protocol framing that an agent reading stdout does not need, and the per-pane object repeats every key name for every row.
For comparison, `tasks-axi list` returns `count: 0` plus a TOON body, and `gh-axi` returns TOON tables with a shared header row.

### Principle 2: Minimal default schemas

Verdict: gap.

The default list row carries eleven fields, including a nested `scroll` object and two title variants.

```
$ herdr pane list --session <lab>
... "agent_status", "cwd", "focused", "foreground_cwd", "pane_id", "revision",
    "scroll": {"max_offset_from_bottom", "offset_from_bottom", "viewport_rows"},
    "tab_id", "terminal_id", "terminal_title", "terminal_title_stripped", "workspace_id"
```

There is no way to narrow or widen that set, because no `--fields` flag exists.

```
$ herdr pane get w1:p1 --fields pane_id --session <lab>
usage: herdr pane get <pane_id>
[exit=2]
```

`herdr workspace create` is worse: a single creation returns the new workspace, its first tab, and its root pane with all pane fields inline.

### Principle 3: Content truncation

Verdict: gap.

`herdr pane read` and `herdr agent read` return unbounded terminal text with no size hint and no `--full` escape hatch.

```
$ herdr pane run w1:p1 'seq 1 400' --session <lab>
$ herdr pane read w1:p1 --source recent --session <lab> | wc -c
437
$ herdr pane read w1:p1 --source recent --lines 2000 --session <lab> | wc -c
1980
```

The default is a fixed line window rather than a truncation contract.
The agent is never told how much scrollback exists beyond what it received, so it cannot tell a complete read from a clipped one.
`herdr pane read --help` documents `--source`, `--lines`, `--format`, and `--ansi`, and no full-content flag.

### Principle 4: Pre-computed aggregates

Verdict: gap.

List output carries no total count and no derived summary.

```
$ herdr agent list --session <lab>
{"id":"cli:agent:list","result":{"agents":[],"type":"agent_list"}}
```

The information an orchestrating agent almost always wants next, namely how many agents are working, idle, or blocked, is absent, even though `agent_status` is already computed per pane and appears in `herdr pane list`.
`herdr api snapshot` does aggregate the whole session, but it returns the entire layout tree, so it is not a cheap substitute.

```
$ herdr api snapshot --session <lab>
{"id": "cli:api:snapshot", "result": {"snapshot": {"agents": [], "focused_pane_id": "w1:p1", "focused_tab_id": "w1:t1", "focused_workspace_id": "w1", "layouts": [{"area": {"height": 23, "width": 54, "x": 26, "y": 1}, ...
```

### Principle 5: Definitive empty states

Verdict: gap.

An empty result is an empty JSON array with no statement that zero is the answer.

```
$ herdr workspace list --session <lab>
{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[]}}
```

Compare `tasks-axi list`, which prints `tasks: 0 ready tasks in this backlog`.
The Herdr form leaves an agent unsure whether the session is empty or whether the query was mis-scoped, which invites a redundant second call.

### Principle 6: Structured errors and exit codes

Verdict: gap.

Errors are structured, but they are written to stderr rather than stdout, so an agent capturing stdout alone sees nothing at all.

```
$ herdr pane get w1:p99 --session <lab>
[exit=1]
-- stdout --
-- stderr --
{"error":{"code":"pane_not_found","message":"pane w1:p99 not found"},"id":"cli:pane:get"}
```

Unknown flags are rejected loudly with exit code 2, which is correct, but the message names no valid alternatives, so correcting it costs a second `--help` call.

```
$ herdr pane list --workspac w1 --session <lab>
[exit=2]
unknown option: --workspac
```

Errors carry no suggested next command.

```
$ herdr tab list --workspace w9 --session <lab>
{"error":{"code":"workspace_not_found","message":"workspace w9 not found"},"id":"cli:tab:list"}
```

Idempotence is inconsistent.
`herdr workspace focus w1` run twice succeeds both times with exit 0, and `herdr pane zoom w1:p1 --off` on an unzoomed pane returns `"changed":false` with exit 0, which is the desired behavior.
But a repeated close is an error rather than an acknowledged no-op.

```
$ herdr tab close w1:t2 --session <lab>
{"id":"cli:tab:close","result":{"type":"ok"}}
[exit=0]
$ herdr tab close w1:t2 --session <lab>
{"error":{"code":"tab_not_found","message":"tab w1:t2 not found"},"id":"cli:tab:close"}
[exit=1]
```

There are no interactive prompts in the command surface, which is the one part of this principle that already holds.

### Principle 7: Ambient context

Verdict: gap.

Herdr ships real session integrations and installs them from an explicit, idempotent, per-target setup command.

```
$ herdr integration status --session <lab>
pi: not installed (/root/.pi/agent/extensions/herdr-agent-state.ts)
claude: not installed (/root/.claude/hooks/herdr-agent-state.sh)
codex: not installed (/root/.codex/herdr-agent-state.sh)
opencode: not installed (/root/.config/opencode/plugins/herdr-agent-state.js)
...
```

The default targets required by the principle, Claude Code, Codex, and OpenCode, are all present, alongside twelve more.
The gap is direction: these hooks report agent state *into* Herdr and print nothing to the agent.
`src/integration/assets/claude/herdr-agent-state.sh` sends a `pane.report_agent_session` request over the socket and exits, so the `SessionStart` hook injects no context.
The agent therefore begins every session blind to the session it is running inside and must issue an explicit call to learn anything.

An installable skill exists and is a genuine strength.

```
$ herdr --skill | head -3
---
name: herdr
description: "Control Herdr, a terminal multiplexer for coding agents. ..."
```

But the skill is hand-maintained at `skills/herdr/SKILL.md` and is not generated from a home view, because there is no home view to generate it from, so the single-source-of-truth rule cannot currently hold.

### Principle 8: Content first

Verdict: gap.

Running the binary bare never shows live data.
Inside a Herdr-managed pane, which is exactly where an agent runs, it is refused.

```
$ herdr
error: nested herdr is disabled by default.
see configuration if you want to enable it.

"inception detected. we need to go deeper... said no one ever."
[exit=1]
```

Outside a pane it tries to take over the terminal instead, and without a TTY it aborts.

```
$ env -u HERDR_ENV herdr < /dev/null
thread 'main' panicked at ratatui-0.30.0/src/init.rs:299:16:
failed to initialize terminal: Os { code: 6, kind: Uncategorized, message: "No such device or address" }
[exit=101]
```

Neither path yields state.
The closest thing to a dashboard is `herdr status`, which reports versions and sockets rather than session content.

```
$ herdr status --session <lab>
client:
  version: 0.8.0
  channel: stable
  protocol: 19
server:
  status: running
  ...
```

Group commands invoked bare print a usage list, not data.

```
$ herdr pane --session <lab>
herdr pane commands:
  herdr pane list [--workspace <workspace_id>]
  ...
```

### Principle 9: Contextual disclosure

Verdict: gap.

No command emits next-step suggestions, on success, on empty results, or on errors.

```
$ herdr agent list --session <lab>
{"id":"cli:agent:list","result":{"agents":[],"type":"agent_list"}}
```

There is no `help[n]` block anywhere in the output surface, whereas `gh-axi` closes its home view with `help[1]: Run gh-axi <command> <subcommand> ...`.
The `pane_not_found` and `agent_not_found` errors shown under principle 6 likewise end without pointing at `herdr pane list`, which is the command that resolves them.

### Principle 10: Consistent way to get help

Verdict: partial pass, with one gap.

The `--version` fast path is genuinely fast and correct for the two long-standing spellings.

```
$ herdr --version
herdr 0.8.0
$ herdr -V
herdr 0.8.0
```

Measured wall time for `herdr --version` was 2 ms, well inside the ergonomics budget.
The lowercase spelling is rejected.

```
$ herdr -v
unknown option: -v
run 'herdr --help' for usage
```

Per-subcommand help exists and is scoped rather than dumping the whole manual.

```
$ herdr pane list --help --session <lab>
List panes

Usage: herdr pane list [OPTIONS]

Options:
      --workspace <WORKSPACE_ID>
```

That help is thin compared with the bar: it lists no defaults and carries no usage examples, and some subcommand help is a single usage line.

```
$ herdr agent list --help --session <lab>
List agents

Usage: herdr agent list
```

There is no home view, so the required self-identification, an executable path with the home directory collapsed to `~` plus a one-sentence description, is absent.

### Summary

| Principle | Verdict |
| --- | --- |
| 1. Token-efficient output | gap |
| 2. Minimal default schemas | gap |
| 3. Content truncation | gap |
| 4. Pre-computed aggregates | gap |
| 5. Definitive empty states | gap |
| 6. Structured errors and exit codes | gap |
| 7. Ambient context | gap |
| 8. Content first | gap |
| 9. Contextual disclosure | gap |
| 10. Consistent way to get help | partial pass |

## 2. Change list

Every change below extends the existing `herdr api` surface and the existing socket-backed command groups.
None of them introduces a parallel output path, a second binary, or a separate agent-only executable.
The runtime and client boundary in `AGENTS.md` still applies: the aggregates and status facts named here are runtime facts owned by the server and exposed through the API, while the rendering of them is client-side formatting at the output boundary.

1. Add an agent output mode to the existing socket-backed commands that renders the API result as TOON at the output boundary, keeping the internal request and response types on JSON, and make it the default for those commands. Satisfies principle 1.
2. Strip the protocol envelope, `id`, `result`, and `type`, from rendered agent output, since it is transport framing rather than data. Satisfies principles 1 and 2.
3. Reduce the default list schemas for `pane list`, `agent list`, `tab list`, `workspace list`, and `worktree list` to an identifier, a human label, and a status, and move the remaining fields behind an explicit request. Satisfies principle 2.
4. Add a `--fields` flag to those list commands and to the corresponding `get` commands so an agent can ask for additional fields by name. Satisfies principle 2.
5. Truncate `pane read` and `agent read` output by default with an explicit total-size hint, and add a `--full` flag that returns the untruncated content. Satisfies principle 3.
6. Add a total count to every list response, alongside the returned page size. Satisfies principle 4.
7. Add a pre-computed agent-status summary, counts by state, to `agent list` and to the workspace and tab views, derived from the `agent_status` the server already computes. Satisfies principle 4.
8. Render an empty result as an explicit zero statement carrying the query scope rather than an empty array. Satisfies principle 5.
9. Move structured errors from stderr to stdout in the same rendered format as normal output, and reserve stderr for diagnostics. Satisfies principle 6.
10. Extend the unknown-flag error to name the valid flags for the specific subcommand that rejected it, and give renamed or removed flags a targeted replacement hint. Satisfies principle 6.
11. Make destructive commands idempotent, so closing an already-closed pane, tab, or workspace is an acknowledged no-op with exit code 0 rather than a not-found error. Satisfies principle 6.
12. Add a compact session-context payload to the API and have the existing session-start integrations emit it to stdout, so the hook that currently only reports agent state into Herdr also returns ambient context to the agent. Satisfies principle 7.
13. Generate `skills/herdr/SKILL.md` from the same content the new home view prints, and add a staleness check to the existing maintenance test suite. Satisfies principle 7.
14. Make the bare invocation inside a Herdr-managed pane print the home view instead of refusing as nested recursion, keeping the refusal only for the interactive terminal-application path. Satisfies principle 8.
15. Make the home view show live session content, workspaces, tabs, panes, and agents with their states, scoped to the current session and working directory. Satisfies principle 8.
16. Make each group command invoked bare, `herdr pane`, `herdr agent`, and the rest, show that group's live data with its command list demoted to a help block. Satisfies principles 8 and 9.
17. Add a `help[n]` block of parameterized next-step commands to list and mutation output, and omit it on self-contained detail views. Satisfies principle 9.
18. Add a resolving next-step command to every error, naming the command that fixes the specific failure. Satisfies principles 6 and 9.
19. Accept `-v` as a third spelling of the version fast path. Satisfies principle 10.
20. Extend per-subcommand `--help` with flag defaults and two or three usage examples per subcommand. Satisfies principle 10.
21. Add the tool identification line to the home view, the absolute path of the running executable with the home directory collapsed to `~`, plus a one-sentence description. Satisfies principle 10.

## 3. Non-goals

The interactive terminal user interface is out of scope.
Herdr is a terminal workspace manager with a human-facing full-screen application, and AXI governs its command-line surface, not its rendering, keybindings, or layout behavior.

The existing JSON shape of the socket protocol itself is out of scope.
TOON is an output-boundary concern by the principle's own wording, so the wire protocol, its schema at `docs/next/api/herdr-api.schema.json`, and `PROTOCOL_VERSION` stay JSON, and no protocol bump is implied.

Human-facing command output compatibility is deliberately not preserved as a hard constraint.
Changing default list schemas and moving errors to stdout is a visible change for anyone scripting against the current output, and the retrofit should treat that as intended rather than adding a compatibility mode.

Nothing else was waived.
All ten principles are treated as mandatory and every gap found has a corresponding change above.

## 4. Evidence

Both transcripts were captured against an isolated named Herdr session on `herdr 0.8.0`.
The trailing `--session <lab>` argument is the isolation flag required by the harness and is shown redacted.

### Bare invocation

```
$ herdr           # bare invocation, inside a Herdr-managed pane (HERDR_ENV=1)
error: nested herdr is disabled by default.
see configuration if you want to enable it.

"inception detected. we need to go deeper... said no one ever."
[exit=1]
```

The same binary invoked outside a Herdr pane, with no controlling terminal, attempts to start the full-screen application instead of printing anything an agent can read.

```
$ env -u HERDR_ENV herdr < /dev/null
thread 'main' panicked at ratatui-0.30.0/src/init.rs:299:16:
failed to initialize terminal: Os { code: 6, kind: Uncategorized, message: "No such device or address" }
[exit=101]
```

### Hot-path invocation

`herdr pane list` is the hot path: it is the call an orchestrating agent makes to discover what exists before acting.

```
$ herdr pane list --session <lab>
{"id":"cli:pane:list","result":{"panes":[{"agent_status":"unknown","cwd":"/tmp","focused":true,"foreground_cwd":"/tmp","pane_id":"w1:p1","revision":1,"scroll":{"max_offset_from_bottom":393,"offset_from_bottom":0,"viewport_rows":23},"tab_id":"w1:t1","terminal_id":"term_65a86bc35b6d81","terminal_title":"root@work:/tmp","terminal_title_stripped":"root@work:/tmp","workspace_id":"w1"}],"type":"pane_list"}}
[exit=0]
```

One pane produces eleven top-level fields plus a three-field nested `scroll` object, wrapped in a protocol envelope, with no count, no next-step hints, and no statement of scope.
