use super::{
    Attribute, AttributeName, Element, Hook, Id, Kind, ProjectionEdit, ReplaceMode, Role,
    StatePatch, Text, Value,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Mutation {
    edits: Vec<MutationEdit>,
}

impl Mutation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, edit: MutationEdit) {
        self.edits.push(edit);
    }

    #[must_use]
    pub fn with(edit: MutationEdit) -> Self {
        Self { edits: vec![edit] }
    }

    #[must_use]
    pub fn edits(&self) -> &[MutationEdit] {
        &self.edits
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationEdit {
    Projection(ProjectionEdit),
    Patch(Patch),
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Patch {
    Insert {
        parent: Id,
        index: usize,
        element: Element,
    },
    Replace {
        id: Id,
        element: Element,
        mode: ReplaceMode,
    },
    Remove {
        id: Id,
    },
    Move {
        id: Id,
        parent: Id,
        index: usize,
    },
    ReorderChildren {
        parent: Id,
        children: Vec<Id>,
    },
    SetKind {
        id: Id,
        kind: Kind,
    },
    SetRole {
        id: Id,
        role: Role,
    },
    SetLabel {
        id: Id,
        label: Option<Text>,
    },
    SetClasses {
        id: Id,
        classes: Vec<super::Class>,
    },
    SetAttribute {
        id: Id,
        name: AttributeName,
        value: Value,
    },
    RemoveAttribute {
        id: Id,
        name: AttributeName,
    },
    SetText {
        id: Id,
        text: Option<Text>,
    },
    SetHooks {
        id: Id,
        hooks: Vec<Hook>,
    },
    SetState {
        id: Id,
        state: StatePatch,
    },
}

impl From<ProjectionEdit> for MutationEdit {
    fn from(edit: ProjectionEdit) -> Self {
        Self::Projection(edit)
    }
}

impl From<Patch> for MutationEdit {
    fn from(patch: Patch) -> Self {
        Self::Patch(patch)
    }
}

impl From<(AttributeName, Value)> for Attribute {
    fn from((name, value): (AttributeName, Value)) -> Self {
        Self { name, value }
    }
}
