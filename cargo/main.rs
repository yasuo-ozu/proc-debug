use anyhow::Result;
use cargo::core::{compiler, resolver, Package, PackageId, PackageIdSpec, PackageSet, Resolve};
use cargo::ops::WorkspaceResolve;
use cargo::{CargoResult, GlobalContext};
use clap::Parser;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Input for `cargo proc-debug` command
#[derive(Parser)]
#[command(bin_name = "cargo", version, author, disable_help_subcommand = true)]
enum Subcommand {
    #[command(name = "proc-debug", version, author, disable_version_flag = true)]
    ProcDebug(Arguments),
}

#[derive(Parser)]
struct Arguments {
    /// specify the manifest path for this library
    #[arg(long, short = 'm', value_name = "PATH")]
    manifest_path: Option<PathBuf>,

    /// debug macros called only from the specified packages
    #[arg(long, short = 'p', value_name = "PACKAGE")]
    package: Vec<String>,

    /// debug macro calls only in this package's library
    #[arg(long)]
    lib: bool,

    /// debug macro calls in all bins
    #[arg(long)]
    bins: bool,

    /// debug macro calls only in specified binary
    #[arg(long, value_name = "NAME")]
    bin: Vec<String>,

    /// debug macro calls in all examples
    #[arg(long)]
    examples: bool,

    /// debug macro calls only in specified example
    #[arg(long, value_name = "NAME")]
    example: Vec<String>,

    /// debug macro calls in library tests
    #[arg(long)]
    tests: bool,

    /// debug macro calls only in specified test target
    #[arg(long, value_name = "NAME")]
    test: Vec<String>,

    /// debug macro calls in all benches
    #[arg(long)]
    benches: bool,

    /// debug macro calls only in specified benchmark
    #[arg(long, value_name = "NAME")]
    bench: Vec<String>,

    /// space or comma separated list of features to activate
    #[arg(short = 'F', long, value_name = "FEATURES")]
    features: Option<String>,

    /// activate all available features
    #[arg(long)]
    all_features: bool,

    /// do not activate the `default` feature
    #[arg(long)]
    no_default_features: bool,

    /// show version
    #[arg(long, short = 'v')]
    version: bool,

    /// check for the target triple
    #[arg(long)]
    target: Option<String>,

    /// absolute (begins with '::') or partial path to filter debugging proc-macros
    #[arg(long, short = 'P')]
    path: Vec<String>,

    /// do not omit longer outputs
    #[arg(long)]
    verbose: bool,

    /// keywords to filter debugging proc-macros
    #[arg(value_name = "KEYWORD")]
    keywords: Vec<String>,
}

impl Arguments {
    fn get_env(&self) -> String {
        let mut ret = format!("-a");
        for p in &self.path {
            ret += &format!(" --path \"{}\"", p);
        }
        if self.verbose {
            ret += " -v";
        }
        for k in &self.keywords {
            ret += &format!(" \"{}\"", k);
        }
        ret
    }

    fn extend_args(&self, args: &mut Command) {
        if let Some(p) = &self.manifest_path {
            args.arg("--manifest-path");
            args.arg(p.to_str().unwrap());
        }
        for p in &self.package {
            args.arg("--package");
            args.arg(p);
        }
        if self.lib {
            args.arg("--lib");
        }
        for b in &self.bin {
            args.arg("--bin");
            args.arg(b);
        }
        for e in &self.example {
            args.arg("--example");
            args.arg(e);
        }
        if self.tests {
            args.arg("--tests");
        }
        if self.benches {
            args.arg("--benches");
        }
        if self.examples {
            args.arg("--examples");
        }
        for t in &self.test {
            args.arg("--test");
            args.arg(t);
        }
        for b in &self.bench {
            args.arg("--bench");
            args.arg(b);
        }
        if let Some(f) = &self.features {
            args.arg("--features");
            args.arg(f);
        }
        if self.all_features {
            args.arg("--all-features");
        }
        if self.no_default_features {
            args.arg("--no-default-features");
        }
        if let Some(t) = &self.target {
            args.arg("--target");
            args.arg(t);
        }
    }
}

fn find_manifest_path() -> std::io::Result<PathBuf> {
    let dir = std::env::current_dir()?;
    let mut dir = dir.as_path();
    loop {
        let path = dir.join("Cargo.toml");
        if path.try_exists()? {
            return Ok(path);
        }
        dir = match dir.parent() {
            Some(parent) => parent,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Cargo.toml not found",
                ))
            }
        };
    }
}

