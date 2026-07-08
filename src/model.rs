use std::collections::{BTreeMap, BTreeSet};

use super::{
    Attribute, ChangeFlags, ChangeSet, Command, Element, Error, ErrorCode, Event, Id, Key, KeyPath,
    Kind, Mutation, MutationEdit, Patch, Phase, PointerCapture, PointerId, Presence,
    ProjectionEdit, ProjectionReplaceMode, ProjectionSlot, ProjectionSource, ReplaceMode, Report,
    Result, Route, RouteStep, SelectorInvalidation, SelectorMetadataChange, SelectorTraversal,
    State, VirtualProjection, transaction::Transaction,
};

/// Monotonic retained model revision for snapshot cache identity.
///
/// The value advances after committed snapshot-observable changes.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ModelRevision(u64);

impl ModelRevision {
    /// Creates a revision from a raw value for cache adapters and tests.
    ///
    /// Model advancement is still owned by `Model`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw revision value for cache adapters, serialization, and test assertions.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn advance(&mut self) {
        self.0 = self.0.checked_add(1).expect("model revision overflow");
    }
}

#[derive(Debug)]
pub struct Model {
    pub(crate) nodes: Vec<Option<Node>>,
    pub(crate) root: Id,
    pub(crate) focus: Option<Id>,
    pub(crate) pointer_captures: BTreeMap<PointerId, Id>,
    pub(crate) projection_caches: BTreeMap<ProjectionSlot, ProjectionCache>,
    pub(crate) pending_sources: BTreeMap<ProjectionSlot, PendingProjection>,
    pub(crate) dirty_slots: BTreeSet<ProjectionSlot>,
    pub(crate) virtual_anchors: BTreeMap<(ProjectionSlot, Key), State>,
    pub(crate) changes: ChangeSet,
    pub(crate) revision: ModelRevision,
    #[cfg(test)]
    pub(crate) failpoint: Option<Failpoint>,
}

impl Model {
    pub fn new(root: Element) -> Result<Self> {
        root.validate()?;
        let mut model = Self::blank();
        let root_id = model.alloc_node(root, Owner::Root, None, KeyPath::root(), None)?;
        model.root = root_id;
        Ok(model)
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::new(Element::root()).expect("empty root is valid")
    }

    #[must_use]
    pub const fn root(&self) -> Id {
        self.root
    }

    #[must_use]
    pub const fn revision(&self) -> ModelRevision {
        self.revision
    }

