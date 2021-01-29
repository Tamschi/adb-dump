# adb-dump

<!-- markdownlint-disable no-duplicate-heading -->

[![Lib.rs](https://img.shields.io/badge/Lib.rs-*-84f)](https://lib.rs/crates/adb-dump)
[![Crates.io](https://img.shields.io/crates/v/adb-dump)](https://crates.io/crates/adb-dump)
[![Docs.rs](https://docs.rs/adb-dump/badge.svg)](https://docs.rs/crates/adb-dump)

![Rust 1.49.0](https://img.shields.io/static/v1?logo=Rust&label=&message=1.49.0&color=grey)
[![CI](https://github.com/Tamschi/adb-dump/workflows/CI/badge.svg?branch=develop)](https://github.com/Tamschi/adb-dump/actions?query=workflow%3ACI+branch%3Adevelop)
![Crates.io - License](https://img.shields.io/crates/l/adb-dump/0.0.1)

[![GitHub](https://img.shields.io/static/v1?logo=GitHub&label=&message=%20&color=grey)](https://github.com/Tamschi/adb-dump)
[![open issues](https://img.shields.io/github/issues-raw/Tamschi/adb-dump)](https://github.com/Tamschi/adb-dump/issues)
[![open pull requests](https://img.shields.io/github/issues-pr-raw/Tamschi/adb-dump)](https://github.com/Tamschi/adb-dump/pulls)
[![crev reviews](https://web.crev.dev/rust-reviews/badge/crev_count/adb-dump.svg)](https://web.crev.dev/rust-reviews/crate/adb-dump/)

A LineageOS update soft-bricked my phone, and I could not find any good data-dump software. This tries(!) to pull )(almost) everything it can as accurately as it can into reasonably-sized ZIP files. **Note that certain cache (sub)folders are ignored by default!**

Developed mainly against TWRP sitting in its main menu.

## Warning

**Always validate your backups after making them!**

This software comes without any warranties regarding data integrity whatsoever (see licenses for more information), and some of the libraries it depends on are not as reliable as they should be. I tried to work around this, but I can't say with certainty that there aren't any silent errors left.

(If you know a good *reliable* archive library then please tell me about it!)

## Installation

### bin

```cmd
cargo install adb-dump
```

### lib

Please use [cargo-edit](https://crates.io/crates/cargo-edit) to always add the latest version of this library:

```cmd
cargo add adb-dump
```

## Example

### bin

```cmd
REM Dump as much data as is accessible, split into one TAR file per root subdirectory + one for files in the root subdirectory (as adb-dump_root).

REM Root subdirectories are present in the archives.

adb-dump --split /
```

### lib

```rust
// TODO_EXAMPLE
```

## CLI

```text
TODO
```

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## [Code of Conduct](CODE_OF_CONDUCT.md)

## [Changelog](CHANGELOG.md)

## Versioning

`adb-dump` strictly follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with the following exceptions:

* The minor version will not reset to 0 on major version changes (except for v1).  
Consider it the global feature level.
* The patch version will not reset to 0 on major or minor version changes (except for v0.1 and v1).  
Consider it the global patch level.

This includes the Rust version requirement specified above.  
Earlier Rust versions may be compatible, but this can change with minor or patch releases.

Which versions are affected by features and patches can be determined from the respective headings in [CHANGELOG.md](CHANGELOG.md).
