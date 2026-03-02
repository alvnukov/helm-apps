use include_dir::{include_dir, Dir, DirEntry};
use std::fs;
use std::io;
use std::path::Path;

static EMBEDDED_HELM_APPS: Dir<'_> = include_dir!("$OUT_DIR/helm-apps");

pub fn has_helm_apps_chart() -> bool {
    EMBEDDED_HELM_APPS.get_file("Chart.yaml").is_some()
}

pub fn extract_helm_apps_chart(dst: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(dst)?;
    write_dir(&EMBEDDED_HELM_APPS, dst)
}

fn write_dir(dir: &Dir<'_>, dst: &Path) -> Result<(), io::Error> {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(child) => {
                let next = dst.join(child.path().file_name().unwrap_or_default());
                fs::create_dir_all(&next)?;
                write_dir(child, &next)?;
            }
            DirEntry::File(file) => {
                let target = dst.join(file.path().file_name().unwrap_or_default());
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(target, file.contents())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_chart_is_available() {
        assert!(
            has_helm_apps_chart(),
            "embedded helm-apps Chart.yaml not found"
        );
    }
}
