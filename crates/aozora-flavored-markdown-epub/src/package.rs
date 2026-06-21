//! Phase 4 — package the bundle into a `.epub` ZIP.
//!
//! ## Specifications consumed
//!
//! - **OCF abstract container layout** — [EPUB 3 OCF §3.4](https://www.w3.org/TR/epub-33/#sec-container-abstract).
//!   The `mimetype` file must be the *first* entry in the ZIP, stored
//!   uncompressed (no extra fields allowed), with byte content
//!   exactly `application/epub+zip` (20 bytes, no trailing newline).
//! - **ZIP file format** — [PKWARE APPNOTE 6.3.10 §4.4](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT).
//!   The local file header for `mimetype` carries
//!   `compression method = 0` (Stored). Subsequent entries are
//!   compressed with method 8 (Deflate) per OCF §3.5.
//! - **`META-INF/container.xml`** is the second entry (still
//!   Deflated); `OEBPS/package.opf` is the OPF root referenced by
//!   container.xml's `<rootfile full-path="OEBPS/package.opf">`.
//!
//! The `zip` crate is invoked with `default-features = false +
//! deflate`, so it neither pulls in `time` (its default-on
//! `_deflate-any` companion) nor brings in alternative compressors
//! we don't need.

use std::fs;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use zip::write::{SimpleFileOptions, ZipWriter};

use crate::compose::Bundle;
use crate::{Error, Result};

pub(crate) fn write(out: &Path, bundle: &Bundle) -> Result<()> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| Error::PackageIo {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let file = fs::File::create(out).map_err(|source| Error::PackageIo {
        path: out.to_path_buf(),
        source,
    })?;
    let mut zip = ZipWriter::new(BufWriter::new(file));

    write_stored(&mut zip, "mimetype", bundle.mimetype.as_bytes(), out)?;
    write_deflated(
        &mut zip,
        "META-INF/container.xml",
        bundle.container.as_bytes(),
        out,
    )?;
    write_deflated(
        &mut zip,
        "OEBPS/package.opf",
        bundle.package_opf.as_bytes(),
        out,
    )?;
    write_deflated(
        &mut zip,
        "OEBPS/nav.xhtml",
        bundle.nav_xhtml.as_bytes(),
        out,
    )?;
    for asset in &bundle.assets {
        write_deflated(&mut zip, &asset.path, &asset.contents, out)?;
    }
    for item in &bundle.spine {
        write_deflated(&mut zip, &item.path, &item.contents, out)?;
    }
    zip.finish().map_err(|source| Error::Package {
        path: out.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn write_stored<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    out_path: &Path,
) -> Result<()> {
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(name, opts)
        .map_err(|source| Error::Package {
            path: out_path.to_path_buf(),
            source,
        })?;
    zip.write_all(bytes).map_err(|source| Error::PackageIo {
        path: out_path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn write_deflated<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    out_path: &Path,
) -> Result<()> {
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(name, opts)
        .map_err(|source| Error::Package {
            path: out_path.to_path_buf(),
            source,
        })?;
    zip.write_all(bytes).map_err(|source| Error::PackageIo {
        path: out_path.to_path_buf(),
        source,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::Bundle;

    fn minimal_bundle() -> Bundle {
        Bundle {
            mimetype: "application/epub+zip",
            container: String::new(),
            package_opf: String::new(),
            nav_xhtml: String::new(),
            spine: vec![],
            assets: vec![],
        }
    }

    #[test]
    fn write_produces_a_nonempty_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("book.epub");

        write(&out, &minimal_bundle()).expect("write should succeed");

        assert!(out.exists(), "the .epub file should exist");
        let len = fs::metadata(&out).expect("metadata").len();
        assert!(len > 0, "the .epub file should be non-empty");
    }

    #[test]
    fn write_creates_missing_parent_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("nested/sub/book.epub");

        write(&out, &minimal_bundle()).expect("write should create parents");

        assert!(out.exists(), "the nested .epub file should exist");
    }

    #[test]
    fn write_fails_when_parent_is_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Create a regular file, then aim the output *inside* it so that
        // `create_dir_all` on the parent must fail (NotADirectory).
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"not a directory").expect("write blocker");
        let out = blocker.join("book.epub");

        let err = write(&out, &minimal_bundle()).expect_err("write should fail");
        assert!(
            matches!(err, Error::PackageIo { ref path, .. } if path == &blocker),
            "expected PackageIo for the parent path, got {err:?}",
        );
    }
}
