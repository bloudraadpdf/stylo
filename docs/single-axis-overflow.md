# Single-axis overflow computation

The local Moegoe specification `docs/specs/css-overflow-3.md`, section 3.1,
preserves `clip` when the opposite axis scrolls. Only `visible` computes
to `auto` beside a scrollable value.

- [x] Establish the computed-style reproducer before production changes:
      18 of 150 values fail across block, flex and grid displays.
- [x] Record strict duplication: 31 clones / 457 lines / 2,463 tokens in
      the overflow value and style adjuster files.
- [x] Express the opposite-axis computation on the typed overflow value.
- [x] Validate all 25 value pairs and all 335 library tests; lint and format.
- [x] Check duplication, commit and push the Stylo change.
- [ ] Update Moegoe's pins and validate the computed-style cascade and layout.

The reproducer and its evidence are in Moegoe's
`crates/moegoe-css/tests/single_axis_scroll_computed_values.rs` and
`target/recovery/single-axis-scroll-computed-before.log`. It uses an author
stylesheet passed to Stylo. An earlier fixture omitted the inline style
identity and produced default values; that result is not defect evidence.

Strict Clippy retains 4,466 library and 4,474 test diagnostics, matching the
retained baseline. None points to the changed code. Formatting and whitespace
checks pass. Strict duplication remains 31 clones / 457 lines / 2,463 tokens.
