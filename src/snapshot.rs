use super::{
    Attribute, Class, Hook, Id, Key, KeyPath, Kind, Model, Presence, ProjectionSlot, Result, Role,
    State, Text, VirtualProjection,
};

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
    pub fn revision(&self) -> u64 {
        self.model.revision()
    }

    #[must_use]
    pub fn get(&self, id: Id) -> Option<NodeRef<'_>> {
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
