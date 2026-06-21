use std::collections::{BTreeMap, BTreeSet};

use super::{
    ChangeSet, Command, Id, Key, Model, PointerId, ProjectionSlot, Report, State,
    model::{Node, PendingProjection, ProjectionCache},
};

#[derive(Debug)]
pub(crate) struct Transaction {
    journal: Journal,
    changes: ChangeSet,
    commands: Vec<Command>,
}

impl Transaction {
    pub(crate) fn new(model: &Model) -> Self {
        let mut journal = Journal::default();
        journal.entries.push(Undo::Allocation {
            nodes_len: model.nodes.len(),
        });
        Self {
            journal,
            changes: ChangeSet::new(),
            commands: Vec::new(),
        }
    }

    pub(crate) fn merge_changes(&mut self, changes: ChangeSet) {
        self.changes.merge(changes);
    }

    pub(crate) fn push_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub(crate) fn record_node(&mut self, model: &Model, id: Id) {
        if self.journal.nodes.insert(id.index()) {
            self.journal.entries.push(Undo::Node {
                index: id.index(),
                node: model.nodes.get(id.index()).cloned().flatten(),
            });
        }
    }

    pub(crate) fn record_projection_cache(&mut self, model: &Model, slot: &ProjectionSlot) {
        if self.journal.projection_caches.insert(slot.clone()) {
            self.journal.entries.push(Undo::ProjectionCache {
                slot: slot.clone(),
                cache: model.projection_caches.get(slot).cloned(),
            });
        }
    }

    pub(crate) fn record_pending_source(&mut self, model: &Model, slot: &ProjectionSlot) {
        if self.journal.pending_sources.insert(slot.clone()) {
            self.journal.entries.push(Undo::PendingSource {
                slot: slot.clone(),
                source: model.pending_sources.get(slot).cloned(),
            });
        }
    }

    pub(crate) fn record_dirty_slot(&mut self, model: &Model, slot: &ProjectionSlot) {
        if self.journal.dirty_slots.insert(slot.clone()) {
            self.journal.entries.push(Undo::DirtySlot {
                slot: slot.clone(),
                was_dirty: model.dirty_slots.contains(slot),
            });
        }
    }

    pub(crate) fn record_virtual_anchor(
        &mut self,
        model: &Model,
        slot: &ProjectionSlot,
        key: &Key,
    ) {
        let anchor = (slot.clone(), key.clone());
        if self.journal.virtual_anchors.insert(anchor.clone()) {
            self.journal.entries.push(Undo::VirtualAnchor {
                slot: anchor.0.clone(),
                key: anchor.1.clone(),
                state: model.virtual_anchors.get(&anchor).cloned(),
            });
        }
    }

    pub(crate) fn record_virtual_anchors_for_slot(&mut self, model: &Model, slot: &ProjectionSlot) {
        let keys: Vec<_> = model
            .virtual_anchors
            .keys()
            .filter_map(|(anchor_slot, key)| (anchor_slot == slot).then_some(key.clone()))
            .collect();
        for key in keys {
            self.record_virtual_anchor(model, slot, &key);
        }
    }

    pub(crate) fn record_focus(&mut self, model: &Model) {
        if !self.journal.focus {
            self.journal.focus = true;
            self.journal
                .entries
                .push(Undo::Focus { focus: model.focus });
        }
    }

    pub(crate) fn record_pointer_captures(&mut self, model: &Model) {
        if !self.journal.pointer_captures {
            self.journal.pointer_captures = true;
            self.journal.entries.push(Undo::PointerCaptures {
                captures: model.pointer_captures.clone(),
            });
        }
    }

    pub(crate) fn commit(self, model: &mut Model) -> Report {
        if !self.changes.is_empty() || self.dirty_slot_state_changed(model) {
            model.revision += 1;
        }
        model.changes.merge(self.changes.clone());
        Report::with_commands(self.changes, self.commands)
    }

    fn dirty_slot_state_changed(&self, model: &Model) -> bool {
        self.journal.entries.iter().any(|entry| {
            matches!(
                entry,
                Undo::DirtySlot { slot, was_dirty }
                    if model.dirty_slots.contains(slot) != *was_dirty
            )
        })
    }

    pub(crate) fn rollback(self, model: &mut Model) {
        for entry in self.journal.entries.into_iter().rev() {
            match entry {
                Undo::Allocation { nodes_len } => model.nodes.truncate(nodes_len),
                Undo::Node { index, node } => {
                    if index >= model.nodes.len() {
                        model.nodes.resize_with(index + 1, || None);
                    }
                    model.nodes[index] = node;
                }
                Undo::ProjectionCache { slot, cache } => match cache {
                    Some(cache) => {
                        model.projection_caches.insert(slot, cache);
                    }
                    None => {
                        model.projection_caches.remove(&slot);
                    }
                },
                Undo::PendingSource { slot, source } => match source {
                    Some(source) => {
                        model.pending_sources.insert(slot, source);
                    }
                    None => {
                        model.pending_sources.remove(&slot);
                    }
                },
                Undo::DirtySlot { slot, was_dirty } => {
                    if was_dirty {
                        model.dirty_slots.insert(slot);
                    } else {
                        model.dirty_slots.remove(&slot);
                    }
                }
                Undo::VirtualAnchor { slot, key, state } => {
                    let anchor = (slot, key);
                    match state {
                        Some(state) => {
                            model.virtual_anchors.insert(anchor, state);
                        }
                        None => {
                            model.virtual_anchors.remove(&anchor);
                        }
                    }
                }
                Undo::Focus { focus } => model.focus = focus,
                Undo::PointerCaptures { captures } => model.pointer_captures = captures,
            }
        }
    }
}

#[derive(Debug, Default)]
struct Journal {
    entries: Vec<Undo>,
    nodes: BTreeSet<usize>,
    projection_caches: BTreeSet<ProjectionSlot>,
    pending_sources: BTreeSet<ProjectionSlot>,
    dirty_slots: BTreeSet<ProjectionSlot>,
    virtual_anchors: BTreeSet<(ProjectionSlot, Key)>,
    focus: bool,
    pointer_captures: bool,
}

#[derive(Debug)]
enum Undo {
    Allocation {
        nodes_len: usize,
    },
    Node {
        index: usize,
        node: Option<Node>,
    },
    ProjectionCache {
        slot: ProjectionSlot,
        cache: Option<ProjectionCache>,
    },
    PendingSource {
        slot: ProjectionSlot,
        source: Option<PendingProjection>,
    },
    DirtySlot {
        slot: ProjectionSlot,
        was_dirty: bool,
    },
    VirtualAnchor {
        slot: ProjectionSlot,
        key: Key,
        state: Option<State>,
    },
    Focus {
        focus: Option<Id>,
    },
    PointerCaptures {
        captures: BTreeMap<PointerId, Id>,
    },
}
