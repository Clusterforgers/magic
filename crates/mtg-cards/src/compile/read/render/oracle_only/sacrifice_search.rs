//! A sacrifice and a search in one sentence: "..., sacrifice this enchantment, search your library for up to two creature
//! cards, put those cards onto the battlefield, then shuffle." The renderers end the sacrifice with a period and call the
//! found cards "them".

use crate::ability::Ability;
use crate::effect::Effect;

/// Adds the wordings of an ability that sacrifices a permanent and then searches, as one sentence.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let searches = matches!(
        effects,
        [Effect::Sacrifice { .. }, Effect::Search { .. }, ..]
    );
    if !searches {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| joined(text)).collect();
    texts.extend(found);
}

/// The text with the search after a comma and the found cards as "those cards".
fn joined(text: &str) -> Option<String> {
    let at = [". Then search your library", ". Search your library"]
        .iter()
        .find_map(|opening| text.find(opening).map(|at| (at, opening.len())))?;
    let (head, tail) = text.split_at(at.0);
    let tail = &tail[at.1 - "search your library".len()..];
    let tail = tail
        .replacen(", put them onto", ", put those cards onto", 1)
        .replacen(", you put them onto", ", put those cards onto", 1);
    Some(format!("{head}, {tail}"))
}
