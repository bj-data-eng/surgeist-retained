use super::{CommandName, EventName, Id, PointerId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hook {
    pub trigger: Trigger,
    pub command: CommandName,
}

impl Hook {
    #[must_use]
    pub fn new(trigger: Trigger, command: CommandName) -> Self {
        Self { trigger, command }
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    Event(EventKind),
    Intent(Intent),
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    Command,
    Select,
    Focus,
    Drag,
    Menu,
    Edit,
    Navigate,
    Custom(EventName),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    target: Id,
    trigger: Trigger,
    phase: Phase,
    command: CommandName,
    route: Route,
}

impl Command {
    #[must_use]
    pub fn new(
        target: Id,
        trigger: Trigger,
        phase: Phase,
        command: CommandName,
        route: Route,
    ) -> Self {
        Self {
            target,
            trigger,
            phase,
            command,
            route,
        }
    }

    #[must_use]
    pub const fn target(&self) -> Id {
        self.target
    }

    #[must_use]
    pub fn trigger(&self) -> &Trigger {
        &self.trigger
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub fn command(&self) -> &CommandName {
        &self.command
    }

    #[must_use]
    pub fn route(&self) -> &Route {
        &self.route
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
    PointerEnter,
    PointerLeave,
    PointerDown,
    PointerUp,
    Click,
    ContextMenu,
    KeyDown,
    KeyUp,
    Input,
    Change,
    Focus,
    Blur,
    Select,
    DragStart,
    Drag,
    DragEnd,
    Custom(EventName),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    target: Id,
    trigger: Trigger,
    propagation: Propagation,
    pointer: Option<PointerId>,
}

impl Event {
    #[must_use]
    pub fn new(target: Id, kind: EventKind) -> Self {
        Self::with_trigger(target, Trigger::Event(kind))
    }

    #[must_use]
    pub fn intent(target: Id, intent: Intent) -> Self {
        Self::with_trigger(target, Trigger::Intent(intent))
    }

    #[must_use]
    fn with_trigger(target: Id, trigger: Trigger) -> Self {
        Self {
            target,
            trigger,
            propagation: Propagation::Bubble,
            pointer: None,
        }
    }

    #[must_use]
    pub fn with_propagation(mut self, propagation: Propagation) -> Self {
        self.propagation = propagation;
        self
    }

    #[must_use]
    pub fn with_pointer(mut self, pointer: PointerId) -> Self {
        self.pointer = Some(pointer);
        self
    }

    #[must_use]
    pub const fn target(&self) -> Id {
        self.target
    }

    #[must_use]
    pub fn trigger(&self) -> &Trigger {
        &self.trigger
    }

    #[must_use]
    pub const fn propagation(&self) -> Propagation {
        self.propagation
    }

    #[must_use]
    pub const fn pointer(&self) -> Option<PointerId> {
        self.pointer
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Route {
    steps: Vec<RouteStep>,
}

impl Route {
    #[must_use]
    pub fn new(steps: Vec<RouteStep>) -> Self {
        Self { steps }
    }

    #[must_use]
    pub fn steps(&self) -> &[RouteStep] {
        &self.steps
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteStep {
    pub id: Id,
    pub phase: Phase,
}

impl RouteStep {
    #[must_use]
    pub const fn new(id: Id, phase: Phase) -> Self {
        Self { id, phase }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Propagation {
    TargetOnly,
    Bubble,
    CaptureThenBubble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Capture,
    Target,
    Bubble,
}
