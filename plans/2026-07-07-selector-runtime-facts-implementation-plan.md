# Retained Selector Runtime Facts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose retained-owned selector and runtime facts so root/style/runtime can query stable identity, metadata, traversal, state, and invalidation facts without reaching through retained internals.

**Architecture:** Add a selector-facts facade around existing retained snapshots instead of moving CSS parsing or selector matching into retained. Keep authored element metadata, derived traversal facts, externally supplied runtime facts, and invalidation facts as separate typed surfaces. Make canonical versus projected traversal an explicit caller choice so root/style can select policy without relying on method naming conventions.

**Tech Stack:** Rust 2024 crate, standard library only, existing retained model/snapshot/state/change modules, crate tests in `src/tests.rs`. Public API artifact generation is root-owned for this crate; do not recreate crate-local `api/` tooling.

---

## Current Support Inventory

Retained already owns these raw facts:

- `Element` stores key, kind/tag, role, label, classes, attributes, text, hooks, and children in `src/element.rs`.
- `NodeRef` exposes key, kind, role, attributes, classes, text, state, key path, canonical parent, and projected parent in `src/snapshot.rs`.
- `Snapshot` exposes root, revision, key lookup, canonical children, projected children, canonical ancestors, projected ancestors, canonical descendants, class scans, role scans, hooks, and dirty projection slots.
- `State` stores presence, disabled, hovered, active, focused, focus-within, pointer-captured, selected, pressed, checked, and expanded.
- `Model` already updates focus and pointer capture state, and app-authored `StatePatch` updates presence, disabled, selected, checked, and expanded.
- `ChangeFlags` and `ChangeSet` already report inserted, removed, moved, changed IDs, projection slots, and broad flags for structure, kind, role, classes, attributes, presence, state, focus, pointer capture, and projection.

Retained does not yet expose selector-ready facts for sibling position, type-aware structural selectors, explicit traversal policy, runtime-owned pseudo-class state intake for hover/active/pressed and host-backed control state, or selector-specific invalidation pressure.

## Boundary Decisions

This plan intentionally keeps the following out of `surgeist-retained`:

- CSS parsing, selector syntax, specificity, cascade, and matching policy.
- Root lowering from CSS/style into retained queries.
- Runtime/window host event interpretation.
- Pseudo-element materialization.
- Named-slot selector traversal policy. The first implementation exposes canonical traversal and projected default-slot traversal only; named-slot selector facts remain explicitly unsupported until root defines how named slots participate in product selector matching.

Retained will only expose facts and reports. Root/style may use those facts to implement selectors, and runtime/window/root may supply dynamic state through retained-owned intake commands.

## File Structure

- Modify `src/lib.rs`: reexport new selector and runtime fact APIs from the crate front door.
- Modify `src/snapshot.rs`: expose selector fact queries from `Snapshot` and `NodeRef`.
- Modify `src/model.rs`: add internal helpers for canonical/projected traversal, sibling position, type position, runtime state intake, and invalidation reports.
- Modify `src/state.rs`: add typed runtime state patch/intake types distinct from app-authored `StatePatch`.
- Modify `src/change.rs`: add selector invalidation types and attach them to `ChangeSet`.
- Modify `src/mutation.rs`: add runtime-state patch command if the intake belongs in transactional mutation batches.
- Modify `src/tests.rs`: add focused tests for selector facts, traversal policy, structural inputs, runtime state intake, and invalidation reports.

Do not create or update `api/public-api.txt` in this crate unless root restores crate-local API artifact ownership.

---

### Task 1: Add Explicit Selector Traversal Policy

**Files:**
- Modify: `src/snapshot.rs`
- Modify: `src/model.rs`
- Modify: `src/lib.rs`
- Test: `src/tests.rs`

- [ ] **Step 1: Write failing traversal-policy tests**

Add these tests near existing projection traversal tests in `src/tests.rs`:

```rust
#[test]
fn selector_traversal_policy_names_canonical_and_projected_trees() {
    let mut model = Model::new(
        Element::root().with_child(element("host", "host").with_child(element("button", "canonical"))),
    )
    .unwrap();
    let host = model.snapshot().children(model.root()).unwrap().next().unwrap();
    let canonical = model.snapshot().children(host).unwrap().next().unwrap();
    let slot = ProjectionSlot::default(host);

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "projected")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let projected = model
        .snapshot()
        .projected_children(slot)
        .unwrap()
        .next()
        .unwrap();

    let snapshot = model.snapshot();
    assert_eq!(
        snapshot
            .selector_children(host, SelectorTraversal::Canonical)
            .unwrap()
            .collect::<Vec<_>>(),
        vec![canonical]
    );
    assert_eq!(
        snapshot
            .selector_children(host, SelectorTraversal::ProjectedDefaultSlot)
            .unwrap()
            .collect::<Vec<_>>(),
        vec![projected]
    );
    assert_eq!(
        snapshot
            .selector_parent(projected, SelectorTraversal::Canonical)
            .unwrap(),
        None
    );
    assert_eq!(
        snapshot
            .selector_parent(projected, SelectorTraversal::ProjectedDefaultSlot)
            .unwrap(),
        Some(host)
    );
    assert_eq!(
        snapshot
            .selector_parent(canonical, SelectorTraversal::ProjectedDefaultSlot)
            .unwrap(),
        None
    );
}
```

