//! Writers of the r30-1b pack: an ability that reveals any number of cards of a kind from its controller's hand and adds
//! mana for each card revealed ("{T}: Reveal any number of artifact cards in your hand. Add {C}{C} for each card revealed
//! this way."). Only the checker needs the wording; no template reads it.

use super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;

/// The wordings of a reveal for mana, each the wording of the same ability with only the mana added and the reveal put
/// in front of it.
pub(super) fn r301b_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some((kinds, plain)) = revealed_for_mana(ability) else {
        return;
    };
    let reveal = kinds
        .iter()
        .map(|kind| format!("Reveal any number of {kind}"))
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for text in written(&plain) {
        let Some((head, added)) = text.split_once(": Add ") else {
            continue;
        };
        let Some(added) = added.strip_suffix('.') else {
            continue;
        };
        for said in &reveal {
            found.push(format!(
                "{head}: {said}. Add {added} for each card revealed this way."
            ));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The kind of the revealed cards with where they are ("artifact cards in your hand", "creature cards with power 5 or
/// greater from your hand") and the ability with the mana alone, for an activated ability that chooses any number of
/// cards from its controller's hand, reveals them and adds mana once for each.
fn revealed_for_mana(ability: &Ability) -> Option<(Vec<String>, Ability)> {
    let Ability::Activated { effects, .. } = ability else {
        return None;
    };
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(selector),
            count,
            bind,
            rest_as: None,
        },
        Effect::Reveal {
            what: Ref::Bound(shown),
        },
        Effect::AddMana {
            times:
                Amount::CountOf {
                    of: Ref::Bound(counted),
                    filter,
                },
            ..
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    let any = count.min == Amount::Fixed(0) && count.max.is_none();
    (any && shown == bind && counted == bind && filter.0.is_empty()).then_some(())?;
    let in_hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)];
    in_hand
        .iter()
        .all(|pred| selector.0.contains(pred))
        .then_some(())?;
    let kind = Selector(
        selector
            .0
            .iter()
            .filter(|pred| !in_hand.contains(pred))
            .cloned()
            .collect(),
    );
    (!kind.0.is_empty()).then_some(())?;
    let mut cards = kind.clone();
    cards.0.push(Pred::In(ZoneKind::Hand));
    let named = phrase_text(&cards, Form::All)?;
    let named = named.strip_suffix(" in hand").unwrap_or(&named).to_owned();
    let named = if named.contains(" cards") {
        named
    } else {
        format!("{named} cards")
    };
    let kinds = vec![
        format!("{named} in your hand"),
        format!("{named} from your hand"),
    ];
    let mut plain = ability.clone();
    if let Ability::Activated { effects, .. } = &mut plain {
        let mut added = effects.pop()?;
        if let Effect::AddMana { times, .. } = &mut added {
            *times = Amount::Fixed(1);
        }
        *effects = vec![added];
    }
    Some((kinds, plain))
}
