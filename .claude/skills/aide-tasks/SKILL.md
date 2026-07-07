---
name: aide-tasks
description: Use when the user gives a specification/requirements doc and wants it turned into aide jobs — e.g. "turn this spec into aide tasks", "break this down into aide jobs", "create aide tasks for X", "author aide.yml files for this", or any request to plan/schedule work for the aide watcher. Covers evaluating the spec for ambiguity/completeness, decomposing it into a serial/parallel task graph, and authoring aide.yml + prompt files in DRAFT so the user decides when each one runs.
---

# aide-tasks

Turn a specification into one or more `aide.yml` + prompt-file job directories
for the aide watcher (see `README.md`, `docs/spec.md`, `docs/aide.yml`, and
the `AideJob` struct in `aide/src/job.rs` for the authoritative schema — reread
whichever of these you need, this skill only summarizes them). Never launch
anything yourself; you are authoring files for the watcher to pick up later.

Work through these steps in order. Don't skip ahead to authoring files before
the spec has actually been evaluated — these jobs run **unattended** (the
watcher's own system prompt tells the agent "no one is watching this session
in real time... make the best autonomous judgment call rather than waiting
for input"), so an ambiguous or incomplete job doesn't get caught by a human
mid-run — it either stalls, silently does the wrong thing, or corrupts a
shared repo before anyone notices.

## 1. Find the workspace

Figure out which directory is the aide workspace (the argument the user
passes to `aide path/to/workspace`) — ask if it isn't obvious from context.
Then recursively scan it for existing `aide.yml` files (same glob the
scanner uses: `<workspace>/**/aide.yml`) and record every `id` and `window`
already in use. Uniqueness is workspace-wide and silent — the watcher just
refuses to schedule a colliding job — so this set has to include *every*
existing job, not just ones related to this spec.

## 2. Evaluate the specification — don't decompose an unclear one

Read the full spec, then check it against all three:

- **Unambiguous** — every instruction has one reasonable reading; no
  "handle appropriately," "etc.," or undefined terms.
- **Complete** — names which repo(s)/dir(s) are involved, what "done" looks
  like for each piece of work, and any ordering constraints between pieces.
- **Clear** — acceptance criteria are stated or directly inferable, not left
  for the eventual agent to invent.

If any of these fail, stop and ask the user — use `AskUserQuestion` for
concrete choices (which repo, which branch, ordering, scope boundaries), or
a direct question for open-ended gaps. Do not guess at scope, repo paths, or
"done" criteria and do not proceed to decomposition until the gaps are
resolved. A spec that passes this bar doesn't need per-task clarification
later — that's the whole point of clearing it up now.

## 3. Decompose into a task graph

Break the spec into the smallest units that each have one checkable outcome
and can be described in a fully self-contained prompt (no mid-run questions
needed). Then:

- Model ordering with `dependencies`, not by cramming steps into one job:
  if task B needs task A's result, put A's `id` in B's `dependencies` — the
  watcher only unblocks B once A resolves to the literal status `SUCCESS`
  (not `DONE`, not `FAILURE`).
- Tasks with no data/order dependency on each other are siblings with empty
  `dependencies` — they become eligible together and the watcher runs them
  in parallel, each in its own tmux window.
- If two tasks would touch the same repo with overlapping changes, make one
  depend on the other rather than letting them run in parallel — parallel
  tasks must not write to the same worktree (see step 4).
- Prefer several well-scoped tasks over one large one, but don't fragment
  past the point where a split-off task would silently need a sibling's
  output without a `dependencies` edge to guarantee it exists first.
- Before writing any files, sketch the resulting graph (which tasks are
  serial, which are parallel) and show it to the user — an ordering mistake
  is much cheaper to catch here than after files exist.

## 4. Repos, worktrees, branches — never disturb existing work

For every git repo a task will modify:

- Give that task's `git` entry its own `worktree` path — never point two
  *parallel* tasks (no dependency edge between them), or a task and the
  user's own primary checkout, at the same worktree. Convention (matches
  the README example): `<repo-parent>/<repo-name>-worktrees/<task-id>`.
- Exception: a chain of tasks linked by `dependencies` (A → B → C) that all
  modify the same repo toward the same net goal may deliberately *share*
  one worktree/branch across the chain instead of each cutting a new one —
  since `dependencies` guarantees they run strictly one after another, there's
  no concurrent-write risk, and it avoids B needing to rebase or re-pull A's
  changes into a fresh worktree. This is a choice, not a default: only take
  it when the tasks are genuinely building on the same change; otherwise
  give each its own worktree per the rule above. If sharing, point every
  task's `git` entry at the same `worktree` path and say so explicitly in
  each downstream task's `prompt.md` (e.g. "continue on the branch left by
  `<task-id>`, don't create a new one").
- Check the repo's actual default branch (don't assume `master`) and check
  `git worktree list` before assigning a path, so you're not colliding with
  a worktree that already exists for unrelated work.
- Leave `root` and the `git` entry's `dir` pointing at the existing primary
  checkout, as the schema intends — that's where repo-specific context
  (`AGENTS.md`, `CLAUDE.md`) lives, and the watcher's own system prompt
  separately instructs the agent to make any changes in the `worktree`, on
  a new branch, never in the primary checkout. Don't try to redirect `root`
  to the worktree yourself; it isn't how the convention works and isn't
  necessary for isolation.
