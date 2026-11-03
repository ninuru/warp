use super::refreshed_selection_index;

#[test]
fn selection_follows_identity_when_results_reorder() {
    let identities = [Some("second".to_string()), Some("first".to_string())];

    assert_eq!(refreshed_selection_index("first", 0, &identities), Some(1));
}

#[test]
fn removed_selection_clamps_to_remaining_results() {
    let identities = [Some("first".to_string()), Some("second".to_string())];

    assert_eq!(
        refreshed_selection_index("removed", 3, &identities),
        Some(1)
    );
}

#[test]
fn removed_selection_skips_rows_without_identity() {
    let identities = [Some("first".to_string()), None, Some("second".to_string())];

    assert_eq!(
        refreshed_selection_index("removed", 1, &identities),
        Some(0)
    );
}

#[test]
fn empty_results_have_no_selection() {
    assert_eq!(refreshed_selection_index("removed", 1, &[]), None);
}

#[test]
fn unrelated_results_do_not_preserve_picker_selection() {
    assert_eq!(refreshed_selection_index("removed", 1, &[None, None]), None);
}
