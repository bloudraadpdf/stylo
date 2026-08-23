# Background clip text value

CSS Backgrounds 4 defines each `background-clip` layer as one visual box or
the order-independent `border-area || text` pair. The Servo value currently
omits `text` and cannot represent the pair.

## Work plan

- [x] Confirm the local CSS grammar and the current generated keyword value.
- [x] Record the strict duplication baseline for the affected value and
  property generator files.
- [x] Add parser tests for every valid value, both pair orders, duplicate
  rejection, and trailing-token rejection.
- [x] Replace the Servo keyword value with a closed Rust enum that preserves
  `text` and the `border-area text` union.
- [x] Keep the existing Gecko keyword metadata and image-layer mapping.
- [x] Run the focused tests, formatting, Clippy, and strict duplication scan.
- [ ] Commit the Stylo change atomically and update Moegoe to that revision.

Strict Clippy reaches 4,088 existing repository errors with Rust 1.96. The
affected crate compiles and all 213 tests pass. The post-change duplication
scan adds no clone to the affected files.
