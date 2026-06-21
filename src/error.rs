use std::{error, fmt};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    code: ErrorCode,
    message: String,
}

impl Error {
    #[must_use]
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl error::Error for Error {}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidString,
    EmptyCommand,
    DuplicateKey,
    MissingNode,
    StaleId,
    InvalidParent,
    InvalidIndex,
    InvalidVirtualRange,
    InvalidVirtualItem,
    Cycle,
    InvalidMove,
    InvalidPatch,
    InvalidProjection,
    InvalidRoute,
    UnresolvedProjection,
    DisabledTarget,
    IneligibleTarget,
    UnsupportedFeature,
}
