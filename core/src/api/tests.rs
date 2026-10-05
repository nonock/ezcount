use super::*;
use crate::storage::Store;

fn state() -> (AppState, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ezcount-api-{}", uuid::Uuid::new_v4()));
    let (store, _) = Store::open(&dir.join("db.sqlite3")).unwrap();
    (AppState::new(store, Vec::new()).unwrap(), dir)
}

/// The commands the frontend calls, from the generated bindings.
fn bound_commands() -> Vec<String> {
    let bindings = include_str!("../../../src/bindings.ts");
    bindings
        .split("TAURI_INVOKE(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn every_bound_command_is_dispatched() {
    let (state, dir) = state();
    let native_only = [
        "native_features",
        "share_text",
        "share_file",
        "set_bars_color",
        "save_download",
        "save_file",
    ];
    let commands = bound_commands();
    assert!(commands.len() > 20, "found {commands:?}");
    for command in commands
        .iter()
        .filter(|c| !native_only.contains(&c.as_str()))
    {
        // Missing arguments fail, but not as an unknown command.
        if let Err(e) = invoke(&state, command, "{}").await {
            assert!(!e.starts_with("Unknown command"), "{e}");
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn runs_commands_by_name() {
    let (state, dir) = state();
    // Creating groups needs an account; a local one is enough here.
    let names = ["Alice".to_string(), "Bob".to_string()];
    let doc = doc::new_group_doc("Trip", "EUR", &names).unwrap();
    let group = state.store().insert(doc, None).unwrap();
    assert_eq!(
        invoke(
            &state,
            "create_group",
            r#"{"name":"Trip","currency":"EUR"}"#
        )
        .await
        .unwrap_err(),
        "Invalid argument participants: invalid type: null, expected a sequence"
    );
    let alice = &group.participants[0].id;
    let bob = &group.participants[1].id;
    let args = serde_json::json!({
        "groupId": group.id,
        "expense": {
            "title": "Dinner",
            "amount_cents": 3000,
            "paid_by": alice,
            "splits": [{"participant_id": alice, "shares": 1}, {"participant_id": bob, "shares": 2}],
        },
    });
    invoke(&state, "add_expense", &args.to_string())
        .await
        .unwrap();
    let balances: Vec<ParticipantBalance> = serde_json::from_str(
        &invoke(
            &state,
            "get_balances",
            &serde_json::json!({ "groupId": group.id }).to_string(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let net = |id: &str| {
        balances
            .iter()
            .find(|b| b.participant_id == id)
            .unwrap()
            .net_cents
    };
    assert_eq!((net(alice), net(bob)), (2000, -2000));

    assert_eq!(
        invoke(&state, "get_group", r#"{"groupId":"nope"}"#)
            .await
            .unwrap_err(),
        get_group(&state, "nope").unwrap_err()
    );
    assert_eq!(
        invoke(&state, "get_group", "{}").await.unwrap_err(),
        "Invalid argument groupId: invalid type: null, expected a string"
    );
    assert_eq!(
        invoke(&state, "nope", "{}").await.unwrap_err(),
        "Unknown command nope"
    );
    let _ = std::fs::remove_dir_all(dir);
}
