use std::panic::Location;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    pub location: Location<'static>,
    pub kind: ErrorKind,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.kind)
    }
}
impl std::error::Error for Error {}

impl Error {
    #[track_caller]
    pub fn new(e: ErrorKind) -> Self {
        Error {
            location: *Location::caller(),
            kind: e,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("{0}")]
    IOError(#[from] std::io::Error),
    #[error("{0}")]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error("Not enough space to ingest")]
    InsufficientSpace,
    #[error("{0}")]
    CustomError(String),
}

impl Error {
    #[track_caller]
    pub fn custom_error(msg: impl std::fmt::Display) -> Self {
        Self {
            location: *Location::caller(),
            kind: ErrorKind::CustomError(format!("{}", msg)),
        }
    }
}

impl<T> From<T> for Error
where
    T: Into<ErrorKind>,
{
    #[track_caller]
    fn from(e: T) -> Error {
        Error {
            location: *Location::caller(),
            kind: e.into(),
        }
    }
}
