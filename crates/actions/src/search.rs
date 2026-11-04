use crate::Action;

/// The actions a command bar offers for `query`, matching the label or the id.
pub fn matching<A: Action>(query: &str) -> Vec<A> {
    let query = query.trim().to_lowercase();
    A::ALL
        .iter()
        .copied()
        .filter(|action| action.in_palette())
        .filter(|action| {
            query.is_empty()
                || action.label().to_lowercase().contains(&query)
                || action.id().contains(&query)
        })
        .collect()
}
