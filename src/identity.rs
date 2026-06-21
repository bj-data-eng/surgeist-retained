use std::{fmt, str::FromStr};

use super::{
    error::{Error, Result},
    projection::{ProjectionSlot, SlotKey},
    string::validate_key,
};

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Id {
    index: u32,
    generation: u32,
}

impl Id {
    pub(crate) const fn new(index: usize, generation: u32) -> Self {
        Self {
            index: index as u32,
            generation,
        }
    }

    #[must_use]
    pub(crate) const fn index(self) -> usize {
        self.index as usize
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({}:{})", self.index, self.generation)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Key(String);

impl Key {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        validate_key(value.as_ref()).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Key").field(&self.0).finish()
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Key {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KeyPath {
    components: Vec<KeyPathComponent>,
}

impl KeyPath {
    #[must_use]
    pub fn root() -> Self {
        Self {
            components: vec![KeyPathComponent::Root],
        }
    }

    #[must_use]
    pub fn canonical_key(&self, key: &Key) -> Self {
        self.with(KeyPathComponent::CanonicalKey(key.clone()))
    }

    #[must_use]
    pub fn canonical_index(&self, index: usize) -> Self {
        self.with(KeyPathComponent::CanonicalIndex(index))
    }

    #[must_use]
    pub fn projection_slot(&self, slot: &ProjectionSlot) -> Self {
        self.with(KeyPathComponent::ProjectionSlot(slot.key().clone()))
    }

    #[must_use]
    pub fn projected_key(&self, key: &Key) -> Self {
        self.with(KeyPathComponent::ProjectedKey(key.clone()))
    }

    #[must_use]
    pub fn projected_index(&self, index: usize) -> Self {
        self.with(KeyPathComponent::ProjectedIndex(index))
    }

    #[must_use]
    pub fn virtual_item(&self, key: &Key) -> Self {
        self.with(KeyPathComponent::VirtualItem(key.clone()))
    }

    #[must_use]
    pub fn components_len(&self) -> usize {
        self.components.len()
    }

    fn with(&self, component: KeyPathComponent) -> Self {
        let mut components = self.components.clone();
        components.push(component);
        Self { components }
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, component) in self.components.iter().enumerate() {
            if index > 0 {
                f.write_str("/")?;
            }
            write!(f, "{component}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum KeyPathComponent {
    Root,
    CanonicalKey(Key),
    CanonicalIndex(usize),
    ProjectionSlot(SlotKey),
    ProjectedKey(Key),
    ProjectedIndex(usize),
    VirtualItem(Key),
}

impl fmt::Display for KeyPathComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => f.write_str("root"),
            Self::CanonicalKey(key) => write!(f, "child:{key}"),
            Self::CanonicalIndex(index) => write!(f, "child@{index}"),
            Self::ProjectionSlot(SlotKey::Default) => f.write_str("slot:default"),
            Self::ProjectionSlot(SlotKey::Named(name)) => write!(f, "slot:{name}"),
            Self::ProjectedKey(key) => write!(f, "projected:{key}"),
            Self::ProjectedIndex(index) => write!(f, "projected@{index}"),
            Self::VirtualItem(key) => write!(f, "virtual:{key}"),
        }
    }
}
