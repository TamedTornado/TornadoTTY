# Ready notification coalescing

## Dogfood failure

After restarting Zentty and selecting a restored Codex pane, Jason received two
desktop notifications for one completed response. Both notifications activated
the same pane.

The journal established two independent source observations for the same pane:

```text
desktop-attention id=3 result=sent pane=pane-14
terminal-notification pane=pane-14
desktop-attention id=4 result=sent pane=pane-14
```

The authenticated Codex lifecycle first changed the pane to ready. Two seconds
later Ghostty decoded the terminal notification containing the completed
response text. The canonical agent store correctly accepted the richer text,
but `AttentionInbox` included text in the identity of every attention state and
therefore treated the enrichment as a second completion.

## Repair

Ready attention identity now consists of the target, agent, state, and
interaction—not its progressively enriched text. If text or location changes
while the same ready state remains current, the existing inbox item is updated
without queuing another desktop delivery.

This is state-based reconciliation, not a timing window. A transition out of
ready removes the signature and resolves the prior item, so a later
`Running -> Ready` transition still creates and delivers a new completion.
Needs-input identity continues to include its text because a new question or
approval request can be substantively different even when its phase is
unchanged.

No timer, notification cache, GTK workaround, or second notification authority
was added.

## Focused evidence

The regression was written first and initially failed with two inbox items
instead of one. It now proves both sides of the contract:

```text
Ready -> enriched Ready: one item, zero additional deliveries
Ready -> Running -> Ready: second item, one additional delivery
```

Receipts:

```text
cargo test --offline -p zentty-core --test attention_inbox: 13 PASS
cargo test --offline -p zentty-core attention: PASS
rustfmt --check: PASS
git diff --check: PASS
```

Human confirmation remains required after the fixed binary is installed and a
subsequent real Codex completion occurs while its pane is not actively viewed.
