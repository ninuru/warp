use std::path::{Path, PathBuf};

use fuzzy_match::{FuzzyMatchResult, match_indices_case_insensitive};
use itertools::Itertools;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use super::ProjectSearchItem;
use super::scan::{expand_root, scan_roots};
use crate::search::QueryFilter;
use crate::search::command_palette::mixer::CommandPaletteItemAction;
use crate::search::data_source::{Query, QueryResult};
use crate::search::mixer::{DataSourceRunErrorWrapper, SyncDataSource};
use crate::workspace::tab_settings::TabSettings;
use crate::workspace::view::projects::{ProjectRef, project_name};

const OPEN_PROJECT_SCORE_FACTOR: i64 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectCandidate {
    pub root: PathBuf,
    pub name: String,
    pub display_path: String,
    pub is_open: bool,
}

#[derive(Default)]
pub struct ProjectDataSource {
    candidates: Vec<ProjectCandidate>,
}

fn display_path(root: &Path, home: &Path) -> String {
    match root.strip_prefix(home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".to_string(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => root.display().to_string(),
    }
}

pub fn merge_candidates(
    open: &[ProjectRef],
    active: &Path,
    scanned: Vec<PathBuf>,
    home: &Path,
) -> Vec<ProjectCandidate> {
    let open_candidates = open
        .iter()
        .filter(|project| project.root != active)
        .map(|project| ProjectCandidate {
            root: project.root.clone(),
            name: project.name.clone(),
            display_path: display_path(&project.root, home),
            is_open: true,
        });
    let scanned_candidates = scanned
        .into_iter()
        .filter(|root| root != active && !open.iter().any(|project| &project.root == root))
        .map(|root| ProjectCandidate {
            name: project_name(&root),
            display_path: display_path(&root, home),
            root,
            is_open: false,
        })
        .sorted_by(|a, b| a.display_path.cmp(&b.display_path));
    open_candidates.chain(scanned_candidates).collect()
}

pub fn rank_candidates(candidates: &[ProjectCandidate], query: &str) -> Vec<ProjectSearchItem> {
    let count = candidates.len() as i64;
    candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let match_result = if query.is_empty() {
                Some(FuzzyMatchResult {
                    score: count - index as i64,
                    matched_indices: vec![],
                })
            } else {
                match_indices_case_insensitive(&candidate.display_path, query).map(
                    |mut match_result| {
                        if candidate.is_open {
                            match_result.score *= OPEN_PROJECT_SCORE_FACTOR;
                        }
                        match_result
                    },
                )
            };
            match_result.map(|match_result| ProjectSearchItem {
                candidate: candidate.clone(),
                match_result,
            })
        })
        .collect()
}

impl ProjectDataSource {
    pub fn refresh(
        &mut self,
        open: Vec<ProjectRef>,
        active: PathBuf,
        ctx: &mut ModelContext<Self>,
    ) {
        let home = dirs::home_dir().unwrap_or_default();
        let roots = TabSettings::as_ref(ctx)
            .project_roots
            .iter()
            .map(|raw| expand_root(raw, &home))
            .collect_vec();
        self.candidates = merge_candidates(&open, &active, scan_roots(&roots), &home);
    }
}

impl Entity for ProjectDataSource {
    type Event = ();
}

impl SyncDataSource for ProjectDataSource {
    type Action = CommandPaletteItemAction;

    fn run_query(
        &self,
        query: &Query,
        _app: &AppContext,
    ) -> Result<Vec<QueryResult<Self::Action>>, DataSourceRunErrorWrapper> {
        if !query.filters.contains(&QueryFilter::ProjectSessions) {
            return Ok(vec![]);
        }
        Ok(rank_candidates(&self.candidates, query.text.as_str())
            .into_iter()
            .map(QueryResult::from)
            .collect())
    }
}

#[cfg(test)]
#[path = "project_data_source_tests.rs"]
mod tests;