    #[must_use]
    pub fn snapshot(&self) -> super::Snapshot<'_> {
        super::Snapshot::new(self)
    }

    pub fn apply(&mut self, patch: Patch) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.apply_patch_inner(patch, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn apply_projection(&mut self, projection: ProjectionEdit) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.apply_projection_inner(projection, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn mutate(&mut self, mutation: Mutation) -> Result<Report> {
        self.transaction(|model, transaction| {
            for edit in mutation.edits() {
                let changes = match edit {
                    MutationEdit::Projection(projection) => {
                        model.apply_projection_inner(projection.clone(), transaction)?
                    }
                    MutationEdit::Patch(patch) => {
                        model.apply_patch_inner(patch.clone(), transaction)?
                    }
                };
                transaction.merge_changes(changes);
            }
            Ok(())
        })
    }

    pub fn resolve_projection(&mut self, slot: ProjectionSlot) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.resolve_projection_inner(slot, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn resolve_dirty_projections(&mut self) -> Result<Report> {
        self.transaction(|model, transaction| {
            let slots: Vec<_> = model.dirty_slots.iter().cloned().collect();
            for slot in slots {
                let changes = model.resolve_projection_inner(slot, transaction)?;
                transaction.merge_changes(changes);
            }
            Ok(())
        })
    }

    pub fn route(&self, event: Event) -> Result<Route> {
        let target = match event.pointer() {
            Some(pointer) => self
                .pointer_captures
                .get(&pointer)
                .copied()
                .unwrap_or(event.target()),
            None => event.target(),
        };
        self.ensure_live(target)?;
        if !self.is_input_eligible(target)? {
            return Err(Error::new(
                ErrorCode::IneligibleTarget,
                format!("target {target:?} is not input eligible"),
            ));
        }

        let mut ancestors = self.projected_ancestors_vec(target)?;
        ancestors.reverse();
        let mut steps = Vec::new();
        match event.propagation() {
            super::Propagation::TargetOnly => steps.push(RouteStep::new(target, Phase::Target)),
            super::Propagation::Bubble => {
                steps.push(RouteStep::new(target, Phase::Target));
                steps.extend(
                    ancestors
                        .into_iter()
                        .rev()
                        .map(|id| RouteStep::new(id, Phase::Bubble)),
                );
            }
            super::Propagation::CaptureThenBubble => {
                steps.extend(
                    ancestors
                        .iter()
                        .copied()
                        .map(|id| RouteStep::new(id, Phase::Capture)),
                );
                steps.push(RouteStep::new(target, Phase::Target));
                steps.extend(
                    ancestors
                        .into_iter()
                        .rev()
                        .map(|id| RouteStep::new(id, Phase::Bubble)),
                );
            }
        }
        Ok(Route::new(steps))
    }

    pub fn dispatch(&mut self, event: Event) -> Result<Report> {
        self.transaction(|model, transaction| {
            let route = model.route(event.clone())?;
            for step in route.steps() {
                let node = model.node(step.id)?;
                for hook in node.element.hooks() {
                    if hook.trigger == *event.trigger() {
                        transaction.push_command(Command::new(
                            step.id,
                            hook.trigger.clone(),
                            step.phase,
                            hook.command.clone(),
                            route.clone(),
                        ));
                    }
                }
            }
            Ok(())
        })
    }

    pub fn focus(&mut self, id: Option<Id>) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.focus_inner(id, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn capture_pointer(&mut self, capture: PointerCapture) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.capture_pointer_inner(capture, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn release_pointer(&mut self, pointer: PointerId) -> Result<Report> {
        self.transaction(|model, transaction| {
            let changes = model.release_pointer_inner(pointer, transaction)?;
            transaction.merge_changes(changes);
            Ok(())
        })
    }

    pub fn take_changes(&mut self) -> ChangeSet {
        std::mem::take(&mut self.changes)
    }

    fn focus_inner(&mut self, id: Option<Id>, transaction: &mut Transaction) -> Result<ChangeSet> {
        if let Some(id) = id {
            self.ensure_live(id)?;
            if !self.is_input_eligible(id)? {
                return Err(Error::new(
                    ErrorCode::IneligibleTarget,
                    "focus target is ineligible",
                ));
            }
        }

        let mut changes = ChangeSet::new();
        if self.focus == id {
            return Ok(changes);
        }
        if let Some(old) = self.focus
            && self.node(old).is_ok()
        {
            transaction.record_node(self, old);
            self.node_mut(old)?.state.focused = false;
            changes.change(old, ChangeFlags::empty().state().focus());
            Self::invalidate_selector_state(&mut changes, old);
        }
        transaction.record_focus(self);
        self.focus = id;
        if let Some(new) = id {
            transaction.record_node(self, new);
            self.node_mut(new)?.state.focused = true;
            changes.change(new, ChangeFlags::empty().state().focus());
            Self::invalidate_selector_state(&mut changes, new);
        }
        self.recompute_focus_within(transaction, &mut changes);
        Ok(changes)
    }

    fn capture_pointer_inner(
        &mut self,
        capture: PointerCapture,
        transaction: &mut Transaction,
    ) -> Result<ChangeSet> {
        self.ensure_live(capture.target)?;
        if !self.is_input_eligible(capture.target)? {
            return Err(Error::new(
                ErrorCode::IneligibleTarget,
                "pointer capture target is ineligible",
            ));
        }
        let old_target = self.pointer_captures.get(&capture.pointer).copied();
        if old_target == Some(capture.target) && self.node(capture.target)?.state.pointer_captured {
            return Ok(ChangeSet::new());
        }
        let mut changes = ChangeSet::new();
        transaction.record_pointer_captures(self);
        self.pointer_captures
            .insert(capture.pointer, capture.target);
        if let Some(old_target) = old_target
            && old_target != capture.target
            && !self.has_pointer_capture_target(old_target)
        {
            self.set_pointer_captured(old_target, false, transaction, &mut changes)?;
        }
        self.set_pointer_captured(capture.target, true, transaction, &mut changes)?;
        Ok(changes)
    }

    fn release_pointer_inner(
        &mut self,
        pointer: PointerId,
        transaction: &mut Transaction,
    ) -> Result<ChangeSet> {
        let mut changes = ChangeSet::new();
        if let Some(target) = self.pointer_captures.get(&pointer).copied() {
            transaction.record_pointer_captures(self);
            self.pointer_captures.remove(&pointer);
            if !self.has_pointer_capture_target(target) {
                self.set_pointer_captured(target, false, transaction, &mut changes)?;
            }
        }
        Ok(changes)
    }

    fn transaction(
        &mut self,
        body: impl FnOnce(&mut Self, &mut Transaction) -> Result<()>,
    ) -> Result<Report> {
        let mut transaction = Transaction::new(self);
        match body(self, &mut transaction) {
            Ok(()) => Ok(transaction.commit(self)),
            Err(error) => {
                transaction.rollback(self);
                Err(error)
            }
        }
    }

    pub(crate) fn node(&self, id: Id) -> Result<&Node> {
        self.nodes
            .get(id.index())
            .and_then(Option::as_ref)
            .filter(|node| node.id == id)
            .ok_or_else(|| Error::new(ErrorCode::MissingNode, format!("missing node {id:?}")))
    }

    pub(crate) fn node_mut(&mut self, id: Id) -> Result<&mut Node> {
        self.nodes
            .get_mut(id.index())
            .and_then(Option::as_mut)
            .filter(|node| node.id == id)
            .ok_or_else(|| Error::new(ErrorCode::MissingNode, format!("missing node {id:?}")))
    }

    pub(crate) fn canonical_children(&self, id: Id) -> Result<Vec<Id>> {
        Ok(self.node(id)?.children.clone())
    }

    pub(crate) fn projected_children(&self, slot: ProjectionSlot) -> Result<Vec<Id>> {
        if self.dirty_slots.contains(&slot) {
            return Err(Error::new(
                ErrorCode::UnresolvedProjection,
                "projection slot is dirty",
            ));
        }
        if let Some(cache) = self.projection_caches.get(&slot) {
            return Ok(cache.children.clone());
        }
        if matches!(slot.key(), super::SlotKey::Default) {
            return self.canonical_children(slot.host());
        }
        Ok(Vec::new())
    }

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
            .filter(|candidate| {
                self.node(**candidate)
                    .map(|node| node.element.kind() == &kind)
                    .unwrap_or(false)
            })
            .count();
        let same_type_total = siblings
            .iter()
            .filter(|candidate| {
                self.node(**candidate)
                    .map(|node| node.element.kind() == &kind)
                    .unwrap_or(false)
            })
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

    pub(crate) fn cached_virtual_projection(
        &self,
        slot: ProjectionSlot,
    ) -> Result<Option<&VirtualProjection>> {
        if self.dirty_slots.contains(&slot) {
            return Err(Error::new(
                ErrorCode::UnresolvedProjection,
                "projection slot is dirty",
            ));
        }
        Ok(self
            .projection_caches
            .get(&slot)
            .and_then(|cache| cache.virtual_projection.as_ref()))
    }

    pub(crate) fn dirty_slots(&self) -> impl Iterator<Item = ProjectionSlot> + '_ {
        self.dirty_slots.iter().cloned()
    }

    pub(crate) fn key_lookup(&self, key_path: &KeyPath) -> Option<Id> {
        self.nodes
            .iter()
            .flatten()
            .find(|node| &node.key_path == key_path)
            .map(|node| node.id)
    }

    pub(crate) fn all_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.nodes.iter().flatten().map(|node| node.id)
    }

    pub(crate) fn ancestors_vec(&self, id: Id) -> Result<Vec<Id>> {
        let mut out = Vec::new();
        let mut current = self.node(id)?.parent;
        while let Some(id) = current {
            out.push(id);
            current = self.node(id)?.parent;
        }
        Ok(out)
    }

    pub(crate) fn projected_ancestors_vec(&self, id: Id) -> Result<Vec<Id>> {
        self.ensure_projected_edge_resolved(id)?;
        let mut out = Vec::new();
        let mut current = self.effective_projected_parent(id)?;
        while let Some(id) = current {
            self.ensure_projected_edge_resolved(id)?;
            out.push(id);
            current = self.effective_projected_parent(id)?;
        }
        Ok(out)
    }

    pub(crate) fn descendants_vec(&self, id: Id) -> Result<Vec<Id>> {
        self.ensure_live(id)?;
        let mut out = Vec::new();
        let mut stack = self.node(id)?.children.clone();
        while let Some(id) = stack.pop() {
            out.push(id);
            stack.extend(self.node(id)?.children.iter().rev().copied());
        }
        Ok(out)
    }

    pub(crate) fn effective_presence(&self, id: Id) -> Result<Presence> {
        let node = self.node(id)?;
        if node.state.presence != Presence::Visible {
            return Ok(node.state.presence);
        }
        for ancestor in self.projected_ancestors_vec(id)? {
            let state = &self.node(ancestor)?.state;
            if state.presence != Presence::Visible {
                return Ok(state.presence);
            }
        }
        Ok(Presence::Visible)
    }

    pub(crate) fn is_input_eligible(&self, id: Id) -> Result<bool> {
        let node = self.node(id)?;
        if node.state.disabled || node.state.presence != Presence::Visible {
            return Ok(false);
        }
        for ancestor in self.projected_ancestors_vec(id)? {
            let state = &self.node(ancestor)?.state;
            if state.disabled || state.presence != Presence::Visible {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn has_pointer_capture_target(&self, target: Id) -> bool {
        self.pointer_captures
            .values()
            .any(|captured| *captured == target)
    }

    fn set_pointer_captured(
        &mut self,
        target: Id,
        captured: bool,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        if self.node(target)?.state.pointer_captured != captured {
            transaction.record_node(self, target);
            self.node_mut(target)?.state.pointer_captured = captured;
            changes.change(target, ChangeFlags::empty().state().pointer_capture());
            Self::invalidate_selector_state(changes, target);
        }
        Ok(())
    }

    fn blank() -> Self {
        Self {
            nodes: Vec::new(),
            root: Id::new(0, 1),
            focus: None,
            pointer_captures: BTreeMap::new(),
            projection_caches: BTreeMap::new(),
            pending_sources: BTreeMap::new(),
            dirty_slots: BTreeSet::new(),
            virtual_anchors: BTreeMap::new(),
            changes: ChangeSet::new(),
            revision: ModelRevision::new(0),
            #[cfg(test)]
            failpoint: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_failpoint(&mut self, failpoint: Failpoint) {
        self.failpoint = Some(failpoint);
    }

    #[cfg(test)]
    pub(crate) fn clear_failpoint(&mut self) {
        self.failpoint = None;
    }

    #[cfg(test)]
    fn fail_at(&self, failpoint: Failpoint) -> Result<()> {
        if self.failpoint == Some(failpoint) {
            return Err(Error::new(
                ErrorCode::UnsupportedFeature,
                format!("injected retained transaction failure at {failpoint:?}"),
            ));
        }
        Ok(())
    }

    #[cfg(not(test))]
    fn fail_at(&self, _failpoint: Failpoint) -> Result<()> {
        Ok(())
    }

    fn ensure_live(&self, id: Id) -> Result<()> {
        self.node(id).map(|_| ())
    }

    fn alloc_node(
        &mut self,
        mut element: Element,
        owner: Owner,
        parent: Option<Id>,
        key_path: KeyPath,
        match_key: Option<Key>,
    ) -> Result<Id> {
        element.validate()?;
        let children = element.take_children();
        let id = Id::new(self.nodes.len(), 1);
        let node = Node {
            id,
            element,
            owner,
            parent,
            children: Vec::new(),
            projected_parent: None,
            state: State::default(),
            key_path,
            match_key,
        };
        self.nodes.push(Some(node));
        for (index, child) in children.into_iter().enumerate() {
            let child_key_path = self.child_key_path(id, &child, index)?;
            let child_match_key = child.key().cloned();
            let child_id = self.alloc_node(
                child,
                Owner::Canonical { parent: id },
                Some(id),
                child_key_path,
                child_match_key,
            )?;
            self.node_mut(id)?.children.push(child_id);
        }
        Ok(id)
    }

    fn child_key_path(&self, parent: Id, child: &Element, index: usize) -> Result<KeyPath> {
        let parent_path = self.node(parent)?.key_path.clone();
        Ok(match child.key() {
            Some(key) => parent_path.canonical_key(key),
            None => parent_path.canonical_index(index),
        })
    }

    fn child_index(&self, parent: Id, id: Id) -> Result<usize> {
        self.node(parent)?
            .children
            .iter()
            .position(|child| *child == id)
            .ok_or_else(|| Error::new(ErrorCode::InvalidParent, "node is not a child of parent"))
    }

    fn invalidate_selector_metadata(
        changes: &mut ChangeSet,
        id: Id,
        facts: SelectorMetadataChange,
    ) {
        if facts != SelectorMetadataChange::empty() {
            changes.selector_invalidation(SelectorInvalidation::Metadata { id, facts });
        }
    }

    fn invalidate_selector_state(changes: &mut ChangeSet, id: Id) {
        changes.selector_invalidation(SelectorInvalidation::State { id });
    }

    fn invalidate_selector_siblings(changes: &mut ChangeSet, parent: Id) {
        changes.selector_invalidation(SelectorInvalidation::Siblings {
            parent,
            traversal: SelectorTraversal::Canonical,
        });
        changes.selector_invalidation(SelectorInvalidation::Siblings {
            parent,
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        });
    }

    fn invalidate_projected_selector_siblings(changes: &mut ChangeSet, parent: Id) {
        changes.selector_invalidation(SelectorInvalidation::Siblings {
            parent,
            traversal: SelectorTraversal::ProjectedDefaultSlot,
        });
    }

    fn selector_projected_parent_for_invalidation(&self, id: Id) -> Option<Id> {
        let node = self.node(id).ok()?;
        match &node.owner {
            Owner::Projection { slot } => Some(slot.host()),
            Owner::Root | Owner::Canonical { .. } => node.projected_parent,
        }
    }

    fn projected_node_metadata_change(
        old_element: &Element,
        old_match_key: Option<&Key>,
        new_element: &Element,
        new_match_key: Option<&Key>,
    ) -> SelectorMetadataChange {
        let mut facts = SelectorMetadataChange::empty();
        if old_element.kind() != new_element.kind() {
            facts = facts.kind();
        }
        if old_match_key != new_match_key {
            facts = facts.key();
        }
        if old_element.role() != new_element.role() {
            facts = facts.role();
        }
        if old_element.label() != new_element.label() {
            facts = facts.label();
        }
        if old_element.classes() != new_element.classes() {
            facts = facts.classes();
        }
        if old_element.attributes() != new_element.attributes() {
            facts = facts.attributes();
        }
        if old_element.text_content() != new_element.text_content() {
            facts = facts.text();
        }
        facts
    }

    fn ensure_unique_child_key(
        &self,
        parent: Id,
        key: Option<&Key>,
        except: Option<Id>,
    ) -> Result<()> {
        let Some(key) = key else {
            return Ok(());
        };
        for child in &self.node(parent)?.children {
            if Some(*child) == except {
                continue;
            }
            if self.node(*child)?.element.key() == Some(key) {
                return Err(Error::new(
                    ErrorCode::DuplicateKey,
                    format!("duplicate sibling key `{key}`"),
                ));
            }
        }
        Ok(())
    }

    fn apply_patch_inner(
        &mut self,
        patch: Patch,
        transaction: &mut Transaction,
    ) -> Result<ChangeSet> {
        let mut changes = ChangeSet::new();
        match patch {
            Patch::Insert {
                parent,
                index,
                element,
            } => {
                self.ensure_live(parent)?;
                if index > self.node(parent)?.children.len() {
                    return Err(Error::new(
                        ErrorCode::InvalidIndex,
                        "insert index out of bounds",
                    ));
                }
                self.ensure_unique_child_key(parent, element.key(), None)?;
                let key_path = self.child_key_path(parent, &element, index)?;
                let match_key = element.key().cloned();
                let id = self.alloc_node(
                    element,
                    Owner::Canonical { parent },
                    Some(parent),
                    key_path,
                    match_key,
                )?;
                transaction.record_node(self, parent);
                self.node_mut(parent)?.children.insert(index, id);
                self.refresh_child_key_paths(parent, transaction)?;
                changes.insert(id);
                changes.change(parent, ChangeFlags::empty().structure());
                Self::invalidate_selector_siblings(&mut changes, parent);
            }
            Patch::Replace { id, element, mode } => {
                self.ensure_live(id)?;
                element.validate()?;
                let old_kind = self.node(id)?.element.kind().clone();
                if mode == ReplaceMode::PreserveCompatible && old_kind != *element.kind() {
                    return Err(Error::new(
                        ErrorCode::InvalidPatch,
                        "replace kind is incompatible with PreserveCompatible",
                    ));
                }
                let parent = self.node(id)?.parent;
                let projected_parent = self.selector_projected_parent_for_invalidation(id);
                if let Some(parent) = parent {
                    self.ensure_unique_child_key(parent, element.key(), Some(id))?;
                }
                let key_path = match parent {
                    Some(parent) => {
                        let index = self.child_index(parent, id)?;
                        self.child_key_path(parent, &element, index)?
                    }
                    None => KeyPath::root(),
                };
                let match_key = element.key().cloned();
                let mut element = element;
                let children = element.take_children();
                transaction.record_node(self, id);
                self.remove_children(id, transaction, &mut changes)?;
                {
                    transaction.record_node(self, id);
                    let node = self.node_mut(id)?;
                    node.element = element;
                    node.key_path = key_path;
                    node.match_key = match_key;
                }
                for (index, child) in children.into_iter().enumerate() {
                    let child_key_path = self.child_key_path(id, &child, index)?;
                    let match_key = child.key().cloned();
                    let child_id = self.alloc_node(
                        child,
                        Owner::Canonical { parent: id },
                        Some(id),
                        child_key_path,
                        match_key,
                    )?;
                    transaction.record_node(self, id);
                    self.node_mut(id)?.children.push(child_id);
                    changes.insert(child_id);
                }
                changes.change(id, ChangeFlags::empty().structure().kind());
                Self::invalidate_selector_metadata(
                    &mut changes,
                    id,
                    SelectorMetadataChange::empty()
                        .key()
                        .kind()
                        .role()
                        .label()
                        .classes()
                        .attributes()
                        .text(),
                );
                Self::invalidate_selector_siblings(&mut changes, id);
                if let Some(parent) = parent {
                    Self::invalidate_selector_siblings(&mut changes, parent);
                }
                if let Some(projected_parent) = projected_parent {
                    Self::invalidate_projected_selector_siblings(&mut changes, projected_parent);
                }
            }
            Patch::Remove { id } => {
                if id == self.root {
                    return Err(Error::new(ErrorCode::InvalidPatch, "cannot remove root"));
                }
                let parent = self.node(id)?.parent.ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvalidParent,
                        "removed node has no canonical parent",
                    )
                })?;
                transaction.record_node(self, parent);
                self.node_mut(parent)?.children.retain(|child| *child != id);
                self.remove_subtree(id, transaction, &mut changes)?;
                changes.change(parent, ChangeFlags::empty().structure());
                Self::invalidate_selector_siblings(&mut changes, parent);
            }
            Patch::Move { id, parent, index } => {
                if id == self.root {
                    return Err(Error::new(ErrorCode::InvalidMove, "cannot move root"));
                }
                self.ensure_live(parent)?;
                if self.descendants_vec(id)?.contains(&parent) {
                    return Err(Error::new(
                        ErrorCode::Cycle,
                        "cannot move node into descendant",
                    ));
                }
                let old_parent = self.node(id)?.parent.ok_or_else(|| {
                    Error::new(
                        ErrorCode::InvalidParent,
                        "moved node has no canonical parent",
                    )
                })?;
                let old_index = self.child_index(old_parent, id)?;
                if index > self.node(parent)?.children.len() {
                    return Err(Error::new(
                        ErrorCode::InvalidIndex,
                        "move index out of bounds",
                    ));
                }
                let key = self.node(id)?.element.key().cloned();
                self.ensure_unique_child_key(parent, key.as_ref(), Some(id))?;
                let insert_index = if old_parent == parent && index > old_index {
                    index - 1
                } else {
                    index
                };
                if old_parent == parent && insert_index == old_index {
                    return Ok(ChangeSet::new());
                }
                transaction.record_node(self, old_parent);
                transaction.record_node(self, parent);
                transaction.record_node(self, id);
                self.node_mut(old_parent)?
                    .children
                    .retain(|child| *child != id);
                if insert_index > self.node(parent)?.children.len() {
                    return Err(Error::new(
                        ErrorCode::InvalidIndex,
                        "move index out of bounds after removing source node",
                    ));
                }
                self.node_mut(parent)?.children.insert(insert_index, id);
                self.node_mut(id)?.parent = Some(parent);
                self.node_mut(id)?.owner = Owner::Canonical { parent };
                self.refresh_child_key_paths(old_parent, transaction)?;
                self.refresh_child_key_paths(parent, transaction)?;
                changes.move_node(id);
                changes.change(old_parent, ChangeFlags::empty().structure());
                changes.change(parent, ChangeFlags::empty().structure());
                Self::invalidate_selector_siblings(&mut changes, old_parent);
                Self::invalidate_selector_siblings(&mut changes, parent);
            }
            Patch::ReorderChildren { parent, children } => {
                self.ensure_live(parent)?;
                let existing = self.node(parent)?.children.clone();
                let mut sorted_existing = existing.clone();
                let mut sorted_new = children.clone();
                sorted_existing.sort();
                sorted_new.sort();
                if sorted_existing != sorted_new {
                    return Err(Error::new(
                        ErrorCode::InvalidPatch,
                        "reorder must contain the existing child set",
                    ));
                }
                if children == existing {
                    return Ok(ChangeSet::new());
                }
                transaction.record_node(self, parent);
                self.node_mut(parent)?.children = children;
                self.refresh_child_key_paths(parent, transaction)?;
                changes.change(parent, ChangeFlags::empty().structure());
                Self::invalidate_selector_siblings(&mut changes, parent);
            }
            Patch::SetKind { id, kind } => {
                if *self.node(id)?.element.kind() != kind {
                    let parent = self.node(id)?.parent;
                    let projected_parent = self.selector_projected_parent_for_invalidation(id);
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_kind(kind);
                    changes.change(id, ChangeFlags::empty().kind());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().kind(),
                    );
                    if let Some(parent) = parent {
                        Self::invalidate_selector_siblings(&mut changes, parent);
                    }
                    if let Some(projected_parent) = projected_parent {
                        Self::invalidate_projected_selector_siblings(
                            &mut changes,
                            projected_parent,
                        );
                    }
                }
            }
            Patch::SetRole { id, role } => {
                if self.node(id)?.element.role() != role {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_role(role);
                    changes.change(id, ChangeFlags::empty().role());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().role(),
                    );
                }
            }
            Patch::SetLabel { id, label } => {
                if self.node(id)?.element.label() != label.as_ref() {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_label(label);
                    changes.change(id, ChangeFlags::empty().label());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().label(),
                    );
                }
            }
            Patch::SetClasses { id, classes } => {
                if self.node(id)?.element.classes() != classes.as_slice() {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_classes(classes);
                    changes.change(id, ChangeFlags::empty().classes());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().classes(),
                    );
                }
            }
            Patch::SetAttribute { id, name, value } => {
                let existing = self
                    .node(id)?
                    .element
                    .attributes()
                    .iter()
                    .find(|attribute| attribute.name == name);
                let changed = match existing {
                    Some(attribute) => attribute.value != value,
                    None => true,
                };
                if changed {
                    transaction.record_node(self, id);
                    self.node_mut(id)?
                        .element
                        .set_attribute(Attribute { name, value });
                    changes.change(id, ChangeFlags::empty().attributes());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().attributes(),
                    );
                }
            }
            Patch::RemoveAttribute { id, name } => {
                if self
                    .node(id)?
                    .element
                    .attributes()
                    .iter()
                    .any(|attribute| attribute.name == name)
                {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.remove_attribute(&name);
                    changes.change(id, ChangeFlags::empty().attributes());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().attributes(),
                    );
                }
            }
            Patch::SetText { id, text } => {
                if self.node(id)?.element.text_content() != text.as_ref() {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_text(text);
                    changes.change(id, ChangeFlags::empty().text());
                    Self::invalidate_selector_metadata(
                        &mut changes,
                        id,
                        SelectorMetadataChange::empty().text(),
                    );
                }
            }
            Patch::SetHooks { id, hooks } => {
                if self.node(id)?.element.hooks() != hooks.as_slice() {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.element.set_hooks(hooks);
                    changes.change(id, ChangeFlags::empty().hooks());
                }
            }
            Patch::SetState { id, state } => {
                let before = self.node(id)?.state.clone();
                let mut after = before.clone();
                let changed = after.apply_patch(&state);
                if changed {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.state = after;
                    let mut flags = ChangeFlags::empty().state();
                    if self.node(id)?.state.presence != before.presence {
                        flags = flags.presence();
                    }
                    changes.change(id, flags);
                    Self::invalidate_selector_state(&mut changes, id);
                    self.release_invalid_focus_and_capture(transaction, &mut changes);
                }
            }
            Patch::SetRuntimeState { id, state } => {
                let before = self.node(id)?.state.clone();
                let mut after = before.clone();
                let changed = after.apply_runtime_patch(&state);
                if changed {
                    transaction.record_node(self, id);
                    self.node_mut(id)?.state = after;
                    changes.change(id, ChangeFlags::empty().state().runtime_state());
                    Self::invalidate_selector_state(&mut changes, id);
                    if self.node(id)?.state.disabled != before.disabled {
                        self.release_invalid_focus_and_capture(transaction, &mut changes);
                    }
                }
            }
        }
        Ok(changes)
    }

    fn apply_projection_inner(
        &mut self,
        projection: ProjectionEdit,
        transaction: &mut Transaction,
    ) -> Result<ChangeSet> {
        self.ensure_live(projection.slot().host())?;
        validate_projection_source(projection.source())?;
        let slot = projection.slot();
        let pending = PendingProjection {
            source: projection.source().clone(),
            mode: projection.mode(),
        };
        let mut changes = ChangeSet::new();
        let already_dirty = self.dirty_slots.contains(&slot);
        let equivalent_pending = self.pending_sources.get(&slot) == Some(&pending);
        let clean_equivalent_cache = !already_dirty
            && pending.mode != ProjectionReplaceMode::ResetIdentity
            && self
                .projection_caches
                .get(&slot)
                .is_some_and(|cache| cache.source == pending.source);
        if clean_equivalent_cache {
            return Ok(changes);
        }
        if !already_dirty || !equivalent_pending {
            changes.change_projection_slot(slot.clone());
            changes.change(slot.host(), ChangeFlags::empty().projection());
        }
        transaction.record_pending_source(self, &slot);
        transaction.record_dirty_slot(self, &slot);
        self.pending_sources.insert(slot.clone(), pending);
        self.dirty_slots.insert(slot);
        Ok(changes)
    }

    fn resolve_projection_inner(
        &mut self,
        slot: ProjectionSlot,
        transaction: &mut Transaction,
    ) -> Result<ChangeSet> {
        self.ensure_live(slot.host())?;
        if !self.dirty_slots.contains(&slot) && !self.pending_sources.contains_key(&slot) {
            return Ok(ChangeSet::new());
        }
        transaction.record_pending_source(self, &slot);
        let Some(pending) = self.pending_sources.remove(&slot) else {
            transaction.record_dirty_slot(self, &slot);
            self.dirty_slots.remove(&slot);
            return Ok(ChangeSet::new());
        };
        self.fail_at(Failpoint::AfterPendingSourceRemoval)?;

        let old_children = self
            .projection_caches
            .get(&slot)
            .map(|cache| cache.children.clone())
            .unwrap_or_default();
        transaction.record_projection_cache(self, &slot);
        let old_cache = self.projection_caches.remove(&slot);
        self.fail_at(Failpoint::AfterProjectionCacheRemoval)?;
        let mut changes = ChangeSet::new();
        let children = match &pending.source {
            ProjectionSource::Elements(elements) => self.resolve_element_source(
                &slot,
                &old_children,
                elements,
                pending.mode,
                transaction,
                &mut changes,
            )?,
            ProjectionSource::Virtual(virtual_projection) => self.resolve_virtual_source(
                &slot,
                &old_children,
                virtual_projection,
                pending.mode,
                transaction,
                &mut changes,
            )?,
        };

        if children != old_children {
            changes.change_projection_slot(slot.clone());
            changes.change(slot.host(), ChangeFlags::empty().projection());
        }

        self.fail_at(Failpoint::BeforeProjectionCacheInsert)?;
        self.projection_caches.insert(
            slot.clone(),
            ProjectionCache {
                source: pending.source.clone(),
                children,
                virtual_projection: match pending.source {
                    ProjectionSource::Virtual(virtual_projection) => Some(virtual_projection),
                    ProjectionSource::Elements(_) => None,
                },
            },
        );
        transaction.record_dirty_slot(self, &slot);
        self.dirty_slots.remove(&slot);
        drop(old_cache);
        Ok(changes)
    }

    fn resolve_element_source(
        &mut self,
        slot: &ProjectionSlot,
        old_children: &[Id],
        elements: &[Element],
        mode: ProjectionReplaceMode,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<Vec<Id>> {
        validate_duplicate_element_keys(elements)?;
        let mut used = BTreeSet::new();
        let mut resolved = Vec::new();
        for (index, element) in elements.iter().cloned().enumerate() {
            let key = element.key().cloned();
            let candidate =
                self.match_projected_child(old_children, index, key.as_ref(), element.kind(), mode);
            let id = if let Some(id) = candidate {
                used.insert(id);
                self.replace_projection_node(
                    ProjectionNodeUpdate {
                        id,
                        element,
                        slot,
                        index,
                        match_key: key.clone(),
                    },
                    transaction,
                    changes,
                )?;
                id
            } else {
                let key_path = self.projected_key_path(slot, index, key.as_ref())?;
                let id = self.alloc_node(
                    element,
                    Owner::Projection { slot: slot.clone() },
                    None,
                    key_path,
                    key,
                )?;
                transaction.record_node(self, id);
                self.node_mut(id)?.projected_parent = Some(slot.host());
                changes.insert(id);
                id
            };
            resolved.push(id);
        }
        for old in old_children {
            if !used.contains(old) && !resolved.contains(old) {
                self.remove_projection_subtree(*old, slot, transaction, changes)?;
                self.fail_at(Failpoint::AfterOldChildRemoval)?;
            }
        }
        Ok(resolved)
    }

    fn resolve_virtual_source(
        &mut self,
        slot: &ProjectionSlot,
        old_children: &[Id],
        virtual_projection: &VirtualProjection,
        mode: ProjectionReplaceMode,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<Vec<Id>> {
        let mut used = BTreeSet::new();
        let mut resolved = Vec::new();
        for item in virtual_projection.items() {
            let candidate = self.match_virtual_projected_child(
                old_children,
                item.key(),
                item.element().kind(),
                mode,
            );
            let element = item.element().clone();
            let id = if let Some(id) = candidate {
                used.insert(id);
                self.replace_projection_node(
                    ProjectionNodeUpdate {
                        id,
                        element,
                        slot,
                        index: item.index(),
                        match_key: Some(item.key().clone()),
                    },
                    transaction,
                    changes,
                )?;
                id
            } else {
                transaction.record_virtual_anchor(self, slot, item.key());
                if let Some(anchor) = self
                    .virtual_anchors
                    .remove(&(slot.clone(), item.key().clone()))
                {
                    self.fail_at(Failpoint::AfterVirtualAnchorRemoval)?;
                    let key_path = self
                        .node(slot.host())?
                        .key_path
                        .projection_slot(slot)
                        .virtual_item(item.key());
                    let id = self.alloc_node(
                        element,
                        Owner::Projection { slot: slot.clone() },
                        None,
                        key_path,
                        Some(item.key().clone()),
                    )?;
                    transaction.record_node(self, id);
                    self.node_mut(id)?.state = anchor;
                    transaction.record_node(self, id);
                    self.node_mut(id)?.projected_parent = Some(slot.host());
                    changes.insert(id);
                    id
                } else {
                    let key_path = self
                        .node(slot.host())?
                        .key_path
                        .projection_slot(slot)
                        .virtual_item(item.key());
                    let id = self.alloc_node(
                        element,
                        Owner::Projection { slot: slot.clone() },
                        None,
                        key_path,
                        Some(item.key().clone()),
                    )?;
                    transaction.record_node(self, id);
                    self.node_mut(id)?.projected_parent = Some(slot.host());
                    changes.insert(id);
                    id
                }
            };
            resolved.push(id);
        }
        for old in old_children {
            if !used.contains(old) && !resolved.contains(old) {
                if let Ok(node) = self.node(*old)
                    && let Some(key) = node.match_key.clone()
                {
                    transaction.record_virtual_anchor(self, slot, &key);
                    self.virtual_anchors
                        .insert((slot.clone(), key), node.state.durable_anchor());
                }
                self.remove_projection_subtree(*old, slot, transaction, changes)?;
                self.fail_at(Failpoint::AfterOldChildRemoval)?;
            }
        }
        Ok(resolved)
    }

    fn match_projected_child(
        &self,
        old_children: &[Id],
        index: usize,
        key: Option<&Key>,
        kind: &Kind,
        mode: ProjectionReplaceMode,
    ) -> Option<Id> {
        if mode == ProjectionReplaceMode::ResetIdentity {
            return None;
        }
        let candidate = key
            .and_then(|key| {
                old_children.iter().copied().find(|id| {
                    self.node(*id)
                        .map(|node| node.match_key.as_ref() == Some(key))
                        .unwrap_or(false)
                })
            })
            .or_else(|| old_children.get(index).copied());
        candidate.filter(|id| {
            mode == ProjectionReplaceMode::PreserveIdentity
                || self
                    .node(*id)
                    .map(|node| node.element.kind() == kind)
                    .unwrap_or(false)
        })
    }

    fn match_virtual_projected_child(
        &self,
        old_children: &[Id],
        key: &Key,
        kind: &Kind,
        mode: ProjectionReplaceMode,
    ) -> Option<Id> {
        if mode == ProjectionReplaceMode::ResetIdentity {
            return None;
        }
        old_children
            .iter()
            .copied()
            .find(|id| {
                self.node(*id)
                    .map(|node| node.match_key.as_ref() == Some(key))
                    .unwrap_or(false)
            })
            .filter(|id| {
                mode == ProjectionReplaceMode::PreserveIdentity
                    || self
                        .node(*id)
                        .map(|node| node.element.kind() == kind)
                        .unwrap_or(false)
            })
    }

    fn replace_projection_node(
        &mut self,
        update: ProjectionNodeUpdate<'_>,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        let ProjectionNodeUpdate {
            id,
            mut element,
            slot,
            index,
            match_key,
        } = update;
        let old_element = self.node(id)?.element.clone();
        let old_match_key = self.node(id)?.match_key.clone();
        let metadata_change = Self::projected_node_metadata_change(
            &old_element,
            old_match_key.as_ref(),
            &element,
            match_key.as_ref(),
        );
        let children = element.take_children();
        let key_path = if let Some(key) = &match_key {
            self.node(slot.host())?
                .key_path
                .projection_slot(slot)
                .projected_key(key)
        } else {
            self.node(slot.host())?
                .key_path
                .projection_slot(slot)
                .projected_index(index)
        };
        transaction.record_node(self, id);
        self.remove_children(id, transaction, changes)?;
        transaction.record_node(self, id);
        let node = self.node_mut(id)?;
        node.element = element;
        node.owner = Owner::Projection { slot: slot.clone() };
        node.parent = None;
        node.projected_parent = Some(slot.host());
        node.key_path = key_path;
        node.match_key = match_key;
        for (child_index, child) in children.into_iter().enumerate() {
            let child_key_path = self.child_key_path(id, &child, child_index)?;
            let child_match_key = child.key().cloned();
            let child_id = self.alloc_node(
                child,
                Owner::Canonical { parent: id },
                Some(id),
                child_key_path,
                child_match_key,
            )?;
            transaction.record_node(self, id);
            self.node_mut(id)?.children.push(child_id);
            changes.insert(child_id);
        }
        let mut flags = ChangeFlags::empty().projection();
        if metadata_change.has_kind() {
            flags = flags.kind();
        }
        changes.change(id, flags);
        Self::invalidate_selector_metadata(changes, id, metadata_change);
        if metadata_change.has_kind() {
            Self::invalidate_projected_selector_siblings(changes, slot.host());
        }
        self.fail_at(Failpoint::AfterProjectedChildReuse)?;
        Ok(())
    }

    fn projected_key_path(
        &self,
        slot: &ProjectionSlot,
        index: usize,
        key: Option<&Key>,
    ) -> Result<KeyPath> {
        let host_path = self.node(slot.host())?.key_path.projection_slot(slot);
        Ok(match key {
            Some(key) => host_path.projected_key(key),
            None => host_path.projected_index(index),
        })
    }

    fn remove_children(
        &mut self,
        id: Id,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        let children = self.node(id)?.children.clone();
        for child in children {
            self.remove_subtree(child, transaction, changes)?;
        }
        transaction.record_node(self, id);
        self.node_mut(id)?.children.clear();
        Ok(())
    }

    fn remove_subtree(
        &mut self,
        id: Id,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        let children = self.node(id)?.children.clone();
        for child in children {
            self.remove_subtree(child, transaction, changes)?;
        }
        self.remove_hosted_projection_slots(id, transaction, changes)?;
        transaction.record_node(self, id);
        self.nodes[id.index()] = None;
        changes.remove(id);
        if self.focus == Some(id) {
            transaction.record_focus(self);
            self.focus = None;
            self.recompute_focus_within(transaction, changes);
        }
        transaction.record_pointer_captures(self);
        self.pointer_captures.retain(|_, target| *target != id);
        Ok(())
    }

    fn remove_hosted_projection_slots(
        &mut self,
        host: Id,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        let slots: BTreeSet<_> = self
            .projection_caches
            .keys()
            .chain(self.pending_sources.keys())
            .chain(self.dirty_slots.iter())
            .filter(|slot| slot.host() == host)
            .cloned()
            .collect();

        for slot in slots {
            transaction.record_projection_cache(self, &slot);
            if let Some(cache) = self.projection_caches.remove(&slot) {
                for child in cache.children {
                    if self.node(child).is_ok() {
                        self.remove_projection_subtree(child, &slot, transaction, changes)?;
                    }
                }
            }
            transaction.record_pending_source(self, &slot);
            self.pending_sources.remove(&slot);
            transaction.record_dirty_slot(self, &slot);
            self.dirty_slots.remove(&slot);
            transaction.record_virtual_anchors_for_slot(self, &slot);
            self.virtual_anchors
                .retain(|(anchor_slot, _), _| anchor_slot != &slot);
        }
        Ok(())
    }

    fn remove_projection_subtree(
        &mut self,
        id: Id,
        _slot: &ProjectionSlot,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) -> Result<()> {
        self.remove_subtree(id, transaction, changes)
    }

    fn refresh_child_key_paths(&mut self, parent: Id, transaction: &mut Transaction) -> Result<()> {
        let children = self.node(parent)?.children.clone();
        for (index, child) in children.into_iter().enumerate() {
            let element = self.node(child)?.element.clone();
            let key_path = self.child_key_path(parent, &element, index)?;
            self.refresh_subtree_key_paths(child, key_path, transaction)?;
        }
        Ok(())
    }

    fn refresh_subtree_key_paths(
        &mut self,
        id: Id,
        key_path: KeyPath,
        transaction: &mut Transaction,
    ) -> Result<()> {
        transaction.record_node(self, id);
        self.node_mut(id)?.key_path = key_path;
        let children = self.node(id)?.children.clone();
        for (index, child) in children.into_iter().enumerate() {
            let element = self.node(child)?.element.clone();
            let child_key_path = self.child_key_path(id, &element, index)?;
            self.refresh_subtree_key_paths(child, child_key_path, transaction)?;
        }
        Ok(())
    }

    fn effective_projected_parent(&self, id: Id) -> Result<Option<Id>> {
        let node = self.node(id)?;
        Ok(node.projected_parent.or(node.parent))
    }

    fn ensure_projected_edge_resolved(&self, id: Id) -> Result<()> {
        match &self.node(id)?.owner {
            Owner::Canonical { parent } => {
                let slot = ProjectionSlot::default(*parent);
                if self.dirty_slots.contains(&slot) {
                    return Err(Error::new(
                        ErrorCode::UnresolvedProjection,
                        "canonical default projection is dirty",
                    ));
                }
            }
            Owner::Projection { slot } if self.dirty_slots.contains(slot) => {
                return Err(Error::new(
                    ErrorCode::UnresolvedProjection,
                    "projection-owned node belongs to a dirty slot",
                ));
            }
            Owner::Root | Owner::Projection { .. } => {}
        }
        Ok(())
    }

    fn release_invalid_focus_and_capture(
        &mut self,
        transaction: &mut Transaction,
        changes: &mut ChangeSet,
    ) {
        let mut cleared_focus = false;
        if let Some(focused) = self.focus
            && !self.is_input_eligible(focused).unwrap_or(false)
        {
            transaction.record_focus(self);
            self.focus = None;
            cleared_focus = true;
            if self.node(focused).is_ok() {
                transaction.record_node(self, focused);
                self.node_mut(focused).expect("node checked").state.focused = false;
                changes.change(focused, ChangeFlags::empty().state().focus());
                Self::invalidate_selector_state(changes, focused);
            }
        }
        if cleared_focus {
            self.recompute_focus_within(transaction, changes);
        }
        let captures: Vec<_> = self
            .pointer_captures
            .iter()
            .filter_map(|(pointer, target)| {
                (!self.is_input_eligible(*target).unwrap_or(false)).then_some((*pointer, *target))
            })
            .collect();
        for (pointer, _) in captures {
            let Ok(release_changes) = self.release_pointer_inner(pointer, transaction) else {
                continue;
            };
            changes.merge(release_changes);
        }
    }

    fn recompute_focus_within(&mut self, transaction: &mut Transaction, changes: &mut ChangeSet) {
        let focused_ancestors: BTreeSet<_> = self
            .focus
            .and_then(|focused| self.projected_ancestors_vec(focused).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();
        let ids: Vec<_> = self.all_ids().collect();
        for id in ids {
            let should_focus_within = focused_ancestors.contains(&id);
            let Ok(node) = self.node(id) else {
                continue;
            };
            if node.state.focus_within != should_focus_within {
                transaction.record_node(self, id);
                self.node_mut(id).expect("node checked").state.focus_within = should_focus_within;
                changes.change(id, ChangeFlags::empty().state());
                Self::invalidate_selector_state(changes, id);
            }
        }
    }
}

struct ProjectionNodeUpdate<'a> {
    id: Id,
    element: Element,
    slot: &'a ProjectionSlot,
    index: usize,
    match_key: Option<Key>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failpoint {
    AfterPendingSourceRemoval,
    AfterProjectionCacheRemoval,
    AfterProjectedChildReuse,
    AfterVirtualAnchorRemoval,
    AfterOldChildRemoval,
    BeforeProjectionCacheInsert,
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub id: Id,
    pub element: Element,
    pub owner: Owner,
    pub parent: Option<Id>,
    pub children: Vec<Id>,
    pub projected_parent: Option<Id>,
    pub state: State,
    pub key_path: KeyPath,
    pub match_key: Option<Key>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Owner {
    Root,
    Canonical { parent: Id },
    Projection { slot: ProjectionSlot },
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectionCache {
    pub(crate) source: ProjectionSource,
    pub(crate) children: Vec<Id>,
    pub(crate) virtual_projection: Option<VirtualProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingProjection {
    pub(crate) source: ProjectionSource,
    pub(crate) mode: ProjectionReplaceMode,
}

fn validate_projection_source(source: &ProjectionSource) -> Result<()> {
    match source {
        ProjectionSource::Elements(elements) => validate_duplicate_element_keys(elements),
        ProjectionSource::Virtual(_) => Ok(()),
    }
}

fn validate_duplicate_element_keys(elements: &[Element]) -> Result<()> {
    let mut keys = BTreeSet::new();
    for element in elements {
        element.validate()?;
        if let Some(key) = element.key()
            && !keys.insert(key.clone())
        {
            return Err(Error::new(
                ErrorCode::DuplicateKey,
                format!("duplicate projected key `{key}`"),
            ));
        }
    }
    Ok(())
}
