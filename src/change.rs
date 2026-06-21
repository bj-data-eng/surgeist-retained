use std::collections::BTreeMap;

use super::{Command, Id, ProjectionSlot};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChangeFlags {
    structure: bool,
    kind: bool,
    role: bool,
    label: bool,
    classes: bool,
    attributes: bool,
    text: bool,
    hooks: bool,
    presence: bool,
    state: bool,
    focus: bool,
    pointer_capture: bool,
    projection: bool,
}

impl ChangeFlags {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            structure: false,
            kind: false,
            role: false,
            label: false,
            classes: false,
            attributes: false,
            text: false,
            hooks: false,
            presence: false,
            state: false,
            focus: false,
            pointer_capture: false,
            projection: false,
        }
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::empty()
    }

    #[must_use]
    pub const fn structure(mut self) -> Self {
        self.structure = true;
        self
    }

    #[must_use]
    pub const fn kind(mut self) -> Self {
        self.kind = true;
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
    pub const fn hooks(mut self) -> Self {
        self.hooks = true;
        self
    }

    #[must_use]
    pub const fn presence(mut self) -> Self {
        self.presence = true;
        self
    }

    #[must_use]
    pub const fn state(mut self) -> Self {
        self.state = true;
        self
    }

    #[must_use]
    pub const fn focus(mut self) -> Self {
        self.focus = true;
        self
    }

    #[must_use]
    pub const fn pointer_capture(mut self) -> Self {
        self.pointer_capture = true;
        self
    }

    #[must_use]
    pub const fn projection(mut self) -> Self {
        self.projection = true;
        self
    }

    #[must_use]
    pub const fn has_structure(self) -> bool {
        self.structure
    }

    #[must_use]
    pub const fn has_kind(self) -> bool {
        self.kind
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

    #[must_use]
    pub const fn has_hooks(self) -> bool {
        self.hooks
    }

    #[must_use]
    pub const fn has_presence(self) -> bool {
        self.presence
    }

    #[must_use]
    pub const fn has_projection(self) -> bool {
        self.projection
    }

    #[must_use]
    pub const fn has_state(self) -> bool {
        self.state
    }

    #[must_use]
    pub const fn has_focus(self) -> bool {
        self.focus
    }

    #[must_use]
    pub const fn has_pointer_capture(self) -> bool {
        self.pointer_capture
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.structure |= other.structure;
        self.kind |= other.kind;
        self.role |= other.role;
        self.label |= other.label;
        self.classes |= other.classes;
        self.attributes |= other.attributes;
        self.text |= other.text;
        self.hooks |= other.hooks;
        self.presence |= other.presence;
        self.state |= other.state;
        self.focus |= other.focus;
        self.pointer_capture |= other.pointer_capture;
        self.projection |= other.projection;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChangeSet {
    inserted: Vec<Id>,
    removed: Vec<Id>,
    moved: Vec<Id>,
    changed: BTreeMap<Id, ChangeFlags>,
    projection_slots: Vec<ProjectionSlot>,
}

impl ChangeSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty()
            && self.removed.is_empty()
            && self.moved.is_empty()
            && self.changed.is_empty()
            && self.projection_slots.is_empty()
    }

    #[must_use]
    pub fn inserted(&self) -> &[Id] {
        &self.inserted
    }

    #[must_use]
    pub fn removed(&self) -> &[Id] {
        &self.removed
    }

    #[must_use]
    pub fn moved(&self) -> &[Id] {
        &self.moved
    }

    pub fn changed(&self) -> impl Iterator<Item = (Id, ChangeFlags)> + '_ {
        self.changed.iter().map(|(id, flags)| (*id, *flags))
    }

    #[must_use]
    pub fn changed_projection_slots(&self) -> &[ProjectionSlot] {
        &self.projection_slots
    }

    pub(crate) fn insert(&mut self, id: Id) {
        push_unique(&mut self.inserted, id);
    }

    pub(crate) fn remove(&mut self, id: Id) {
        push_unique(&mut self.removed, id);
    }

    pub(crate) fn move_node(&mut self, id: Id) {
        push_unique(&mut self.moved, id);
    }

    pub(crate) fn change(&mut self, id: Id, flags: ChangeFlags) {
        if flags.is_empty() {
            return;
        }
        self.changed.entry(id).or_default().merge(flags);
    }

    pub(crate) fn change_projection_slot(&mut self, slot: ProjectionSlot) {
        push_unique(&mut self.projection_slots, slot);
    }

    pub(crate) fn merge(&mut self, other: Self) {
        for id in other.inserted {
            self.insert(id);
        }
        for id in other.removed {
            self.remove(id);
        }
        for id in other.moved {
            self.move_node(id);
        }
        for (id, flags) in other.changed {
            self.change(id, flags);
        }
        for slot in other.projection_slots {
            self.change_projection_slot(slot);
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Report {
    changes: ChangeSet,
    commands: Vec<Command>,
}

impl Report {
    #[must_use]
    pub fn new(changes: ChangeSet) -> Self {
        Self {
            changes,
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_commands(changes: ChangeSet, commands: Vec<Command>) -> Self {
        Self { changes, commands }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }

    #[must_use]
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
}

fn push_unique<T: Clone + Eq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}
