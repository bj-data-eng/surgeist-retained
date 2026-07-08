use super::model::Failpoint;
use super::*;

fn key(value: &str) -> Key {
    Key::new(value).unwrap()
}

fn tag(value: &str) -> Tag {
    Tag::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn attr_name(value: &str) -> AttributeName {
    AttributeName::new(value).unwrap()
}

fn value(value: &str) -> Value {
    Value::new(value).unwrap()
}

fn element(name: &str, key_value: &str) -> Element {
    Element::tagged(tag(name)).with_key(key(key_value))
}

fn projection_child_keys(model: &Model, slot: ProjectionSlot) -> Vec<Option<Key>> {
    model
        .snapshot()
        .projected_children(slot)
        .unwrap()
        .map(|id| model.node(id).unwrap().match_key.clone())
        .collect()
}

fn projection_cache_child_keys(model: &Model, slot: &ProjectionSlot) -> Vec<Option<Key>> {
    model
        .projection_caches
        .get(slot)
        .unwrap()
        .children
        .iter()
        .map(|id| model.node(*id).unwrap().match_key.clone())
        .collect()
}

fn virtual_projection(start: usize, len: usize) -> VirtualProjection {
    let items = (start..start + len)
        .map(|index| {
            VirtualItem::new(
                index,
                key(&format!("row-{index}")),
                Element::tagged(tag("row")).with_text(text(&format!("Row {index}"))),
            )
            .unwrap()
        })
        .collect();
    VirtualProjection::dense(
        200_000,
        VirtualRange::new(start, start + len).unwrap(),
        items,
    )
    .unwrap()
}

#[test]
fn invalid_strings_fail_before_storage() {
    assert_eq!(
        Tag::new("bad tag").unwrap_err().code(),
        ErrorCode::InvalidString
    );
    assert_eq!(
        CommandName::new("   ").unwrap_err().code(),
        ErrorCode::EmptyCommand
    );
    assert!(Text::new("hello\nworld").is_ok());
}

#[test]
fn new_model_validates_duplicate_sibling_keys() {
    let root = Element::root()
        .with_child(element("div", "same"))
        .with_child(element("span", "same"));
    assert_eq!(
        Model::new(root).unwrap_err().code(),
        ErrorCode::DuplicateKey
    );
}

#[test]
fn canonical_patch_inserts_and_snapshots_children() {
    let mut model = Model::empty();
    let root = model.root();
    let report = model
        .apply(Patch::Insert {
            parent: root,
            index: 0,
            element: element("section", "main"),
        })
        .unwrap();
    assert_eq!(report.changes().inserted().len(), 1);
    let child = report.changes().inserted()[0];
    let snapshot = model.snapshot();
    assert_eq!(
        snapshot.children(root).unwrap().collect::<Vec<_>>(),
        vec![child]
    );
    assert_eq!(snapshot.get(child).unwrap().key(), Some(&key("main")));
}

#[test]
fn mutation_is_atomic_on_error() {
    let mut model = Model::empty();
    let root = model.root();
    let missing = Id::new(99, 1);
    let mut mutation = Mutation::new();
    mutation.push(
        Patch::Insert {
            parent: root,
            index: 0,
            element: element("section", "ok"),
        }
        .into(),
    );
    mutation.push(
        Patch::Insert {
            parent: missing,
            index: 0,
            element: element("section", "bad"),
        }
        .into(),
    );
    assert!(model.mutate(mutation).is_err());
    assert!(
        model
            .snapshot()
            .children(root)
            .unwrap()
            .collect::<Vec<_>>()
            .is_empty()
    );
}

#[test]
fn canonical_patches_reject_duplicate_sibling_keys_and_update_key_paths() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("row", "a"))
            .with_child(element("row", "b")),
    )
    .unwrap();
    let root = model.root();
    let children = model.snapshot().children(root).unwrap().collect::<Vec<_>>();
    let first = children[0];
    let second = children[1];

    assert_eq!(
        model
            .apply(Patch::Insert {
                parent: root,
                index: 2,
                element: element("row", "a"),
            })
            .unwrap_err()
            .code(),
        ErrorCode::DuplicateKey
    );
    assert_eq!(
        model
            .apply(Patch::Replace {
                id: second,
                element: element("row", "a"),
                mode: ReplaceMode::PreserveCompatible,
            })
            .unwrap_err()
            .code(),
        ErrorCode::DuplicateKey
    );

    model
        .apply(Patch::Replace {
            id: second,
            element: element("row", "c"),
            mode: ReplaceMode::PreserveCompatible,
        })
        .unwrap();
    let snapshot = model.snapshot();
    assert_eq!(
        snapshot.find_key(&KeyPath::root().canonical_key(&key("b"))),
        None
    );
    assert_eq!(
        snapshot.find_key(&KeyPath::root().canonical_key(&key("c"))),
        Some(second)
    );

    model
        .apply(Patch::Move {
            id: first,
            parent: root,
            index: 2,
        })
        .unwrap();
    assert_eq!(
        model.snapshot().children(root).unwrap().collect::<Vec<_>>(),
        vec![second, first]
    );
}