fn ensure_proc_debug_crate(sysroot: &Path, version: &str) -> Result<PathBuf> {
    let url = format!("https://github.com/yasuo-ozu/proc-debug/archive/refs/tags/v{version}.zip");
    let mut path = PathBuf::from(sysroot);
    path.push(format!("proc-debug-{version}"));
    if !path.exists() {
        let data = reqwest::blocking::get(url)?
            .bytes()?
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;
        archive.extract(sysroot)?;
    }
    Ok(path)
}

fn resolve_workspace<'gctx>(
    args: &Arguments,
    gctx: &'gctx GlobalContext,
) -> CargoResult<(PathBuf, WorkspaceResolve<'gctx>)> {
    let manifest_path = args
        .manifest_path
        .clone()
        .map(|p| std::path::absolute(p).unwrap())
        .unwrap_or_else(|| find_manifest_path().unwrap());
    let mut workspace = cargo::core::Workspace::new(&manifest_path, &gctx)?;
    workspace.set_ignore_lock(true);
    let target_dir = workspace.target_dir().as_path_unlocked().to_owned();
    let mut sysroot = target_dir.clone();
    sysroot.push("proc-debug-root");
    let lib_path = ensure_proc_debug_crate(&sysroot, env!("CARGO_PKG_VERSION"))?;
    let mut lib_manifest_path = lib_path.clone();
    lib_manifest_path.push("Cargo.toml");
    workspace.load(&lib_manifest_path)?;

    let kinds = compiler::CompileKind::from_requested_targets(
        gctx,
        args.target.iter().cloned().collect::<Vec<_>>().as_slice(),
    )?;
    let mut target_data = compiler::RustcTargetData::new(&workspace, kinds.as_slice())?;
    let features = resolver::CliFeatures::from_command_line(
        args.features.as_slice(),
        args.all_features,
        !args.no_default_features,
    )?;

    let package_specs = args
        .package
        .iter()
        .cloned()
        .map(|o| cargo::core::PackageIdSpec::new(o))
        .chain(Some(PackageIdSpec::new("proc-debug".to_owned())))
        .collect::<Vec<_>>();
    cargo::ops::resolve_ws_with_opts(
        &workspace,
        &mut target_data,
        kinds.as_slice(),
        &features,
        package_specs.as_slice(),
        if args.test.len() + args.example.len() + args.bench.len() > 0
            || args.tests
            || args.examples
            || args.benches
        {
            resolver::HasDevUnits::Yes
        } else {
            resolver::HasDevUnits::No
        },
        resolver::ForceAllTargets::No,
    )
    .map(|o| (lib_path, o))
}

fn resolve_deps(
    pids: impl IntoIterator<Item = PackageId>,
    resolve: &Resolve,
) -> BTreeSet<PackageId> {
    let mut unresolved_deps: BTreeSet<_> = pids.into_iter().collect();
    let mut resolved_deps = BTreeSet::new();
    while unresolved_deps.len() > 0 {
        let ret = unresolved_deps
            .iter()
            .map(|d| resolve.deps(d.clone()).map(|(a, _)| a))
            .flatten()
            .collect::<BTreeSet<_>>();
        resolved_deps.extend(&unresolved_deps);
        unresolved_deps = ret.difference(&resolved_deps).cloned().collect();
    }
    resolved_deps
}

fn resolve_all_packages(
    package_set: &PackageSet,
    resolve: &Resolve,
    proc_filter: &[String],
) -> Vec<PackageId> {
    let lib_packages = package_set
        .package_ids()
        .filter(|pid| pid.clone().name() == "proc-debug")
        .collect::<Vec<_>>();
    let lib_package_deps = resolve_deps(lib_packages, resolve);
    let proc_packages = package_set
        .packages()
        .filter(|pkg| matches!(pkg.library(), Some(targ) if targ.proc_macro()))
        .filter(|pkg| {
            proc_filter.len() == 0 || proc_filter.iter().any(|m| pkg.name() == m.as_str())
        })
        .map(|pkg| pkg.package_id())
        .collect::<BTreeSet<_>>();
    proc_packages
        .difference(&lib_package_deps)
        .cloned()
        .collect::<Vec<_>>()
}

