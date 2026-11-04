//! Besides Tauri's own build: what the speech engine (transcribe-cpp, see
//! `src/local.rs` and Cargo.toml) needs to link and to ship.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Where the engine's libraries are gathered for the bundles: next to the
/// executable on Windows (tauri.windows.conf.json), in `/usr/lib/Parler`
/// (deb, rpm) or `usr/lib` (AppImage) on Linux (tauri.linux.conf.json).
const STAGING: &str = "transcribe-libs";

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "macos" => link_clang_runtime(),
        "linux" => {
            // Where the packages put libtranscribe and the ggml modules.
            println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/Parler:$ORIGIN/../lib");
            // `cargo run` and `cargo test` find them where they were built.
            if env::var("PROFILE").as_deref() != Ok("release") {
                if let Some(dir) = env::var_os("DEP_TRANSCRIBE_CPP_RUNTIME_DIR") {
                    println!(
                        "cargo:rustc-link-arg=-Wl,-rpath,{}",
                        Path::new(&dir).display()
                    );
                }
            }
        }
        _ => {}
    }
    stage_engine_libraries();
    tauri_build::build()
}

/// macOS: ggml's Metal code checks the system version with `@available`.
/// Built for an older macOS than the SDK's (11.0 here), clang turns those
/// checks into calls to `__isPlatformVersionAtLeast`, from clang's runtime,
/// which rustc doesn't link. Builds for the host's own version fold them
/// away, so only release builds would fail.
fn link_clang_runtime() {
    let Ok(output) = std::process::Command::new("clang")
        .arg("--print-resource-dir")
        .output()
    else {
        println!("cargo:warning=clang not found: can't link libclang_rt.osx.a");
        return;
    };
    let resource_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let dir = Path::new(&resource_dir).join("lib/darwin");
    if !dir.join("libclang_rt.osx.a").exists() {
        println!("cargo:warning=libclang_rt.osx.a not in {}", dir.display());
        return;
    }
    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}

/// Windows and Linux: the engine is a shared library plus backend modules
/// (Vulkan, one CPU module per instruction set) loaded at launch. They are
/// copied to `transcribe-libs/` for the bundles. (On Windows they need the
/// Visual C++ runtime, which the bundles ship too: `bundleVCRuntime` in
/// tauri.windows.conf.json.) macOS links the engine in: nothing to ship,
/// the folder stays empty.
fn stage_engine_libraries() {
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_RUNTIME_DIR");
    println!("cargo:rerun-if-env-changed=DEP_TRANSCRIBE_CPP_MODULE_DIR");
    let dest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join(STAGING);
    // Fresh each time, so a module dropped upstream never lingers.
    let _ = fs::remove_dir_all(&dest);
    fs::create_dir_all(&dest).expect("create transcribe-libs/");

    let Some(runtime_dir) = env::var_os("DEP_TRANSCRIBE_CPP_RUNTIME_DIR") else {
        return;
    };
    let mut dirs = vec![PathBuf::from(runtime_dir)];
    if let Some(module_dir) = env::var_os("DEP_TRANSCRIBE_CPP_MODULE_DIR") {
        dirs.push(PathBuf::from(module_dir));
    }

    // Each library once, under the name the loader asks for. Linux keeps a
    // chain of names (libggml.so -> .so.0 -> .so.0.9.4) that the packagers
    // would turn into three copies: the shortest versioned name is the
    // linked one, a bare `.so` the dlopen'd module's.
    let mut best: BTreeMap<String, (usize, PathBuf, String)> = BTreeMap::new();
    for dir in &dirs {
        println!("cargo:rerun-if-changed={}", dir.display());
        let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
        for path in entries.flatten().map(|e| e.path()) {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let (stem, rank) = match versioned_so(&name) {
                Some((stem, 0)) => (stem.to_string(), usize::MAX),
                Some((stem, depth)) => (stem.to_string(), depth - 1),
                None if name.ends_with(".dll") || name.ends_with(".dylib") => (name.clone(), 0),
                None => continue,
            };
            if best.get(&stem).is_none_or(|(r, _, _)| rank < *r) {
                best.insert(stem, (rank, path, name));
            }
        }
    }
    assert!(
        !best.is_empty(),
        "no transcribe-cpp libraries in {dirs:?}: the bundles would load no engine"
    );
    for (_, src, name) in best.values() {
        fs::copy(src, dest.join(name)).unwrap_or_else(|e| panic!("copy {}: {e}", src.display()));
    }
}

/// `libfoo.so` → ("libfoo", 0), `libfoo.so.0.9` → ("libfoo", 2); `None`
/// for anything else.
fn versioned_so(name: &str) -> Option<(&str, usize)> {
    let at = name.find(".so")?;
    let (stem, rest) = (&name[..at], &name[at + 3..]);
    if rest.is_empty() {
        return Some((stem, 0));
    }
    let parts: Vec<&str> = rest.strip_prefix('.')?.split('.').collect();
    parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .then_some((stem, parts.len()))
}
