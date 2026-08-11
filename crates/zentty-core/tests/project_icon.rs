use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use zentty_core::{ProjectIconCache, ProjectIconLookup};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(1);

struct TempProject(std::path::PathBuf);

impl TempProject {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn project() -> TempProject {
    let path = std::env::temp_dir().join(format!(
        "zentty-project-icon-{}-{}",
        std::process::id(),
        NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).expect("temporary project");
    TempProject(path)
}

fn write(root: &std::path::Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("candidate parent")).expect("candidate parent");
    fs::write(path, bytes).expect("candidate bytes");
}

const SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg'></svg>";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\ncontrolled";
const ICO: &[u8] = b"\0\0\x01\0controlled";

#[test]
fn ordered_candidates_and_markup_fallback_match_the_source() {
    let root = project();
    write(root.path(), "public/favicon.png", PNG);
    write(root.path(), "favicon.svg", SVG);
    let mut cache = ProjectIconCache::default();
    assert_eq!(
        cache.resolve_at(root.path(), 0).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join("favicon.svg"))
    );

    fs::remove_file(root.path().join("favicon.svg")).expect("remove first");
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 1).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join("public/favicon.png"))
    );

    fs::remove_file(root.path().join("public/favicon.png")).expect("remove second");
    write(root.path(), "static/site.png", PNG);
    write(
        root.path(),
        "index.html",
        br#"<link rel="stylesheet"><link href="/static/site.png?ignored=1" rel="shortcut icon">"#,
    );
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 2).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join("static/site.png"))
    );

    fs::remove_file(root.path().join("index.html")).expect("remove html");
    write(root.path(), "icons/site.svg", SVG);
    write(
        root.path(),
        "app/root.tsx",
        br"{ href: '/icons/site.svg', rel: 'icon' }",
    );
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 3).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join("icons/site.svg"))
    );
}

#[test]
fn appicon_chooses_the_largest_declared_image() {
    let root = project();
    let iconset = "Assets.xcassets/AppIcon.appiconset";
    write(root.path(), &format!("{iconset}/scaled.png"), PNG);
    write(root.path(), &format!("{iconset}/unscaled.png"), PNG);
    write(root.path(), &format!("{iconset}/invalid.png"), PNG);
    write(
        root.path(),
        &format!("{iconset}/Contents.json"),
        br#"{"images":[
            {"size":"4x4","scale":"4x","filename":"scaled.png"},
            {"size":"10x10","scale":"1x","filename":"unscaled.png"},
            {"size":"-999x-999","scale":"NaNx","filename":"invalid.png"}
        ]}"#,
    );
    let mut cache = ProjectIconCache::default();
    assert_eq!(
        cache.resolve_at(root.path(), 0).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join(iconset).join("scaled.png"))
    );
}

#[cfg(unix)]
#[test]
fn symlinks_must_resolve_inside_the_canonical_project() {
    use std::os::unix::fs::symlink;

    let root = project();
    write(root.path(), "assets/icon.svg", SVG);
    symlink("assets/icon.svg", root.path().join("favicon.svg")).expect("inside symlink");
    let mut cache = ProjectIconCache::default();
    assert_eq!(
        cache.resolve_at(root.path(), 0).expect("resolve"),
        ProjectIconLookup::Hit(root.path().join("assets/icon.svg"))
    );

    let outside = project();
    write(outside.path(), "outside.svg", SVG);
    fs::remove_file(root.path().join("favicon.svg")).expect("replace symlink");
    fs::remove_file(root.path().join("assets/icon.svg")).expect("remove inside icon");
    symlink(
        outside.path().join("outside.svg"),
        root.path().join("favicon.svg"),
    )
    .expect("outside symlink");
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 1).expect("resolve"),
        ProjectIconLookup::Miss
    );
}