#[test]
fn canonical_replace_mode_names_canonical_semantics() {
    let mut model = Model::new(Element::root().with_child(element("button", "run"))).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();

    assert_eq!(
        model
            .apply(Patch::Replace {
                id: target,
                element: element("section", "run"),
                mode: ReplaceMode::PreserveCompatible,
            })
            .unwrap_err()
            .code(),
        ErrorCode::InvalidPatch
    );

    model
        .apply(Patch::Replace {
            id: target,
            element: element("button", "run").with_text(text("updated")),
            mode: ReplaceMode::AllowKindChange,
        })
        .unwrap();

    assert_eq!(
        model.snapshot().get(target).unwrap().text(),
        Some(&text("updated"))
    );

    model
        .apply(Patch::Replace {
            id: target,
            element: element("section", "run").with_text(text("section")),
            mode: ReplaceMode::AllowKindChange,
        })
        .unwrap();

    assert_eq!(
        model.snapshot().get(target).unwrap().kind(),
        &Kind::Element(tag("section"))
    );
}

#[test]
fn projection_updates_projected_children_without_rewriting_canonical_children() {
    let mut model = Model::empty();
    let root = model.root();
    let slot = ProjectionSlot::default(root);
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "run")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    assert_eq!(
        model
            .snapshot()
            .projected_children(slot.clone())
            .err()
            .unwrap()
            .code(),
        ErrorCode::UnresolvedProjection
    );
    let report = model.resolve_projection(slot.clone()).unwrap();
    let projected = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(projected.len(), 1);
    assert!(
        model
            .snapshot()
            .children(root)
            .unwrap()
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(report.changes().changed_projection_slots().contains(&slot));
}

#[test]
fn selector_traversal_policy_names_canonical_and_projected_trees() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("host", "host").with_child(element("button", "canonical"))),
    )
    .unwrap();
    let host = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();
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

