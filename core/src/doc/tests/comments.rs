use super::*;

#[test]
fn comments_under_an_expense() {
    let (doc, g) = sample();
    let (alice, bob) = (&g.participants[0].id, &g.participants[1].id);
    let splits = vec![split(alice, 1), split(bob, 1)];
    add_expense(&doc, "Dinner", 3000, alice.clone(), splits, None, None).unwrap();
    doc.commit();
    let e = read_group(&doc).unwrap().expenses.remove(0);
    assert!(add_comment(&doc, &e.id, "  ", None).is_err());
    assert!(add_comment(&doc, &e.id, &"a".repeat(MAX_COMMENT_CHARS + 1), None).is_err());
    assert!(add_comment(&doc, "nope", "Hello", None).is_err());

    // Two people comment at once, each on their device: both stay.
    let other = fork(&doc);
    add_comment(&doc, &e.id, " Was it this much? ", Some(bob.as_str())).unwrap();
    add_comment(&other, &e.id, "With the tip", Some(alice.as_str())).unwrap();
    doc.commit();
    other.commit();
    merge(&doc, &other);
    let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
    let mut texts: Vec<&str> = comments.iter().map(|c| c.text.as_str()).collect();
    texts.sort_unstable();
    assert_eq!(texts, ["Was it this much?", "With the tip"]);
    let bobs = comments
        .iter()
        .find(|c| c.by.as_ref() == Some(bob))
        .unwrap();

    // They follow the expense to the trash and back.
    delete_expense(&doc, &e.id, None).unwrap();
    assert_eq!(read_group(&doc).unwrap().trash[0].expense.comments.len(), 2);
    restore_expense(&doc, &e.id, None).unwrap();

    delete_comment(&doc, &bobs.id).unwrap();
    assert!(delete_comment(&doc, &bobs.id).is_err());
    let comments = read_group(&doc).unwrap().expenses.remove(0).comments;
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].text, "With the tip");
}
