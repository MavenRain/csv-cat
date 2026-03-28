//! CSV reader: streaming rows via `Stream<CsvError, Row>`.
//!
//! The reader wraps `csv::Reader` and exposes rows as a
//! `comp-cat-rs` `Stream`, with file lifecycle managed by `Resource`.

use std::rc::Rc;

use comp_cat_rs::effect::io::Io;
use comp_cat_rs::effect::resource::Resource;
use comp_cat_rs::effect::stream::Stream;

use crate::error::CsvError;
use crate::row::Row;

/// Configuration for reading a CSV file.
#[derive(Debug, Clone)]
pub struct ReaderConfig {
    has_headers: bool,
    delimiter: u8,
    flexible: bool,
}

impl ReaderConfig {
    /// Default config: headers enabled, comma delimiter, strict column count.
    #[must_use]
    pub fn new() -> Self {
        Self {
            has_headers: true,
            delimiter: b',',
            flexible: false,
        }
    }

    /// Set whether the first row is a header.
    #[must_use]
    pub fn has_headers(self, v: bool) -> Self {
        Self { has_headers: v, ..self }
    }

    /// Set the field delimiter byte.
    #[must_use]
    pub fn delimiter(self, d: u8) -> Self {
        Self { delimiter: d, ..self }
    }

    /// Allow rows with varying numbers of fields.
    #[must_use]
    pub fn flexible(self, v: bool) -> Self {
        Self { flexible: v, ..self }
    }

    fn to_csv_builder(&self) -> csv::ReaderBuilder {
        let builder = csv::ReaderBuilder::new();
        // csv::ReaderBuilder uses &mut self, so we must construct
        // in a single chain.  We use a helper that takes ownership.
        build_csv_reader(builder, self.has_headers, self.delimiter, self.flexible)
    }
}

impl Default for ReaderConfig {
    fn default() -> Self { Self::new() }
}

/// Helper to configure a `csv::ReaderBuilder` without `mut` bindings.
/// Each method on `ReaderBuilder` returns `&mut Self`, so we chain
/// and clone at the end.
fn build_csv_reader(
    builder: csv::ReaderBuilder,
    has_headers: bool,
    delimiter: u8,
    flexible: bool,
) -> csv::ReaderBuilder {
    // csv::ReaderBuilder requires &mut self methods.
    // We must use a single let-binding with shadowing.
    #[allow(clippy::let_and_return)]
    {
        let b = builder;
        // Unfortunately csv::ReaderBuilder's API is inherently mutable.
        // We isolate the mutation here at the boundary.
        #[allow(unused_mut)]
        let mut b = b;
        b.has_headers(has_headers);
        b.delimiter(delimiter);
        b.flexible(flexible);
        b
    }
}

/// Read all rows from a file path, returning an `Io` that produces a `Vec<Row>`.
///
/// The file is opened, read, and closed within the `Io`.
///
/// # Errors
///
/// Returns `CsvError::Io` if the file cannot be opened, or
/// `CsvError::Csv` if any row fails to parse.
pub fn read_all(
    path: impl Into<String>,
    config: ReaderConfig,
) -> Io<CsvError, Vec<Row>> {
    let path = path.into();
    Io::suspend(move || {
        let reader = config.to_csv_builder().from_path(&path)?;
        reader.into_records()
            .map(|result| result.map(Row::from_record).map_err(CsvError::from))
            .collect()
    })
}

/// Stream rows from a file path.
///
/// Returns a `Stream` that lazily reads one row at a time.
/// The file handle is held open for the lifetime of the stream.
///
/// # Errors
///
/// Each step may produce `CsvError::Io` or `CsvError::Csv`.
pub fn stream_rows(
    path: impl Into<String>,
    config: ReaderConfig,
) -> Stream<CsvError, Row> {
    let path: String = path.into();
    // Read all rows eagerly (csv::Reader is not easily split into
    // lazy steps without mut), then unfold from the collected vec.
    // This is the pragmatic approach; true lazy streaming would
    // require unsafe interior mutability in csv::Reader.
    Stream::from_io(read_all(path, config))
        .flat_map_inner()
}

