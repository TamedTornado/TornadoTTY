# Codex updater exits a restored pane to the shell

## Dogfood finding

When a restored Codex pane encounters Codex's own update prompt, accepting the
update installs the new Codex version and exits that process. The pane then
returns to its shell instead of automatically resuming the prior session.

This is not workspace-topology loss: the worklane and pane remain present. It is
also distinct from a failed initial restore, so Zentty must not show the lost-pane
recovery flow after a successfully launched session later exits.

## Decision

Zentty will not special-case this lifecycle in the current port. Reliably
distinguishing an updater exit from an intentional Codex exit would require
tracking tool versions or fingerprints, inferring updater outcomes, and
relaunching an agent command after its child process has ended. That is invasive,
tool-specific orchestration and risks restarting sessions that a user intended
to close.

The maintainable behavior is therefore:

- preserve the pane and worklane;
- return visibly to the shell when the successfully started Codex process exits;
- allow the user to run the normal resume command again; and
- prefer an upstream Codex behavior that resumes after a successful self-update
  if that workflow is changed in the future.

No timer, version detector, exit-code exception, or automatic retry was added.
This remains a documented product limitation rather than a falsely generalized
agent lifecycle feature.
