use super::Id;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub(crate) presence: Presence,
    pub(crate) disabled: bool,
    pub(crate) hovered: bool,
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) focus_within: bool,
    pub(crate) pointer_captured: bool,
    pub(crate) selected: bool,
    pub(crate) pressed: bool,
    pub(crate) checked: Option<bool>,
    pub(crate) expanded: Option<bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            presence: Presence::Visible,
            disabled: false,
            hovered: false,
            active: false,
            focused: false,
            focus_within: false,
            pointer_captured: false,
            selected: false,
            pressed: false,
            checked: None,
            expanded: None,
        }
    }
}

impl State {
    #[must_use]
    pub const fn presence(&self) -> Presence {
        self.presence
    }

    #[must_use]
    pub const fn disabled(&self) -> bool {
        self.disabled
    }

    #[must_use]
    pub const fn hovered(&self) -> bool {
        self.hovered
    }

    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub const fn focused(&self) -> bool {
        self.focused
    }

    #[must_use]
    pub const fn focus_within(&self) -> bool {
        self.focus_within
    }

    #[must_use]
    pub const fn pointer_captured(&self) -> bool {
        self.pointer_captured
    }

    #[must_use]
    pub const fn selected(&self) -> bool {
        self.selected
    }

    #[must_use]
    pub const fn pressed(&self) -> bool {
        self.pressed
    }

    #[must_use]
    pub const fn checked(&self) -> Option<bool> {
        self.checked
    }

    #[must_use]
    pub const fn expanded(&self) -> Option<bool> {
        self.expanded
    }

    #[must_use]
    pub fn durable_anchor(&self) -> Self {
        Self {
            presence: Presence::Visible,
            disabled: false,
            hovered: false,
            active: false,
            focused: false,
            focus_within: false,
            pointer_captured: false,
            selected: self.selected,
            pressed: false,
            checked: self.checked,
            expanded: self.expanded,
        }
    }

    pub(crate) fn apply_patch(&mut self, patch: &StatePatch) -> bool {
        let before = self.clone();
        if let Some(value) = patch.presence {
            self.presence = value;
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
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Visible,
    Hidden,
    RetainedOnly,
}

/// Application-authored retained state changes.
///
/// Use builder methods so only app-mutable state can be authored through
/// `Patch::SetState`.
///
/// ```compile_fail
/// use surgeist_retained::StatePatch;
///
/// let _patch = StatePatch {
///     selected: Some(true),
///     ..StatePatch::new()
/// };
/// ```
///
/// ```compile_fail
/// use surgeist_retained::StatePatch;
///
/// let _patch = StatePatch {
///     hovered: Some(true),
///     ..StatePatch::new()
/// };
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatePatch {
    presence: Option<Presence>,
    disabled: Option<bool>,
    selected: Option<bool>,
    checked: Option<Option<bool>>,
    expanded: Option<Option<bool>>,
}

impl StatePatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    #[must_use]
    pub fn presence(mut self, presence: Presence) -> Self {
        self.presence = Some(presence);
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointerId(u64);

impl PointerId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerCapture {
    pub pointer: PointerId,
    pub target: Id,
}

impl PointerCapture {
    #[must_use]
    pub const fn new(pointer: PointerId, target: Id) -> Self {
        Self { pointer, target }
    }
}