Run:

```sh
cargo test selector_traversal_policy_names_canonical_and_projected_trees
```

Expected: FAIL because `SelectorTraversal`, `selector_children`, and `selector_parent` do not exist.

- [ ] **Step 2: Add public traversal policy type**

In `src/snapshot.rs`, add above `pub struct Snapshot`:

```rust
/// Tree view used when exposing retained facts to selector matchers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectorTraversal {
    /// Inspect only canonical retained children and parents.
    Canonical,
    /// Inspect the projected/effective tree through each node's default projection slot.
    ///
    /// Named projection slots are not part of selector traversal in this plan.
    ProjectedDefaultSlot,
}
```

- [ ] **Step 3: Export traversal policy**

In `src/lib.rs`, change:

```rust
pub use snapshot::{NodeRef, Snapshot};
```

to:

```rust
pub use snapshot::{NodeRef, SelectorTraversal, Snapshot};
```

- [ ] **Step 4: Add model helpers for selector children and parents**

In `src/model.rs`, add these `pub(crate)` helpers near existing traversal helpers:

```rust
pub(crate) fn selector_children(
    &self,
    id: Id,
    traversal: super::SelectorTraversal,
) -> Result<Vec<Id>> {
    match traversal {
        super::SelectorTraversal::Canonical => self.canonical_children(id),
        super::SelectorTraversal::ProjectedDefaultSlot => {
            self.projected_children(ProjectionSlot::default(id))
        }
    }
}

pub(crate) fn selector_parent(
    &self,
    id: Id,
    traversal: super::SelectorTraversal,
) -> Result<Option<Id>> {
    self.ensure_live(id)?;
    match traversal {
        super::SelectorTraversal::Canonical => Ok(self.node(id)?.parent),
        super::SelectorTraversal::ProjectedDefaultSlot => {
            self.ensure_projected_edge_resolved(id)?;
            let Some(parent) = self.effective_projected_parent(id)? else {
                return Ok(None);
            };
            let siblings = self.selector_children(parent, traversal)?;
            if siblings.contains(&id) {
                Ok(Some(parent))
            } else {
                Ok(None)
            }
        }
    }
}
```

This membership check is required because canonical children are shadowed when a
clean default projection cache exists. A node has a projected-default parent
only when it appears in that parent's projected-default child list.

- [ ] **Step 5: Add snapshot traversal methods**

In `impl Snapshot<'_>` in `src/snapshot.rs`, add:

```rust
pub fn selector_children(
    &self,
    id: Id,
    traversal: SelectorTraversal,
) -> Result<impl Iterator<Item = Id> + '_> {
    Ok(self.model.selector_children(id, traversal)?.into_iter())
}

pub fn selector_parent(&self, id: Id, traversal: SelectorTraversal) -> Result<Option<Id>> {
    self.model.selector_parent(id, traversal)
}
```

- [ ] **Step 6: Run focused and full checks**

Run:

```sh
cargo test selector_traversal_policy_names_canonical_and_projected_trees
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```sh
git add src/snapshot.rs src/model.rs src/lib.rs src/tests.rs
git commit -m "Expose selector traversal policy"
```

---

### Task 2: Add Selector Sibling And Structural Facts

**Files:**
- Modify: `src/snapshot.rs`
- Modify: `src/model.rs`
- Test: `src/tests.rs`

- [ ] **Step 1: Write failing structural fact tests**

Add this test near traversal tests in `src/tests.rs`:

```rust
#[test]
fn selector_sibling_and_structural_facts_are_derived_from_policy_tree() {
    let model = Model::new(
        Element::root()
            .with_child(element("row", "a"))
            .with_child(element("item", "b"))
            .with_child(element("row", "c")),
    )
    .unwrap();
    let snapshot = model.snapshot();
    let children = snapshot.children(model.root()).unwrap().collect::<Vec<_>>();
    let first = children[0];
    let middle = children[1];
    let last = children[2];

    assert_eq!(
        snapshot
            .selector_sibling_facts(middle, SelectorTraversal::Canonical)
            .unwrap(),
        SelectorSiblingFacts::new(
            Some(model.root()),
            Some(first),
            Some(last),
            false,
            false,
            false,
            Some(SelectorIndex::new(1)),
            Some(SelectorCount::new(3).unwrap()),
            Some(SelectorIndex::new(0)),
            Some(SelectorCount::new(1).unwrap()),
        )
    );

    assert_eq!(
        snapshot
            .selector_sibling_facts(first, SelectorTraversal::Canonical)
            .unwrap()
            .type_count(),
        Some(SelectorCount::new(2).unwrap())
    );
    assert_eq!(
        snapshot
            .selector_sibling_facts(last, SelectorTraversal::Canonical)
            .unwrap()
            .type_index(),
        Some(SelectorIndex::new(1))
    );
}

