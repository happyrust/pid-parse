![oxidized-mdf's build status][build-status]

# oxidized-mdf

oxidized-mdf provides a reader library for SQL Server Database File written in Rust.

In order to read a MDF file checkout the code and run the example:

```bash
cargo run --example print_all -- data/spg_verein_TST.mdf

# For more information use the help:
cargo run --example print_all -- --help
```

## Why is This Crate Licensed Under the GPLv3?

The code is based on [OrcaMDF][1] and the original code is licensed under the GPLv3.

[1]: https://github.com/improvedk/OrcaMDF "OrcaMDF - A C# parser for MDF files"
[build-status]: https://gitlab.com/schrieveslaach/oxidized-mdf/badges/master/pipeline.svg "Build Status"
