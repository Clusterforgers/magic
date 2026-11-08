//! Two sentences of "each player" in a row joined by ", then" with the subject said once: "Each player discards their
//! hand, then returns up to three cards from their graveyard to their hand." The renderers write each step as a sentence
//! of its own that names every player again.

const NEXT: &str = ". Each player ";

/// Adds, for each wording with exactly one sentence of "each player" right after another sentence of "each player", the
/// wording with the two joined by ", then".
pub(super) fn add(texts: &mut Vec<String>) {
    let joined: Vec<String> = texts.iter().filter_map(|text| join(text)).collect();
    for text in joined {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

fn join(text: &str) -> Option<String> {
    let mut joins = text
        .match_indices(NEXT)
        .filter_map(|(at, _)| joined_at(&text[..at], &text[at + NEXT.len()..]));
    let joined = joins.next()?;
    joins.next().is_none().then_some(joined)
}

/// The two sentences around a join joined, when the sentence before is a plain "each player" one.
fn joined_at(head: &str, tail: &str) -> Option<String> {
    // The sentence before is the one that ends at the join, opened by the start, a period or a line break.
    let before = head
        .rsplit(['.', '\n'])
        .next()
        .unwrap_or(head)
        .trim_start()
        .to_lowercase();
    let before = before.strip_prefix("then ").unwrap_or(&before);
    // Only a plain verb ("discards") opens the sentence before: "each player who ..." or "each player may ..." would
    // carry its condition or choice into the joined step.
    let verb = before.strip_prefix("each player ")?.split(' ').next()?;
    (verb.ends_with('s') && verb != "has" && !before.contains(" may "))
        .then(|| format!("{head}, then {tail}"))
}

#[cfg(test)]
mod tests {
    //! Goal: only a plain "each player" sentence before another one takes ", then"; a choice, a condition or another
    //! subject keeps the two sentences.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::plain(
        "Each player sacrifices a creature. Each player loses 2 life.",
        Some("Each player sacrifices a creature, then loses 2 life.")
    )]
    #[case::after_exile(
        "Exile ~. Each player discards their hand. Each player draws a card.",
        Some("Exile ~. Each player discards their hand, then draws a card.")
    )]
    #[case::a_choice_before(
        "Each player may sacrifice a creature. Each player loses 2 life.",
        None
    )]
    #[case::a_condition_before(
        "Each player who controls a creature sacrifices it. Each player loses 2 life.",
        None
    )]
    #[case::another_subject("You sacrifice a creature. Each player loses 2 life.", None)]
    #[case::three_sentences(
        "Each player draws a card. Each player discards a card. Each player loses 2 life.",
        None
    )]
    fn fix20_only_a_plain_each_player_sentence_takes_then(
        #[case] text: &str,
        #[case] joined: Option<&str>,
    ) {
        assert_eq!(join(text).as_deref(), joined);
    }
}