fn modify_rust_file(content: String) -> Result<String> {
    let content =
        comment::rust::strip(content).map_err(|_| anyhow::Error::msg("Cannot remove comment"))?;
    let mut modified = Vec::new();
    for line in content.lines() {
        let line = line.replace(
            "#[proc_macro]",
            "#[::proc_debug::proc_debug]\n#[proc_macro]",
        );
        let line = line.replace(
            "#[proc_macro_attribute]",
            "#[::proc_debug::proc_debug]\n#[proc_macro_attribute]",
        );
        let line = line.replace(
            "#[proc_macro_derive",
            "#[::proc_debug::proc_debug]\n#[proc_macro_derive",
        );
        modified.push(line);
    }
    Ok(modified.join("\n"))
}

fn modify_toml_file(content: String, lib_path: &Path) -> Result<String> {
    if content.find("proc-debug").is_some() {
        Ok(content)
    } else {
        Ok(format!(
            "{content}\n\n[dependencies.proc-debug]\npath = \"{}\"",
            lib_path.to_str().unwrap()
        ))
    }
}

fn backup_and_modify(
    path: PathBuf,
    f: impl FnOnce(String) -> Result<String>,
) -> Result<Option<PathBuf>> {
    let fname = path.file_name().unwrap().to_str().unwrap();
    let mut bak_path = path.clone();
    bak_path.set_file_name(format!("{fname}.proc-debug-bak"));
    if bak_path.exists() {
        return Ok(None);
    }
    let content = String::from_utf8(std::fs::read(&path)?)?;
    let modified = f(content)?;
    std::fs::rename(&path, &bak_path)?;
    if let Err(e) = std::fs::write(&path, modified) {
        std::fs::rename(&bak_path, &path)?;
        Err(e)?;
    }
    Ok(Some(path))
}

fn unmodify(path: &Path) -> std::io::Result<()> {
    let fname = path.file_name().unwrap().to_str().unwrap();
    let mut bak_path = path.to_owned();
    bak_path.set_file_name(format!("{fname}.proc-debug-bak"));
    let _ = std::fs::remove_file(path);
    std::fs::rename(bak_path, path)
}

fn modify_files_of_package(pkg: &Package, lib_path: &Path) -> Result<Vec<PathBuf>> {
    let target = pkg.library().unwrap();
    let mut src_path = target.src_path().path().unwrap().to_owned();
    if !src_path.is_absolute() {
        let mut new_path = pkg.manifest_path().to_owned();
        new_path.pop();
        new_path.extend(src_path.into_iter());
        src_path = new_path;
    }
    let mut ret = Vec::new();
    let src_path = src_path.canonicalize()?;
    ret.extend(backup_and_modify(src_path, |content| {
        modify_rust_file(content)
    })?);
    ret.extend(backup_and_modify(
        pkg.manifest_path().to_owned(),
        |content| modify_toml_file(content, lib_path),
    )?);
    Ok(ret)
}

fn main() {
    let Subcommand::ProcDebug(args) = Subcommand::parse();
    if args.version {
        println!("cargo-proc-debug {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let context =
        cargo::util::context::GlobalContext::default().unwrap_or_else(|e| panic!("{}", e));
    let (
        lib_path,
        WorkspaceResolve {
            targeted_resolve,
            pkg_set,
            ..
        },
    ) = resolve_workspace(&args, &context).unwrap_or_else(|e| panic!("{}", e));
    let proc_filter = args
        .path
        .iter()
        .filter_map(|s| {
            if s.starts_with("::") {
                s.split(":").skip(2).next().map(|s| s.to_owned())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let pkg_ids = resolve_all_packages(&pkg_set, &targeted_resolve, proc_filter.as_slice());
    struct Guard(Vec<PathBuf>);
    impl Drop for Guard {
        fn drop(&mut self) {
            for p in &self.0 {
                let _ = unmodify(p);
            }
        }
    }
    let mut modified_packages = Guard(Vec::new());
    for id in &pkg_ids {
        modified_packages.0.extend(
            modify_files_of_package(&pkg_set.get_one(id.clone()).unwrap(), lib_path.as_path())
                .unwrap_or_else(|e| panic!("{}", e)),
        );
        println!("PKG {}", &id);
    }
    let mut command = Command::new(std::env::var("CARGO").unwrap_or("cargo".to_owned()));
    command.arg("check");
    args.extend_args(&mut command);
    command.env("PROC_DEBUG_FLAGS", args.get_env());
    let _ = command.status().unwrap_or_else(|e| panic!("{e}"));
}
