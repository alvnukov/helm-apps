use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(err) = run() {
        panic!("failed to prepare embedded helm-apps asset: {err}");
    }
}

fn run() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| format!("CARGO_MANIFEST_DIR: {e}"))?);
    let src = manifest_dir.join("../../charts/helm-apps");
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|e| format!("OUT_DIR: {e}"))?);
    let dst = out_dir.join("helm-apps");

    if !src.join("Chart.yaml").exists() {
        return Err(format!(
            "source chart not found: {}",
            src.join("Chart.yaml").display()
        ));
    }

    emit_rerun_markers(&src).map_err(|e| format!("emit rerun markers: {e}"))?;
    copy_dir_replace(&src, &dst).map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
    Ok(())
}

fn emit_rerun_markers(root: &Path) -> io::Result<()> {
    println!("cargo:rerun-if-changed={}", root.display());
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_markers(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

fn copy_dir_replace(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_recursive(src, dst)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let out_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            fs::create_dir_all(&out_path)?;
            copy_dir_recursive(&entry_path, &out_path)?;
        } else {
            fs::copy(&entry_path, &out_path)?;
        }
    }
    Ok(())
}

