use std::path::{Path, PathBuf};

use super::*;
use crate::search::item::SearchItem;

const HOME: &str = "/home/u";

fn project(path: &str) -> ProjectRef {
    ProjectRef::for_root(PathBuf::from(path))
}

fn display_paths(candidates: &[ProjectCandidate]) -> Vec<&str> {
    candidates
        .iter()
        .map(|candidate| candidate.display_path.as_str())
        .collect()
}

#[test]
fn merge_lists_open_projects_first_and_excludes_active() {
    let open = [
        project("/home/u/projects/api"),
        project("/home/u/IdeaProjects/warp"),
    ];
    let scanned = vec![
        PathBuf::from("/home/u/projects/zeta"),
        PathBuf::from("/home/u/IdeaProjects/warp"),
        PathBuf::from("/home/u/projects/api"),
        PathBuf::from("/home/u/IdeaProjects/alpha"),
    ];

    let candidates = merge_candidates(
        &open,
        Path::new("/home/u/projects/api"),
        scanned,
        Path::new(HOME),
    );

    assert_eq!(
        display_paths(&candidates),
        vec![
            "~/IdeaProjects/warp",
            "~/IdeaProjects/alpha",
            "~/projects/zeta"
        ]
    );
    assert!(candidates[0].is_open);
    assert!(!candidates[1].is_open);
    assert_eq!(candidates[0].name, "warp");
}

#[test]
fn rank_keeps_merge_order_for_empty_query_and_fuzzy_matches_display_path() {
    let candidates = merge_candidates(
        &[],
        Path::new(HOME),
        vec![
            PathBuf::from("/home/u/IdeaProjects/warp"),
            PathBuf::from("/home/u/projects/api"),
        ],
        Path::new(HOME),
    );

    let all = rank_candidates(&candidates, "");
    assert_eq!(all.len(), 2);
    assert!(all[0].score() > all[1].score());

    let matched = rank_candidates(&candidates, "wrp");
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0].candidate.display_path, "~/IdeaProjects/warp");
    let warp = Path::new("/home/u/IdeaProjects/warp");
    assert!(matches!(
        matched[0].accept_result(),
        CommandPaletteItemAction::SwitchProject { path } if path == warp
    ));
}
