//! r40-7 wordings of a surveil the next steps read "this way": "Surveil 1. If you put a noncreature, nonland card into
//! your graveyard this way, put that card into your hand." (the cards put into the graveyard, `Surveil.as`) and
//! "Surveil 2. Then for each card you put on top of your library, you draw a card and you lose 3 life." (the cards
//! kept on top, `Surveil.kept_as`). Each is worded from the same ability that ends with a plain surveil.

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use mtg_core::card::CardType;

/// Adds the wordings of an ability whose last steps read what its surveil did.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(effects) = effects_of(ability) else {
        return;
    };
    let Some(at) = effects
        .iter()
        .position(|effect| matches!(effect, Effect::Surveil { .. }))
    else {
        return;
    };
    let Some(Effect::Surveil {
        who: Ref::You,
        count,
        bind,
        kept,
    }) = effects.get(at)
    else {
        return;
    };
    let rest = &effects[at + 1..];
    let sentence = match (bind, kept) {
        (Some(bind), None) => graved_sentence(bind, rest),
        (None, Some(kept)) => kept_sentence(kept, rest),
        _ => None,
    };
    let Some(sentence) = sentence else {
        return;
    };
    let mut twin = ability.clone();
    if let Some(steps) = effects_of_mut(&mut twin) {
        steps.truncate(at);
        steps.push(Effect::Surveil {
            who: Ref::You,
            count: count.clone(),
            bind: None,
            kept: None,
        });
    }
    for text in super::render::written_variants(&twin) {
        let found = format!("{text} {sentence}");
        if text.ends_with('.') && !texts.contains(&found) {
            texts.push(found);
        }
    }
}

/// "If you put a noncreature, nonland card into your graveyard this way, put that card into your hand.": the cards the
/// surveil put into the graveyard that are of no type the filter excludes return to their owner's hand.
fn graved_sentence(bind: &str, rest: &[Effect]) -> Option<String> {
    let [
        Effect::Move {
            what: Ref::Each(Selector(preds)),
            to: Destination::Hand,
            bind: None,
        },
    ] = rest
    else {
        return None;
    };
    let [Pred::Is(named), filter @ .., Pred::In(ZoneKind::Graveyard)] = preds.as_slice() else {
        return None;
    };
    if *named != Ref::Bound(bind.to_owned()) || filter.is_empty() {
        return None;
    }
    let words: Option<Vec<String>> = filter
        .iter()
        .map(|pred| match pred {
            Pred::Not(inner) => match inner.as_ref() {
                Pred::Type(kind) => Some(format!("non{}", type_word(*kind)?)),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let words = words?.join(", ");
    let article = if words.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!(
        "If you put {article} {words} card into your graveyard this way, put that card into your hand."
    ))
}

/// "Then for each card you put on top of your library, you draw a card and you lose 3 life.": the number of cards the
/// surveil kept on top is fixed first (the draw moves them), then that many cards are drawn and three times that much
/// life is lost.
fn kept_sentence(kept: &str, rest: &[Effect]) -> Option<String> {
    let [
        Effect::Let {
            bind: number,
            amount:
                Amount::CountOf {
                    of: Ref::Bound(counted),
                    filter,
                },
        },
        Effect::Draw {
            who: Ref::You,
            count: Amount::Bound(drawn),
            bind: None,
        },
        Effect::LoseLife {
            who: Ref::You,
            amount: Amount::Times { by, of },
        },
    ] = rest
    else {
        return None;
    };
    let fixed = Amount::Bound(number.clone());
    (counted == kept
        && filter.is_empty()
        && drawn == number
        && **of == fixed
        && *by > 0)
        .then(|| {
            format!(
                "Then for each card you put on top of your library, you draw a card and you lose {by} life."
            )
        })
}

/// The word of a card type after "non".
fn type_word(kind: CardType) -> Option<&'static str> {
    Some(match kind {
        CardType::Creature => "creature",
        CardType::Land => "land",
        CardType::Artifact => "artifact",
        CardType::Enchantment => "enchantment",
        CardType::Planeswalker => "planeswalker",
        CardType::Instant => "instant",
        CardType::Sorcery => "sorcery",
        _ => return None,
    })
}

/// The steps of an ability that has them.
fn effects_of(ability: &Ability) -> Option<&Vec<Effect>> {
    match ability {
        Ability::Triggered { effects, .. }
        | Ability::Loyalty { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Spell { effects, .. } => Some(effects),
        _ => None,
    }
}

/// The steps of an ability that has them, to change.
fn effects_of_mut(ability: &mut Ability) -> Option<&mut Vec<Effect>> {
    match ability {
        Ability::Triggered { effects, .. }
        | Ability::Loyalty { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Spell { effects, .. } => Some(effects),
        _ => None,
    }
}
