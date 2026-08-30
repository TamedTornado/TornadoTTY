# Visible-pane notification policy

## Dogfood finding

Jason reported that Zentty produced a desktop sound and banner when a Codex pane
needed attention even though that pane was already visible at the top of another
monitor. Zentty correctly suppressed notifications only for the actively focused
pane. That policy was too noisy for a multi-monitor workflow, but changing it
unconditionally would also remove useful notifications for users who expect
focus—not visibility—to control delivery.

## Product decision

Notifications settings now include **Notify when pane is visible**. It defaults
to enabled, preserving existing behavior. When disabled:

- the actively viewed pane remains suppressed by the existing rule;
- a pane displayed in the viewport of a mapped, non-minimized Zentty window also
  suppresses its desktop banner and sound;
- the attention item is retained in Zentty's inbox and sidebar state;
- a pane in another worklane, outside the horizontal pane viewport, in an
  unmapped window, or in a minimized window still produces desktop delivery.

The decision reads the live setting from the target window and uses actual GTK
window, surface, pane-frame, and viewport state. It does not use a timer, infer
visibility from focus, or maintain a second notification authority.

GTK can establish that a window and pane are mapped and presented in Zentty's
viewport. It cannot portably establish whether an unrelated compositor surface
partially or completely occludes that window. “Visible” therefore means
displayed by Zentty in a mapped, non-minimized window—not proven unobscured at
every physical pixel.

## Test-first discoveries and repair

The configuration parser test initially failed because the field did not exist.
The pure delivery-policy test initially failed because the decision type did not
exist. The settings actor initially reached the real switch by its mnemonic but
did not change it; the actor was corrected to send Space after focus rather than
weakening the product assertion.

The real attention actor then proved the opt-out path with two real Zentty
windows, real PTYs, authenticated agent IPC, a private D-Bus notification
service, and a displayed but unfocused target pane. No desktop notification was
emitted, while the inbox retained the unresolved item. The unchanged-default
actor separately proved that the same real path still delivers when the setting
is absent or enabled.

One nested-X11 invocation failed before product startup because the sandbox's
user-namespace projection gave Xvfb an unusable socket owner. A later invocation
also omitted the actor's required private D-Bus session. Neither environmental
failure was counted as product evidence; the final receipts used the controlled
X11 compositor and private D-Bus session outside that namespace.

## Focused receipts

```text
cargo test --offline -p zentty-core --test app_config: 26 PASS
cargo test --offline -p zentty-core --test attention_inbox: 13 PASS
cargo test --offline -p zentty-linux notification_service::tests: 4 PASS
cargo test --offline -p zentty-linux notification_update_preserves_symlink_comments_unknowns_and_mode: 1 PASS
rust-notifications-settings-x11: PASS (physical input, persistence, restart)
rust-attention-inbox-x11 with visible-pane opt-out: PASS (desktop suppressed, inbox retained)
rust-attention-inbox-x11 with default policy: PASS (real private-D-Bus delivery)
touched-file rustfmt --check: PASS
shell syntax checks: PASS
git diff --check: PASS
```

Repository-wide `cargo fmt --check` still reports formatting differences in the
untouched `agent_fleet.rs` and `sidebar.rs`. They were not silently reformatted
into this feature commit.

## Human validation remaining

Automated product evidence is complete. Jason still needs to disable the option
in the installed build and confirm the preferred behavior on the real GNOME
multi-monitor desktop before GH-140 is closed.
