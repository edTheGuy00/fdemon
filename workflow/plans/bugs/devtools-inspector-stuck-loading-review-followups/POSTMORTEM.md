# Postmortem: Inspector Stuck on "Loading widget tree…"

**Status:** Fixed (2026-05-13)
**Branch:** `fix/devtools-improvements`
**Real fix commit:** see `handler/devtools/mod.rs:handle_enter_devtools_mode` lazy-start path.

---

## TL;DR

The Inspector panel rendered "Loading widget tree…" forever on the first DevTools
entry. After producing two BUG.md files, an 8-task original plan, a 12-task
review-followup plan, and a 6-agent code review — **the actual bug was never
identified**, because no one had a runtime log of the failing path. Once
diagnostic `info!` instrumentation was added and a single log was captured, the
bug fell out in the first 4 lines of that log: the lazy-start path of
`handle_enter_devtools_mode` engaged the fetch-debounce on its own follow-up
message, blocking the only `FetchWidgetTree` dispatch.

The fix is a 1-line deletion (a `record_fetch_start()` call) plus an updated
comment.

---

## Actual root cause

`handle_enter_devtools_mode` (the lazy-start branch, when `perf_shutdown_tx is
None`, i.e. the perf task hasn't been spawned yet) does this:

1. Returns `UpdateAction::StartPerformanceMonitoring` as the action.
2. Returns `Message::RequestWidgetTree` as the follow-up message.
3. **(The bug)** Called `inspector.record_fetch_start()` before queuing the
   follow-up — setting `loading=true` and `last_fetch_time=now`.

`process.rs` then runs the follow-up `RequestWidgetTree` through `update()` in
the **same** synchronous cycle. The handler's first check
(`is_fetch_debounced()`) returns `true` because `loading=true` (set 0 ms ago)
— so it returns early and **never dispatches `FetchWidgetTree`**. The spawn
task never runs. `loading=true` stays set forever. The UI is stuck.

A misleading comment claimed "the actual tree fetch is dispatched via the
StartPerformanceMonitoring action path" — but no such code path exists. The
perf monitoring action only handles performance polling; it has never
dispatched a widget-tree fetch.

The "smoking gun" log excerpt from `tmp/fdemon-1778683786113-19988.log`:

```
21:50:10.679  handle_enter_devtools_mode ENTRY                loading=false root=None vm_connected=true
21:50:10.679  record_fetch_start + queued RequestWidgetTree follow-up
21:50:10.679  RequestWidgetTree ENTRY                         loading=true  last_fetch_elapsed_ms=Some(0)
21:50:10.680  RequestWidgetTree debounced                     loading=true  last_fetch_elapsed_ms=Some(0)
```

Everything beyond line 4 — isolate resolution, readiness poll, RPC call,
terminal handler — is absent from the log because it was never invoked.

---

## The fix

`crates/fdemon-app/src/handler/devtools/mod.rs:232-243` — removed the
`record_fetch_start()` call from the lazy-start follow-up site. The follow-up
`RequestWidgetTree` handler in `update.rs` already calls `record_fetch_start()`
itself just before dispatching `FetchWidgetTree`, so the invariant is still
maintained at the correct point.

Regression test:
`crates/fdemon-app/src/handler/tests.rs::test_enter_devtools_lazy_start_followup_dispatches_fetch_widget_tree`
— asserts the full chain: `handle_enter_devtools_mode → follow-up
RequestWidgetTree → FetchWidgetTree action`, with explicit checks that
`loading=false` and `last_fetch_time=None` at the moment the follow-up runs.

---

## Why the prior plans missed it

The original `BUG.md` listed **5 hypotheses**, ranked from most to least
likely. **All 5 were wrong.**

| # | Hypothesis | Status | Verdict |
|---|-----------|--------|---------|
| 1 | Readiness poller eats the timeout budget | Phase 4 fix (shrink poll) | Irrelevant — the poll never runs |
| 2 | `main_isolate_id` picks the wrong isolate | Phase 3 fix (resolve Flutter UI isolate) | Irrelevant — isolate resolution never runs |
| 3 | `r` refresh is debounce-blocked for 2 s after a failed fetch | Phase 2 fix (clear_fetch_debounce) | Irrelevant — the debounce engages *before* any fetch fails (there's no fetch to fail) |
| 4 | Spawn task's `msg_tx.send` failure is silently dropped | Phase 2 fix (promote to error log) | Irrelevant — the spawn task never runs, so no send ever happens |
| 5 | Inspector path emits only `debug!`/`trace!` logs | Phase 1 fix (add `info!` instrumentation) | **The only useful Phase 1 work** — it's what eventually surfaced the real bug |

The 12-task **review-followup** plan (this directory) was based on a 6-agent
code review of the post-Phase-1-through-4 branch. The agents reviewed against
architectural correctness, code style, API hygiene, and security. **None of
them had a runtime log.** All 12 tasks landed correctly and improved the code,
but none addressed the user-visible bug — the Inspector was still stuck after
all 12 tasks merged.

In retrospect:

- **Hypothesis 3 was directionally right.** A debounce is blocking
  `RequestWidgetTree`. But the BUG.md narrative ("after a failed fetch") sent
  every subsequent investigation toward the failure-path debounce-clear fix,
  which was real but unrelated to the actual stuck case. The actual debounce
  fires *before* the first fetch ever runs, not after.
- **Task 07** of the review-followup plan (Use `record_fetch_start()` at
  auto-fetch sites) *exacerbated* this exact bug at this exact site — it
  replaced `inspector.loading = true` with `inspector.record_fetch_start()`.
  Both forms engage the debounce (since `is_fetch_debounced()` returns true
  whenever `loading=true`), but the new form also sets `last_fetch_time`,
  making the bug strictly worse (cooldown extended). The task implementor
  noticed the debounce collision and added a "NOTE" comment claiming "the
  actual tree fetch is dispatched via the StartPerformanceMonitoring action
  path" — which was factually wrong. No reviewer caught the false claim.

---

## Process lessons

### Speculative code review is not a substitute for a runtime log

Before this round, ~22 distinct tasks landed (8 original + 12 followups + 2
review meta-tasks) without anyone capturing a single failing-flow log of the
actual stuck-inspector path. Once a log was captured (after temporarily
adding `info!` markers at the dispatch entry points and terminal handlers),
the bug was found by reading 4 lines of log output.

**Rule for future bug plans:** when the user-visible symptom is reproducible
on demand, **the first task is always "capture a runtime log of the failing
path".** Do not generate hypothesis-driven task plans before producing the
log. Phase 1 of the original `BUG.md` named exactly this — but its
"Measurable Outcome" ("a maintainer reading the log can determine which of
the 5 hypotheses fired") implicitly assumed the answer was on the list. None
of the 5 hypotheses fired, because none of them were the bug.

### Verify causal claims in comments

The comment "the actual tree fetch is dispatched via the
StartPerformanceMonitoring action path" was load-bearing for the design
choice (engaging the debounce in advance "so the spinner shows immediately")
but was false. A 2-minute grep would have surfaced that no
`StartPerformanceMonitoring` code path dispatches `FetchWidgetTree` or
`RequestWidgetTree`. If a comment names a specific causal chain, that chain
should be greppable; if it's not, the comment is decoration.

### The 6-agent review caught code-style problems, not behavioral bugs

The review agents (architecture enforcer, code quality, logic & reasoning,
risks & tradeoffs, security, bug fix) produced ~12 actionable findings. They
were all *correct* about what they saw — e.g. `clear_isolate_cache` was an
unnecessary alias, `AutoRehydrate` was dead code, the VM Service auth token
leaked into logs. But they did not (and could not) verify that the integrated
behavior matched the user's expectation. **Static review finds local defects;
empirical observation finds whole-flow defects.** Both are needed. Neither
substitutes for the other.

### Phase 1 instrumentation was the only Phase that mattered

Of the 8 tasks in the original BUG plan, only Phase 1 (add `info!`
instrumentation) was on-path for finding the bug. The other 7 made the
codebase better but did not fix the user-visible problem. In a future plan
of similar shape, consider scheduling Phase 1, **stopping**, capturing a log,
then deciding which (if any) of the later phases are still warranted.

---

## What's left

- `workflow/plans/bugs/devtools-inspector-stuck-loading/BUG.md` — its
  `## Bug Reports` section can remain as written, but it should be annotated
  to record that all 5 hypotheses were wrong and the actual cause is
  documented here.
- The original plan's Phases 2-5 stand on their own merits (cleaner code,
  defense-in-depth) but should not be cited as having fixed the stuck-inspector
  bug.
- The 12-task review-followup plan stands on its own merits as code hygiene.
  No items should be cited as having fixed the stuck-inspector bug either.
- This fix supersedes the user-visible-bug claim of both prior plans.
