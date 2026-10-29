//! The activated abilities of an object in the words of its card (`ObjectView.activated`).

use mtg_cards::AbilityDef;
use mtg_cards::ability::{Ability, LoyaltyCost};
use mtg_cards::compile::{render_ability, render_cost};
use mtg_cards::cost::{Cost, CostItem};
use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaType;
use mtg_engine::abilities::AbilitySource;
use mtg_engine::ability_reader::{abilities_of, keyword_adding};
use mtg_engine::state::GameState;

use crate::view::ActivatedView;

/// Words that end the phrase of a target in an effect ("target creature" in "Destroy target creature.").
const PHRASE_ENDS: [&str; 26] = [
    ",",
    ".",
    ";",
    " and ",
    " to ",
    " until ",
    " gets ",
    " gains ",
    " deals ",
    " can't ",
    " doesn't ",
    " into ",
    " onto ",
    " on top",
    " or ",
    " draws ",
    " loses ",
    " discards ",
    " sacrifices ",
    " mills ",
    " becomes ",
    " fights ",
    " attacks ",
    " blocks ",
    " phases ",
    " puts ",
];

/// Words before "target" that belong to its phrase ("any target", "another target", "up to two target").
const PHRASE_STARTS: [&str; 11] = [
    "any", "another", "other", "one", "two", "three", "four", "five", "X", "each", "of",
];

/// The activated abilities of `id` in oracle wording: the plain tap-for-mana abilities of `mana` first, then every
/// activated and loyalty ability `abilities_of` lists, with the address `PriorityOptions.abilities` uses. `name` stands
/// for the card's own name (`~`) in the words.
pub fn activated_views(
    state: &GameState,
    id: ObjectId,
    mana: &[AbilityDef],
    name: &str,
) -> Vec<ActivatedView> {
    let mut views: Vec<ActivatedView> = mana
        .iter()
        .map(|AbilityDef::Mana(ability)| plain_mana(ability.produces))
        .collect();
    let mut place = 0;
    for (address, rules) in abilities_of(state, id) {
        let loyalty = match &*rules {
            Ability::Loyalty { cost, .. } => Some(*cost),
            Ability::Activated { .. } => None,
            _ => continue,
        };
        place += 1;
        let keyword = match address.source {
            AbilitySource::PrintedKeyword(_) | AbilitySource::GrantedKeyword(_) => {
                keyword_adding(state, address).map(|keyword| keyword_line(keyword.name(), &rules))
            }
            AbilitySource::Printed | AbilitySource::Granted(_) => None,
        };
        let mut view = worded(&rules, name, loyalty).unwrap_or_else(|| ActivatedView {
            ability: None,
            cost: match &*rules {
                Ability::Activated { cost, .. } => render_cost(cost)
                    .map(|text| named(&text, name))
                    .unwrap_or_default(),
                _ => loyalty.map(loyalty_text).unwrap_or_default(),
            },
            effect: keyword
                .clone()
                .unwrap_or_else(|| format!("Activated ability {place}")),
            targets: Vec::new(),
            loyalty,
            mana: false,
            keyword: None,
            unworded: true,
        });
        view.ability = Some(address);
        view.mana = rules.is_mana_ability();
        view.keyword = keyword;
        views.push(view);
    }
    views
}

/// The keyword line that adds an ability: the keyword's name and the mana of the ability's cost (`Cycling {2}`).
fn keyword_line(keyword: &str, rules: &Ability) -> String {
    let mana = match rules {
        Ability::Activated { cost, .. } => cost
            .0
            .iter()
            .find(|item| matches!(item, CostItem::Mana(_)))
            .and_then(|item| render_cost(&Cost(vec![item.clone()]))),
        _ => None,
    };
    mana.map_or_else(|| keyword.to_owned(), |mana| format!("{keyword} {mana}"))
}

/// `{T}: Add {G}.`, the ability a basic land type gives and any plain tap-for-mana ability.
fn plain_mana(produces: ManaType) -> ActivatedView {
    let symbol = match produces {
        ManaType::White => "{W}",
        ManaType::Blue => "{U}",
        ManaType::Black => "{B}",
        ManaType::Red => "{R}",
        ManaType::Green => "{G}",
        ManaType::Colorless => "{C}",
    };
    ActivatedView {
        ability: None,
        cost: "{T}".to_owned(),
        effect: format!("Add {symbol}."),
        targets: Vec::new(),
        loyalty: None,
        mana: true,
        keyword: None,
        unworded: false,
    }
}

/// The ability in the writer's words split at its colon, or `None` when the writer cannot word it.
fn worded(rules: &Ability, name: &str, loyalty: Option<LoyaltyCost>) -> Option<ActivatedView> {
    let text = named(&render_ability(rules)?, name);
    let (cost, effect) = split_cost(&text)?;
    Some(ActivatedView {
        ability: None,
        cost: cost.to_owned(),
        effect: effect.to_owned(),
        targets: target_phrases(effect, rules.targets().len()),
        loyalty,
        mana: false,
        keyword: None,
        unworded: false,
    })
}

