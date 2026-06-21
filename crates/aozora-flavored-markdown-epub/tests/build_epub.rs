//! End-to-end tests for the public [`build`] entry point: run it on a
//! fixture manuscript and inspect the produced EPUB container.

mod common;

use std::path::{Path, PathBuf};

use aozora_flavored_markdown_epub::{BuildOptions, Error, build};
use common::{Entry, entry_text, fixture, read_epub};

const HORIZONTAL_BOOK: &str = "\
title = \"Test Book\"
creator = \"Test Author\"
language = \"ja\"
writing_mode = \"horizontal\"
";

const VERTICAL_BOOK: &str = "\
title = \"縦書きの本\"
creator = \"著者\"
language = \"ja\"
writing_mode = \"vertical\"
";

fn build_into(dir: &Path, out_name: &str) -> PathBuf {
    let out = dir.join(out_name);
    build(&BuildOptions {
        input: &dir.join("manuscript"),
        metadata: &dir.join("book.toml"),
        output: &out,
    })
    .expect("build succeeds");
    out
}

fn opf(entries: &[Entry]) -> String {
    entry_text(entries, "OEBPS/package.opf")
}

#[test]
fn produces_spec_compliant_ocf_container() {
    let dir = fixture(
        HORIZONTAL_BOOK,
        &[
            ("001-intro.md", "# Intro\n\nHello.\n"),
            ("002-body.md", "# Body\n\nWorld.\n"),
        ],
    );
    let out = build_into(dir.path(), "book.epub");
    let entries = read_epub(&out);

    // mimetype must be the first entry, Stored, exactly 20 bytes.
    assert_eq!(entries[0].name, "mimetype");
    assert_eq!(entries[0].compression, zip::CompressionMethod::Stored);
    assert_eq!(entries[0].bytes, b"application/epub+zip");

    // container.xml is the second entry.
    assert_eq!(entries[1].name, "META-INF/container.xml");

    // every entry uses only Stored or Deflated (OCF constraint).
    for e in &entries {
        assert!(
            matches!(
                e.compression,
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            "{} uses an unexpected compression method",
            e.name
        );
    }

    // all required members are present.
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    for required in [
        "OEBPS/package.opf",
        "OEBPS/nav.xhtml",
        "OEBPS/css/aozora-md.css",
        "OEBPS/chapter-001.xhtml",
        "OEBPS/chapter-002.xhtml",
    ] {
        assert!(names.contains(&required), "missing {required}");
    }

    // container.xml points at the OPF root.
    let container = entry_text(&entries, "META-INF/container.xml");
    assert!(container.contains("OEBPS/package.opf"));
}

#[test]
fn spine_and_manifest_agree_in_lexicographic_order() {
    let dir = fixture(
        HORIZONTAL_BOOK,
        &[("001-intro.md", "# Intro\n"), ("002-body.md", "# Body\n")],
    );
    let out = build_into(dir.path(), "book.epub");
    let opf = opf(&read_epub(&out));

    // chapters keep lexicographic order in both manifest and spine.
    assert!(opf.find("ch001").unwrap() < opf.find("ch002").unwrap());
    // every spine idref is backed by a manifest item.
    assert!(opf.contains(r#"<item id="ch001""#));
    assert!(opf.contains(r#"<item id="ch002""#));
    assert!(opf.contains(r#"<itemref idref="ch001""#));
    assert!(opf.contains(r#"<itemref idref="ch002""#));
}

#[test]
fn nav_lists_every_chapter() {
    let dir = fixture(
        HORIZONTAL_BOOK,
        &[("001-a.md", "# A\n"), ("002-b.md", "# B\n")],
    );
    let entries = read_epub(&build_into(dir.path(), "book.epub"));
    let nav = entry_text(&entries, "OEBPS/nav.xhtml");
    assert_eq!(nav.matches("chapter-001.xhtml").count(), 1);
    assert_eq!(nav.matches("chapter-002.xhtml").count(), 1);
}

#[test]
fn vertical_book_binds_right_to_left() {
    let dir = fixture(VERTICAL_BOOK, &[("001.md", "# 章\n\n本文。\n")]);
    let opf = opf(&read_epub(&build_into(dir.path(), "v.epub")));
    assert!(
        opf.contains(r#"<spine page-progression-direction="rtl">"#),
        "opf: {opf}"
    );
}

#[test]
fn horizontal_book_binds_left_to_right() {
    let dir = fixture(HORIZONTAL_BOOK, &[("001.md", "# Ch\n")]);
    let opf = opf(&read_epub(&build_into(dir.path(), "h.epub")));
    assert!(
        opf.contains(r#"<spine page-progression-direction="ltr">"#),
        "opf: {opf}"
    );
}

#[test]
fn rejects_invalid_language() {
    let dir = fixture(
        "title = \"T\"\ncreator = \"A\"\nlanguage = \"japanese\"\n",
        &[("001.md", "x")],
    );
    let err = build(&BuildOptions {
        input: &dir.path().join("manuscript"),
        metadata: &dir.path().join("book.toml"),
        output: &dir.path().join("o.epub"),
    })
    .unwrap_err();
    assert!(matches!(
        err,
        Error::MetadataInvalid {
            field: "language",
            ..
        }
    ));
}

#[test]
fn rejects_malformed_metadata_toml() {
    let dir = fixture("= not valid toml =", &[("001.md", "x")]);
    let err = build(&BuildOptions {
        input: &dir.path().join("manuscript"),
        metadata: &dir.path().join("book.toml"),
        output: &dir.path().join("o.epub"),
    })
    .unwrap_err();
    assert!(matches!(err, Error::MetadataParse { .. }));
}

#[test]
fn reports_missing_input_directory() {
    let dir = fixture(HORIZONTAL_BOOK, &[]);
    let err = build(&BuildOptions {
        input: &dir.path().join("does-not-exist"),
        metadata: &dir.path().join("book.toml"),
        output: &dir.path().join("o.epub"),
    })
    .unwrap_err();
    assert!(matches!(err, Error::DiscoverIo { .. }));
}
