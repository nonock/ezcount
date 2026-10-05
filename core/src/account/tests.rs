use super::*;

#[test]
fn the_profile_is_set_and_removed() {
    let doc = LoroDoc::new();
    assert_eq!(profile(&doc), Profile::default());
    let picture = "data:image/png;base64,AAAA";
    set_profile(
        &doc,
        " Alice ",
        Some(picture),
        Some("fr76 3000 6000 0112 3456 7890 189"),
    )
    .unwrap();
    assert_eq!(
        profile(&doc),
        Profile {
            name: Some("Alice".into()),
            avatar: Some(picture.into()),
            iban: Some("FR7630006000011234567890189".into()),
        }
    );
    assert!(set_profile(&doc, &"a".repeat(MAX_NAME_CHARS + 1), None, None).is_err());
    assert!(set_profile(&doc, "Alice", Some("data:text/html;base64,AAAA"), None).is_err());
    // One digit off.
    let wrong = Some("FR7630006000011234567890188");
    assert_eq!(
        set_profile(&doc, "Alice", None, wrong).unwrap_err(),
        "This IBAN is not valid"
    );
    set_profile(&doc, "", None, Some(" ")).unwrap();
    assert_eq!(profile(&doc), Profile::default());
}

#[test]
fn groups_and_identities_round_trip() {
    let doc = LoroDoc::new();
    let secret = |s: &str| Secret::new(s.to_string());
    add_group(&doc, "g1", "http://relay", &secret("secret-1")).unwrap();
    add_group(&doc, "g2", "http://relay", &secret("secret-2")).unwrap();
    set_identity(&doc, "g1", "p-alice").unwrap();

    let mut list = groups(&doc).unwrap();
    list.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].group_id, "g1");
    assert_eq!(list[0].secret.expose(), "secret-1");
    assert_eq!(identities(&doc).unwrap().get("g1").unwrap(), "p-alice");

    set_archived(&doc, "g1", true).unwrap();
    set_archived(&doc, "g2", true).unwrap();
    set_archived(&doc, "g2", false).unwrap();
    assert_eq!(archived(&doc), vec!["g1".to_string()]);

    remove_group(&doc, "g1").unwrap();
    assert_eq!(groups(&doc).unwrap().len(), 1);
    assert!(identities(&doc).unwrap().is_empty());
    assert!(archived(&doc).is_empty());
}

#[test]
fn concurrent_join_and_identity_both_survive() {
    let (a, b) = (LoroDoc::new(), LoroDoc::new());
    add_group(&a, "g1", "http://relay", &Secret::new("s".into())).unwrap();
    a.commit();
    b.import(&a.export(loro::ExportMode::Snapshot).unwrap())
        .unwrap();

    // Phone joins another group while the laptop picks an identity in the first.
    add_group(&a, "g2", "http://relay", &Secret::new("s2".into())).unwrap();
    set_identity(&b, "g1", "p-bob").unwrap();
    a.commit();
    b.commit();
    a.import(&b.export(loro::ExportMode::Snapshot).unwrap())
        .unwrap();

    assert_eq!(groups(&a).unwrap().len(), 2);
    assert_eq!(identities(&a).unwrap().get("g1").unwrap(), "p-bob");
}
