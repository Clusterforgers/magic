//! What a payment leads to for different players: "If you do, each opponent loses 2 life and you gain 2 life." beside the
//! steps in a row (", then you gain 2 life").

/// Who a step after the first may be about.
const SUBJECTS: [&str; 4] = ["you ", "each opponent ", "each player ", "that player "];

/// The wordings with "and" for ", then" between steps of different players, after "If you do,".
pub(super) fn add(texts: &mut Vec<String>) {
    const LEAD: &str = "If you do, ";
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, tail) = text.split_at(text.find(LEAD)? + LEAD.len());
            let joined = SUBJECTS.iter().fold(tail.to_owned(), |so_far, subject| {
                so_far.replace(&format!(", then {subject}"), &format!(" and {subject}"))
            });
            (joined != tail).then(|| format!("{head}{joined}"))
        })
        .collect();
    texts.extend(found);
}