#[test]
fn selector_count_rejects_zero() {
    assert_eq!(SelectorCount::new(0), None);
    assert_eq!(SelectorCount::new(1).unwrap().get(), 1);
}
```

Run:

```sh
cargo test selector_sibling_and_structural_facts_are_derived_from_policy_tree
cargo test selector_count_rejects_zero
```

Expected: FAIL because the structural fact types and method do not exist.

- [ ] **Step 2: Add structural fact types**

In `src/snapshot.rs`, add near `SelectorTraversal`:

```rust
/// Zero-based selector sibling position.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectorIndex(usize);

impl SelectorIndex {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Total siblings participating in a selector sibling set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectorCount(std::num::NonZeroUsize);

impl SelectorCount {
    #[must_use]
    pub const fn new(value: usize) -> Option<Self> {
        match std::num::NonZeroUsize::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

/// Derived sibling and type-position facts for structural selectors.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectorSiblingFacts {
    parent: Option<Id>,
    previous_sibling: Option<Id>,
    next_sibling: Option<Id>,
    first_child: bool,
    last_child: bool,
    only_child: bool,
    sibling_index: Option<SelectorIndex>,
    sibling_count: Option<SelectorCount>,
    type_index: Option<SelectorIndex>,
    type_count: Option<SelectorCount>,
}

impl SelectorSiblingFacts {
    pub(crate) const fn new(
        parent: Option<Id>,
        previous_sibling: Option<Id>,
        next_sibling: Option<Id>,
        first_child: bool,
        last_child: bool,
        only_child: bool,
        sibling_index: Option<SelectorIndex>,
        sibling_count: Option<SelectorCount>,
        type_index: Option<SelectorIndex>,
        type_count: Option<SelectorCount>,
    ) -> Self {
        Self {
            parent,
            previous_sibling,
            next_sibling,
            first_child,
            last_child,
            only_child,
            sibling_index,
            sibling_count,
            type_index,
            type_count,
        }
    }

    #[must_use]
    pub const fn parent(&self) -> Option<Id> {
        self.parent
    }

    #[must_use]
    pub const fn previous_sibling(&self) -> Option<Id> {
        self.previous_sibling
    }

    #[must_use]
    pub const fn next_sibling(&self) -> Option<Id> {
        self.next_sibling
    }

    #[must_use]
    pub const fn is_first_child(&self) -> bool {
        self.first_child
    }

    #[must_use]
    pub const fn is_last_child(&self) -> bool {
        self.last_child
    }

    #[must_use]
    pub const fn is_only_child(&self) -> bool {
        self.only_child
    }

    #[must_use]
    pub const fn sibling_index(&self) -> Option<SelectorIndex> {
        self.sibling_index
    }

    #[must_use]
    pub const fn sibling_count(&self) -> Option<SelectorCount> {
        self.sibling_count
    }

    #[must_use]
    pub const fn type_index(&self) -> Option<SelectorIndex> {
        self.type_index
    }

