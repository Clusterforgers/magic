//! Other Oracle wordings of the same zone move: a card returned "with an additional +1/+1 counter" enters with that
//! counter as any other ("with a +1/+1 counter"), and a card in an opponent's graveyard goes to "their hand", the hand
//! of its owner.

/// Each row is a phrase the writers give and the phrase the Oracle text uses for it.
const ROWS: &[(&str, &str)] = &[
    (
        " to the battlefield with a +1/+1 counter on ",
        " to the battlefield with an additional +1/+1 counter on ",
    ),
    (
        " to the battlefield with two +1/+1 counters on ",
        " to the battlefield with two additional +1/+1 counters on ",
    ),
    (
        " to the battlefield with three +1/+1 counters on ",
        " to the battlefield with three additional +1/+1 counters on ",
    ),
    (
        " from an opponent's graveyard to its owner's hand",
        " from an opponent's graveyard to their hand",
    ),
    (
        " from an opponent's graveyard to their owners' hands",
        " from an opponent's graveyard to their hand",
    ),
];

/// Adds the Oracle wording of each row to the wordings that have the writers' phrase.
pub fn add(texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            ROWS.iter()
                .filter(|(written, _)| text.contains(written))
                .map(|(written, oracle)| text.replace(written, oracle))
        })
        .collect();
    texts.extend(found);
}
