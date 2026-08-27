# Ghostty-compatible Linux shortcut preset v1

GH-119 derives this preset from Ghostty `Config.Keybinds.init` at revision
`80054768edbffd5df8568782e528363033a49192`, the public patched revision pinned
by `linux/ghostty.lock` and embedded by the tested ReleaseSafe product. The
macOS `scripts/ghosttykit.lock` is not Linux's source authority.

Zentty storage uses `command` for Linux Ctrl, `control` for Linux Super, and
`option` for Linux Alt. Character keyvals are logical so bindings follow the
active layout; arrows, Tab, and function keys are physical.

| Ghostty Linux action | Ghostty chord | Zentty command | Stored chord |
| --- | --- | --- | --- |
| new window | Ctrl+Shift+N | `app.new_window` | `command+shift+n` |
| close window | Alt+F4 | `app.close_window` | `option+f4` |
| new tab | Ctrl+Shift+T | `worklane.new` | `command+shift+t` |
| next/previous tab | Ctrl+[Shift]+Tab | `worklane.next` / `worklane.previous` | `command+[shift]+tab` |
| close surface | Ctrl+Shift+W | `pane.close_focused` | `command+shift+w` |
| split right/down | Ctrl+Shift+O/E | `pane.split.horizontal` / `pane.split.vertical` | `command+shift+o/e` |
| previous/next split | Ctrl+Super+[/] | `pane.focus.previous` / `pane.focus.next` | `command+control+[/]` |
| directional split focus | Ctrl+Alt+arrows | `pane.focus.*` | `command+option+arrow` |
| directional split resize | Ctrl+Super+Shift+arrows | `pane.resize.*` | `command+control+shift+arrow` |
| search | Ctrl+Shift+F | `pane.search.find` | `command+shift+f` |
| command palette | Ctrl+Shift+P | `command_palette.show` | `command+shift+p` |
| open/reload config | Ctrl+, / Ctrl+Shift+, | `app.open_settings` / `app.reload_config` | `command+,` / `command+shift+,` |

Linux fullscreen deliberately remains F11. Ghostty's Ctrl+Enter fullscreen is
not installed. Non-conflicting Zentty bindings retained by the preset are
sidebar toggle, Clean Copy, latest notification, height arrangements, pane
movement/reset, and bookmarks. Copy Path and Undo Close Pane are omitted
because they would steal Ghostty's Ctrl+Shift+C copy and Ctrl+Shift+T new-tab
expectations. Applying the preset atomically replaces all current overrides;
unlisted default commands are explicitly unbound by the existing preset path.