#[test]
fn negative_cache_expires_and_explicit_invalidation_is_immediate() {
    let root = project();
    let mut cache = ProjectIconCache::new(300);
    assert_eq!(
        cache.resolve_at(root.path(), 10).expect("resolve"),
        ProjectIconLookup::Miss
    );
    write(root.path(), "favicon.svg", SVG);
    assert_eq!(
        cache.resolve_at(root.path(), 309).expect("cached"),
        ProjectIconLookup::Miss
    );
    assert_eq!(
        cache.resolve_at(root.path(), 310).expect("expired"),
        ProjectIconLookup::Hit(root.path().join("favicon.svg"))
    );
    fs::remove_file(root.path().join("favicon.svg")).expect("remove icon");
    assert!(cache.invalidate(root.path()));
    assert_eq!(
        cache.resolve_at(root.path(), 311).expect("invalidated"),
        ProjectIconLookup::Miss
    );

    write(root.path(), "favicon.svg", SVG);
    cache.invalidate_all();
    assert_eq!(
        cache.resolve_at(root.path(), 312).expect("invalidate all"),
        ProjectIconLookup::Hit(root.path().join("favicon.svg"))
    );

    let default_root = project();
    let mut default_cache = ProjectIconCache::default();
    assert_eq!(
        default_cache
            .resolve_at(default_root.path(), 0)
            .expect("default miss"),
        ProjectIconLookup::Miss
    );
    write(default_root.path(), "favicon.svg", SVG);
    assert_eq!(
        default_cache
            .resolve_at(default_root.path(), 299)
            .expect("default cached"),
        ProjectIconLookup::Miss
    );
    assert!(matches!(
        default_cache.resolve_at(default_root.path(), 300),
        Ok(ProjectIconLookup::Hit(_))
    ));
}

#[test]
fn markup_reads_are_bounded_and_paths_cannot_escape_or_be_network_urls() {
    let root = project();
    let outside = project();
    write(outside.path(), "outside.png", PNG);
    write(
        root.path(),
        "index.html",
        format!(
            "<link rel='icon' href='../{}/outside.png'>",
            outside.path().file_name().unwrap().to_string_lossy()
        )
        .as_bytes(),
    );
    let mut cache = ProjectIconCache::default();
    assert_eq!(
        cache.resolve_at(root.path(), 0).expect("escape"),
        ProjectIconLookup::Miss
    );

    write(
        root.path(),
        "index.html",
        b"<link rel='icon' href='https://example.test/a.png'>",
    );
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 1).expect("network"),
        ProjectIconLookup::Miss
    );

    write(root.path(), "index.html", &vec![b'x'; 262_145]);
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 2).expect("bounded"),
        ProjectIconLookup::Miss
    );

    let mut bounded_source = vec![b' '; 16 * 1024];
    let link = b"<link rel='icon' href='accepted.ico'>";
    bounded_source[..link.len()].copy_from_slice(link);
    write(root.path(), "index.html", &bounded_source);
    write(root.path(), "accepted.ico", ICO);
    cache.invalidate(root.path());
    assert_eq!(
        cache
            .resolve_at(root.path(), 3)
            .expect("bounded source accepted"),
        ProjectIconLookup::Hit(root.path().join("accepted.ico"))
    );

    fs::remove_file(root.path().join("accepted.ico")).expect("remove ico");
    fs::remove_file(root.path().join("index.html")).expect("remove source");
    let mut moderately_sized_png = vec![0; 2 * 1024 * 1024];
    moderately_sized_png[..PNG.len()].copy_from_slice(PNG);
    write(root.path(), "favicon.png", &moderately_sized_png);
    cache.invalidate(root.path());
    assert_eq!(
        cache
            .resolve_at(root.path(), 4)
            .expect("bounded icon accepted"),
        ProjectIconLookup::Hit(root.path().join("favicon.png"))
    );

    fs::write(
        root.path().join("favicon.png"),
        vec![0; 8 * 1024 * 1024 + 1],
    )
    .expect("oversized icon");
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 5).expect("oversized icon"),
        ProjectIconLookup::Miss
    );

    write(root.path(), "favicon.svg", b"not an image");
    cache.invalidate(root.path());
    assert_eq!(
        cache.resolve_at(root.path(), 6).expect("invalid payload"),
        ProjectIconLookup::Miss
    );
}
