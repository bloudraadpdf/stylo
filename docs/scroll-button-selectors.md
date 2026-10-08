# Scroll button selectors

CSS Overflow 5 section 3 defines `::scroll-button()` with a direction or `*`.
The argument accepts physical and logical directions, plus `prev` and `next`.
The parser stores a `ScrollButtonDirection`, or `None` for `*`.
Serialisation uses lower-case keywords and retains the selector argument.

The selector parser permits `:enabled` and `:disabled` only after a pseudo-element
that declares support for those states. This does not classify them as user-action
states or permit them after `::before` or `::scroll-marker`.

The regression failed before the change: `::scroll-button( UP )` had no parsed
selector. It checks each direction with focus and enabled states, round trips,
and rejected arguments. Geometry, logical-direction resolution, button generation
and activation belong to the consumer. This change supplies typed syntax only.

Specification used: Moegoe's repository-local `docs/specs/css-overflow-5.md`.