    #[must_use]
    pub const fn type_count(&self) -> Option<SelectorCount> {
        self.type_count
    }
}
```

Use private fields plus accessors because these values have cross-field invariants. The crate-private constructor is for retained internals and tests; downstream callers consume facts through methods.

- [ ] **Step 3: Export structural fact types**

In `src/lib.rs`, update the snapshot export:

```rust
pub use snapshot::{
    NodeRef, SelectorCount, SelectorIndex, SelectorSiblingFacts, SelectorTraversal, Snapshot,
};
```

- [ ] **Step 4: Add model helper**

In `src/model.rs`, add:

```rust
pub(crate) fn selector_sibling_facts(
    &self,
    id: Id,
    traversal: super::SelectorTraversal,
) -> Result<super::SelectorSiblingFacts> {
    let Some(parent) = self.selector_parent(id, traversal)? else {
        return Ok(super::SelectorSiblingFacts::new(
            None, None, None, false, false, false, None, None, None, None,
        ));
    };
    let siblings = self.selector_children(parent, traversal)?;
    let index = siblings
        .iter()
        .position(|candidate| *candidate == id)
        .ok_or_else(|| Error::new(ErrorCode::InvalidParent, "selector parent missing child"))?;
    let kind = self.node(id)?.element.kind().clone();
    let same_type_before = siblings[..index]
        .iter()
        .filter(|candidate| self.node(**candidate).map(|node| node.element.kind() == &kind).unwrap_or(false))
        .count();
    let same_type_total = siblings
        .iter()
        .filter(|candidate| self.node(**candidate).map(|node| node.element.kind() == &kind).unwrap_or(false))
        .count();

    Ok(super::SelectorSiblingFacts::new(
        Some(parent),
        index.checked_sub(1).map(|previous| siblings[previous]),
        siblings.get(index + 1).copied(),
        index == 0,
        index + 1 == siblings.len(),
        siblings.len() == 1,
        Some(super::SelectorIndex::new(index)),
        super::SelectorCount::new(siblings.len()),
        Some(super::SelectorIndex::new(same_type_before)),
        super::SelectorCount::new(same_type_total),
    ))
}
```

- [ ] **Step 5: Add snapshot method**

In `src/snapshot.rs`, add:

```rust
pub fn selector_sibling_facts(
    &self,
    id: Id,
    traversal: SelectorTraversal,
) -> Result<SelectorSiblingFacts> {
    self.model.selector_sibling_facts(id, traversal)
}
```

- [ ] **Step 6: Run checks**

Run:

```sh
cargo test selector_sibling_and_structural_facts_are_derived_from_policy_tree
cargo test selector_count_rejects_zero
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```sh
git add src/snapshot.rs src/model.rs src/lib.rs src/tests.rs
git commit -m "Expose selector structural facts"
```

---

### Task 3: Add Runtime State Fact Intake

**Files:**
- Modify: `src/state.rs`
- Modify: `src/model.rs`
- Modify: `src/mutation.rs`
- Modify: `src/lib.rs`
- Test: `src/tests.rs`

- [ ] **Step 1: Write failing runtime state intake test**

Add this test near state tests in `src/tests.rs`:

```rust
#[test]
fn runtime_state_patch_updates_externally_supplied_pseudo_class_facts() {
    let mut model = Model::new(Element::root().with_child(element("button", "run"))).unwrap();
    let target = model.snapshot().children(model.root()).unwrap().next().unwrap();

    let report = model
        .apply(Patch::SetRuntimeState {
            id: target,
            state: RuntimeStatePatch::new()
                .hovered(true)
                .active(true)
                .pressed(true)
                .disabled(true)
                .selected(true)
                .checked(Some(true))
                .expanded(Some(false)),
        })
        .unwrap();

    let state = model.snapshot().get(target).unwrap().state();
    assert!(state.hovered());
    assert!(state.active());
    assert!(state.pressed());
    assert!(state.disabled());
    assert!(state.selected());
    assert_eq!(state.checked(), Some(true));
    assert_eq!(state.expanded(), Some(false));
    let (_, flags) = report.changes().changed().next().unwrap();
    assert!(flags.has_state());
    assert!(flags.has_runtime_state());
}

#[test]
fn runtime_disabled_state_releases_focus_and_pointer_capture() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("section", "parent").with_child(element("button", "run"))),
    )
    .unwrap();
    let parent = model.snapshot().children(model.root()).unwrap().next().unwrap();
    let target = model.snapshot().children(parent).unwrap().next().unwrap();

    model.focus(Some(target)).unwrap();
    model
        .capture_pointer(PointerCapture::new(PointerId::new(7), target))
        .unwrap();

    model
        .apply(Patch::SetRuntimeState {
            id: target,
            state: RuntimeStatePatch::new().disabled(true),
        })
        .unwrap();

    let state = model.snapshot().get(target).unwrap().state();
    assert!(!state.focused());
    assert!(!state.pointer_captured());
    assert!(!model.snapshot().get(parent).unwrap().state().focus_within());
}
```

Run:

```sh
cargo test runtime_state_patch_updates_externally_supplied_pseudo_class_facts
cargo test runtime_disabled_state_releases_focus_and_pointer_capture
```

Expected: FAIL because `RuntimeStatePatch`, `Patch::SetRuntimeState`, and `has_runtime_state` do not exist.

- [ ] **Step 2: Add runtime state patch type**

In `src/state.rs`, add after `StatePatch`:

```rust
/// Runtime-authored retained state changes supplied by root/window/runtime.
///
/// This is separate from `StatePatch` because app-authored state and host
/// interaction facts have different owners.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStatePatch {
    hovered: Option<bool>,
    active: Option<bool>,
    pressed: Option<bool>,
    disabled: Option<bool>,
    selected: Option<bool>,
    checked: Option<Option<bool>>,
    expanded: Option<Option<bool>>,
}

impl RuntimeStatePatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = Some(hovered);
        self
    }

    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = Some(active);
        self
    }

    #[must_use]
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    #[must_use]
    pub fn checked(mut self, checked: Option<bool>) -> Self {
        self.checked = Some(checked);
        self
    }

    #[must_use]
    pub fn expanded(mut self, expanded: Option<bool>) -> Self {
        self.expanded = Some(expanded);
        self
    }
}
```

`RuntimeStatePatch` intentionally overlaps some fields with app-authored
`StatePatch` because root/runtime may need to mirror selector-visible state from
host-backed controls. Root integration must choose one source of authority per
fact for a given node. Retained applies transactions in order and reports the
resulting selector invalidation.

Also add this method to `impl State`:

```rust
pub(crate) fn apply_runtime_patch(&mut self, patch: &RuntimeStatePatch) -> bool {
    let before = self.clone();
    if let Some(value) = patch.hovered {
        self.hovered = value;
    }
    if let Some(value) = patch.active {
        self.active = value;
    }
    if let Some(value) = patch.pressed {
        self.pressed = value;
    }
    if let Some(value) = patch.disabled {
        self.disabled = value;
    }
    if let Some(value) = patch.selected {
        self.selected = value;
    }
    if let Some(value) = patch.checked {
        self.checked = value;
    }
    if let Some(value) = patch.expanded {
        self.expanded = value;
    }
    *self != before
}
```

- [ ] **Step 3: Export runtime state patch**

In `src/lib.rs`, change:

```rust
pub use state::{PointerCapture, PointerId, Presence, State, StatePatch};
```

to:

```rust
pub use state::{PointerCapture, PointerId, Presence, RuntimeStatePatch, State, StatePatch};
```

- [ ] **Step 4: Add runtime state change flag**

In `src/change.rs`, add a private field to `ChangeFlags`:

```rust
runtime_state: bool,
```

Initialize, merge, and expose it:

```rust
#[must_use]
pub const fn runtime_state(mut self) -> Self {
    self.runtime_state = true;
    self
}

#[must_use]
pub const fn has_runtime_state(self) -> bool {
    self.runtime_state
}
```

Update `empty()` and `merge()` consistently.

- [ ] **Step 5: Add mutation variant**

In `src/mutation.rs`, import `RuntimeStatePatch` and add:

```rust
SetRuntimeState {
    id: Id,
    state: RuntimeStatePatch,
},
```

Put it next to `SetState`.

- [ ] **Step 6: Apply runtime state transactionally**

In `src/model.rs`, add a `Patch::SetRuntimeState` arm:

```rust
Patch::SetRuntimeState { id, state } => {
    let before = self.node(id)?.state.clone();
    let mut after = before.clone();
    let changed = after.apply_runtime_patch(&state);
    if changed {
        transaction.record_node(self, id);
        self.node_mut(id)?.state = after;
        changes.change(id, ChangeFlags::empty().state().runtime_state());
        if self.node(id)?.state.disabled != before.disabled {
            self.release_invalid_focus_and_capture(transaction, &mut changes);
        }
    }
}
```

Do not make runtime patches directly set focus, focus-within, or pointer capture. Focus and pointer capture remain owned by their existing APIs. Runtime `disabled` changes must still run the same eligibility cleanup path as app-authored state changes so focused and captured nodes cannot remain active after becoming ineligible. Update `release_invalid_focus_and_capture` as part of this task so when it clears focus it also calls `recompute_focus_within(transaction, changes)`.

- [ ] **Step 7: Run checks**

Run:

```sh
cargo test runtime_state_patch_updates_externally_supplied_pseudo_class_facts
cargo test runtime_disabled_state_releases_focus_and_pointer_capture
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 8: Commit**

```sh
git add src/state.rs src/model.rs src/mutation.rs src/change.rs src/lib.rs src/tests.rs
git commit -m "Add runtime state fact intake"
```

---

### Task 4: Add Selector Metadata Lookup Facts

**Files:**
- Modify: `src/snapshot.rs`
- Modify: `src/model.rs`
- Modify: `src/lib.rs`
- Test: `src/tests.rs`

- [ ] **Step 1: Write failing metadata lookup tests**

Add this test near metadata tests in `src/tests.rs`:

```rust
#[test]
fn selector_metadata_facts_expose_tag_key_class_role_and_attributes() {
    let model = Model::new(
        Element::root().with_child(
            Element::tagged(tag("button"))
                .with_key(key("run"))
                .with_role(Role::Button)
                .with_label(text("Run"))
                .with_class(Class::new("primary").unwrap())
                .with_class(Class::new("cta").unwrap())
                .with_attribute(Attribute::new(attr_name("data-mode"), value("fast slow")))
                .with_text(text("Launch")),
        ),
    )
    .unwrap();
    let target = model.snapshot().children(model.root()).unwrap().next().unwrap();
    let facts = model.snapshot().selector_metadata(target).unwrap();

    assert_eq!(facts.id(), target);
    assert_eq!(facts.key(), Some(&key("run")));
    assert_eq!(facts.kind(), &Kind::Element(tag("button")));
    assert_eq!(facts.role(), Role::Button);
    assert_eq!(facts.label(), Some(&text("Run")));
    assert_eq!(facts.text(), Some(&text("Launch")));
    assert!(facts.has_class(&Class::new("primary").unwrap()));
    assert!(facts.has_class(&Class::new("cta").unwrap()));
    assert_eq!(
        facts.attribute(&attr_name("data-mode")).map(Value::as_str),
        Some("fast slow")
    );
}
```

Run:

```sh
cargo test selector_metadata_facts_expose_tag_key_class_role_and_attributes
```

Expected: FAIL because `selector_metadata` and `SelectorMetadata` do not exist.

- [ ] **Step 2: Add metadata facts wrapper**

In `src/snapshot.rs`, add:

```rust
impl<'a> NodeRef<'a> {
    #[must_use]
    pub fn label(&self) -> Option<&'a Text> {
        self.node.element.label()
    }
}

/// Borrowed retained metadata facts used by selector matchers.
#[derive(Clone, Copy, Debug)]
pub struct SelectorMetadata<'a> {
    node: NodeRef<'a>,
}

