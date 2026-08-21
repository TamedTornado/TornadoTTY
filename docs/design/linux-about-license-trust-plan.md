# Linux About and packaged-license trust plan

Issue: GH-74, child of GH-23.

## Outcome

Deliver the source About outcome on Linux with truthful build identity and a
searchable license surface whose entries and full texts come from the exact
notice graph already produced for the staged and Debian products.

## Test-first order

1. Extend the single package-notice producer with strict UI catalog metadata
   and focused negatives for missing notice text, duplicate identity, unsafe
   paths/URLs, and metadata drift. Do not add a second dependency scanner.
2. Add focused Rust model tests for compiled build identity, staged/installed
   notice-root discovery, strict catalog parsing, completeness, bounded files,
   stable sorting/search, and reviewed HTTPS links.
3. Add a dedicated GTK About/licenses module and one named workspace action.
   Keep it out of the application coordinator and existing catch-all views.
4. Drive the staged product through the real command palette, inspect exact
   build fields and real generated dependency entries/text, search/select with
   physical input, capture Docs/source link intents through a controlled
   desktop handler, and prove presentation itself performs no network action.
5. Extend installed-package qualification to assert the same catalog identity
   against the extracted/installed artifact. Run affected focused suites and
   exact real GTK journeys before changing inventory status.

## Boundaries

- `linux/scripts/collect-package-notices` remains the one notice/dependency
  producer used by both staged and packaged products.
- The application reads only validated local resources. It never downloads
  license text or metadata.
- External links are reviewed HTTPS values and open only after a user action.
- Missing or malformed resources render a visible recoverable diagnostic; they
  never become an empty successful catalog.
- GH-74 does not implement crash reporting, updates, or performance diagnostics.
