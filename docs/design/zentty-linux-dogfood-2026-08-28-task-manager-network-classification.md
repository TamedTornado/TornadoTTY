# Zentty Linux dogfood — Task Manager network classification

Date: 2026-08-28
Tracking: GH-65

## Discovery

The qualification matrix classified `task-manager-network-accounting` as a
release-tier NOT_IMPLEMENTED cell. That contradicted GH-65 and the ratified
product decision that network accounting is a deferred enhancement rather than
an initial Linux-port requirement.

Source review also corrected the premise that network throughput was missing
macOS parity. macOS renders a Network column, but
`TaskManagerProcessSampler.sample(rootPIDs:now:)` constructs every
`TaskManagerProcessTree` with `networkBytesPerSecond: nil`. The source shortcut
description promises CPU and memory usage, not network accounting. Linux
therefore improves the unavailable presentation by hiding the column until a
real backend exists.

## Decision and repair

- Keep the cell explicit as NOT_IMPLEMENTED under GH-65.
- Move it from `release` to `full`, matching the already-deferred container and
  cgroup isolation cell.
- Do not invent a privileged packet-accounting subsystem merely to turn the
  matrix green.
- Do not substitute interface totals, instantaneous socket ownership, TCP-only
  diagnostics, or fabricated zero values for per-pane process-tree accounting.

## Qualification meaning

This correction does not convert the cell into a PASS. It removes the false
claim that an optional Linux enhancement blocks release qualification while
retaining it as visible future/full-scope work.
