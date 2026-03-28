//! # csv-cat
//!
//! CSV processing built on [`comp-cat-rs`](https://crates.io/crates/comp-cat-rs).
//!
//! All operations return `Io<CsvError, _>` for composable effect handling.
//! File handles are managed via `Resource`.  Rows stream as `Stream<CsvError, Row>`.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use csv_cat::{reader, writer, row::Row, error::CsvError};
//!
//! // Read all rows from a file
//! let rows = reader::read_all("data.csv", reader::ReaderConfig::new()).run()?;
//!
//! // Process rows
//! let names: Vec<String> = rows.iter()
//!     .filter_map(|row| row.get(0).ok().map(String::from))
//!     .collect();
//!
//! // Read from a string
//! let rows = reader::from_str("a,b\n1,2\n", reader::ReaderConfig::new()).run()?;
//!
//! // Write to a string
//! let output = writer::to_string(
//!     writer::WriterConfig::new(),
//!     Some(vec!["name".into(), "age".into()]),
//!     rows,
//! ).run()?;
//! ```

pub mod error;
pub mod row;
pub mod reader;
pub mod writer;