- If the spec or user already named a specific worktree/branch to use,
  respect it rather than inventing a new one.

## 5. Shared context across tasks in the same spec

Tasks split from the same spec often need context beyond what's in their own
prompt — a decision made while clarifying the spec in step 2, a naming or
schema convention two tasks must agree on, a note one task's agent should
leave for a later one. Each job only ever sees its own `prompt.md`, so don't
assume the spec's prose reaches every task on its own; if a task needs
something a sibling produced or decided, say explicitly where to find it.
Two ways to wire that up — pick whichever fits, and name it explicitly in
the consuming task's `prompt.md` (never assume the agent will go looking on
its own):

- **Point-to-point (default for 1–2 consumers):** have the downstream task
  read the specific upstream task's `workspace/output-*.md` (see
  `docs/spec.md`'s Workspace folder section) inside that task's own job
  directory. Name the producer's job directory and expected output filename
  explicitly — this is the same mechanism step 7 uses for "this task
  consumes another task's output."
- **Shared folder (when several/all tasks need the same context):** create
  one folder at the spec level, sibling to the per-task job directories —
  e.g. `<workspace>/<spec-slug>/shared/` — and tell every task that needs it
  to read (and, if appropriate, append to) a specific file there, e.g.
  `shared/context.md`. Seed that file yourself while authoring with
  whatever step 2's clarification settled that more than one task needs
  (shared conventions, schema, decisions), so tasks don't have to
  rediscover it independently. If a task also writes to the shared file,
  say so in its prompt and note it should only rely on writes from tasks
  that strictly precede it via `dependencies` — the watcher gives no other
  ordering guarantee, and parallel siblings must never write to the same
  shared file concurrently.

Reach for either mechanism only when a task genuinely needs something a
sibling or predecessor produced or decided — most tasks should stay
self-contained per step 3.

## 6. Assign ids and windows

Derive a short kebab-case slug per task from its title, and disambiguate
against the full set gathered in step 1 (plus siblings in this same batch)
until every `id` and `window` is unique. `id` and `window` may be the same
string.

## 7. Author the files

Layout: `<workspace>/<spec-slug>/<task-id>/aide.yml` and `prompt.md`,
directories side by side (the scanner globs recursively, so nesting under a
spec-named parent is fine).

Fill `aide.yml` per the schema (`docs/aide.yml`, `AideJob` in
`aide/src/job.rs`):

- `title`, `id`, `window` — from step 6.
- `status: DRAFT` — always; see step 8.
- `dependencies` — other tasks' `id`s, from step 3's graph.
- `executeAfter` — omit unless the spec genuinely calls for a delayed
  start; if set, it must be RFC3339.
- `root` / `dirs` / `git` — per step 4; give every entry a short
  `description` so the agent (which sees this list without the full spec)
  knows what it's looking at.
- `agent.codex.arguments` — if the user hasn't specified model/reasoning
  effort/sandbox flags, propose sensible defaults (e.g. matching the
  README example) explicitly and flag the choice rather than silently
  picking one.
- `prompt-file: prompt.md`.

Write `prompt.md` as task-specific instructions only:

- Self-contained and unambiguous; states its own definition of done.
- If the job has more than one `git`/`dirs` entry, say explicitly which
  one(s) this task's work applies to — the watcher's system prompt only
  lists what's *available*, not what to use.
- If this task consumes another task's output or shares context per step 5,
  say where to find it (the dependency's job directory /
  `workspace/output-*.md`, or the spec-level `shared/` folder) — name the
  path explicitly since the agent has no other way to discover it.
- If this task shares a worktree/branch with a predecessor per step 4's
  exception, say so explicitly ("continue on the branch left by
  `<task-id>`") rather than leaving the agent to assume a fresh one.
- Don't restate the generic conventions (worktree isolation, `.temp`
  outcome reporting, the `workspace/` folder) — the watcher's own system
  prompt already covers those for every job; repeating them here just adds
  noise.

Before considering a task done, check it against what
`aide/src/validator.rs` enforces (there's no `aide validate` command yet,
so this is a manual pass): `title`/`id`/`window`/`root`/`prompt-file`
non-empty, `status` one of `DRAFT`/`READY`/`RUNNING`/`DONE`/`SUCCESS`/
`FAILURE`, `executeAfter` RFC3339 if present, every `dirs`/`git` entry has
non-empty `name` and `dir`.

## 8. Status discipline

Every task starts life as `status: DRAFT` — never write `READY` (let alone
`RUNNING`/`DONE`/`SUCCESS`/`FAILURE`, which belong to the watcher/agent) while
authoring. Once the whole batch is written and you've re-verified the
dependency graph, worktrees, and prompts are right, tell the user what was
created and ask which — if any — should move to `READY` now (typically the
tasks with no unresolved `dependencies`). Only flip `DRAFT` → `READY`
yourself once the user confirms which tasks; otherwise stop after step 7 and
leave every task in `DRAFT` for them to promote by hand.

## 9. Final check

Re-scan the workspace to confirm no `id`/`window` collision was introduced.
Summarize for the user: the task list, the dependency graph (what's serial,
what's parallel), which repos/worktrees/branches each task uses, and the
current `status` of each.