impl<'a> SelectorMetadata<'a> {
    #[must_use]
    pub fn id(&self) -> Id {
        self.node.id()
    }

    #[must_use]
    pub fn key(&self) -> Option<&'a Key> {
        self.node.key()
    }

    #[must_use]
    pub fn kind(&self) -> &'a Kind {
        self.node.kind()
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.node.role()
    }

    #[must_use]
    pub fn label(&self) -> Option<&'a Text> {
        self.node.label()
    }

    #[must_use]
    pub fn classes(&self) -> &'a [Class] {
        self.node.classes()
    }

    #[must_use]
    pub fn attributes(&self) -> &'a [Attribute] {
        self.node.attributes()
    }

    #[must_use]
    pub fn text(&self) -> Option<&'a Text> {
        self.node.text()
    }

    #[must_use]
    pub fn has_class(&self, class: &Class) -> bool {
        self.classes().contains(class)
    }

    #[must_use]
    pub fn attribute(&self, name: &super::AttributeName) -> Option<&'a super::Value> {
        self.attributes()
            .iter()
            .find(|attribute| &attribute.name == name)
            .map(|attribute| &attribute.value)
    }
}
```

This wrapper exposes retained-owned facts only. It does not implement CSS attribute selector matching.

- [ ] **Step 3: Add snapshot method**

In `src/snapshot.rs`, add:

```rust
#[must_use]
pub fn selector_metadata(&self, id: Id) -> Option<SelectorMetadata<'_>> {
    self.get(id).map(|node| SelectorMetadata { node })
}
```

- [ ] **Step 4: Export metadata facts**

In `src/lib.rs`, include `SelectorMetadata` in the snapshot reexports:

```rust
pub use snapshot::{
    NodeRef, SelectorCount, SelectorIndex, SelectorMetadata, SelectorSiblingFacts,
    SelectorTraversal, Snapshot,
};
```

- [ ] **Step 5: Run checks**

Run:

```sh
cargo test selector_metadata_facts_expose_tag_key_class_role_and_attributes
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```sh
git add src/snapshot.rs src/lib.rs src/tests.rs
git commit -m "Expose selector metadata facts"
```

