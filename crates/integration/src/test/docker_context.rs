use std::collections::HashMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use settings::Setting;
use warp::integration_testing::step::new_step_with_default_assertions;
use warp::integration_testing::terminal::util::current_shell_starter_and_version;
use warp::integration_testing::terminal::{
    assert_input_editor_contents, wait_until_bootstrapped_single_pane_for_tab,
};
use warp::integration_testing::view_getters::single_terminal_view_for_tab;
use warp::terminal::session_settings::{HonorPS1, SavedPrompt};
use warp::terminal::shell::ShellType;
use warpui_core::async_assert;
use warpui_core::integration::{AssertionCallback, TestStep};

use crate::Builder;
use crate::util::{ShellRcType, write_rc_files_for_test};

fn assert_context(expected: &'static str) -> AssertionCallback {
    Box::new(move |app, window_id| {
        single_terminal_view_for_tab(app, window_id, 0).read(app, |view, ctx| {
            let text = view.full_prompt(ctx);
            async_assert!(text.contains(expected), "Expected {expected}, got {text}")
        })
    })
}

pub fn test_docker_context_selector() -> Builder {
    let prompt = serde_json::json!({
        "CustomChipSelection": {
            "chips": [{"chip": "DockerContext", "config": {}}],
            "same_line_prompt_enabled": false,
            "separator": "None"
        }
    });
    let screenshot = |step: TestStep, name: &str| {
        if std::env::var_os("WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS").is_some() {
            step.with_take_screenshot(name)
        } else {
            step
        }
    };
    Builder::new()
        .set_should_run_test(|| {
            cfg!(unix)
                && current_shell_starter_and_version().0.shell_type() != ShellType::PowerShell
        })
        .with_user_defaults(HashMap::from([
            (HonorPS1::storage_key().to_owned(), "false".to_owned()),
            (
                SavedPrompt::storage_key().to_owned(),
                serde_json::to_string(&prompt).unwrap(),
            ),
        ]))
        .with_setup(|utils| {
            let dir = utils.test_dir();
            let bin = dir.join("bin");
            fs::create_dir_all(&bin).unwrap();
            let docker = bin.join("docker");
            fs::write(
                &docker,
                r#"#!/bin/sh
case "$1 $2" in
    'context show') cat "$HOME/docker-context" ;;
    'context ls') printf 'default\nremote-dev\n' ;;
    'context use')
        [ "$3" = '--' ] && [ "$4" = 'remote-dev' ] || exit 1
        printf '%s\n' "$4" > "$HOME/docker-context"
        ;;
    *) exit 1 ;;
esac
"#,
            )
            .unwrap();
            #[cfg(unix)]
            fs::set_permissions(&docker, fs::Permissions::from_mode(0o755)).unwrap();
            fs::write(dir.join("docker-context"), "default\n").unwrap();
            utils.set_env(
                "PATH",
                Some(format!(
                    "{}:{}",
                    bin.display(),
                    std::env::var("PATH").unwrap()
                )),
            );
            write_rc_files_for_test(
                &dir,
                r#"export PATH="$HOME/bin:$PATH""#,
                [ShellRcType::Bash, ShellRcType::Zsh],
            );
            write_rc_files_for_test(&dir, "set -gx PATH $HOME/bin $PATH", [ShellRcType::Fish]);
        })
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(
            new_step_with_default_assertions("Docker context shows current context")
                .add_assertion(assert_context("default")),
        )
        .with_step(
            new_step_with_default_assertions("Preserve draft while changing context")
                .with_keystrokes(&["e", "c", "h", "o"]),
        )
        .with_step(screenshot(
            new_step_with_default_assertions("Open Docker context selector")
                .with_click_on_saved_position("docker_context_chip"),
            "docker-context-menu.png",
        ))
        .with_step(
            new_step_with_default_assertions("Select remote Docker context")
                .with_keystrokes(&["r", "e", "m", "o", "t", "e", "enter"]),
        )
        .with_step(screenshot(
            new_step_with_default_assertions("Docker context refreshes and draft remains")
                .add_assertion(assert_context("remote-dev"))
                .add_assertion(assert_input_editor_contents(0, "echo")),
            "docker-context-switched.png",
        ))
}
