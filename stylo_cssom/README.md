# Stylo CSSOM

Stylo owns CSS syntax admission, source processing and serialisation.
This crate exposes CSSOM rules, declarations, selectors, registrations and
Typed OM values, using Stylo's native property and value grammars.
Vendor syntax translation is part of the same boundary.

Inline declarations retain admitted vendor names, values and priorities.
CSSOM mutations use the authored names. Rendering projects these declarations
through the stylesheet translation rules for the selected compatibility mode.
Other modes ignore them.

```text
CSS text -> stylo_cssom -> typed CSS values -> DOM bindings and rendering
                 |
                 v
         stylo_cssom_model
```

The CSSOM model has no DOM or rendering dependency. Consumers must not
implement token parsers or duplicate property grammars. URL callbacks
receive decoded URL values; they never receive CSS tokens.

Run the native tests with `cargo test -p stylo -p stylo_cssom -p stylo_cssom_model`.