---

### Task 5: Add Selector Invalidation Reports

**Files:**
- Modify: `src/change.rs`
- Modify: `src/model.rs`
- Modify: `src/lib.rs`
- Test: `src/tests.rs`

- [ ] **Step 1: Write failing invalidation tests**

Add these tests near change-report tests in `src/tests.rs`:

```rust
#[test]
fn selector_invalidation_reports_metadata_and_structure_pressure() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("row", "a"))
            .with_child(element("row", "b")),
    )
    .unwrap();
    let children = model.snapshot().children(model.root()).unwrap().collect::<Vec<_>>();
    let first = children[0];

    let report = model
        .apply(Patch::SetClasses {
            id: first,
            classes: vec![Class::new("selected").unwrap()],
        })
        .unwrap();
    assert_eq!(
        report.changes().selector_invalidations(),
        &[SelectorInvalidation::Metadata {
            id: first,
            facts: SelectorMetadataChange::empty().classes(),
        }]
    );

    let report = model
        .apply(Patch::Move {
            id: first,
            parent: model.root(),
            index: 2,
        })
        .unwrap();
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::Canonical,
        }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        }));

    let report = model
        .apply(Patch::SetKind {
            id: first,
            kind: Kind::Element(tag("item")),
        })
        .unwrap();
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Metadata {
            id: first,
            facts: SelectorMetadataChange::empty().kind(),
        }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::Canonical,
        }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        }));

    let report = model
        .apply(Patch::Replace {
            id: first,
            element: Element::tagged(tag("item"))
                .with_key(key("a"))
                .with_class(Class::new("replacement").unwrap()),
            mode: ReplaceMode::AllowKindChange,
        })
        .unwrap();
    assert!(report.changes().selector_invalidations().contains(
        &SelectorInvalidation::Metadata {
            id: first,
            facts: SelectorMetadataChange::empty()
                .key()
                .kind()
                .role()
                .label()
                .classes()
                .attributes()
                .text(),
        }
    ));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: first,
            traversal: SelectorTraversal::Canonical,
        }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::Canonical,
        }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: model.root(),
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        }));
}

#[test]
fn selector_invalidation_reports_focus_runtime_state_pressure() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("section", "parent").with_child(element("button", "target"))),
    )
    .unwrap();
    let parent = model.snapshot().children(model.root()).unwrap().next().unwrap();
    let target = model.snapshot().children(parent).unwrap().next().unwrap();

    let report = model.focus(Some(target)).unwrap();
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::State { id: target }));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::State { id: parent }));
}

#[test]
fn selector_invalidation_reports_projected_type_pressure_when_kind_changes() {
    let mut model = Model::new(Element::root().with_child(element("host", "host"))).unwrap();
    let host = model.snapshot().children(model.root()).unwrap().next().unwrap();
    let slot = ProjectionSlot::default(host);

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "run")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let projected = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("section", "run")]),
            ProjectionReplaceMode::PreserveIdentity,
        ))
        .unwrap();
    let report = model.resolve_projection(slot.clone()).unwrap();

    assert_eq!(
        model
            .snapshot()
            .projected_children(slot.clone())
            .unwrap()
            .next(),
        Some(projected)
    );
    assert!(report.changes().selector_invalidations().contains(
        &SelectorInvalidation::Metadata {
            id: projected,
            facts: SelectorMetadataChange::empty().kind(),
        }
    ));
    assert!(report
        .changes()
        .selector_invalidations()
        .contains(&SelectorInvalidation::Siblings {
            parent: host,
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        }));
}
```

