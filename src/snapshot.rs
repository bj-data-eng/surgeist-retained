use super::{
    Attribute, Class, Hook, Id, Key, KeyPath, Kind, Model, Presence, ProjectionSlot, Result, Role,
    State, Text, VirtualProjection,
};

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
    #[allow(clippy::too_many_arguments)]
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

#[derive(Clone, Copy, Debug)]
pub struct Snapshot<'a> {
    model: &'a Model,
}

impl<'a> Snapshot<'a> {
    #[must_use]
    pub(crate) const fn new(model: &'a Model) -> Self {
        Self { model }
    }

    #[must_use]
    pub fn root(&self) -> Id {
        self.model.root()
    }

    #[must_use]
    pub fn revision(&self) -> super::ModelRevision {
        self.model.revision()
    }

    #[must_use]
    pub fn get(&self, id: Id) -> Option<NodeRef<'a>> {
        self.model.node(id).ok().map(|node| NodeRef { node })
    }

    #[must_use]
    pub fn find_key(&self, key_path: &KeyPath) -> Option<Id> {
        self.model.key_lookup(key_path)
    }

    pub fn children(&self, id: Id) -> Result<impl Iterator<Item = Id> + '_> {
        Ok(self.model.canonical_children(id)?.into_iter())
    }

    pub fn projected_children(
        &self,
        slot: ProjectionSlot,
    ) -> Result<impl Iterator<Item = Id> + '_> {
        Ok(self.model.projected_children(slot)?.into_iter())
    }

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

    pub fn selector_sibling_facts(
        &self,
        id: Id,
        traversal: SelectorTraversal,
    ) -> Result<SelectorSiblingFacts> {
        self.model.selector_sibling_facts(id, traversal)
    }

    #[must_use]
    pub fn selector_metadata(&self, id: Id) -> Option<SelectorMetadata<'a>> {
        self.get(id).map(|node| SelectorMetadata { node })
    }

    pub fn ancestors(&self, id: Id) -> Result<impl Iterator<Item = Id> + '_> {
        Ok(self.model.ancestors_vec(id)?.into_iter())
    }

    pub fn projected_ancestors(&self, id: Id) -> Result<impl Iterator<Item = Id> + '_> {
        Ok(self.model.projected_ancestors_vec(id)?.into_iter())
    }

    pub fn descendants(&self, id: Id) -> Result<impl Iterator<Item = Id> + '_> {
        Ok(self.model.descendants_vec(id)?.into_iter())
    }

    pub fn virtual_projection(&self, slot: ProjectionSlot) -> Result<Option<&VirtualProjection>> {
        self.model.cached_virtual_projection(slot)
    }

    pub fn effective_presence(&self, id: Id) -> Result<Presence> {
        self.model.effective_presence(id)
    }

    pub fn is_input_eligible(&self, id: Id) -> Result<bool> {
        self.model.is_input_eligible(id)
    }

    pub fn by_class(&self, class: &Class) -> impl Iterator<Item = Id> + '_ {
        let ids: Vec<_> = self
            .model
            .all_ids()
            .filter(|id| {
                self.model
                    .node(*id)
                    .map(|node| node.element.classes().contains(class))
                    .unwrap_or(false)
            })
            .collect();
        ids.into_iter()
    }

    pub fn by_role(&self, role: Role) -> impl Iterator<Item = Id> + '_ {
        let ids: Vec<_> = self
            .model
            .all_ids()
            .filter(|id| {
                self.model
                    .node(*id)
                    .map(|node| node.element.role() == role)
                    .unwrap_or(false)
            })
            .collect();
        ids.into_iter()
    }

    pub fn hooks(&self, id: Id) -> Result<&[Hook]> {
        Ok(self.model.node(id)?.element.hooks())
    }

    pub fn dirty_slots(&self) -> impl Iterator<Item = ProjectionSlot> + '_ {
        self.model.dirty_slots()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NodeRef<'a> {
    pub(crate) node: &'a super::model::Node,
}

impl<'a> NodeRef<'a> {
    #[must_use]
    pub fn id(&self) -> Id {
        self.node.id
    }

    #[must_use]
    pub fn key(&self) -> Option<&'a Key> {
        self.node.element.key()
    }

    #[must_use]
    pub fn kind(&self) -> &'a Kind {
        self.node.element.kind()
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.node.element.role()
    }

    #[must_use]
    pub fn label(&self) -> Option<&'a Text> {
        self.node.element.label()
    }

    #[must_use]
    pub fn attributes(&self) -> &'a [Attribute] {
        self.node.element.attributes()
    }

    #[must_use]
    pub fn classes(&self) -> &'a [Class] {
        self.node.element.classes()
    }

    #[must_use]
    pub fn text(&self) -> Option<&'a Text> {
        self.node.element.text_content()
    }

    #[must_use]
    pub fn state(&self) -> &'a State {
        &self.node.state
    }

    #[must_use]
    pub fn key_path(&self) -> &'a KeyPath {
        &self.node.key_path
    }

    #[must_use]
    pub fn parent(&self) -> Option<Id> {
        self.node.parent
    }

    #[must_use]
    pub fn projected_parent(&self) -> Option<Id> {
        self.node.projected_parent
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
