# Zentty Linux dogfood: CLI epic decomposition

Date: 2026-08-15

## Why this record exists

GH-22 had accumulated the scope of an epic while remaining shaped as a single
feature issue. That made progress reporting misleading: several substantial
CLI foundations were implemented, but the open source-parity, shell,
protocol, agent, and product-boundary work could only be described as a large
undifferentiated remainder.

The operator also caught that recent feature reports omitted the authoritative
qualification totals. Feature-specific test counts are useful evidence, but
they cannot replace the Linux matrix because they hide BLOCKED, XFAIL, and
NOT_IMPLEMENTED cells. Omitting those totals was a reporting regression, not a
change in qualification state.

## Repair

GH-22 is now explicitly titled and structured as the CLI epic. Its executable
scope is decomposed into:

- GH-42: source inventory, parser, aliases, output, and schemas;
- GH-43: authenticated topology/control routing and recovery;
- GH-44: grids, layouts, splits, and exact pane sizing;
- GH-45: Bash, Zsh, Fish, and Nushell integration;
- GH-46: agent-event parity and durable bookkeeping;
- GH-47: remaining Copilot, OpenCode, Pi, and OMP launchers;
- GH-48: secure protocol/recovery and external scripting contract; and
- GH-49: staged and installed real-system qualification.

Each child has acceptance criteria, real-system integration requirements,
mutation/golden expectations where appropriate, and mandatory closeout
evidence. The parent also links rather than duplicates packaging (GH-9),
controlled CI (GH-10), tmux/agent-team behavior (GH-14), and multi-window
authority (GH-32).

The parent distinguishes completed foundations from epic completion. Commits
`9b4681d`, `61e1a20`, `a842478`, `a7d82b9`, and `bf64654` are recorded as
foundation only; the child inventories and real-system qualification may
still expose missing source behavior.

## Reporting rule restored

Every substantive implementation report must now include both its focused
test receipts and the current authoritative matrix totals, followed by the
three separate claims:

1. implemented local suite passed;
2. release qualification passed; and
3. full Linux qualification passed.

No report may imply exhaustive QA while a required cell is FAIL, BLOCKED,
XFAIL, or NOT_IMPLEMENTED. A Valgrind success remains **PASS with reviewed
suppressions**, with raw and post-suppression totals, rather than an
unsuppressed-clean claim.

## Current authoritative receipt

The current `build/linux/qualification-summary.json` records:

| Status | Cells |
| --- | ---: |
| PASS | 132 |
| FAIL | 0 |
| BLOCKED | 7 |
| XFAIL | 1 |
| NOT_IMPLEMENTED | 22 |
| **Total** | **162** |

Claims from that receipt:

- implemented local suite passed: **true**;
- product-boundary qualification passed: **true**;
- release qualification passed: **false**;
- full Linux qualification passed: **false**.

This decomposition changes planning and reporting only. It does not convert
any matrix cell to PASS and is not cited as a qualification rerun.
