use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    paths
}

#[test]
fn expand_root_resolves_home_prefix() {
    let home = Path::new("/home/u");
    assert_eq!(expand_root("~/x", home), PathBuf::from("/home/u/x"));
    assert_eq!(expand_root("~", home), PathBuf::from("/home/u"));
    assert_eq!(expand_root("/opt/src", home), PathBuf::from("/opt/src"));
}

#[test]
fn scan_keeps_directories_and_dot_directories_only() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("warp")).unwrap();
    fs::create_dir(root.join(".config")).unwrap();
    fs::write(root.join("notes.txt"), "").unwrap();

    let scanned = sorted(scan_roots(&[root.to_path_buf()]));

    assert_eq!(scanned, vec![root.join(".config"), root.join("warp")]);
}

#[cfg(unix)]
#[test]
fn scan_skips_symlinked_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("real")).unwrap();
    std::os::unix::fs::symlink(root.join("real"), root.join("link")).unwrap();

    assert_eq!(scan_roots(&[root.to_path_buf()]), vec![root.join("real")]);
}

#[test]
fn scan_ignores_missing_roots() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join("warp")).unwrap();

    let scanned = scan_roots(&[root.join("missing"), root.to_path_buf()]);

    assert_eq!(scanned, vec![root.join("warp")]);
}

#[test]
fn scan_dedupes_overlapping_roots() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("projects/warp")).unwrap();

    let scanned = sorted(scan_roots(&[
        root.to_path_buf(),
        root.join("projects"),
        root.join("projects/"),
    ]));

    assert_eq!(
        scanned,
        vec![root.join("projects"), root.join("projects/warp")]
    );
}
