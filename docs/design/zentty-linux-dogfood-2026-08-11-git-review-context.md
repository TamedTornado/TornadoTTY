# Zentty Linux dogfood — Git and review context

Date: 2026-08-11
Issue: GH-18 (`project.git-review-icons` subset)

## Slice contract

The test-first contract is
[`linux-git-review-context-feature-plan.md`](linux-git-review-context-feature-plan.md).
This report is updated while the slice is built, not reconstructed afterward.

## Source and design discoveries

- Source Zentty separates fast local Git context from slower GitHub review
  resolution. It resolves the real repository root and branch/detached state
  first, then uses the branch-configured remote, `origin`, and remaining remotes
  in that order.
- Source invokes `gh pr view <branch> --repo <owner/repo> --json ...`; it does
  not own a forge client. Linux will retain that real executable boundary and
  test it with a controlled `gh`-compatible executable backed by a local forge,
  rather than adding an in-product mock path.
- Source builds branch links for GitHub, GitLab, Bitbucket, and unknown HTTP(S)
  Git hosts. Linux must additionally enforce the GH-18 hostile-link boundary
  before a URL reaches the desktop launcher.
- PR CI precedence is failure, then running, then passed. Approval and conflict
  apply only to open PRs; merged and closed PRs stop at terminal state.

## Construction record

- Planning started from GH-18's downstream acceptance criteria and the current
  Swift resolver, store, chrome, and resolver tests. Bookmarks, icons, and Open
  With were explicitly excluded so this does not become another multi-feature
  implementation slice.
- The initial focused core suite now creates real repositories and invokes real
  Git for nested-root, branch, detached-HEAD, dirty-tree, non-repository, and
  remote behavior. A real executable at the `gh` boundary proves PR precedence
  and argv routing. Remote parsing rejects non-web schemes, credentials,
  traversal, query-bearing clone URLs, and control-character injection.
- The first controlled X11 product run failed before product launch because the
  new journey accepted the matrix's repository-relative staged binary path and
  then changed into its temporary repository before `exec`. The shell therefore
  resolved the binary relative to the fixture. The journey now canonicalizes a
  relative product path before changing CWD, matching the established product
  journey contract. This was a harness path bug, not a pass or environmental
  waiver.
- The coordinator now owns one two-second GLib observation source and permits
  at most one blocking-pool probe. It resolves the real foreground process CWD,
  falls back to the pane's durable launch CWD, skips SSH panes, and bounds each
  pass to 24 panes. Source-compatible review refresh intervals are 15 seconds
  while checks run, 60 seconds for other open/draft reviews, 90 seconds without
  a PR, and five minutes for merged/closed reviews; background panes use 90
  seconds. Focus changes, manual refresh, and authenticated agent idle/session
  completion force a refresh without creating another poller.
- A failed `gh` refresh initially risked replacing useful review state with an
  error-only context. The repair preserves the last review only when repository
  root and reference are unchanged, marks the result stale/error, and exposes
  the fetch age. A branch or repository change cannot inherit that evidence.
- The real staged-product journey passed in private Xvfb and private nested
  Cage Wayland sessions. Each run launched the actual Zentty/Ghostty product in
  a dirty nested real repository, invoked the real `git` and a real executable
  `gh`-compatible boundary backed by the controlled local HTTP forge, drove the
  command palette with compositor input, and observed exact validated branch
  and pull-request URLs at an independent `xdg-open` executable boundary.
- The architecture contract correctly failed after `ProjectContextRuntime` was
  added because the new coordinator, field, functions, actions, hashes, and
  single-authority projection were not inventoried. The contract and its
  validator now enumerate that owner explicitly; both the positive validator
  and its negative self-tests pass. No second workspace, pane registry, async
  runtime, or product-specific test application was introduced.
- The first governed mutation run covered 194 mutants across pure Git policy
  and GTK coordination: 144 were caught, 22 were compiler-unviable, 27 survived,
  and the EOF-loop inversion timed out. Surviving accessor/error/URL-boundary
  mutations exposed missing assertions; added tests caught five of them. The
  remaining 22 are GTK coordinator glue whose observable behavior is covered
  by the staged X11/Wayland journeys but is outside cargo-mutants' Cargo-test
  process. This is recorded rather than misrepresented as a perfect mutation
  score. The EOF inversion is an infinite read loop and is killed by the
  mutation timeout. Both runs used `.cargo/mutants.toml` plus explicit
  `--gitignore=true`; ignored `build/linux-deps` was never copied.
- Mutation rerun exposed a real fixture isolation defect: worktree and
  non-repository paths used only a process-local counter, so stale paths from a
  prior interrupted process could collide. Temporary auxiliary paths now
  include the process ID and counter; the real linked-worktree test passes from
  a fresh cargo-mutants scratch tree.
