use super::Id;

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub presence: Presence,
    pub disabled: bool,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub focus_within: bool,
    pub pointer_captured: bool,
    pub selected: bool,
    pub pressed: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
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
        if let Some(value) = patch.hovered {
            self.hovered = value;
        }
        if let Some(value) = patch.active {
            self.active = value;
        }
        if let Some(value) = patch.selected {
            self.selected = value;
        }
        if let Some(value) = patch.pressed {
            self.pressed = value;
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

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateFlag {
    Hovered,
    Active,
    Focused,
    FocusWithin,
    PointerCaptured,
    Disabled,
    Selected,
    Pressed,
    Checked,
    Expanded,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StatePatch {
    pub presence: Option<Presence>,
    pub disabled: Option<bool>,
    pub hovered: Option<bool>,
    pub active: Option<bool>,
    pub selected: Option<bool>,
    pub pressed: Option<bool>,
    pub checked: Option<Option<bool>>,
    pub expanded: Option<Option<bool>>,
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
