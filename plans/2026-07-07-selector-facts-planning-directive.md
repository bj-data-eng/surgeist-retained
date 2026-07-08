# Retained Selector Facts Planning Directive

This directive asks `surgeist-retained` to design a crate-local implementation
plan for the retained-tree facts needed by the new CSS/style surface.

Root is moving Surgeist-to-Surgeist lowering into the root facade. Retained
should not own CSS parsing, style cascade, selector matching policy, or lowering
adapters. Retained should expose the stable tree, identity, traversal, and state
facts that root/style/runtime can use without reaching through retained
internals.

## Scope

Write an implementation plan for retained-owned selector and runtime fact
support. The plan should cover only retained responsibilities:

- element identity facts: tag name, stable key, classes, attributes, roles, and
  other authored metadata retained already owns or should own
- tree traversal facts: parent, children, previous sibling, next sibling, first
  and last child, and sibling index facts needed by complex and structural
  selectors
- canonical versus projected traversal policy: which tree is selector matching
  allowed to inspect, and how retained exposes that choice explicitly
- runtime-state fact intake: a typed way to associate state such as hover,
  focus, active, disabled, selected, checked, and similar pseudo-class facts
  with retained identities without making retained depend on window or runtime
- invalidation facts: change reports that let root/style determine which
  selector matches may need recomputation after identity, metadata, tree, or
  runtime-state changes

## CSS/Style Features This Enables

The retained plan should explicitly account for the fact needs of:

- tag, class, key, and compound selectors
- child, descendant, adjacent sibling, and general sibling combinators
- attribute selectors, including exact, list-token, dash-prefix, prefix,
  suffix, substring, and case-sensitivity variants as represented by CSS/style
- structural selectors such as `:first-child`, `:last-child`, `:only-child`,
  `:nth-child`, `:nth-last-child`, `:nth-of-type`, and related type-aware forms
- `:root` and `:scope` anchors
- `:is`, `:where`, and `:not`, to the extent retained facts are needed by the
  root/style matcher
- `:has` and relative selectors as a policy question, including traversal cost
  and invalidation consequences
- runtime pseudo-classes that depend on state supplied by runtime/window/root

## Boundary Rules

Do not add CSS parsing or selector matching to retained unless the plan makes a
separate, reviewed case for why retained must own that behavior. The expected
boundary is:

- CSS owns syntax and authored selector/value contracts.
- Style owns style rules, cascade/resolution data, and selector-facing style
  models.
- Root owns Surgeist-to-Surgeist lowering and integration policy.
- Runtime/window own host event and interaction state sources.
- Retained owns stable identity, retained tree facts, metadata facts, state-fact
  storage or intake contracts, and invalidation reports.

Do not materialize pseudo-elements as anonymous retained product nodes in this
plan. Pseudo-element materialization requires a separate root product decision.

## Planning Requirements

The implementation plan should:

- identify the public retained APIs root/style/runtime would call
- distinguish authored metadata from derived traversal facts
- distinguish retained-owned facts from externally supplied runtime facts
- specify how facts are updated when nodes are inserted, removed, reparented, or
  metadata changes
- specify invalidation reports with enough detail for root/style to avoid
  whole-tree recomputation when possible
- include tests for fact updates, traversal, structural selector inputs,
  attribute/class metadata, runtime-state intake, and invalidation reports
- call out any unsupported selector facts explicitly rather than silently
  approximating browser behavior

## Review Gate

Before implementation, have a clean-context reviewer check the retained plan
against:

- this directive
- retained's crate boundary
- `/Users/codex/Development/surgeist/guidance/surgeist-rust-modeling-guide.md`
- the root inventory at
  `/Users/codex/Development/surgeist/plans/2026-07-04-css-integration-support-inventory.md`

Completion for this directive is a reviewed implementation plan in retained's
`plans/` folder, not code changes.
