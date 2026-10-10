# Custom functions (`@function`)

Specification: Moegoe `docs/specs/css-mixins-1.md`, pinned to csswg-drafts
`e13fe879106a`. Where the specification and WPT `css/css-mixins/` disagree,
WPT adjudicates.

## Model

```
 @function --f(--a <length>: 1px) returns <length> { --l: ..; result: ..; @media .. }
        │ style::stylesheets::rule_parser (CssRuleType::Function)
        ▼
 CssRule::Function(FunctionRule)            CSSOM: CSSFunctionRule
   ├─ CssRule::FunctionDeclarations          CSSFunctionDeclarations.style
   │     FunctionDescriptors [--l, result]     = CSSFunctionDescriptors
   └─ CssRule::Media/Supports/Container { FunctionDeclarations .. }
        │ CascadeData::add_rule_list: @media and @supports fold statically,
        │ @container stays a per-element condition
        ▼
 CustomFunction  ──▶ CascadeData.custom_functions: LayerOrderedMap
        │
        │ cascade: <dashed-function> reference in a VariableValue
        ▼
 FunctionScope = [innermost tree .. document] from (element, CascadeLevel)
        ▼
 evaluation frames  ──▶  token sequence substituted like var()
```

## Parsing

- The prelude is `<function-token> <function-parameter>#? ) [returns <css-type>]?`.
  A `<css-type>` is one syntax component or `type(<syntax>)`; the return type
  defaults to the universal syntax. A typed default must parse against its
  type unless it holds an arbitrary substitution function.
- The body accepts `--*` locals and `result`, without `!important`, plus
  `@media`, `@supports` and `@container`. Runs of declarations become
  `FunctionDeclarations` rules, as the CSSOM requires.
- `<dashed-function>` is a fourth `SubstitutionFunction` beside `var()`,
  `env()` and `attr()`. Arguments are `<declaration-value>#?`; a
  `{}`-wrapped argument holds commas, an empty argument and a leading
  `<dashed-ident> :` are invalid. A property value that contains one is
  assumed valid at parse time.

## Evaluation

- Arguments substitute in the caller's scope before the call.
- A frame binds parameters, then locals. A `var()` lookup walks
  frame → caller frame → element custom properties.
- Parameters resolve in order. A default sees earlier parameters only.
- Locals resolve lazily, but all of them resolve before the result, so a
  cycle through an unused local still invalidates the call.
- Locals named like a typed parameter compute against that type. After
  substitution, `initial` gives the parameter value and `inherit` gives the
  caller's value; other CSS-wide keywords give the guaranteed-invalid value.
  `result` keeps CSS-wide keywords unless the return type is typed.
- Cycles: an explicit stack of substitution contexts, bindings and function
  identities. A repeated context marks every context above it cyclic, and a
  cyclic context yields the guaranteed-invalid value.
- Element-level ordering stays in the Tarjan pass of `substitute_all`. A
  call adds edges to the free variables of the reachable functions: names
  that the parameters and unconditional locals do not bind. If a reachable
  function has a typed parameter or return type, the call takes the
  non-custom references of its body, so typed font-relative work is
  deferred.

## Scoping

- Function lookup is tree-scoped. The declaration's `CascadeLevel` selects
  the tree: same tree, outer trees for `::part()`, and the own or slot
  shadow root for `:host` and `::slotted()`. The chain then walks out to the
  document. User agent and user levels use their own origin data.
- A name inside a function body resolves from the function's definition
  tree outwards.
- Within one tree the stronger layer wins, then the later rule.
- `@container` inside a body evaluates against the calling element; for a
  pseudo-element, against its originating element.

## Spec and WPT conflicts

- C1 `dashed-function-eval`, "Missing only argument" and "Missing one
  argument of several": a parameter without a default and without an
  argument invalidates the call. The specification leaves the parameter
  guaranteed-invalid and evaluates the function.
- C2 `function-parameter-scoping`, "Default referencing a later parameter is
  guaranteed-invalid": defaults see earlier parameters only. The
  specification resolves all arguments as one rule.

## Out of scope (separate features)

`if()`, `inherit()`, `attr()` re-substitution and tainting, and
`revert-layer` or `revert-rule` produced by substitution. The tests
`local-if-substitution`, `local-inherit-substitution`,
`local-attr-substitution` and `function-parameter-types.tentative` depend on
them.

## Tasks

- [x] Parse `@function`, `FunctionDeclarations`, `<dashed-function>`; CSSOM model.
- [ ] Moegoe CSSOM bindings: `CSSFunctionRule`, `CSSFunctionDeclarations`,
      `CSSFunctionDescriptors`.
- [ ] Compile functions into `CascadeData`; layer order; media folding.
- [ ] Evaluate calls: arguments, defaults, locals, result, types, keywords.
- [ ] Cycles: substitution context stack; Tarjan edges for free variables.
- [ ] Tree scopes; `@container` in bodies.
