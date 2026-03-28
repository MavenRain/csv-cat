//! Project-wide error type.

/// All errors in csv-cat.
#[derive(Debug)]
pub enum CsvError {
    /// Underlying csv crate error.
    Csv(csv::Error),
    /// IO error.
    Io(std::io::Error),
    /// A field was missing or could not be accessed.
    MissingField { index: usize },
    /// Deserialization of a typed record failed.
    Deserialize(String),
}

impl std::fmt::Display for CsvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Csv(e) => write!(f, "CSV error: {e}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::MissingField { index } => write!(f, "missing field at index {index}"),
            Self::Deserialize(msg) => write!(f, "deserialization error: {msg}"),
        }
    }
}

impl std::error::Error for CsvError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Csv(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::MissingField { .. } | Self::Deserialize(_) => None,
        }
    }
}

impl From<csv::Error> for CsvError {
    fn from(e: csv::Error) -> Self { Self::Csv(e) }
}

impl From<std::io::Error> for CsvError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}
