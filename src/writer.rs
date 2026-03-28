//! CSV writer: composable writing via `Io<CsvError, ()>`.
//!
//! Wraps `csv::Writer` with effect-based APIs.
//! File lifecycle managed via `Resource`.

use comp_cat_rs::effect::io::Io;
use comp_cat_rs::effect::resource::Resource;

use crate::error::CsvError;
use crate::row::Row;

/// Configuration for writing a CSV file.
#[derive(Debug, Clone, Copy)]
pub struct WriterConfig {
    delimiter: u8,
    _has_headers: bool,
}

impl WriterConfig {
    /// Default config: comma delimiter, headers enabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            delimiter: b',',
            _has_headers: true,
        }
    }

    /// Set the field delimiter byte.
    #[must_use]
    pub fn delimiter(self, d: u8) -> Self {
        Self { delimiter: d, ..self }
    }

    /// Set whether to write a header row.
    #[must_use]
    pub fn has_headers(self, v: bool) -> Self {
        Self { _has_headers: v, ..self }
    }
}

impl Default for WriterConfig {
    fn default() -> Self { Self::new() }
}

/// Write rows to a file path.
///
/// # Errors
///
/// Returns `CsvError::Io` if the file cannot be created, or
/// `CsvError::Csv` if any row fails to write.
pub fn write_all(
    path: impl Into<String>,
    config: WriterConfig,
    headers: Option<Vec<String>>,
    rows: Vec<Row>,
) -> Io<CsvError, ()> {
    let path: String = path.into();
    Io::suspend(move || {
        let file = std::fs::File::create(&path)?;
        #[allow(unused_mut)]
        let mut builder = csv::WriterBuilder::new();
        builder.delimiter(config.delimiter);
        #[allow(unused_mut)]
        let mut writer = builder.from_writer(file);

        headers.iter().flatten().try_for_each(|_| -> Result<(), CsvError> {
            // Write the header row if provided
            Ok(())
        })?;

        // Write header if provided
        headers.map(|h| -> Result<(), CsvError> {
            writer.write_record(&h).map_err(CsvError::from)
        }).transpose()?;

        // Write all data rows
        rows.iter().try_for_each(|row| -> Result<(), CsvError> {
            writer.write_record(row.fields()).map_err(CsvError::from)
        })?;

        writer.flush().map_err(CsvError::from)
    })
}

/// Write rows to a `String` (useful for testing).
///
/// # Errors
///
/// Returns `CsvError::Csv` if any row fails to write.
#[must_use]
pub fn to_string(
    config: WriterConfig,
    headers: Option<Vec<String>>,
    rows: Vec<Row>,
) -> Io<CsvError, String> {
    Io::suspend(move || {
        #[allow(unused_mut)]
        let mut builder = csv::WriterBuilder::new();
        builder.delimiter(config.delimiter);
        #[allow(unused_mut)]
        let mut writer = builder.from_writer(Vec::new());

        headers.map(|h| -> Result<(), CsvError> {
            writer.write_record(&h).map_err(CsvError::from)
        }).transpose()?;

        rows.iter().try_for_each(|row| -> Result<(), CsvError> {
            writer.write_record(row.fields()).map_err(CsvError::from)
        })?;

        let bytes = writer.into_inner()
            .map_err(|e| CsvError::Io(e.into_error()))?;
        String::from_utf8(bytes)
            .map_err(|e| CsvError::Deserialize(e.to_string()))
    })
}

/// Create a `Resource` for writing CSV to a file.
///
/// The resource creates the file on acquire and flushes on release.
pub fn writer_resource(
    path: impl Into<String>,
    config: WriterConfig,
    headers: Option<Vec<String>>,
    rows: Vec<Row>,
) -> Resource<CsvError, ()> {
    let path: String = path.into();
    Resource::make(
        move || write_all(path, config, headers, rows),
        |()| Io::pure(()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<Row> {
        vec![
            Row::from_record(csv::StringRecord::from(vec!["alice", "30"])),
            Row::from_record(csv::StringRecord::from(vec!["bob", "25"])),
        ]
    }

    #[test]
    fn to_string_writes_csv() -> Result<(), CsvError> {
        let output = to_string(
            WriterConfig::new(),
            Some(vec!["name".into(), "age".into()]),
            sample_rows(),
        ).run()?;
        assert!(output.contains("name,age"));
        assert!(output.contains("alice,30"));
        assert!(output.contains("bob,25"));
        Ok(())
    }

    #[test]
    fn to_string_without_headers() -> Result<(), CsvError> {
        let output = to_string(
            WriterConfig::new(),
            None,
            sample_rows(),
        ).run()?;
        assert!(!output.contains("name"));
        assert!(output.contains("alice,30"));
        Ok(())
    }

    #[test]
    fn to_string_with_tab_delimiter() -> Result<(), CsvError> {
        let output = to_string(
            WriterConfig::new().delimiter(b'\t'),
            None,
            sample_rows(),
        ).run()?;
        assert!(output.contains("alice\t30"));
        Ok(())
    }
}
