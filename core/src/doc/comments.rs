//! Comments under an expense, kept in a map of their own (see the parent module).

use super::*;

/// A comment as stored.
#[derive(Deserialize)]
pub(super) struct DocComment {
    pub(super) expense: String,
    pub(super) text: String,
    pub(super) at: String,
    #[serde(default)]
    pub(super) by: Option<String>,
}

/// The comments of each expense, the oldest first.
pub(super) fn read_comments(doc: &LoroDoc) -> HashMap<String, Vec<ExpenseComment>> {
    let mut comments: HashMap<String, Vec<ExpenseComment>> = HashMap::new();
    for (id, c) in entries::<DocComment>(&doc.get_map(COMMENTS), COMMENTS) {
        let Some(created_at) = read_time(Some(c.at)) else {
            continue;
        };
        comments.entry(c.expense).or_default().push(ExpenseComment {
            id,
            text: c.text.chars().take(MAX_COMMENT_CHARS).collect(),
            created_at,
            by: c.by,
        });
    }
    for list in comments.values_mut() {
        list.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
    }
    comments
}

/// Adds a comment under an expense, from `by` when the app knows who the user is.
pub fn add_comment(doc: &LoroDoc, expense_id: &str, text: &str, by: Option<&str>) -> Res<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err("A comment can't be empty".to_string());
    }
    if text.chars().count() > MAX_COMMENT_CHARS {
        return Err(format!(
            "This comment is too long ({MAX_COMMENT_CHARS} characters at most)"
        ));
    }
    if child_map(&doc.get_map(EXPENSES), expense_id).is_none() {
        return Err("Expense not found".to_string());
    }
    let mut comment = HashMap::from([
        ("expense".to_string(), LoroValue::from(expense_id)),
        ("text".to_string(), text.into()),
        ("at".to_string(), timestamp(Utc::now()).into()),
    ]);
    if let Some(by) = by {
        comment.insert("by".to_string(), by.into());
    }
    doc.get_map(COMMENTS)
        .insert(&Uuid::new_v4().to_string(), LoroValue::Map(comment.into()))
        .map_err(doc_err)
}

/// Removes a comment.
pub fn delete_comment(doc: &LoroDoc, comment_id: &str) -> Res<()> {
    let comments = doc.get_map(COMMENTS);
    if comments.get(comment_id).is_none() {
        return Err("This comment no longer exists".to_string());
    }
    comments.delete(comment_id).map_err(doc_err)
}
