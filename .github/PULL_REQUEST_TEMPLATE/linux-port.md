## Linux port review

- Issue and acceptance criteria:
- Qualification cells:
- Dogfood entry:
- Exact commands run:
- Remaining non-PASS cells:

### Product and test boundary

- [ ] Product code is Rust and new unsafe code is isolated in the named Ghostty adapter.
- [ ] Product integration tests launch the delivered `zentty-linux` artifact.
- [ ] Ghostty, GTK, PTYs, display services, and external drivers are real where applicable.
- [ ] Deterministic children/fixtures are test data, not alternate application components.
- [ ] Missing infrastructure remains BLOCKED or NOT_IMPLEMENTED; exit 77 did not pass.
- [ ] The matrix contains no aggregate runner, support self-test, or mutation-suite cell.

### Test-first evidence

- [ ] Focused and real-boundary tests failed for the intended semantic reason before implementation.
- [ ] Positive, failure, teardown, and ownership/lifecycle cases now pass.
- [ ] Valgrind runs retain raw and suppression-enabled receipts and use only reviewed suppressions.
- [ ] ReleaseSafe Valgrind remains XFAIL until its tracked defect is repaired.

### Review

- [ ] The diff is focused and `git diff --check` passes.
- [ ] Every presently relevant focused test was run once; aggregate suites were not nested.
- [ ] Discoveries, failures, repairs, and limitations are recorded in dogfood.
- [ ] Claims describe exactly what ran and do not call C-host evidence a Zentty product port.
