use std::collections::BTreeSet;

use super::{
    Element, Id, Key, SlotName,
    error::{Error, ErrorCode, Result},
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectionSlot {
    host: Id,
    key: SlotKey,
}

impl ProjectionSlot {
    #[must_use]
    pub const fn default(host: Id) -> Self {
        Self {
            host,
            key: SlotKey::Default,
        }
    }

    #[must_use]
    pub fn named(host: Id, name: SlotName) -> Self {
        Self {
            host,
            key: SlotKey::Named(name),
        }
    }

    #[must_use]
    pub const fn host(&self) -> Id {
        self.host
    }

    #[must_use]
    pub const fn key(&self) -> &SlotKey {
        &self.key
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SlotKey {
    Default,
    Named(SlotName),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionEdit {
    slot: ProjectionSlot,
    source: ProjectionSource,
    mode: ProjectionReplaceMode,
}

impl ProjectionEdit {
    #[must_use]
    pub fn new(
        slot: ProjectionSlot,
        source: ProjectionSource,
        mode: ProjectionReplaceMode,
    ) -> Self {
        Self { slot, source, mode }
    }

    #[must_use]
    pub fn slot(&self) -> ProjectionSlot {
        self.slot.clone()
    }

    #[must_use]
    pub fn source(&self) -> &ProjectionSource {
        &self.source
    }

    #[must_use]
    pub const fn mode(&self) -> ProjectionReplaceMode {
        self.mode
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionSource {
    Elements(Vec<Element>),
    Virtual(VirtualProjection),
}

/// Projection-owned identity preservation behavior.
///
/// This mode applies when resolving a projection source into retained nodes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionReplaceMode {
    /// Reuse existing projected identity only when the element kind remains compatible.
    PreserveCompatible,
    /// Reuse existing projected identity even when the element kind changes.
    PreserveIdentity,
    /// Allocate fresh projected identity for every resolved item.
    ResetIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualProjection {
    total_count: usize,
    range: VirtualRange,
    items: Vec<VirtualItem>,
    source_revision: Option<SourceRevision>,
}

impl VirtualProjection {
    pub fn dense(total_count: usize, range: VirtualRange, items: Vec<VirtualItem>) -> Result<Self> {
        validate_virtual(total_count, range, &items)?;
        Ok(Self {
            total_count,
            range,
            items,
            source_revision: None,
        })
    }

    #[must_use]
    pub fn with_source_revision(mut self, revision: SourceRevision) -> Self {
        self.source_revision = Some(revision);
        self
    }

    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.total_count
    }

    #[must_use]
    pub const fn range(&self) -> VirtualRange {
        self.range
    }

    #[must_use]
    pub fn items(&self) -> &[VirtualItem] {
        &self.items
    }

    #[must_use]
    pub const fn source_revision(&self) -> Option<SourceRevision> {
        self.source_revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualRange {
    start: usize,
    end: usize,
}

impl VirtualRange {
    pub fn new(start: usize, end: usize) -> Result<Self> {
        if start > end {
            return Err(Error::new(
                ErrorCode::InvalidVirtualRange,
                "virtual range must satisfy start <= end",
            ));
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualItem {
    index: usize,
    key: Key,
    element: Element,
}

impl VirtualItem {
    pub fn new(index: usize, key: Key, element: Element) -> Result<Self> {
        if element.key().is_some() {
            return Err(Error::new(
                ErrorCode::InvalidVirtualItem,
                "virtual item root element must not carry its own key",
            ));
        }
        Ok(Self {
            index,
            key,
            element,
        })
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub fn key(&self) -> &Key {
        &self.key
    }

    #[must_use]
    pub fn element(&self) -> &Element {
        &self.element
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevision(u64);

impl SourceRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

fn validate_virtual(total_count: usize, range: VirtualRange, items: &[VirtualItem]) -> Result<()> {
    if range.start > range.end || range.end > total_count {
        return Err(Error::new(
            ErrorCode::InvalidVirtualRange,
            "virtual range must satisfy start <= end <= total_count",
        ));
    }
    if items.len() != range.len() {
        return Err(Error::new(
            ErrorCode::InvalidVirtualRange,
            "dense virtual range must include exactly one item per logical index",
        ));
    }

    let mut keys = BTreeSet::new();
    for (offset, item) in items.iter().enumerate() {
        let expected = range.start + offset;
        if item.index != expected {
            return Err(Error::new(
                ErrorCode::InvalidVirtualItem,
                format!(
                    "virtual item index {} must match dense logical index {expected}",
                    item.index
                ),
            ));
        }
        if !keys.insert(item.key.clone()) {
            return Err(Error::new(
                ErrorCode::DuplicateKey,
                format!("duplicate virtual item key `{}`", item.key),
            ));
        }
    }

    Ok(())
}
