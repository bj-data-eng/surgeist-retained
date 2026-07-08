//! Retained semantic UI model boundary for Surgeist.
//!
//! This module owns semantic durability, canonical topology, projected
//! traversal, retained interaction state, event routes, mutation reports, and
//! projection identity. It intentionally does not own layout, style, rendering,
//! platform input, widgets, or application command execution.

mod change;
mod element;
mod error;
mod event;
mod identity;
mod model;
mod mutation;
mod projection;
mod snapshot;
mod state;
mod string;
mod transaction;

pub use change::{ChangeFlags, ChangeSet, Report, SelectorInvalidation, SelectorMetadataChange};
pub use element::{Element, Kind, Role};
pub use error::{Error, ErrorCode, Result};
pub use event::{
    Command, Event, EventKind, Hook, Intent, Phase, Propagation, Route, RouteStep, Trigger,
};
pub use identity::{Id, Key, KeyPath};
pub use model::{Model, ModelRevision};
pub use mutation::{Mutation, MutationEdit, Patch, ReplaceMode};
pub use projection::{
    ProjectionEdit, ProjectionReplaceMode, ProjectionSlot, ProjectionSource, SlotKey,
    SourceRevision, VirtualItem, VirtualProjection, VirtualRange,
};
pub use snapshot::{
    NodeRef, SelectorCount, SelectorIndex, SelectorMetadata, SelectorSiblingFacts,
    SelectorTraversal, Snapshot,
};
pub use state::{PointerCapture, PointerId, Presence, RuntimeStatePatch, State, StatePatch};
pub use string::{
    Attribute, AttributeName, Class, CommandName, EventName, SlotName, Tag, Text, Value,
};

#[cfg(test)]
mod tests;
