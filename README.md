# proc-debug crate [![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GitHub Actions]][actions]

[Latest Version]: https://img.shields.io/crates/v/proc-debug.svg
[crates.io]: https://crates.io/crates/proc-debug
[Documentation]: https://img.shields.io/docsrs/proc-debug
[docs.rs]: https://docs.rs/proc-debug/latest/proc-debug/
[GitHub Actions]: https://github.com/yasuo-ozu/proc-debug/actions/workflows/rust.yml/badge.svg
[actions]: https://github.com/yasuo-ozu/proc-debug/actions/workflows/rust.yml

![Screenshot](https://raw.githubusercontent.com/yasuo-ozu/proc-debug/refs/heads/main/proc_debug.png)

# Use proc-debug via CLI (recommended)

```bash
$ cargo install --path cargo
...
$ cargo proc-debug --help
Usage: cargo proc-debug [OPTIONS] [KEYWORD]...

Arguments:
  [KEYWORD]...  keywords to filter debugging proc-macros

Options:
  -m, --manifest-path <PATH>  specify the manifest path for this library
  -p, --package <PACKAGE>     debug macros called only from the specified packages
      --lib                   debug macro calls only in this package's library
      --bins                  debug macro calls in all bins
      --bin <NAME>            debug macro calls only in specified binary
      --examples              debug macro calls in all examples
      --example <NAME>        debug macro calls only in specified example
      --tests                 debug macro calls in library tests
      --test <NAME>           debug macro calls only in specified test target
      --benches               debug macro calls in all benches
      --bench <NAME>          debug macro calls only in specified benchmark
  -F, --features <FEATURES>   space or comma separated list of features to activate
      --all-features          activate all available features
      --no-default-features   do not activate the `default` feature
  -v, --version               show version
      --target <TARGET>       check for the target triple
  -P, --path <PATH>           absolute (begins with '::') or partial path to filter debugging
                              proc-macros
      --verbose               do not omit longer outputs
  -h, --help                  Print help

# Run proc-debug in the crate dir, which calls proc macros
$ cargo proc-debug --verbose
👉 input of my_macro_crate::macro (macro/lib.rs:1008)
...
👉 output of my_macro_crate::macro (macro/lib.rs:1008)
...
```

# Use proc-debug as a library

## Belief configuration

- Cargo.toml

```Cargo.toml
#[dependencies]
proc-debug = "0.1"
```

- lib.rs

```lib.rs ignore
#[proc_macro]
#[proc_debug::proc_debug]
fn my_macro(attr: TokenStream, input: TokenStream) -> TokenStream { .. }
```

- show help (--nocapture is important)

```bash
$ PROC_DEBUG_FLAGS="--help" cargo build --test <test-name> -- --nocapture
Usage: proc-debug [-a] [-n <not...>] [-p <path...>] [-d <depth>] [-c <count>]
                  [-v] [queries...]

Input for `proc-debug`

Options:
  -a, --all            debug all macros
  -n, --not <not>      hide outputs match
  -p, --path <path>    full or partial path of macro definition
  -d, --depth <depth>  depth to show in macro output
  -c, --count <count>  count to show in display
  -v, --verbose        verbose
  -h, --help           Show this help message and exit.
```

- show all dumps (called from `<test-name>`)

```bash
$ PROC_DEBUG_FLAGS="-a" cargo build --test <test-name> -- --nocapture
```

- filtered

```bash
$ PROC_DEBUG_FLAGS="your_macro_path" cargo build --test <test-name> -- --nocapture
```
