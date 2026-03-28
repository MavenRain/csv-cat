# csv-cat

CSV processing built on [comp-cat-rs](https://crates.io/crates/comp-cat-rs).  All operations return `Io<CsvError, _>` for composable effect handling.  Nothing runs until you call `.run()`.

Wraps the battle-tested [csv](https://crates.io/crates/csv) crate for RFC 4180 compliance; adds lazy evaluation, resource safety, and streaming on top.

## Installation

```toml
[dependencies]
csv-cat = "0.1"
```

## Quick start

```rust
use csv_cat::reader::{self, ReaderConfig};
use csv_cat::writer::{self, WriterConfig};
use csv_cat::error::CsvError;

// Read from a string
let rows = reader::from_str(
    "name,age\nalice,30\nbob,25\n",
    ReaderConfig::new(),
).run()?;

// Access fields by index
let name = rows[0].get(0)?;  // "alice"
let age = rows[0].get(1)?;   // "30"

// Write back to a string
let output = writer::to_string(
    WriterConfig::new(),
    Some(vec!["name".into(), "age".into()]),
    rows,
).run()?;
// "name,age\nalice,30\nbob,25\n"
```

## Reading

### From a file

```rust
use csv_cat::reader::{self, ReaderConfig};

let rows = reader::read_all("data.csv", ReaderConfig::new()).run()?;
```

The file is opened, fully read, and closed within the `Io`.  Nothing happens until `.run()`.

### From a string

```rust
use csv_cat::reader::{self, ReaderConfig};

let rows = reader::from_str("a,b,c\n1,2,3\n", ReaderConfig::new()).run()?;
```

### With a Resource

For explicit acquire/release lifecycle:

```rust
use csv_cat::reader::{self, ReaderConfig};

let resource = reader::reader_resource("data.csv", ReaderConfig::new());
let result = resource.use_resource(|rows| {
    // rows: &Vec<Row>, available for the duration of this closure
    let count = rows.len();
    comp_cat_rs::effect::io::Io::pure(count)
}).run()?;
```

### Configuration

```rust
use csv_cat::reader::ReaderConfig;

let config = ReaderConfig::new()
    .has_headers(false)       // first row is data, not headers
    .delimiter(b'\t')         // tab-separated
    .flexible(true);          // allow varying field counts
```

| Method | Default | Description |
|--------|---------|-------------|
| `has_headers(bool)` | `true` | Whether the first row is a header |
| `delimiter(u8)` | `b','` | Field delimiter byte |
| `flexible(bool)` | `false` | Allow rows with different field counts |

## Writing

### To a file

```rust
use csv_cat::writer::{self, WriterConfig};
use csv_cat::row::Row;

let rows = vec![
    Row::from_record(csv::StringRecord::from(vec!["alice", "30"])),
    Row::from_record(csv::StringRecord::from(vec!["bob", "25"])),
];

writer::write_all(
    "output.csv",
    WriterConfig::new(),
    Some(vec!["name".into(), "age".into()]),
    rows,
).run()?;
```

### To a string

```rust
use csv_cat::writer::{self, WriterConfig};

let output = writer::to_string(
    WriterConfig::new(),
    Some(vec!["name".into(), "age".into()]),
    rows,
).run()?;
```

### Configuration

```rust
use csv_cat::writer::WriterConfig;

let config = WriterConfig::new()
    .delimiter(b'\t')         // tab-separated output
    .has_headers(false);      // skip header row
```

## Row

`Row` is a newtype over `csv::StringRecord` with accessor methods.

| Method | Signature | Description |
|--------|-----------|-------------|
| `get(index)` | `usize -> Result<&str, CsvError>` | Get field by index |
| `len()` | `-> usize` | Number of fields |
| `is_empty()` | `-> bool` | Whether the row has zero fields |
| `fields()` | `-> impl Iterator<Item = &str>` | Iterate over all fields |
| `to_vec()` | `-> Vec<String>` | Collect all fields as owned strings |
| `deserialize(headers)` | `-> Result<T, CsvError>` | Deserialize into a typed value via serde |

### Typed deserialization

```rust
use serde::Deserialize;
use csv_cat::reader::{self, ReaderConfig};

#[derive(Deserialize)]
struct Person {
    name: String,
    age: u32,
}

let data = "name,age\nalice,30\nbob,25\n";
let rows = reader::from_str(data, ReaderConfig::new()).run()?;

let headers = csv::StringRecord::from(vec!["name", "age"]);
let person: Person = rows[0].deserialize(Some(&headers))?;
// person.name == "alice", person.age == 30
```

## Error handling

`CsvError` is a hand-rolled enum covering all failure modes:

| Variant | Source | When |
|---------|--------|------|
| `Csv(csv::Error)` | csv crate | Malformed CSV, encoding issues |
| `Io(std::io::Error)` | std | File not found, permission denied |
| `MissingField { index }` | csv-cat | `Row::get` with out-of-bounds index |
| `Deserialize(String)` | csv-cat | `Row::deserialize` type mismatch |

All variants implement `From` for `?` ergonomics:

```rust
use csv_cat::error::CsvError;

fn process() -> Result<(), CsvError> {
    let rows = csv_cat::reader::from_str("a\n1\n", Default::default()).run()?;
    let val = rows[0].get(0)?;  // CsvError::MissingField on failure
    Ok(())
}
```

## Composing with other comp-cat-rs effects

Since everything is `Io<CsvError, _>`, you can compose CSV operations with the full comp-cat-rs toolkit.

### Error recovery

```rust
let rows = reader::read_all("maybe_missing.csv", ReaderConfig::new())
    .handle_error(|_| Vec::new());  // empty vec on failure
```

### Parallel processing

```rust
use comp_cat_rs::effect::fiber::par_zip;

let file_a = reader::read_all("a.csv", ReaderConfig::new());
let file_b = reader::read_all("b.csv", ReaderConfig::new());

// Read both files concurrently on separate threads
let (rows_a, rows_b) = par_zip(
    file_a.map_error(|e| /* ... */),
    file_b.map_error(|e| /* ... */),
).run()?;
```

### Chaining with other IO

```rust
let pipeline = reader::from_str("name\nalice\n", ReaderConfig::new())
    .map(|rows| rows.len())
    .flat_map(|count| {
        comp_cat_rs::effect::io::Io::pure(format!("Read {count} rows"))
    });

let message = pipeline.run()?;
// "Read 1 rows"
```

## Why comp-cat-rs?

The csv crate is excellent on its own.  csv-cat adds:

- **Lazy evaluation**: nothing executes until `.run()`, so you can build and compose pipelines before committing to side effects
- **Resource safety**: `reader_resource` / `writer_resource` use the bracket pattern to guarantee cleanup
- **Composability**: `map`, `flat_map`, `zip`, `handle_error` chain CSV operations with any other `Io`-based effect
- **Concurrency**: `Fiber::fork` and `par_zip` for parallel file processing, with no async/tokio

## License

MIT
