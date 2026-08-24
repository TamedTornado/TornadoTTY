# Rust dependency publication-age policy

Zentty quarantines crates.io releases for seven days before dependency
resolution may select them. `rust-toolchain.toml` pins the Cargo nightly that
implements native `min-publish-age`; `.cargo/config.toml` enables that resolver
feature, denies incompatible publication ages, and sets the global floor to
seven days. Ordinary `cargo update`, build, test, and metadata commands inherit
the policy automatically.

This nightly pin is temporary. GH-96 requires moving to the first reviewed
stable Cargo release that enforces the same configuration, with an ordinary
`cargo update` rejection probe before the nightly-specific pin is removed. The
seven-day floor must remain continuously enforced during that migration.
The selected nightly is dated 2026-08-17 because the 2026-08-24 nightly has a
reproducible Clippy compiler ICE on the GTK/GIO async code; it is not merely an
arbitrary old pin.

The independent audit remains defense in depth because Cargo permits a young
version that is already present in `Cargo.lock`. It is not a substitute command
for Cargo.

`linux/scripts/audit-cargo-publish-age` checks every crates.io package present
in the complete lockfile—including currently inactive optional/target packages—
against the publication time recorded in Cargo's local
crates.io sparse-index cache. Missing or malformed publication metadata fails;
environmental absence is not a pass. The machine receipt is written to
`build/linux/security/cargo-publish-age.json`.

The explicit exception manifest is `cargo-publish-age-exceptions.json`. An exception
must name one exact crate and version, identify Jason as the authorizer, link a
tracking issue and authorization record, provide a substantive justification,
and expire. Unused, aged-out, malformed, duplicate, or expired exceptions fail
the audit. The committed policy starts with no exceptions.

Public CI runs this as an advisory check. CI is not a release authority and the
receipt does not claim release or full Linux qualification.
