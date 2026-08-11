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