Run:

```sh
cargo test selector_invalidation_reports_metadata_and_structure_pressure
cargo test selector_invalidation_reports_focus_runtime_state_pressure
cargo test selector_invalidation_reports_projected_type_pressure_when_kind_changes
```

Expected: FAIL because selector invalidation types and accessors do not exist.

- [ ] **Step 2: Add metadata change detail type**

In `src/change.rs`, import `SelectorTraversal` and add:

```rust
use super::{Command, Id, ProjectionSlot, SelectorTraversal};
```

Then add:

```rust
/// Selector-facing metadata facts that changed for a retained node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SelectorMetadataChange {
    kind: bool,
    key: bool,
    role: bool,
    label: bool,
    classes: bool,
    attributes: bool,
    text: bool,
}

impl SelectorMetadataChange {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            kind: false,
            key: false,
            role: false,
            label: false,
            classes: false,
            attributes: false,
            text: false,
        }
    }

    #[must_use]
    pub const fn kind(mut self) -> Self {
        self.kind = true;
        self
    }

    #[must_use]
    pub const fn key(mut self) -> Self {
        self.key = true;
        self
    }

    #[must_use]
    pub const fn role(mut self) -> Self {
        self.role = true;
        self
    }

    #[must_use]
    pub const fn label(mut self) -> Self {
        self.label = true;
        self
    }

    #[must_use]
    pub const fn classes(mut self) -> Self {
        self.classes = true;
        self
    }

    #[must_use]
    pub const fn attributes(mut self) -> Self {
        self.attributes = true;
        self
    }

    #[must_use]
    pub const fn text(mut self) -> Self {
        self.text = true;
        self
    }

    #[must_use]
    pub const fn has_kind(self) -> bool {
        self.kind
    }

    #[must_use]
    pub const fn has_key(self) -> bool {
        self.key
    }

    #[must_use]
    pub const fn has_role(self) -> bool {
        self.role
    }

    #[must_use]
    pub const fn has_label(self) -> bool {
        self.label
    }

    #[must_use]
    pub const fn has_classes(self) -> bool {
        self.classes
    }

    #[must_use]
    pub const fn has_attributes(self) -> bool {
        self.attributes
    }

    #[must_use]
    pub const fn has_text(self) -> bool {
        self.text
    }

}
```

- [ ] **Step 3: Add invalidation enum and ChangeSet storage**

In `src/change.rs`, add:

```rust
/// Selector recomputation pressure produced by retained mutations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectorInvalidation {
    /// A node's directly matched metadata may have changed.
    Metadata {
        id: Id,
        facts: SelectorMetadataChange,
    },
    /// Sibling/structural facts for children of this parent may have changed.
    Siblings {
        parent: Id,
        traversal: SelectorTraversal,
    },
    /// A projection slot changed and projected selector traversal may need recomputation.
    Projection {
        slot: ProjectionSlot,
    },
    /// Selector-visible retained state facts changed for one node.
    State {
        id: Id,
    },
}
```

Add a `selector_invalidations: Vec<SelectorInvalidation>` field to `ChangeSet`, include it in `is_empty`, and add:

```rust
#[must_use]
pub fn selector_invalidations(&self) -> &[SelectorInvalidation] {
    &self.selector_invalidations
}

pub(crate) fn selector_invalidation(&mut self, invalidation: SelectorInvalidation) {
    push_unique(&mut self.selector_invalidations, invalidation);
}
```

Update `merge()` to merge invalidations.

- [ ] **Step 4: Export invalidation types**

In `src/lib.rs`, change:

```rust
pub use change::{ChangeFlags, ChangeSet, Report};
```

to:

```rust
pub use change::{
    ChangeFlags, ChangeSet, Report, SelectorInvalidation, SelectorMetadataChange,
};
```

- [ ] **Step 5: Emit selector invalidations from existing mutation paths**

In `src/model.rs`, update mutation branches:

