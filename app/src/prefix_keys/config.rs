use std::collections::HashMap;
use std::time::Duration;

use itertools::{Either, Itertools};
use serde::{Deserialize, Serialize};
use warpui::keymap::Keystroke;

const DEFAULT_PREFIX_KEY: &str = "ctrl-a";
const DEFAULT_TIMEOUT_MS: u64 = 1500;
const DEFAULT_BINDINGS: &[(&str, &str)] = &[
    ("h", "pane_group:navigate_left"),
    ("j", "pane_group:navigate_down"),
    ("k", "pane_group:navigate_up"),
    ("%", "pane_group:add_right"),
    ("\"", "pane_group:add_down"),
    ("c", "workspace:new_tab"),
    ("x", "pane_group:close_current_session"),
    ("z", "pane_group:toggle_maximize_pane"),
    ("1", "workspace:activate_first_tab"),
    ("2", "workspace:activate_second_tab"),
    ("3", "workspace:activate_third_tab"),
    ("4", "workspace:activate_fourth_tab"),
    ("5", "workspace:activate_fifth_tab"),
    ("6", "workspace:activate_sixth_tab"),
    ("7", "workspace:activate_seventh_tab"),
    ("8", "workspace:activate_eighth_tab"),
    ("9", "workspace:activate_last_tab"),
    ("^", "workspace:activate_last_active_tab"),
    (",", "workspace:rename_active_tab"),
    ("f", "workspace:open_project_picker"),
    ("l", "workspace:switch_to_previous_project"),
    ("s", "workspace:list_open_projects"),
    ("[", "terminal:select_previous_block"),
];
const DEFAULT_ROOT_BINDINGS: &[(&str, &str)] = &[
    ("alt-1", "workspace:activate_first_tab"),
    ("alt-2", "workspace:activate_second_tab"),
    ("alt-3", "workspace:activate_third_tab"),
    ("alt-4", "workspace:activate_fourth_tab"),
    ("alt-5", "workspace:activate_fifth_tab"),
    ("alt-6", "workspace:activate_sixth_tab"),
    ("alt-7", "workspace:activate_seventh_tab"),
    ("alt-8", "workspace:activate_eighth_tab"),
    ("alt-9", "workspace:activate_last_tab"),
    ("alt-h", "pane_group:navigate_left"),
    ("alt-j", "pane_group:navigate_down"),
    ("alt-k", "pane_group:navigate_up"),
    ("alt-l", "pane_group:navigate_right"),
    ("ctrl-shift-enter", "pane_group:add_right"),
    ("alt-shift-enter", "pane_group:add_down"),
];

#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[serde(default)]
#[schemars(description = "Configuration for the tmux-style prefix key.")]
pub struct PrefixKeysConfig {
    #[schemars(description = "Whether the prefix key and the root bindings are active.")]
    pub enabled: bool,
    #[schemars(
        description = "The prefix key, e.g. \"ctrl-a\". Pressing it twice sends it to the focused view."
    )]
    pub key: String,
    #[schemars(description = "Milliseconds to wait for a key after the prefix before cancelling.")]
    pub timeout_ms: u64,
    #[schemars(
        description = "Keys pressed after the prefix, mapped to keybinding names such as \"pane_group:navigate_left\"."
    )]
    pub bindings: HashMap<String, String>,
    #[schemars(
        description = "Keys that run a keybinding without the prefix, mapped to keybinding names."
    )]
    pub root_bindings: HashMap<String, String>,
}

impl Default for PrefixKeysConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            key: DEFAULT_PREFIX_KEY.to_owned(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            bindings: table_from(DEFAULT_BINDINGS),
            root_bindings: table_from(DEFAULT_ROOT_BINDINGS),
        }
    }
}

fn table_from(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
        .collect()
}

#[derive(Clone, Debug)]
pub struct PrefixKeymap {
    prefix: Keystroke,
    timeout: Duration,
    bindings: Vec<(Keystroke, String)>,
    root: Vec<(Keystroke, String)>,
}

impl PrefixKeymap {
    pub fn parse(config: &PrefixKeysConfig) -> (Self, Vec<String>) {
        let (prefix, prefix_errors) = match parse_keystroke(&config.key) {
            Ok(prefix) => (prefix, vec![]),
            Err(error) => (
                Keystroke::parse(DEFAULT_PREFIX_KEY).expect("default prefix key is valid"),
                vec![format!("prefix key {error}")],
            ),
        };
        let (bindings, binding_errors) = parse_table(&config.bindings);
        let (root, root_errors) = parse_table(&config.root_bindings);
        let keymap = Self {
            prefix,
            timeout: Duration::from_millis(config.timeout_ms),
            bindings,
            root,
        };
        let errors = prefix_errors
            .into_iter()
            .chain(binding_errors)
            .chain(root_errors)
            .collect();
        (keymap, errors)
    }

    pub fn prefix(&self) -> &Keystroke {
        &self.prefix
    }

    pub fn is_prefix(&self, keystroke: &Keystroke) -> bool {
        key_matches(&self.prefix, keystroke)
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn binding_for(&self, keystroke: &Keystroke) -> Option<&str> {
        find_binding(&self.bindings, keystroke)
    }

    pub fn root_binding_for(&self, keystroke: &Keystroke) -> Option<&str> {
        find_binding(&self.root, keystroke)
    }
}

pub fn key_matches(pattern: &Keystroke, actual: &Keystroke) -> bool {
    if pattern == actual {
        return true;
    }
    let mut chars = pattern.key.chars();
    let is_single_non_letter = matches!(
        (chars.next(), chars.next()),
        (Some(c), None) if !c.is_alphabetic()
    );
    is_single_non_letter
        && pattern.key == actual.key
        && pattern.ctrl == actual.ctrl
        && pattern.alt == actual.alt
        && pattern.cmd == actual.cmd
        && pattern.meta == actual.meta
}

fn find_binding<'a>(table: &'a [(Keystroke, String)], keystroke: &Keystroke) -> Option<&'a str> {
    table
        .iter()
        .find(|(pattern, _)| key_matches(pattern, keystroke))
        .map(|(_, name)| name.as_str())
}

fn parse_table(table: &HashMap<String, String>) -> (Vec<(Keystroke, String)>, Vec<String>) {
    table
        .iter()
        .sorted()
        .map(|(key, name)| parse_entry(key, name))
        .partition_map(|entry| match entry {
            Ok(binding) => Either::Left(binding),
            Err(error) => Either::Right(error),
        })
}

fn parse_entry(key: &str, name: &str) -> Result<(Keystroke, String), String> {
    let keystroke = parse_keystroke(key)?;
    let name = name.trim();
    if name.is_empty() || !name.contains(':') {
        return Err(format!(
            "`{key}`: `{name}` is not a keybinding name like \"workspace:new_tab\""
        ));
    }
    Ok((keystroke, name.to_owned()))
}

fn parse_keystroke(source: &str) -> Result<Keystroke, String> {
    let has_shift = source.split('-').any(|component| component == "shift");
    let mut key_chars = source.rsplit('-').next().unwrap_or_default().chars();
    if let (Some(c), None) = (key_chars.next(), key_chars.next())
        && ((has_shift && c.is_lowercase()) || (!has_shift && c.is_uppercase()))
    {
        return Err(format!(
            "`{source}`: letters must be uppercase with shift and lowercase without"
        ));
    }
    Keystroke::parse(source).map_err(|error| format!("`{source}`: {error}"))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
