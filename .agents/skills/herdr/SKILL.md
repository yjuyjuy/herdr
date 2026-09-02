---
name: herdr
description: "Control Herdr, the terminal multiplexer this fleet runs inside: list workspaces, tabs, and panes, split panes, run commands in another pane, read pane output, and start, prompt, and wait on coding agents. Use whenever a task needs to see or drive another terminal, inspect a neighbouring agent's screen, or reason about pane ids like w1:p2."
user-invocable: false
---

# herdr

Terminal multiplexer for coding agents. Panes hold shells, some panes hold recognized agents, and the `herdr` CLI drives both.

Every agent in this fleet is itself running inside a herdr pane. `HERDR_ENV=1` proves it, and `HERDR_PANE_ID`, `HERDR_TAB_ID`, `HERDR_WORKSPACE_ID`, and `HERDR_SESSION` name your own location. Read those variables instead of guessing.

## When to reach for it

- Need another terminal that outlives your command: split a pane and run there. Prefer an ordinary background process for anything that just needs to finish.
- Need to see what another agent's screen actually shows: `herdr agent read`. Nothing else exposes another pane's rendered output.
- Need to drive another agent: `herdr agent prompt`, `agent send-keys`, `agent wait`. In this fleet, prefer the `bin/fm-*` wrappers (see Fleet conventions), which add targeting, submit verification, and reply bookkeeping on top of these.
- Reasoning about a pane id that appeared in a brief or a status file: `herdr pane get <id>` and `herdr agent get <name-or-pane>` resolve it.
- Do not reach for it to gain parallelism inside your own task. Background jobs are cheaper and carry no fleet-wide blast radius.

## Workflows

All commands below were run against an isolated lab session. Outside a lab, drop the helper and let the ambient session apply.

**Orient before acting.** Bare `herdr` does not work inside a pane; it is refused as nested recursion, so there is no home view to read.

```bash
printenv HERDR_PANE_ID HERDR_TAB_ID HERDR_WORKSPACE_ID HERDR_SESSION
herdr pane current --current              # the calling pane, resolved by the server
herdr pane list --workspace "$HERDR_WORKSPACE_ID"
herdr agent list                          # every recognized agent in this session
```

`--current` is a flag on the command, not a pane argument: `herdr pane get --current` fails with `pane_not_found`, while `herdr pane current --current` succeeds.

**Run a command in a fresh sibling pane and read the result.**

```bash
herdr pane split --pane "$HERDR_PANE_ID" --direction right --cwd "$PWD" --no-focus
# read the new id from .result.pane.pane_id
herdr pane run w1:p2 'seq 1 400; echo DONE-MARKER'
herdr pane wait-output w1:p2 --regex '^DONE-MARKER$' --timeout 15000
herdr pane read w1:p2 --source recent-unwrapped --lines 200
```

Anchor the pattern. `pane wait-output` searches the snapshot immediately, and the shell echoes the command you just sent, so an unanchored `--match DONE-MARKER` matches the echoed command line before the command has produced anything.

**Read the right amount of output.** `pane read` has no size hint and no `--full`, so the only way to know whether you saw everything is to raise `--lines` and compare.

```bash
herdr pane read w1:p2 --source recent | wc -c            # 448 with the default window
herdr pane read w1:p2 --source recent --lines 2000 | wc -c  # 1735 - the default clipped it
```

Sources: `visible` is the rendered viewport, `recent` keeps soft wraps, `recent-unwrapped` joins them and is what you want for logs and transcripts, `detection` is the plain bottom-buffer snapshot the agent detector reads. Use `--format ansi` only when styling is the evidence.

**Start an agent in a pane and drive it.**

```bash
herdr pane split --pane "$HERDR_PANE_ID" --direction right --cwd "$PWD" --no-focus
herdr agent start probe --kind jcode --pane w1:p2 --timeout 90000
herdr agent prompt probe "Reply with exactly: LAB-OK" --wait --timeout 120000
herdr agent read probe --source recent-unwrapped --lines 40
```

