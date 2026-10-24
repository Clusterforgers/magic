//! A search whose card is put onto the battlefield, with the shuffle as a sentence of its own: "Search your library for a
//! Forest card and put that card onto the battlefield. Then shuffle." The renderers join the three steps with commas.

const JOINED: &str = ", put that card onto the battlefield";
const SPLIT: &str = " and put that card onto the battlefield";

/// Adds the wordings of a search that puts the card onto the battlefield before the shuffle.
pub(super) fn add(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("Search your library for "))
        .filter_map(|text| {
            let (head, tail) = text.split_once(JOINED)?;
            let (tapped, rest) = match tail.split_once(", then shuffle") {
                Some((tapped, rest)) if tapped.is_empty() || tapped == " tapped" => (tapped, rest),
                _ => return None,
            };
            Some(format!("{head}{SPLIT}{tapped}. Then shuffle{rest}"))
        })
        .collect();
    texts.extend(said);
}
