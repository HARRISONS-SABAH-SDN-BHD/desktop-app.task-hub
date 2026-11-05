use crate::{model::*, runner, storage::AppState};
#[test]
fn direct_send_requires_per_invocation_authorization_before_launch() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::initialize(root.path().into()).unwrap();
    state
        .save_config(
            serde_json::from_str(include_str!("../../../config/examples/library.tasks.json"))
                .unwrap(),
        )
        .unwrap();
    let parameters: Parameters = serde_json::from_value(serde_json::json!({"provider":"Gmail","browser":"Microsoft Edge","recipient":"a@example.com","subject":"Hi","message":"Hello","delivery":"Send directly"})).unwrap();
    let error = runner::manual(&state, "foreground_email", parameters, false).unwrap_err();
    assert!(error.contains("explicit authorization"));
    assert!(state.lock().unwrap().runs.is_empty());
    assert!(state.foreground.snapshot().is_none());
}
