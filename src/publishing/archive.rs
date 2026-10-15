use anyhow::{Context, ensure};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub fn extract(source: &Path, destination: &Path, limit: u64) -> anyhow::Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(source)?)?;
    ensure!(zip.len() <= 20_000, "artifact contains too many files");
    fs::create_dir(destination)?;
    let mut total = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let relative = entry
            .enclosed_name()
            .context("artifact path escapes destination")?;
        ensure!(
            !relative.as_os_str().is_empty() && !entry.name()?.contains('\\'),
            "invalid artifact path"
        );
        ensure!(
            relative.components().count() <= 32,
            "artifact path is too deep"
        );
        ensure!(
            entry
                .unix_mode()
                .is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o040000 | 0o100000)),
            "artifact contains a link or special file"
        );
        total = total
            .checked_add(entry.size())
            .context("extracted length overflow")?;
        ensure!(total <= limit, "artifact exceeds extraction limit");
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        let entry_limit = entry.size().saturating_add(1);
        let copied = std::io::copy(&mut (&mut entry).take(entry_limit), &mut file)?;
        ensure!(copied == entry.size(), "artifact entry size mismatch");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(target, fs::Permissions::from_mode(0o644))?;
        }
    }

    ensure!(
        destination.join("index.html").is_file(),
        "artifact must have index.html at its root; check output directory"
    );
    Ok(())
}