- For `Patch::Insert`, `Patch::Remove`, `Patch::Move`, and `Patch::ReorderChildren`, add sibling invalidations for every parent whose child list changed under both `SelectorTraversal::Canonical` and `SelectorTraversal::ProjectedDefaultSlot`. The projected-default invalidation is required because projected-default traversal falls back to canonical children when no clean default projection cache exists.
- For `Patch::Replace`, add conservative direct metadata invalidation for the replaced node using `SelectorMetadataChange::empty().key().kind().role().label().classes().attributes().text()`, sibling invalidation under both `SelectorTraversal::Canonical` and `SelectorTraversal::ProjectedDefaultSlot` for the replaced node's child list, sibling invalidation under both traversals for the canonical parent when the replacement kind or key changes, and projected-default sibling invalidation for the projected parent when the replaced node is projection-owned or otherwise appears in a projected-default child list.
- For `Patch::SetKind`, add `SelectorInvalidation::Metadata { id, facts: SelectorMetadataChange::empty().kind() }`, sibling invalidation under both `SelectorTraversal::Canonical` and `SelectorTraversal::ProjectedDefaultSlot` for the canonical parent because type-aware structural facts may change for siblings, and projected-default sibling invalidation for the projected parent when the changed node is projection-owned or otherwise appears in a projected-default child list.
- For any replacement key change, include `.key()` in the metadata invalidation; key changes currently occur through `Patch::Replace`.
- For `Patch::SetRole`, use `.role()`.
- For `Patch::SetLabel`, use `.label()`.
- For `Patch::SetClasses`, use `.classes()`.
- For `Patch::SetAttribute` and `Patch::RemoveAttribute`, use `.attributes()`.
- For `Patch::SetText`, use `.text()`.
- For `Patch::SetState`, add `SelectorInvalidation::State { id }`.
- For `Patch::SetRuntimeState`, add `SelectorInvalidation::State { id }`.
- In `focus_inner` and `recompute_focus_within`, add `SelectorInvalidation::State { id }` for each node whose focused or focus-within state changes.
- In pointer capture and release paths, add `SelectorInvalidation::State { id }` for nodes whose pointer-captured state changes.
- For projection apply/resolve paths that call `changes.change_projection_slot(slot.clone())`, also add `SelectorInvalidation::Projection { slot: slot.clone() }`.
- In `replace_projection_node`, add direct `SelectorInvalidation::Metadata` for a reused projected node when its kind, key, role, label, classes, attributes, or text change. If the reused projected node's kind changes, also add `SelectorInvalidation::Siblings { parent: slot.host(), traversal: SelectorTraversal::ProjectedDefaultSlot }` so type-aware projected structural facts are recomputed even when the projected child ID list is unchanged.

Use fully qualified names or add imports:

```rust
SelectorInvalidation, SelectorMetadataChange, SelectorTraversal,
```

- [ ] **Step 6: Run checks**

Run:

```sh
cargo test selector_invalidation_reports_metadata_and_structure_pressure
cargo test selector_invalidation_reports_focus_runtime_state_pressure
cargo test selector_invalidation_reports_projected_type_pressure_when_kind_changes
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```sh
git add src/change.rs src/model.rs src/lib.rs src/tests.rs
git commit -m "Report selector invalidation facts"
```

---

### Task 6: Document Unsupported Selector Fact Policy

**Files:**
- Create: `plans/2026-07-07-selector-runtime-facts-policy.md`
- Test: none

- [ ] **Step 1: Add policy note**

Create `plans/2026-07-07-selector-runtime-facts-policy.md`:

```markdown
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
```

- [ ] **Step 2: Commit**

```sh
git add plans/2026-07-07-selector-runtime-facts-policy.md
git commit -m "Document retained selector fact policy"
```

---

### Task 7: Final Verification And Review

**Files:**
- Inspect: all changed files

- [ ] **Step 1: Run final checks**

Run:

```sh
cargo test -p surgeist-retained
cargo clippy -p surgeist-retained --all-targets -- -D warnings
cargo fmt --check
git status --short --branch
```

Expected: tests/clippy/fmt PASS. Git status should be clean except ahead markers.

- [ ] **Step 2: Self-check boundaries**

Confirm:

- Retained exposes selector facts, not selector matching.
- `SelectorTraversal` makes canonical versus projected policy explicit.
- Runtime state intake is separate from app-authored `StatePatch`.
- Structural facts are derived from retained tree state and covered by tests.
- Selector invalidation reports are more precise than whole-tree recomputation for direct metadata and sibling changes.
- Unsupported selector facts are documented explicitly.
- No pseudo-elements are materialized.
- No CSS/style/root dependencies were added.
- No crate-local API artifact tooling was reintroduced.

- [ ] **Step 3: Request final clean-context review**

Dispatch a clean-context reviewer with:

- `plans/2026-07-07-selector-facts-planning-directive.md`
- this implementation plan
- `guidance/surgeist-rust-modeling-guide.md`
- `/Users/codex/Development/surgeist/guidance/surgeist-rust-modeling-guide.md`
- `/Users/codex/Development/surgeist/plans/2026-07-04-css-integration-support-inventory.md`
- changed source files

Reviewer prompt:

```text
Review the retained selector runtime facts implementation against the directive,
retained crate boundary, root inventory, and modeling guide. Check whether the
implementation exposes facts without taking ownership of CSS parsing, selector
matching, cascade, root lowering, runtime/window host interpretation, or
pseudo-element materialization. Report Critical, Important, and Minor findings.
Completion requires no Critical or Important findings.
```

- [ ] **Step 4: Reconcile review**

If the reviewer reports Critical or Important findings, fix them in a follow-up task and request another clean-context holistic review.

If the reviewer reports no Critical or Important findings, the implementation is ready to hand back to root.