#[test]
fn dirty_projection_blocks_stale_projected_routes() {
    let command = CommandName::new("project.run").unwrap();
    let button =
        element("button", "run").with_hook(Hook::new(Trigger::Event(EventKind::Click), command));
    let mut model = Model::new(Element::root().with_child(button)).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();
    let slot = ProjectionSlot::default(model.root());
    model
        .apply_projection(ProjectionEdit::new(
            slot,
            ProjectionSource::Elements(vec![element("button", "projected")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();

    let projected_ancestors_error = match model.snapshot().projected_ancestors(target) {
        Ok(_) => panic!("dirty projection should block projected ancestor traversal"),
        Err(error) => error,
    };
    assert_eq!(
        projected_ancestors_error.code(),
        ErrorCode::UnresolvedProjection
    );
    assert_eq!(
        model
            .route(Event::new(target, EventKind::Click))
            .unwrap_err()
            .code(),
        ErrorCode::UnresolvedProjection
    );
}

#[test]
fn removing_projection_host_removes_projection_owned_children() {
    let mut model = Model::new(Element::root().with_child(element("panel", "host"))).unwrap();
    let host = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();
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

    model.apply(Patch::Remove { id: host }).unwrap();
    let snapshot = model.snapshot();
    assert!(snapshot.get(host).is_none());
    assert!(snapshot.get(projected).is_none());
    assert!(snapshot.dirty_slots().all(|dirty| dirty != slot));
}

#[test]
fn equivalent_projection_preserves_identity_and_reports_no_changes() {
    let mut model = Model::empty();
    let root = model.root();
    let slot = ProjectionSlot::default(root);
    let source = ProjectionSource::Elements(vec![element("button", "run")]);
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            source.clone(),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let first = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    let apply_report = model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            source,
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    assert!(apply_report.changes().is_empty());
    let report = model.resolve_projection(slot.clone()).unwrap();
    let second = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(first, second);
    assert!(report.changes().is_empty());
}

#[test]
fn projection_replace_modes_control_identity_preservation() {
    let mut model = Model::empty();
    let root = model.root();
    let slot = ProjectionSlot::default(root);
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "run")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let first = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("section", "run")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let kind_changed = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    assert_ne!(first, kind_changed);

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("article", "run")]),
            ProjectionReplaceMode::PreserveIdentity,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let preserved = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(kind_changed, preserved);

    model
        .apply(Patch::SetState {
            id: preserved,
            state: StatePatch::new().selected(true),
        })
        .unwrap();
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("section", "run")]),
            ProjectionReplaceMode::ResetIdentity,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let reset = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    assert_ne!(preserved, reset);
    assert!(!model.snapshot().get(reset).unwrap().state().selected());
}