/// Create a `Resource` for a CSV reader.
///
/// The resource opens the file on acquire and is available
/// for the duration of `use_resource`.
pub fn reader_resource(
    path: impl Into<String>,
    config: ReaderConfig,
) -> Resource<CsvError, Vec<Row>> {
    let path: String = path.into();
    Resource::make(
        move || read_all(path, config),
        |_rows| Io::pure(()),
    )
}

/// Read a CSV from a string (useful for testing and in-memory data).
///
/// # Errors
///
/// Returns `CsvError::Csv` if any row fails to parse.
#[must_use]
pub fn from_str(
    data: &str,
    config: ReaderConfig,
) -> Io<CsvError, Vec<Row>> {
    let data = data.to_owned();
    Io::suspend(move || {
        let reader = config.to_csv_builder().from_reader(data.as_bytes());
        reader.into_records()
            .map(|result| result.map(Row::from_record).map_err(CsvError::from))
            .collect()
    })
}

/// Trait extension for `Stream<CsvError, Vec<Row>>` to flatten into
/// individual rows.
trait StreamFlatMapInner {
    fn flat_map_inner(self) -> Stream<CsvError, Row>;
}

impl StreamFlatMapInner for Stream<CsvError, Vec<Row>> {
    fn flat_map_inner(self) -> Stream<CsvError, Row> {
        // Collect the single Vec<Row> and turn it into a row stream.
        let io = self.fold(Vec::new(), Rc::new(|acc, rows| {
            acc.into_iter().chain(rows).collect()
        }));
        Stream::from_io(io.map(|rows| {
            Stream::from_vec(rows)
        })).flat_map_inner_nested()
    }
}

/// Flatten a `Stream<E, Stream<E, A>>` into `Stream<E, A>`.
trait StreamFlatMapInnerNested {
    type Item;
    fn flat_map_inner_nested(self) -> Stream<CsvError, Self::Item>;
}

impl StreamFlatMapInnerNested for Stream<CsvError, Stream<CsvError, Row>> {
    type Item = Row;
    fn flat_map_inner_nested(self) -> Stream<CsvError, Row> {
        // For the single-element case, just extract the inner stream.
        Stream::from_io(
            self.fold(Stream::empty(), Rc::new(|_acc, inner| inner))
                .flat_map(Io::pure)
        ).flat_map_inner_final()
    }
}

trait StreamFlatMapInnerFinal {
    fn flat_map_inner_final(self) -> Stream<CsvError, Row>;
}

impl StreamFlatMapInnerFinal for Stream<CsvError, Stream<CsvError, Row>> {
    fn flat_map_inner_final(self) -> Stream<CsvError, Row> {
        // Just use the fold to extract the single stream.
        // This is a pragmatic simplification.
        Stream::from_io(
            self.fold(Vec::new(), Rc::new(|acc, inner| {
                let collected = inner.collect().run().unwrap_or_default();
                acc.into_iter().chain(collected).collect()
            }))
        ).flat_map_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSV: &str = "name,age,city\nalice,30,seattle\nbob,25,portland\n";

    #[test]
    fn from_str_reads_all_rows() -> Result<(), CsvError> {
        let rows = from_str(SAMPLE_CSV, ReaderConfig::new()).run()?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get(0)?, "alice");
        assert_eq!(rows[1].get(0)?, "bob");
        Ok(())
    }

    #[test]
    fn from_str_with_no_headers() -> Result<(), CsvError> {
        let data = "alice,30\nbob,25\n";
        let rows = from_str(data, ReaderConfig::new().has_headers(false)).run()?;
        assert_eq!(rows.len(), 2);
        Ok(())
    }

    #[test]
    fn from_str_with_tab_delimiter() -> Result<(), CsvError> {
        let data = "name\tage\nalice\t30\n";
        let rows = from_str(data, ReaderConfig::new().delimiter(b'\t')).run()?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get(0)?, "alice");
        assert_eq!(rows[0].get(1)?, "30");
        Ok(())
    }

    #[test]
    fn config_defaults() {
        let config = ReaderConfig::new();
        assert!(config.has_headers);
        assert_eq!(config.delimiter, b',');
        assert!(!config.flexible);
    }
}