`agent start` needs a pane already sitting at an interactive shell prompt; it never creates layout. Starting a second agent in an occupied pane fails with `agent_pane_busy`. `herdr agent` printed bare lists the installed kinds. `--wait` waits for the first settled `idle`, `done`, or `blocked`; use `agent wait --until blocked` only for a state-specific workflow.

**Diagnose a stuck or misdetected agent.** Detection reads the bottom buffer, so read that buffer and ask the detector what it matched.

```bash
herdr agent read probe --source detection --format text
herdr agent explain probe --json
```

`agent_status: unknown` means an agent is present but unclassified. It is not evidence of completion.

**Interrupt without killing the pane.**

```bash
herdr pane send-keys w1:p2 ctrl+c        # a plain shell
herdr agent send-keys probe ctrl+c       # an agent UI
herdr agent wait probe --until idle --timeout 20000
```

## Fleet conventions

**Sessions.** This fleet does not run in the `default` herdr session. Crewmates run in `firstmate`, human-facing work in `HITL`/`hitl`, and `herdr session list` shows a long tail of stopped throwaways. Your ambient `HERDR_SESSION` is already correct, so pass `--session` only when you deliberately target another one.

**Lifecycle commands against a shared session are dangerous.** `herdr server stop`, `herdr session stop`, and `herdr session delete` take down every pane in that session, including every running crewmate and the supervisor itself. Never run them against `default` or `firstmate`. For any experiment that needs its own server, use `bin/fm-herdr-lab.sh`: `name` mints an `fm-lab-*` session, `provision` starts it and records the live default session as a tripwire, `run` appends the trailing `--session` and refuses lifecycle and server verbs, and `teardown` re-checks refuse-default before each destructive call.

**Prefer the `bin/fm-*` wrappers over raw herdr for crew work.** `fm-spawn.sh` creates the pane and records `backend=herdr`, `herdr_session`, `herdr_workspace_id`, `herdr_tab_id`, and `herdr_pane_id` in `state/<id>.meta`. `fm-send.sh` delivers a steer, verifies the Enter actually submitted, and exits non-zero on a swallowed one; a raw `agent prompt` skips that verification. `fm-watch.sh` owns liveness. Address a crewmate by task id, or by the fully qualified `<herdr-session>:<pane-id>` form; a bare pane id is refused because a "successful" send to the wrong endpoint is worse than a loud failure.

**Herdr 0.8.0 is not AXI-compliant, and `specs/axi-retrofit.md` in this repository is the audited detail.** In practice: output is raw JSON wrapped in an `id`/`result`/`type` envelope, so pipe through `jq`; errors are structured but go to **stderr** with exit 1, so a stdout-only capture sees nothing at all on failure; usage errors exit 2; lists carry no counts and an empty list is a bare `[]`; and nothing suggests a next command. Closing is not idempotent - a second `pane close` on the same id returns `pane_not_found` with exit 1 - while `workspace focus` and `pane zoom --off` repeat cleanly.

**Ids.** `w1` workspace, `w1:t1` tab, `w1:p1` pane. Closed ids are never reused, and a pane moved between workspaces gets a new id. Parse ids out of the JSON response of the command that created them; never derive one from sidebar order or from an example in this file.

**Do not close panes, tabs, or workspaces you did not create.** They belong to another crewmate or to the human.

## Non-goals

- Not a flag reference. Every command takes `--help`, and a bare group such as `herdr pane` or `herdr agent` prints that group's command list.
- Not the TUI. Bare `herdr` inside a pane is refused, and outside a pane without a TTY it panics. There is no read-only dashboard invocation.
- Not a job runner. Herdr gives you a terminal and a screen to read; it does not schedule, retry, or collect results.
- Not for `default`-session lifecycle work, ever. That is the fleet's own session.
- Not the place for crew orchestration policy. Spawn, steer, watch, and teardown rules live in the firstmate `bin/fm-*` tooling.