/// The text with the card's name for `~`.
fn named(text: &str, name: &str) -> String {
    text.replace('~', name)
}

/// The cost and the effect of "cost: effect", split at the first colon outside quotes.
fn split_cost(text: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    for (at, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ':' if !quoted => return Some((text[..at].trim(), text[at + 1..].trim())),
            _ => {}
        }
    }
    None
}

/// The loyalty cost as a card prints it: `+1`, `−2`, `0`, `−X` (with a minus sign).
fn loyalty_text(cost: LoyaltyCost) -> String {
    match cost {
        LoyaltyCost::Plus(n) => format!("+{n}"),
        LoyaltyCost::Minus(n) => format!("\u{2212}{n}"),
        LoyaltyCost::Zero => "0".to_owned(),
        LoyaltyCost::MinusX => "\u{2212}X".to_owned(),
    }
}

/// The phrase of each target in the order the effect names them ("any target", "target creature you control"); empty
/// when the effect does not name as many targets as the ability has.
fn target_phrases(effect: &str, wanted: usize) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut rest = effect;
    while let Some(at) = find_word(rest, "target") {
        let start = phrase_start(&rest[..at]);
        let tail = &rest[at..];
        let end = PHRASE_ENDS
            .iter()
            .filter_map(|word| tail.find(word))
            .min()
            .unwrap_or(tail.len());
        phrases.push(
            format!("{}{}", &rest[start..at], &tail[..end])
                .trim()
                .to_owned(),
        );
        rest = &tail[end..];
    }
    if phrases.len() == wanted {
        phrases
    } else {
        Vec::new()
    }
}

/// Where the word `word` begins in `text`, as a whole word in any case.
fn find_word(text: &str, word: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    lower.match_indices(word).map(|(at, _)| at).find(|&at| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Where the phrase of a target begins in the text before "target": the words of `PHRASE_STARTS` and "up to" just
/// before it belong to it.
fn phrase_start(before: &str) -> usize {
    let mut kept = before.trim_end();
    loop {
        let word = kept.rsplit(' ').next().unwrap_or_default();
        let taken = if kept == "up to" || kept.ends_with(" up to") {
            "up to".len()
        } else if PHRASE_STARTS.contains(&word) {
            word.len()
        } else {
            break;
        };
        kept = kept[..kept.len() - taken].trim_end();
    }
    if kept.is_empty() { 0 } else { kept.len() + 1 }
}

#[cfg(test)]
mod tests {
    //! Goal: the words of an ability split into cost and effect at the colon outside quotes, and the phrase of each target is read off the effect, with "any", "another" and "up to N" before "target" kept and nothing when the count differs.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("{T}: Add {G}.", Some(("{T}", "Add {G}.")))]
    #[case(
        "{2}, {T}, Sacrifice Mind Stone: Draw a card.",
        Some(("{2}, {T}, Sacrifice Mind Stone", "Draw a card."))
    )]
    #[case(
        "{1}: Target creature gains \"{T}: Add {G}.\" until end of turn.",
        Some(("{1}", "Target creature gains \"{T}: Add {G}.\" until end of turn."))
    )]
    #[case("Draw a card.", None)]
    fn the_cost_ends_at_the_first_colon_outside_quotes(
        #[case] text: &str,
        #[case] expected: Option<(&str, &str)>,
    ) {
        assert_eq!(split_cost(text), expected);
    }

    #[rstest]
    #[case("Destroy target creature.", 1, &["target creature"])]
    #[case("Pinger deals 1 damage to any target.", 1, &["any target"])]
    #[case("Tap up to two target creatures.", 1, &["up to two target creatures"])]
    #[case("Return another target creature you control to its owner's hand.", 1, &["another target creature you control"])]
    #[case("Target player draws a card and loses 1 life.", 1, &["Target player"])]
    #[case(
        "Put a +1/+1 counter on target creature. Tap target artifact.",
        2,
        &["target creature", "target artifact"]
    )]
    #[case("Draw a card.", 0, &[])]
    #[case("Destroy target creature.", 2, &[])]
    #[case("Targeted creatures draw.", 0, &[])]
    fn the_phrase_of_each_target_is_read_off_the_effect(
        #[case] effect: &str,
        #[case] wanted: usize,
        #[case] expected: &[&str],
    ) {
        assert_eq!(target_phrases(effect, wanted), expected);
    }

    #[rstest]
    #[case(LoyaltyCost::Plus(1), "+1")]
    #[case(LoyaltyCost::Minus(3), "\u{2212}3")]
    #[case(LoyaltyCost::Zero, "0")]
    #[case(LoyaltyCost::MinusX, "\u{2212}X")]
    fn a_loyalty_cost_is_printed_with_its_sign(#[case] cost: LoyaltyCost, #[case] text: &str) {
        assert_eq!(loyalty_text(cost), text);
    }
}
