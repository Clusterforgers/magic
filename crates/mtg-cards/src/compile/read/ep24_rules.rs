//! The rules of ep24 written as the card words them: unspent mana kept ("You don't lose unspent red mana as steps and
//! phases end", "Players don't lose unspent mana as steps and phases end") or turned into one type ("If you would lose
//! unspent mana, that mana becomes colorless instead"), and mana of one colour among objects.

use mtg_core::mana::ManaType;

use super::selectors::{Form, phrase_text};
use super::triggers::article;
use crate::ability::Ability;
use crate::effect::{Effect, Produce};
use crate::modify::{GameRule, SpendAs};
use crate::refs::{Pred, Ref, Selector, ZoneKind};

/// The word of a mana type in "unspent red mana".
fn type_word(mana: ManaType) -> &'static str {
    match mana {
        ManaType::White => "white",
        ManaType::Blue => "blue",
        ManaType::Black => "black",
        ManaType::Red => "red",
        ManaType::Green => "green",
        ManaType::Colorless => "colorless",
    }
}

/// The mana of one colour among objects: "one mana of any color among legendary permanents you control", "one mana of
/// any of this creature's colors".
pub(super) fn ep24_color_among_text(among: &Selector) -> Option<String> {
    if among.0 == [Pred::This] {
        return Some("one mana of any of this creature's colors".to_owned());
    }
    Some(format!(
        "one mana of any color among {}",
        among_words(among)?
    ))
}

/// The objects as "permanents you control", "cards in your graveyard", "legendary creature cards in your graveyard".
fn among_words(among: &Selector) -> Option<String> {
    let mine = [Pred::OwnedBy(Ref::You), Pred::In(ZoneKind::Graveyard)];
    if among.0.len() == 2 && mine.iter().all(|pred| among.0.contains(pred)) {
        return Some("cards in your graveyard".to_owned());
    }
    Some(phrase_text(among, Form::All)?.replace(" from your graveyard", " in your graveyard"))
}

/// The first plural noun of `words` made singular: "legendary creature cards in your graveyard" is "legendary creature
/// card in your graveyard".
fn singular(words: &str) -> Option<String> {
    const NOUNS: [&str; 5] = ["cards", "permanents", "creatures", "artifacts", "lands"];
    let mut found = false;
    let out: Vec<&str> = words
        .split(' ')
        .map(|word| {
            if !found && NOUNS.contains(&word) {
                found = true;
                word.strip_suffix('s').unwrap_or(word)
            } else {
                word
            }
        })
        .collect();
    found.then(|| out.join(" "))
}

/// Adds "Choose a color of a permanent you control. Add one mana of that color." beside "Add one mana of any color among
/// permanents you control." for an ability that adds the mana of one colour among objects.
pub(super) fn ep24_variants(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    let Some(among) = effects.iter().find_map(|effect| match effect {
        Effect::AddMana {
            mana: Produce::AnyColorAmong(among),
            ..
        } => Some(among),
        _ => None,
    }) else {
        return;
    };
    let (Some(plural), Some(single)) = (
        ep24_color_among_text(among),
        among_words(among).and_then(|words| singular(&words)),
    ) else {
        return;
    };
    let added = format!("Add {plural}");
    let chosen = format!(
        "Choose a color of {} {single}. Add one mana of that color",
        article(&single)
    );
    let more: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(&added))
        .map(|text| text.replacen(&added, &chosen, 1))
        .collect();
    texts.extend(more);
}

/// The sentences of a rule of ep24 without the period; empty for any other rule.
pub(super) fn ep24_rule_texts(rule: &GameRule) -> Vec<String> {
    match rule {
        GameRule::KeepUnspentMana { who, mana } => {
            let subject = match who {
                Ref::You => "You don't",
                Ref::EachPlayer => "Players don't",
                _ => return Vec::new(),
            };
            let kind = mana.map(|mana| format!("{} ", type_word(mana)));
            vec![format!(
                "{subject} lose unspent {}mana as steps and phases end",
                kind.unwrap_or_default()
            )]
        }
        GameRule::SpendManaAs {
            who: Ref::You,
            spend_as,
            spells,
        } => {
            let kind = match spend_as {
                SpendAs::AnyColor => "color",
                SpendAs::AnyType => "type",
                SpendAs::Normal => return Vec::new(),
            };
            let tail = match spells.as_ref().map(|spells| spells.0.as_slice()) {
                None => String::new(),
                Some([Pred::Type(kind)]) => {
                    format!(" to cast {} spells", format!("{kind:?}").to_lowercase())
                }
                Some(_) => return Vec::new(),
            };
            let mut texts = vec![format!(
                "You may spend mana as though it were mana of any {kind}{tail}"
            )];
            // The current Oracle wording of the same rule.
            if *spend_as == SpendAs::AnyType {
                texts.push(format!("You can spend mana of any type{tail}"));
            }
            texts
        }
        GameRule::UnspentManaBecomes {
            who: Ref::You,
            into,
        } => vec![format!(
            "If you would lose unspent mana, that mana becomes {} instead",
            type_word(*into)
        )],
        _ => Vec::new(),
    }
}
