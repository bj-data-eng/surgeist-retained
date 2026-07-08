# Retained Selector Runtime Facts Policy

Retained exposes facts only. It does not parse CSS, lower selectors, compute
specificity, run selector matching, or materialize pseudo-elements.

## Supported Fact Families

- Stable retained IDs and key paths.
- Authored metadata facts: kind/tag, key, role, label, classes, attributes, text.
- Canonical and projected traversal facts selected by `SelectorTraversal`.
- Structural sibling facts derived from the selected traversal tree.
- Retained state facts and runtime-supplied hover/active/pressed plus host-backed control state facts.
- Selector invalidation pressure from retained mutations and projection changes.
- `:root` facts via `Snapshot::root()`.
- `:scope` support through caller-provided scope anchors in root/style; retained exposes IDs and traversal facts but does not choose selector scope.

## Unsupported In Retained

- CSS attribute matching operators. Retained exposes attribute values; style/root
  interpret exact, token, dash-prefix, prefix, suffix, substring, and case
  sensitivity semantics.
- `:is`, `:where`, and `:not` matching. Style/root own selector semantics.
- `:has` matching. Root/style may use retained traversal facts, but retained
  does not perform relative selector search.
- Pseudo-elements. Retained does not create anonymous retained product nodes for
  pseudo-elements.
- Dirty projected traversal. Projected selector queries return retained errors
  for unresolved projection slots rather than silently approximating.
- Named projection slot selector traversal. The first retained selector fact
  surface exposes canonical traversal and projected default-slot traversal only.
- Virtual projection ranges outside materialized retained nodes. Retained
  exposes facts for materialized nodes and virtual projection metadata only.
