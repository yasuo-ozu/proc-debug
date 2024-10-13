use std::path::PathBuf;
fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

fn main() {
    if let Ok(_) = std::env::var("PROC_DEBUG_FLAGS") {
        // Force to rerun all times, to show print
        let mut out_file = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        out_file.push("out.txt");
        std::fs::write(&out_file, format!("{}", random())).unwrap();
        println!("cargo::rerun-if-changed={}", out_file.display());
    }
    println!("cargo::rerun-if-env-changed=PROC_DEBUG_FLAGS");
}