- The first full qualification attempt failed before cells ran because the two
  new cells used an undeclared `git_review_context` capability and the feature
  inventory runner still asserted the prior PARTIAL/NOT_IMPLEMENTED totals.
  Both machine contracts now enumerate the new state, and their negative runner
  suites pass. A concurrent nested-X11 wrapper self-test also returned its
  synthetic private-service status once; a traced rerun and the subsequent
  complete wrapper suite passed without weakening that fail-closed assertion.
- The next full qualification run executed the new Git journeys successfully
  but exposed three existing real-product regressions. An asynchronous initial
  project-context result could make a worklane card structurally incompatible,
  causing generic sidebar metadata refresh to rebuild the card during a real
  drag; the same broad refresh also reprojected terminal/chrome presentation
  during selection and Global Find. The failures were real: worklane drag
  retained intermediate feedback, selection-to-find lost the selection once,
  and Global Find lost its cross-pane navigation anchor once.
- The repair keeps project context as metadata rather than topology. A stable
  hidden fingerprint updates only the project row in place, and the project
  coordinator no longer invokes terminal presentation/layout refresh. Project
  metadata projection is deferred while the command palette, Global Find, or a
  worklane reorder preview owns an interaction; the stored context is projected
  by the next ordinary render. The affected X11 pane-search, Wayland pane-search,
  and X11 sidebar-management journeys all pass against the rebuilt ReleaseSafe
  product. This is a product concurrency repair, not a test delay or retry.
- The same three cells still failed only when the matrix ran them concurrently.
  Reproducing those three journeys concurrently outside the matrix proved the
  hidden coupling: both older harnesses launched their shells in the ambient
  repository. GH-18 therefore made unrelated search/sidebar scenarios perform
  real Git and GitHub review discovery, and the asynchronous review completion
  landed inside their physical-input timing windows. This was test-environment
  drift, not a reason to disable real project discovery. The two harnesses now
  canonicalize the staged binary and launch their real PTYs from private,
  controlled non-repository directories. GH-18 alone owns the real repository
  and local-forge scenario. All three affected journeys pass concurrently after
  this isolation repair; no sleeps, retries, product fixture flags, or mocked
  components were added.
- The final `linux/tests/qualify-local` run passed every presently executable
  support test and matrix command, including both Git/review cells and the
  three repaired concurrent journeys. The authoritative declared totals are
  **PASS=103, FAIL=0, BLOCKED=7, XFAIL=1, NOT_IMPLEMENTED=21**. This establishes
  “implemented local suite passed” only. Release and full Linux qualification
  remain correctly NOT_PASSED. The Debug IBus Valgrind cell remains **PASS with
  reviewed suppressions**; no unsuppressed-clean claim is made.

## Remaining work and uncertainty

- This slice does not implement GH-18 bookmarks/presets, project icons, or the
  broader Open With contract. The feature inventory therefore remains PARTIAL.
- The Git/PR slice has implemented local evidence, but no full Linux QA claim is
  made while the authoritative qualification matrix still contains BLOCKED,
  XFAIL, and NOT_IMPLEMENTED cells.

## 2026-08-12 event-driven refresh closeout

The prior coordinator already had one two-second observation source, adaptive
review intervals, focus/manual forcing, and authenticated agent-completion
forcing. GH-18 still explicitly requires relevant filesystem and process-event
refresh. This closeout will extend that one coordinator rather than introduce a
second watcher/scanner. The existing real product journey is being expanded
first: one branch transition is typed into the real PTY and one is performed by
an independent real Git process. Neither transition may use the manual refresh
action. Both must pass under controlled X11 and Wayland before the inventory is
promoted.

A source audit also corrected the remaining bookmark wording in the GH-18
audit: source `WorkspaceTemplate` has no agent-context field and source
`ZenttyCLI`/`PaneIPCHandler` exposes no bookmark command. Supported agent
context is captured only through the source fields that actually exist—safe
environment overrides, live launch command, title/CWD, and source linkage.
Inventing a Linux-only bookmark CLI or opaque agent payload would be a parity
regression, not acceptance work.

The test-first X11 journey initially failed exactly at the new terminal-process
cell: changing from `feature/review-context` to `process-refresh` in the real
Ghostty PTY did not update projected context before the ten-second deadline.
This was the intended RED result. It established that the existing adaptive
timer, focus action, and manual action did not satisfy the source event contract.
The readiness condition also had to be narrowed from the generic
`project-context pane=pane-1` receipt to the complete initial branch/dirty/PR
receipt; otherwise a process-request diagnostic could masquerade as resolved
context.

