use std::{fmt, str::FromStr};

use super::{
    error::{Error, ErrorCode, Result},
    identity::Key,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: AttributeName,
    pub value: Value,
}

impl Attribute {
    #[must_use]
    pub fn new(name: AttributeName, value: Value) -> Self {
        Self { name, value }
    }
}

macro_rules! token_type {
    ($name:ident, $ctor:ident) => {
        #[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl AsRef<str>) -> Result<Self> {
                validate_ident(value.as_ref(), stringify!($name), false).map(Self)
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(value: &str) -> Result<Self> {
                Self::new(value)
            }
        }
    };
}

token_type!(Tag, tag);
token_type!(Class, class);
token_type!(AttributeName, attribute_name);
token_type!(EventName, event_name);
token_type!(SlotName, slot_name);

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct CommandName(String);

impl CommandName {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        validate_ident(value.as_ref(), "CommandName", true).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CommandName").field(&self.0).finish()
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CommandName {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Value(String);

impl Value {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        validate_value(value.as_ref()).map(Self)
    }

    #[must_use]
    pub fn empty() -> Self {
        Self(String::new())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Value").field(&self.0).finish()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Text(String);

impl Text {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        validate_text(value.as_ref()).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Text").field(&self.0).finish()
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn validate_key(value: &str) -> Result<String> {
    validate_ident(value, "Key", true)
}

pub(crate) fn validate_element_strings(element: &super::Element) -> Result<()> {
    let mut attributes = std::collections::BTreeSet::new();
    for attribute in element.attributes() {
        if !attributes.insert(attribute.name.clone()) {
            return Err(Error::new(
                ErrorCode::InvalidString,
                format!("duplicate attribute `{}`", attribute.name),
            ));
        }
    }

    let mut child_keys = std::collections::BTreeSet::<Key>::new();
    for child in element.children() {
        if let Some(key) = child.key()
            && !child_keys.insert(key.clone())
        {
            return Err(Error::new(
                ErrorCode::DuplicateKey,
                format!("duplicate sibling key `{key}`"),
            ));
        }
        validate_element_strings(child)?;
    }

    Ok(())
}

fn validate_ident(value: &str, field: &str, allow_command_separators: bool) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        let code = if field == "CommandName" {
            ErrorCode::EmptyCommand
        } else {
            ErrorCode::InvalidString
        };
        return Err(Error::new(code, format!("{field} cannot be empty")));
    }

    for ch in trimmed.chars() {
        if ch == '\0' || ch.is_control() || ch.is_whitespace() {
            return Err(Error::new(
                ErrorCode::InvalidString,
                format!("{field} contains unsupported character U+{:04X}", ch as u32),
            ));
        }

        let valid = ch.is_ascii_alphanumeric()
            || matches!(ch, '_' | '-')
            || (allow_command_separators && matches!(ch, '.' | ':' | '/'));
        if !valid {
            return Err(Error::new(
                ErrorCode::InvalidString,
                format!("{field} contains unsupported character `{ch}`"),
            ));
        }
    }

    Ok(trimmed.to_owned())
}

fn validate_value(value: &str) -> Result<String> {
    if value
        .chars()
        .any(|ch| ch == '\0' || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')))
    {
        return Err(Error::new(
            ErrorCode::InvalidString,
            "value contains unsupported control character",
        ));
    }
    Ok(value.to_owned())
}

fn validate_text(value: &str) -> Result<String> {
    if value
        .chars()
        .any(|ch| ch == '\0' || (ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')))
    {
        return Err(Error::new(
            ErrorCode::InvalidString,
            "text contains unsupported control character",
        ));
    }
    Ok(value.to_owned())
}
