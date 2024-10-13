# proc-debug crate [![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GitHub Actions]][actions]

[Latest Version]: https://img.shields.io/crates/v/proc-debug.svg
[crates.io]: https://crates.io/crates/proc-debug
[Documentation]: https://img.shields.io/docsrs/proc-debug
[docs.rs]: https://docs.rs/proc-debug/latest/proc-debug/
[GitHub Actions]: https://github.com/yasuo-ozu/proc-debug/actions/workflows/rust.yml/badge.svg
[actions]: https://github.com/yasuo-ozu/proc-debug/actions/workflows/rust.yml

## Belief configuration

```Cargo.toml
#[dependencies]
proc-debug = "0.1.0"
```

```lib.rs ignore
#[proc_macro]
#[proc_debug::proc_debug]
fn my_macro(..) -> TokenStream { .. }
```

- show help

```bash
$ PROC_DEBUG_FLAGS="--help" cargo build --test <test-name> -- --nocapture
```

- show all dumps

```bash
$ PROC_DEBUG_FLAGS="-a" cargo build --test <test-name> -- --nocapture
```

- filtered

```bash
$ PROC_DEBUG_FLAGS="your_macro_path" cargo build --test <test-name> -- --nocapture
```
