# aide

**aide** is a filesystem-driven orchestrator for background coding-agent
jobs. Author a job as a small YAML file plus a prompt, mark it `READY`,
and a watcher process launches the agent (currently
[Codex](https://github.com/openai/codex)) in its own tmux window and
tracks it through to completion — no manual `cd`-and-type-a-prompt
required, and no need to babysit it.

## Why

Running a coding agent today means switching away from wherever the actual
work happens, opening a terminal, and typing a prompt — every time there's
a task. It gets harder with more than one agent running at once: tracking
which one is doing what, noticing which one is quietly waiting on an
approval, remembering the right flags to launch each one with, and keeping
terminal sessions/tabs organized. None of that is the actual work — it's
overhead around the work, and it can end up costing more attention than
the tasks being offloaded actually need.

## Philosophy

aide is built to be native to the terminal and tmux, rather than requiring
a separate UI to manage agents. It draws a clear line between two
decisions that otherwise get tangled together: what to do yourself versus
what to hand off, and how whatever gets handed off actually runs. aide only
takes care of the second part — what's worth offloading is still entirely
up to the user; aide just makes sure that once something is offloaded, it
gets launched, tracked, and reported on automatically.

Anything offloaded stays a tmux window away: attach to it and continue the
conversation with that same agent, in the same session, with its full
context intact, exactly as if you'd started it yourself.

This is a personal, opinionated tool rather than a general-purpose
product — best suited to people already comfortable living in a terminal.

### A personal note

> I built this because I live in neovim, inside tmux, and the moment I have
to break away from that — switch windows, hunt down a terminal, go type a
prompt into some agent's UI — I'm out of my flow. It only got worse once I
had more than one agent running at a time: keeping track of which one was
doing what, which one had been quietly sitting on an approval prompt for
the last ten minutes, remembering the right flags to launch each one with,
juggling terminal tabs and sessions so nothing got lost. None of that was
the actual work — it was overhead *around* the work, and it was draining
more energy than the tasks I was actually offloading.

> So I built aide to stay out of the way: I decide what to do myself and
what to offload, and aide takes care of getting whatever I offload
launched, tracked, and reported on, without me having to hold any of that
in my head. If I ever need to check in on something, it's still just a
tmux window away — I can attach to it and keep talking to that same agent
without losing any context, exactly as if I'd started it myself.

## How it works

1. **Write a job.** An `aide.yml` file plus a prompt file (see the example
   below), living together in their own directory anywhere in your
   workspace.
2. **Mark it ready.** Set `status: READY` (jobs start as `DRAFT` while
   you're still editing them). If it has `dependencies` or an
   `executeAfter` time, it only becomes eligible once those are satisfied.
3. **Run the watcher**, from inside a tmux session:
   ```bash
   aide path/to/workspace
   ```
4. **It takes it from there.** The watcher opens a new tmux window (it
   won't steal your focus) and launches your configured agent in it, with
   the repos/directories you listed and your prompt handed to it directly.
5. **Watch or jump in.** Because each job runs in its own tmux window, you
   can attach to it at any point to see what it's doing, or take over
   directly — the watcher isn't in the way.
6. **Check the console for status.** The watcher logs each job's lifecycle
   as it happens — `queued`, `scheduled`, `running`, `awaiting approval`,
   and finally `success`, `failure`, or `done` (the agent decides
   success/failure for itself; `done` just means it stopped without saying
   either way).

If a job lists other jobs under `dependencies`, it won't be picked up
until every one of those has finished with `success` — so you can chain
work ("do X, then once X succeeds, do Y") just by wiring up ids.

## Example

`aide.yml`:

```yaml
title: Fix flaky auth test
id: fix-auth-test
window: fix-auth-test
status: READY
dependencies: []
root: /home/me/projects/myapp
git:
  - name: myapp
    dir: /home/me/projects/myapp
    description: Main application repo
    worktree: /home/me/projects/myapp-worktrees/fix-auth-test
agent:
  codex:
    arguments:
      - -a never
      - -s workspace-write
      - -c model_reasoning_effort=medium
prompt-file: prompt.md
```

`prompt.md`:

```markdown
Investigate the intermittent failure in tests/auth_test.rs on main and fix
the root cause. Don't just retry/skip the test.
```

A few notes on the fields:

- `root` is the directory the agent actually runs in — point it at
  whichever repo/directory has the context (`AGENTS.md`, `CLAUDE.md`,
  etc.) the agent should pick up automatically.
- `dirs` and `git` list any *additional* directories/repos the agent
  should have access to, beyond `root` — each with a short description to
  help the agent understand what it's looking at. If a `git` entry needs
  changes made, the agent works in its own git worktree/branch rather than
  touching your primary checkout directly.
- `agent.codex.arguments` are passed straight through to the `codex` CLI
  as-is — full control over model, reasoning effort, sandboxing, approval
  policy, etc.
- `id`/`window` just need to be unique across the workspace; the watcher
  refuses to schedule a job that collides with another.

## Getting started

Requires a working [tmux](https://github.com/tmux/tmux) install and the
[Codex CLI](https://github.com/openai/codex) — the only agent backend
supported so far.

```bash
cargo build --release
# from inside a tmux session:
./target/release/aide path/to/workspace
```

Author your job directories under `path/to/workspace`, flip each one to
`READY` when it's ready to go, and the watcher picks them up automatically
on its next pass.

## Status

Early, single-user project, evolving quickly. Codex is the only supported
agent backend today, with room to add others (Claude, Gemini, ...) later.
`aide` is just the watcher for now — you author `aide.yml`/prompt files by
hand; job-scaffolding and other CLI conveniences are expected to land as
subcommands on the same binary later.

## License

MIT — see [LICENSE](LICENSE).
