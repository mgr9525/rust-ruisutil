use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct Error {
    pub msg: String,
    pub kind: std::io::ErrorKind,
}

impl Error {
    pub fn new<T: Into<String>>(msg: T, kind: std::io::ErrorKind) -> Self {
        Self {
            msg: msg.into(),
            kind,
        }
    }
    pub fn news<T: Into<String>>(msg: T) -> Self {
        Self::new(msg, std::io::ErrorKind::Other)
    }
    pub fn kind(&self) -> std::io::ErrorKind {
        self.kind
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl From<Error> for std::io::Error {
    fn from(value: Error) -> Self {
        std::io::Error::new(value.kind, value.msg)
    }
}
