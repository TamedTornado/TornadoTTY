# Linux dogfood: GitHub repository routing incident

Date: 2026-08-27

## Discovery

The checkout has two Git remotes: the public Linux fork is
`TamedTornado/zentty` (`origin`), while the source application is
`dedene/zentty` (`upstream`). GitHub CLI inferred `upstream` as its default
repository. Unqualified issue commands could therefore mutate the source
author's tracker instead of the fork tracker.

The immediate incident was creation of a Linux title-rendering issue as
`dedene/zentty#82`. It was closed as mistakenly routed and recreated correctly
as `TamedTornado/zentty#132`. No upstream pull request was opened or reviewed
in that incident.

Operator-supplied notification evidence then proved two earlier accidental
comments by the `TamedTornado` account on already-merged upstream pull requests:

- On 2026-08-04, `gh issue comment 16 --body ...` intended for the fork's UX
  parity issue selected `dedene/zentty#16`, the merged clean-copy pull request.
  The comment reported a Linux pane focus-presentation slice at `437086d`.
- On 2026-08-13, `gh issue comment 20 --body ...` intended for the fork's
  Settings epic selected `dedene/zentty#20`, the merged menu-bar status-pill
  pull request. The comment reported the Linux Updates & Privacy slice at
  `235330f` and qualification totals.

Both comments are now absent from GitHub's conversation, review, inline-comment,
and timeline APIs. Their PR `updated_at` values match the notification dates
exactly. Local session transcripts preserve the two unqualified commands and
comment bodies but contain no corresponding deletion command, so this audit
does not claim who removed them.

An audit of every upstream pull request created before the Linux port and
updated after 2026-08-01 found no other unexplained case. PRs #59 and #61 were
legitimately merged on their August update dates; only #16 and #20 were merged
months earlier and updated on the accidental-comment dates.

## Repair and prevention

- GitHub CLI's checkout default is now explicitly `TamedTornado/zentty`.
- Repository agent instructions require every mutating issue/PR command to use
  `--repo TamedTornado/zentty` or `-R TamedTornado/zentty` explicitly.
- Read-only upstream commands must explicitly name `dedene/zentty`.
- Any upstream mutation requires Jason's explicit approval for that exact
  interaction.

The explicit repository argument is the durable control. The CLI default is
only defense in depth and must never be treated as authorization to omit it.
