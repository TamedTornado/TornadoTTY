# Zentty Linux dogfood: route to a usable build

Date: 2026-08-21

## Trigger

After GH74 closed, the public tracker still made the port look substantially
less complete than the product and machine-readable authorities. Jason also
called out the central delivery problem: the port has been underway for weeks
without a crisp point at which he can simply install and use it.

## Discovery

The tracker had 13 open issues, but they were not 13 equivalent unfinished
features:

- four were parent or umbrella trackers: GH1, GH7, GH20, and GH23;
- seven were active implementation or closure issues: GH4, GH11, GH16, GH17,
  GH73, GH75, and GH76;
- GH65 and GH77 were already classified as deferred;
- the GH1 delivery order and child checklist still named completed GH3, GH5,
  GH8, GH14, GH22, GH32, and GH35 as pending work.

The feature inventory reports 25 `IMPLEMENTED`, 23 `PARTIAL`, and 12
`NOT_IMPLEMENTED` entries. None of the `REQUIRED_INITIAL_RELEASE` entries is
`NOT_IMPLEMENTED`. Of the 12 unimplemented entries, Ghostty appearance and
Linux update delivery are platform alternatives; terminal diagnostics and the
remaining agent adapters are explicitly required later.

The qualification matrix reports 188 `PASS`, zero `FAIL`, zero `BLOCKED`, three
`XFAIL`, and four `NOT_IMPLEMENTED`. The two unimplemented worklane product
journeys are real closure work for GH4/GH16, not evidence that worklanes do not
exist. Task Manager network/cgroup accounting is deferred. No non-pass cell was
converted into a pass during this reconciliation.

## Decision

Personal dogfooding and public-release completion are separate milestones. The
shortest honest personal-dogfood route is:

1. GH73 safe close/quit/window lifecycle;
2. one coherent GH4/GH16/GH17 shell and UX closure campaign, including the two
   declared worklane product journeys;
3. required Codex, Claude Code, and OpenCode workflow closure under GH7;
4. Ghostty appearance closure and GH20;
5. an installed dogfood Debian package.

GH76 remains mandatory before calling the public build trustworthy. GH75 update
discovery and GH11's upstream/downstream decision remain public-release and
maintenance work, but do not block Jason from using a manually installed
package. GH65, GH77, and additional `REQUIRED_LATER` agent adapters remain
explicitly deferred.

## Repair

- Updated GH1's count, delivery order, and completed GH3/GH5/GH8 checklist
  entries.
- Updated GH7's completed inventory-governance criteria while retaining the
  sustained installed coding-agent workflow criterion as open.
- Did not close GH4, GH16, GH17, or GH20 from prose receipts alone. Their
  machine-readable partial/unimplemented statuses must be reconciled through
  the owning feature work rather than paperwork.

## Remaining uncertainty

GH20's issue history says substantial appearance work landed, while the feature
inventory still calls `configuration.ghostty-appearance` `NOT_IMPLEMENTED`.
That is a real contradiction to audit and either repair or reclassify; this
report does not resolve it by assertion. Several broad GH4/GH16/GH17 checklist
items also contain both delivered and residual behavior and therefore cannot be
truthfully checked until their closeout campaign narrows the exact remainder.