The implementation extends the existing `ProjectContextRuntime`, rather than
adding another coordinator. The real resolver now asks Git for
`--absolute-git-dir` and canonicalizes the result, which is necessary because a
linked worktree's `.git` is a file pointing at metadata elsewhere. Each live
pane owns a bounded GIO watch set: the repository root and exact `HEAD`,
`index`, `packed-refs`, and `logs/HEAD` metadata paths. There is no recursive
source-tree observer. Process-title, filesystem, focus, explicit command,
authenticated agent completion, and the existing adaptive timer all mark the
same pane-keyed `force_panes` set and feed the single `probe_in_flight` worker.
Pane teardown cancels and drops every owned monitor; a focused GIO test proves
the cancellation primitive, and stale async results still require matching
live canonical CWDs before projection.

The expanded journey uses three independent real boundaries: a branch switch
typed into the actual PTY, a branch switch from an external `git -C` process,
and an authenticated `agent.idle` event sent through the pane's real private
socket and token by the staged Zentty CLI. The first agent assertion failed even
though the raw log showed the correct event because the test expected the
diagnostic fields in reverse order. The assertion was corrected to the product's
stable `pane=... refresh=...` receipt; no product retry or timing allowance was
changed. Both controlled X11 and input-capable nested Wayland then passed the
complete process/filesystem/agent/manual/safe-open journey.

A governed diff-scoped mutation run covered the new canonical Git-directory
resolver branch with `.cargo/mutants.toml` and explicit
`--gitignore=true`: three mutants were generated, two were caught, and one was
compiler-unviable. The ignored multi-gigabyte `build/linux-deps` tree was not
copied. The GIO callbacks remain qualified at the real staged-product boundary
rather than represented as Cargo-unit mutation coverage.

Strict workspace Clippy then rejected the existing `request_probe` function at
102 lines after the event wiring crossed the project's 100-line limit. Rather
than suppress the lint, source selection was extracted into the focused
`collect_probe_sources` helper. The single in-flight owner and observable
behavior are unchanged, and strict all-target Clippy passes.

One local X11 invocation also failed before product startup because the command
was attempted inside the filesystem sandbox, where the host-owned
`/tmp/.X11-unix` appears mapped to `nobody`. Re-running the same controlled
wrapper outside that namespace used the standard root-owned socket directory
and passed. This was an execution-namespace limitation, not an environmental
pass and not a product or harness relaxation.
The first workspace-test invocation likewise reached the known sandbox
boundary: eight real agent-IPC tests could not bind Unix sockets and failed
with `Operation not permitted`. The complete unchanged workspace suite passed
when rerun outside that namespace; the failure was not converted into a skip.

With these cells passing, the inventory's bounded
`project.git-review-icons` item is IMPLEMENTED. GH-18 itself remains open: its
portal/compositor bookmark-management XFAILs and the authoritative Linux matrix
still prevent release or full-Linux qualification claims.

The final rebuilt ReleaseSafe product passed the expanded journey again under
both controlled X11 and input-capable nested Wayland. The complete
`linux/tests/qualify-local` run then passed every presently executable support
test and matrix command in 431.58 seconds. Declared totals are **PASS=110,
FAIL=0, BLOCKED=7, XFAIL=4, NOT_IMPLEMENTED=21**. Implemented-local and
product-boundary qualification passed; release and full Linux qualification
remain correctly NOT_PASSED. Summary SHA-256:
`2593a322bb8aa53633d8b5d6048277f20c8b3948b6d6d33ade155a2cc4feb86c`.

Debug Valgrind remains **PASS with reviewed suppressions**, never an
unsuppressed-clean claim. Its preserved raw receipt reports 426 errors in 426
contexts, 6,240 definite bytes, and 41,397 indirect bytes; the reviewed
post-suppression receipt reports zero errors/contexts and zero definite/indirect
bytes, with 427 suppressed contexts tracked. ReleaseSafe Valgrind remains XFAIL and no
suppression was broadened in this slice.

Reviewing the expanded journey after that pass found one harness-quality defect:
the agent-completion refresh could increment the same context receipt count
used by the later manual-refresh assertion. The assertion now snapshots the
count immediately before the palette action and requires a strictly newer
receipt. Qualification was rerun rather than relying on the earlier summary.

That rerun exposed an unrelated but real controlled-environment cleanup race in
the existing X11 bookmark portal XFAIL. `xdg-document-portal` left its private
`XDG_RUNTIME_DIR/doc` FUSE mount behind after the owned process group exited;
`rm -rf` then failed on the disconnected endpoint before `nested-x11` could
publish its environment report. The matrix correctly rejected the XFAIL as
`MISSING_OR_INVALID_ENVIRONMENT_REPORT` instead of accepting environmental
absence. The wrapper now detects that exact private portal mount, requires
`fusermount3` only when it exists, unmounts it, proves it is gone, and only then
removes the run root. A direct rerun of the real D-Bus/portal/Openbox XFAIL
returned its expected exit 1 and published a complete isolation report with
`run_root_removed=true`; the blocker itself remains XFAIL.
