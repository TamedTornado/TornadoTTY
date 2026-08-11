# Linux Git and review context feature plan

Date: 2026-08-11
Issue: GH-18 (`project.git-review-icons` subset)

## Outcome

The focused terminal's real working directory drives repository, branch, dirty
tree, remote, and pull-request context in Zentty's source-owned window chrome
and worklane sidebar. Users can refresh that state and safely open the branch or
pull request on its remote host.

This is one bounded feature slice. Bookmarks/presets, project icons, and Open
With remain separate GH-18 slices. Zentty does not create worktrees, render a
diff, or become an editor.

## Source contract

- Resolve the canonical repository root with `git`, including nested working
  directories, worktrees, detached HEAD, and non-repositories.
- Prefer the branch's configured remote, then `origin`, then another usable
  remote. Support common GitHub, GitLab, and Bitbucket SSH/HTTPS forms for a
  branch link; only GitHub remotes invoke `gh` for PR state.
- Preserve source PR state precedence: terminal PR state, approval, CI failure
  before running before passed, and merge conflict. A failed refresh preserves
  last-known state and labels it stale/error rather than erasing it.
- Refresh after CWD/focus changes, explicit user request, and agent completion.
  Background refresh is bounded to visible pane repositories and coalesced by
  `(repository root, reference)`; unrelated repositories are never scanned.
- Remote links are data, never shell text. Only validated HTTP(S) links may be
  handed to the desktop URL launcher.

## Test-first construction order

1. Pure tests pin remote parsing, URL encoding, PR JSON/state precedence,
   malformed output, detached references, dirty state, and stale fallback.
2. Real-system core tests create temporary repositories, commits, branches,
   detached HEADs, nested paths, worktrees, and remotes, then invoke real `git`.
3. A controlled executable at the real `gh` subprocess boundary reads PR state
   from a local HTTP forge fixture. Product code receives no fixture hook.
4. GTK journeys start the staged product under controlled X11 and Wayland,
   type into a real Ghostty PTY, change CWD/branch/dirty state, invoke refresh,
   and assert visible/accessibility/log receipts plus safe URL routing.
5. Mutation testing targets parsing and state precedence. Feature inventory,
   architecture contracts, Clippy, ShellCheck, and the presently executable
   qualification matrix run before promotion.

## Acceptance criteria

- [x] Real nested repository and worktree roots resolve canonically.
- [x] Branch and detached HEAD are distinct; dirty state is explicit.
- [x] Remote selection and GitHub repository association match source rules.
- [x] GitHub PR draft/open/merged/closed, approval, checks, conflicts, age,
      refresh failure, and no-PR states are explicit.
- [x] Context is routed by stable window/worklane/pane identity and never leaks
      between panes sharing a branch name.
- [x] Window chrome and sidebar remain useful with narrow widths and long names.
- [x] Manual refresh and branch/PR remote-open actions are keyboard-accessible.
- [x] Hostile schemes, credentials, control characters, traversal, and malformed
      remotes cannot reach the URL launcher.
- [x] Subprocesses have explicit executable resolution, output bounds, timeout,
      and no shell interpolation.
- [x] X11 and Wayland journeys exercise the real product boundary.
- [x] Dogfood records every discovery, failure, repair, and limitation.

## Qualification language

Passing this slice means only that its implemented local cells pass. It is not
full Linux qualification while the authoritative matrix contains BLOCKED,
XFAIL, or NOT_IMPLEMENTED entries.
