//! Row: a newtype over a CSV record.

use crate::error::CsvError;

/// A single row from a CSV file.
///
/// Wraps a `csv::StringRecord` with accessor methods.
/// All fields are private per CLAUDE.md.
#[derive(Debug, Clone)]
pub struct Row {
    record: csv::StringRecord,
}

impl Row {
    /// Wrap a raw `StringRecord`.
    #[must_use]
    pub(crate) fn from_record(record: csv::StringRecord) -> Self {
        Self { record }
    }

    /// Get a field by index.
    ///
    /// # Errors
    ///
    /// Returns `CsvError::MissingField` if the index is out of bounds.
    pub fn get(&self, index: usize) -> Result<&str, CsvError> {
        self.record.get(index)
            .ok_or(CsvError::MissingField { index })
    }

    /// Number of fields in this row.
    #[must_use]
    pub fn len(&self) -> usize {
        self.record.len()
    }

    /// Whether this row has zero fields.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.record.is_empty()
    }

    /// Iterate over all fields as string slices.
    pub fn fields(&self) -> impl Iterator<Item = &str> {
        self.record.iter()
    }

    /// Deserialize this row into a typed value.
    ///
    /// Requires that the type implements `serde::Deserialize` and
    /// that headers have been provided.
    ///
    /// # Errors
    ///
    /// Returns `CsvError::Deserialize` if the record cannot be
    /// deserialized into the target type.
    pub fn deserialize<'de, T: serde::Deserialize<'de>>(
        &'de self,
        headers: Option<&'de csv::StringRecord>,
    ) -> Result<T, CsvError> {
        self.record.deserialize(headers)
            .map_err(|e| CsvError::Deserialize(e.to_string()))
    }

    /// Convert to a `Vec<String>` of all fields.
    #[must_use]
    pub fn to_vec(&self) -> Vec<String> {
        self.record.iter().map(String::from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> Row {
        let record = csv::StringRecord::from(vec!["alice", "30", "seattle"]);
        Row::from_record(record)
    }

    #[test]
    fn get_returns_field_by_index() -> Result<(), CsvError> {
        let row = sample_row();
        assert_eq!(row.get(0)?, "alice");
        assert_eq!(row.get(1)?, "30");
        assert_eq!(row.get(2)?, "seattle");
        Ok(())
    }

    #[test]
    fn get_out_of_bounds_returns_error() {
        let row = sample_row();
        assert!(row.get(99).is_err());
    }

    #[test]
    fn len_and_is_empty() {
        let row = sample_row();
        assert_eq!(row.len(), 3);
        assert!(!row.is_empty());

        let empty = Row::from_record(csv::StringRecord::new());
        assert!(empty.is_empty());
    }

    #[test]
    fn to_vec_collects_all_fields() {
        let row = sample_row();
        assert_eq!(row.to_vec(), vec!["alice", "30", "seattle"]);
    }
}
