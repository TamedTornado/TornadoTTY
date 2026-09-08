# GH-184: dock activation instead of duplicate workspace restoration

Issue [#184](https://github.com/TamedTornado/TornadoTTY/issues/184), scoped
under the broader GApplication ownership issue #171.

## Report and reproduced failure

Jason's dock icon on another monitor opened another instance instead of
presenting the existing application. GNOME dash-to-dock monitor and workspace
isolation were both false; changing those global preferences was not justified.
Installed and source desktop IDs matched. Live X11 window properties were not
available during the initial read-only inspection.

The existing private-session product-smoke journey now has an `activation`
scenario. With the previous build, a second ordinary launch remained alive and
created another workspace instead of forwarding activation and exiting. The
test failed at that assertion (bounded eight-second secondary process timeout).
Neither launch used Jason's real session bus, state, display, or configuration.

## Ownership repair

- Ghostty still initializes its native runtime before GTK, as required by the
  embedding contract. Its private runtime application is not repurposed into
  TornadoTTY's desktop controller, avoiding Ghostty's own activation handlers.
- A focused `DesktopActivation` module owns a plain GTK application with the
  public TornadoTTY ID. It becomes the default application and registers before
  persistence/restore or agent IPC starts. Every host window is associated with
  it, including windows added through the existing coordinator.
- Normal launches use GApplication uniqueness and forward standard activation
  to the owner. The secondary exits without constructing panes or saving a
  workspace. The owner presents its currently active host window.
- Activations arriving during initial construction coalesce into one pending
  request consumed when the coordinator is ready; no retry timer or restore
  replay is added. GTK owns activation platform data handling.
- Explicit `--command`, `--state-directory`, or `--no-session-restore` launches
  retain their independent semantics. Normal launch without a usable session
  bus fails before workspace construction instead of silently restoring a
  second uncoordinated workspace.

This fixes desktop ownership and activation, not all terminal-notification
delivery requirements in #171. That parent remains open. No .desktop identity
change, custom IPC forwarding protocol, additional instance registry, or
Ghostty engine change is introduced.

## Focused checks and discoveries

Integrated ReleaseSafe build passed. The final activation checks passed on
private X11 and private Wayland: ordinary second launch forwards/exits; the
primary still has exactly one window and pane; an explicit isolated command
executes and exits independently; an unavailable bus fails promptly without
workspace creation; physical New Window creates the second window in the same
primary; both windows close normally and the primary exits successfully.

The first post-fix actor reached all activation/New Window assertions but failed
in teardown because it referenced a nonexistent wait-helper name and attempted
an unbound Ctrl+Shift+Q. Corrected the actor to use the existing owned-process
wait and the product's actual Close Window shortcut, twice. Assertions for the
reported defect were not weakened. X11 `_GTK_APPLICATION_ID` readback passed
with the public TornadoTTY ID. The previous opener regression also passed on
both X11 and Wayland against this build. The activation scenarios are scheduled
in the existing release matrix; no aggregate qualification was run.

Shell syntax, the sourced journey's ShellCheck, matrix schema/coverage
validation, and `git diff --check` passed. Workspace-wide `cargo fmt --check`
still reports pre-existing formatting differences in unrelated code; the new
module passed formatting review and its declaration was placed in sorted order.
No repository-wide formatting rewrite was performed.

```sh
ZENTTY_BUILD_OUTPUT_DIR=build/gh184 linux/scripts/build-local
ZENTTY_LINUX_BINARY="$PWD/build/gh184/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=activation \
  linux/tests/nested-x11 linux/tests/rust-product-smoke
ZENTTY_LINUX_BINARY="$PWD/build/gh184/bin/zentty-linux" \
  ZENTTY_RUST_SCENARIO=activation ZENTTY_RUST_PHYSICAL_INPUT=true \
  linux/tests/nested-wayland-input linux/tests/rust-product-smoke
```

Jason reported sluggishness during the work. A bounded read-only check found
the live process still mapped to the old, replaced executable, about 842 MiB
RSS, no empty opener warnings in its last 120 records, and about 53 GiB system
memory available. The concurrent compiler used about three cores and 1.7 GiB;
this does not establish the live slowdown's cause. Jason cannot restart soon
and explicitly asked development to continue. No installation, live restart,
or full qualification was performed. Real second-monitor dock QA remains for
his next convenient restart; these results are isolated automated tests.
