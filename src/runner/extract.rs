//! Extracting runner archives.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::Path;

use flate2::read::GzDecoder;
use zip::ZipArchive;

use crate::error::{Error, Result};

/// Extract `archive` into `dest`, creating `dest` if needed.
///
/// Supports `.zip`, `.tar.gz`, and `.tgz` archives. ZIP entries that would
/// escape `dest` are skipped.
///
/// # Errors
///
/// Returns [`Error::UnsupportedArchive`] for unknown formats, or an I/O,
/// archive, or ZIP error if extraction fails.
pub fn extract(archive: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    let name = archive
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    match name.rsplit_once('.') {
        Some((_, "zip")) => extract_zip(archive, dest),
        Some((stem, "gz"))
            if Path::new(stem).extension().and_then(OsStr::to_str) == Some("tar") =>
        {
            extract_tar_gz(archive, dest)
        }
        Some((_, "tgz")) => extract_tar_gz(archive, dest),
        _ => Err(Error::UnsupportedArchive(name)),
    }
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    tracing::debug!("extracting tar.gz archive {}", archive.display());
    let file = File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(dest)?;
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    tracing::debug!("extracting zip archive {}", archive.display());
    let file = File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;

    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            tracing::warn!("skipping zip entry with unsafe path");
            continue;
        };

        let out_path = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            io::copy(&mut entry, &mut out)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::extract;

    #[test]
    fn extracts_tar_gz_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.tar.gz");
        {
            let file = File::create(&archive).unwrap();
            let encoder = GzEncoder::new(file, Compression::fast());
            let mut builder = tar::Builder::new(encoder);

            let data = b"#!/bin/sh\necho hi\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "config.sh", &data[..])
                .unwrap();

            let encoder = builder.into_inner().unwrap();
            encoder.finish().unwrap();
        }

        let dest = dir.path().join("out");
        extract(&archive, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("config.sh")).unwrap(),
            "#!/bin/sh\necho hi\n"
        );
    }

    #[test]
    fn extracts_nested_zip_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.start_file("config.cmd", options).unwrap();
            zip.write_all(b"echo hi").unwrap();

            zip.start_file("bin/Runner.Listener.exe", options).unwrap();
            zip.write_all(b"binary").unwrap();

            zip.finish().unwrap();
        }

        let dest = dir.path().join("out");
        extract(&archive, &dest).unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("config.cmd")).unwrap(),
            "echo hi"
        );
        assert_eq!(
            fs::read(dest.join("bin").join("Runner.Listener.exe")).unwrap(),
            b"binary"
        );
    }

    #[test]
    fn rejects_unknown_archive_format() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.rar");
        File::create(&archive).unwrap();

        assert!(extract(&archive, &dir.path().join("out")).is_err());
    }

    #[test]
    fn extracts_tgz_alias() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.tgz");
        {
            let file = File::create(&archive).unwrap();
            let encoder = GzEncoder::new(file, Compression::fast());
            let mut builder = tar::Builder::new(encoder);

            let data = b"runner";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "runner", &data[..])
                .unwrap();

            let encoder = builder.into_inner().unwrap();
            encoder.finish().unwrap();
        }

        let dest = dir.path().join("out");
        extract(&archive, &dest).unwrap();

        assert_eq!(fs::read(dest.join("runner")).unwrap(), b"runner");
    }

    #[test]
    fn creates_directories_from_zip_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            zip.add_directory("bin/", options).unwrap();
            zip.start_file("bin/runner", options).unwrap();
            zip.write_all(b"binary").unwrap();
            zip.finish().unwrap();
        }

        let dest = dir.path().join("out");
        extract(&archive, &dest).unwrap();

        assert!(dest.join("bin").is_dir());
        assert_eq!(
            fs::read(dest.join("bin").join("runner")).unwrap(),
            b"binary"
        );
    }

    #[test]
    fn rejects_missing_archive() {
        let dir = tempfile::tempdir().unwrap();

        assert!(extract(&dir.path().join("missing.zip"), &dir.path().join("zip")).is_err());
        assert!(extract(&dir.path().join("missing.tar.gz"), &dir.path().join("tar")).is_err());
    }

    #[test]
    fn skips_zip_entries_that_escape_destination() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("runner.zip");
        let mut wrote = false;
        {
            let file = File::create(&archive).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();

            if zip.start_file("../escape.txt", options).is_ok() {
                zip.write_all(b"x").unwrap();
                wrote = true;
            }
            zip.finish().unwrap();
        }
        if !wrote {
            return;
        }

        let dest = dir.path().join("out");
        extract(&archive, &dest).unwrap();

        assert!(!dir.path().join("escape.txt").exists());
        assert!(!dest.join("escape.txt").exists());
    }
}