#[test]
fn resolve_dirty_projections_is_atomic_across_slots() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("host", "a"))
            .with_child(element("host", "b")),
    )
    .unwrap();
    let hosts = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .collect::<Vec<_>>();
    let first_slot = ProjectionSlot::default(hosts[0]);
    let second_slot = ProjectionSlot::default(hosts[1]);

    model
        .apply_projection(ProjectionEdit::new(
            first_slot.clone(),
            ProjectionSource::Elements(vec![element("button", "one")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model
        .apply_projection(ProjectionEdit::new(
            second_slot.clone(),
            ProjectionSource::Elements(vec![element("button", "two")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();

    model.set_failpoint(Failpoint::BeforeProjectionCacheInsert);
    assert_eq!(
        model.resolve_dirty_projections().unwrap_err().code(),
        ErrorCode::UnsupportedFeature
    );
    model.clear_failpoint();

    let dirty = model.snapshot().dirty_slots().collect::<Vec<_>>();
    assert!(dirty.contains(&first_slot));
    assert!(dirty.contains(&second_slot));
    assert!(!model.projection_caches.contains_key(&first_slot));
    assert!(!model.projection_caches.contains_key(&second_slot));

    model.resolve_dirty_projections().unwrap();
    assert_eq!(
        projection_child_keys(&model, first_slot),
        vec![Some(key("one"))]
    );
    assert_eq!(
        projection_child_keys(&model, second_slot),
        vec![Some(key("two"))]
    );
}

#[test]
fn projection_resolution_rolls_back_after_failure_injection() {
    for failpoint in [
        Failpoint::AfterPendingSourceRemoval,
        Failpoint::AfterProjectionCacheRemoval,
        Failpoint::AfterOldChildRemoval,
        Failpoint::BeforeProjectionCacheInsert,
    ] {
        let mut model = Model::empty();
        let slot = ProjectionSlot::default(model.root());
        model
            .apply_projection(ProjectionEdit::new(
                slot.clone(),
                ProjectionSource::Elements(vec![element("button", "old")]),
                ProjectionReplaceMode::PreserveCompatible,
            ))
            .unwrap();
        model.resolve_projection(slot.clone()).unwrap();
        let old_child = model
            .snapshot()
            .projected_children(slot.clone())
            .unwrap()
            .next()
            .unwrap();
        let source = if failpoint == Failpoint::AfterOldChildRemoval {
            ProjectionSource::Elements(Vec::new())
        } else {
            ProjectionSource::Elements(vec![element("button", "new")])
        };
        model
            .apply_projection(ProjectionEdit::new(
                slot.clone(),
                source,
                ProjectionReplaceMode::PreserveCompatible,
            ))
            .unwrap();

        model.set_failpoint(failpoint);
        assert_eq!(
            model.resolve_projection(slot.clone()).unwrap_err().code(),
            ErrorCode::UnsupportedFeature
        );
        model.clear_failpoint();

        assert_eq!(
            projection_cache_child_keys(&model, &slot),
            vec![Some(key("old"))]
        );
        assert_eq!(
            model.projection_caches.get(&slot).unwrap().children.first(),
            Some(&old_child)
        );
        assert!(model.pending_sources.contains_key(&slot));
        assert!(model.dirty_slots.contains(&slot));
    }
}

#[test]
fn failed_projection_resolution_preserves_revision() {
    let mut model = Model::empty();
    let slot = ProjectionSlot::default(model.root());

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "old")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "new")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    let before_failure = model.revision();

    model.set_failpoint(Failpoint::BeforeProjectionCacheInsert);
    assert_eq!(
        model.resolve_projection(slot).unwrap_err().code(),
        ErrorCode::UnsupportedFeature
    );
    model.clear_failpoint();

    assert_eq!(model.revision(), before_failure);
}

#[test]
fn projection_reuse_failure_restores_reused_node() {
    let mut model = Model::empty();
    let slot = ProjectionSlot::default(model.root());
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "same")]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let reused = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(vec![element("button", "same").with_text(text("changed"))]),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.set_failpoint(Failpoint::AfterProjectedChildReuse);
    assert_eq!(
        model.resolve_projection(slot.clone()).unwrap_err().code(),
        ErrorCode::UnsupportedFeature
    );
    model.clear_failpoint();

    let snapshot = model.snapshot();
    let node = snapshot.get(reused).unwrap();
    assert_eq!(node.key(), Some(&key("same")));
    assert_eq!(node.text(), None);
    assert!(model.pending_sources.contains_key(&slot));
    assert!(model.dirty_slots.contains(&slot));
}

#[test]
fn virtual_anchor_failure_restores_anchor_and_window() {
    let mut model = Model::empty();
    let slot = ProjectionSlot::default(model.root());
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(virtual_projection(0, 1)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let row_0 = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    model
        .apply(Patch::SetState {
            id: row_0,
            state: StatePatch::new().selected(true),
        })
        .unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(virtual_projection(1, 1)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    assert!(
        model
            .virtual_anchors
            .contains_key(&(slot.clone(), key("row-0")))
    );

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(virtual_projection(0, 1)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.set_failpoint(Failpoint::AfterVirtualAnchorRemoval);
    assert_eq!(
        model.resolve_projection(slot.clone()).unwrap_err().code(),
        ErrorCode::UnsupportedFeature
    );
    model.clear_failpoint();

    assert_eq!(
        projection_cache_child_keys(&model, &slot),
        vec![Some(key("row-1"))]
    );
    assert!(
        model
            .virtual_anchors
            .contains_key(&(slot.clone(), key("row-0")))
    );
    assert!(model.pending_sources.contains_key(&slot));
    assert!(model.dirty_slots.contains(&slot));
}

#[test]
fn virtual_projection_validates_dense_ranges_and_duplicate_keys() {
    assert_eq!(
        VirtualRange::new(2, 1).unwrap_err().code(),
        ErrorCode::InvalidVirtualRange
    );

    let item = VirtualItem::new(0, key("row-0"), Element::tagged(tag("row"))).unwrap();
    assert!(VirtualProjection::dense(10, VirtualRange::new(0, 1).unwrap(), vec![item]).is_ok());

    let duplicate = vec![
        VirtualItem::new(0, key("row"), Element::tagged(tag("row"))).unwrap(),
        VirtualItem::new(1, key("row"), Element::tagged(tag("row"))).unwrap(),
    ];
    assert_eq!(
        VirtualProjection::dense(10, VirtualRange::new(0, 2).unwrap(), duplicate)
            .unwrap_err()
            .code(),
        ErrorCode::DuplicateKey
    );

    assert_eq!(
        VirtualItem::new(0, key("row"), element("row", "conflict"))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidVirtualItem
    );
}

#[test]
fn virtual_projection_materializes_only_supplied_items_and_preserves_state_anchor() {
    let mut model = Model::empty();
    let root = model.root();
    let slot = ProjectionSlot::default(root);
    let make_projection = |start| {
        let items = (start..start + 2)
            .map(|index| {
                VirtualItem::new(
                    index,
                    key(&format!("row-{index}")),
                    Element::tagged(tag("row")).with_text(text(&format!("Row {index}"))),
                )
                .unwrap()
            })
            .collect();
        VirtualProjection::dense(200_000, VirtualRange::new(start, start + 2).unwrap(), items)
            .unwrap()
    };

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(make_projection(0)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let first = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(first.len(), 2);
    model
        .apply(Patch::SetState {
            id: first[0],
            state: StatePatch::new().selected(true),
        })
        .unwrap();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(make_projection(1)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    assert_eq!(
        model
            .snapshot()
            .projected_children(slot.clone())
            .unwrap()
            .collect::<Vec<_>>()
            .len(),
        2
    );

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Virtual(make_projection(0)),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot.clone()).unwrap();
    let rematerialized = model
        .snapshot()
        .projected_children(slot.clone())
        .unwrap()
        .next()
        .unwrap();
    assert!(
        model
            .snapshot()
            .get(rematerialized)
            .unwrap()
            .state()
            .selected()
    );
}

#[test]
fn event_routing_emits_commands() {
    let command = CommandName::new("project.run").unwrap();
    let button = element("button", "run")
        .with_hook(Hook::new(Trigger::Event(EventKind::Click), command.clone()));
    let mut model = Model::new(Element::root().with_child(button)).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();
    let report = model
        .dispatch(Event::new(target, EventKind::Click))
        .unwrap();
    assert_eq!(report.commands().len(), 1);
    assert_eq!(report.commands()[0].command(), &command);
}

#[test]
fn model_revision_is_a_semantic_value() {
    let model = Model::empty();
    let revision = model.revision();

    assert_eq!(revision.get(), 0);
    assert_eq!(model.snapshot().revision(), revision);
}

#[test]
fn revision_tracks_snapshot_observable_changes() {
    let command = CommandName::new("project.run").unwrap();
    let button =
        element("button", "run").with_hook(Hook::new(Trigger::Event(EventKind::Click), command));
    let mut model = Model::new(Element::root().with_child(button)).unwrap();
    let initial_revision = model.revision();
    assert_eq!(model.snapshot().revision(), initial_revision);

    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();

    model
        .dispatch(Event::new(target, EventKind::Click))
        .unwrap();
    assert_eq!(model.revision(), initial_revision);

    model
        .apply(Patch::SetText {
            id: target,
            text: Some(text("Run")),
        })
        .unwrap();
    let text_revision = model.revision();
    assert!(text_revision > initial_revision);
    assert_eq!(model.snapshot().revision(), text_revision);

    model
        .apply(Patch::SetText {
            id: target,
            text: Some(text("Run")),
        })
        .unwrap();
    assert_eq!(model.revision(), text_revision);

    model.focus(Some(target)).unwrap();
    let focus_revision = model.revision();
    assert!(focus_revision > text_revision);
    assert_eq!(model.snapshot().revision(), focus_revision);

    model.focus(Some(target)).unwrap();
    assert_eq!(model.revision(), focus_revision);

    let pointer = PointerId::new(1);
    model
        .capture_pointer(PointerCapture::new(pointer, target))
        .unwrap();
    let capture_revision = model.revision();
    assert!(capture_revision > focus_revision);

    model
        .capture_pointer(PointerCapture::new(pointer, target))
        .unwrap();
    assert_eq!(model.revision(), capture_revision);

    let slot = ProjectionSlot::default(model.root());
    let projection_source = ProjectionSource::Elements(vec![element("button", "projected")]);
    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            projection_source.clone(),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    let dirty_revision = model.revision();
    assert!(dirty_revision > capture_revision);
    assert_eq!(model.snapshot().revision(), dirty_revision);

    model.resolve_projection(slot.clone()).unwrap();
    let resolved_revision = model.revision();
    assert!(resolved_revision > dirty_revision);
    assert_eq!(model.snapshot().revision(), resolved_revision);

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            projection_source,
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    model.resolve_projection(slot).unwrap();
    assert_eq!(model.revision(), resolved_revision);
}

#[test]
fn revision_advances_when_projection_resolution_only_clears_dirty_slot() {
    let mut model = Model::empty();
    let slot = ProjectionSlot::default(model.root());
    let initial_revision = model.revision();

    model
        .apply_projection(ProjectionEdit::new(
            slot.clone(),
            ProjectionSource::Elements(Vec::new()),
            ProjectionReplaceMode::PreserveCompatible,
        ))
        .unwrap();
    let dirty_revision = model.revision();
    assert!(dirty_revision > initial_revision);
    assert_eq!(
        model.snapshot().dirty_slots().collect::<Vec<_>>(),
        vec![slot.clone()]
    );

    let report = model.resolve_projection(slot.clone()).unwrap();
    assert!(report.changes().is_empty());
    let resolved_revision = model.revision();
    assert!(resolved_revision > dirty_revision);
    assert!(
        model
            .snapshot()
            .dirty_slots()
            .collect::<Vec<_>>()
            .is_empty()
    );

    model.resolve_projection(slot).unwrap();
    assert_eq!(model.revision(), resolved_revision);
}

#[test]
fn revision_ignores_move_to_current_effective_position() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("button", "a"))
            .with_child(element("button", "b"))
            .with_child(element("button", "c")),
    )
    .unwrap();
    let root = model.root();
    let children = model.snapshot().children(root).unwrap().collect::<Vec<_>>();
    let initial_revision = model.revision();

    let report = model
        .apply(Patch::Move {
            id: children[1],
            parent: root,
            index: 2,
        })
        .unwrap();

    assert!(report.changes().is_empty());
    assert_eq!(model.revision(), initial_revision);
    assert_eq!(
        model.snapshot().children(root).unwrap().collect::<Vec<_>>(),
        children
    );
}

#[test]
fn revision_ignores_reorder_children_to_existing_order() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("button", "a"))
            .with_child(element("button", "b"))
            .with_child(element("button", "c")),
    )
    .unwrap();
    let root = model.root();
    let children = model.snapshot().children(root).unwrap().collect::<Vec<_>>();
    let initial_revision = model.revision();

    let report = model
        .apply(Patch::ReorderChildren {
            parent: root,
            children: children.clone(),
        })
        .unwrap();

    assert!(report.changes().is_empty());
    assert_eq!(model.revision(), initial_revision);
    assert_eq!(
        model.snapshot().children(root).unwrap().collect::<Vec<_>>(),
        children
    );
}

#[test]
fn repeated_attribute_and_state_patches_are_precise_noops() {
    let mut model = Model::new(Element::root().with_child(element("button", "run"))).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();

    let set_attribute = Patch::SetAttribute {
        id: target,
        name: attr_name("data-state"),
        value: value("ready"),
    };
    assert!(
        !model
            .apply(set_attribute.clone())
            .unwrap()
            .changes()
            .is_empty()
    );
    assert!(model.apply(set_attribute).unwrap().changes().is_empty());
    assert!(
        model
            .apply(Patch::RemoveAttribute {
                id: target,
                name: attr_name("missing"),
            })
            .unwrap()
            .changes()
            .is_empty()
    );

    let report = model
        .apply(Patch::SetState {
            id: target,
            state: StatePatch::new().presence(Presence::Hidden),
        })
        .unwrap();
    let (_, flags) = report.changes().changed().next().unwrap();
    assert!(flags.has_state());
    assert!(flags.has_presence());
    assert!(
        model
            .apply(Patch::SetState {
                id: target,
                state: StatePatch::new().presence(Presence::Hidden),
            })
            .unwrap()
            .changes()
            .is_empty()
    );
}

#[test]
fn state_patch_updates_only_app_mutable_state() {
    let mut model = Model::new(Element::root().with_child(element("button", "run"))).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();

    model.focus(Some(target)).unwrap();
    model
        .capture_pointer(PointerCapture::new(PointerId::new(7), target))
        .unwrap();
    model
        .apply(Patch::SetState {
            id: target,
            state: StatePatch::new().selected(true).checked(Some(true)),
        })
        .unwrap();

    let snapshot = model.snapshot();
    let state = snapshot.get(target).unwrap().state();
    assert!(state.focused());
    assert!(!state.focus_within());
    assert!(state.pointer_captured());
    assert!(state.selected());
    assert_eq!(state.checked(), Some(true));
}

#[test]
fn state_patch_builder_exposes_only_app_mutable_state() {
    let mut model = Model::new(Element::root().with_child(element("button", "run"))).unwrap();
    let target = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();

    let patch = StatePatch::new()
        .selected(true)
        .presence(Presence::Hidden)
        .disabled(true)
        .checked(Some(true))
        .expanded(Some(false));

    model
        .apply(Patch::SetState {
            id: target,
            state: patch,
        })
        .unwrap();

    let snapshot = model.snapshot();
    let state = snapshot.get(target).unwrap().state();
    assert!(state.selected());
    assert_eq!(state.presence(), Presence::Hidden);
    assert!(state.disabled());
    assert_eq!(state.checked(), Some(true));
    assert_eq!(state.expanded(), Some(false));
}

#[test]
fn pointer_capture_state_is_aggregate() {
    let mut model = Model::new(
        Element::root()
            .with_child(element("button", "a"))
            .with_child(element("button", "b")),
    )
    .unwrap();
    let targets = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .collect::<Vec<_>>();
    let a = targets[0];
    let b = targets[1];
    let p1 = PointerId::new(1);
    let p2 = PointerId::new(2);

    model.capture_pointer(PointerCapture::new(p1, a)).unwrap();
    model.capture_pointer(PointerCapture::new(p2, a)).unwrap();
    assert!(model.snapshot().get(a).unwrap().state().pointer_captured());

    model.release_pointer(p1).unwrap();
    assert!(model.snapshot().get(a).unwrap().state().pointer_captured());

    model.capture_pointer(PointerCapture::new(p2, b)).unwrap();
    assert!(!model.snapshot().get(a).unwrap().state().pointer_captured());
    assert!(model.snapshot().get(b).unwrap().state().pointer_captured());

    model.release_pointer(p2).unwrap();
    assert!(!model.snapshot().get(b).unwrap().state().pointer_captured());
}

#[test]
fn large_model_localized_patch_is_practical() {
    let mut root = Element::root();
    for index in 0..10_000 {
        root = root.with_child(element("row", &format!("row-{index}")));
    }
    let mut model = Model::new(root).unwrap();
    let first = model
        .snapshot()
        .children(model.root())
        .unwrap()
        .next()
        .unwrap();
    let report = model
        .apply(Patch::SetText {
            id: first,
            text: Some(text("updated")),
        })
        .unwrap();
    assert_eq!(report.changes().changed().count(), 1);
}
