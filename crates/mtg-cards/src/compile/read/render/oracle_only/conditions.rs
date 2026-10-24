//! An intervening "if" the trigger renderer has no wording for: the conditions of static abilities ("if there are four or
//! more card types among cards in your graveyard") and a count of the hand of the player of the event ("if that player
//! has no cards in hand"). The trigger is written without the condition and the condition is put after its event.

use super::super::super::as_long_as;
use super::super::super::draws;
use super::super::super::facts;
use super::super::{pattern_text, variants};
use crate::ability::Ability;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition};

/// Adds the wordings of a trigger whose intervening condition only these writers word.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if: Some(condition),
        limit: None,
        ..
    } = ability
    else {
        return;
    };
    let phrases = phrases(condition);
    let Some(event) = pattern_text(when) else {
        return;
    };
    if phrases.is_empty() {
        return;
    }
    let mut bare = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut bare {
        *intervening_if = None;
    }
    let opening = format!("{event}, ");
    let mut spliced = Vec::new();
    for text in variants(&bare) {
        if let Some(rest) = text.strip_prefix(&opening) {
            for phrase in &phrases {
                spliced.push(format!("{opening}if {phrase}, {rest}"));
            }
        }
    }
    // "At the beginning of each player's upkeep" is another wording of the event.
    spliced.extend(draws::wordings(ability, &spliced));
    texts.extend(spliced);
}

/// The ways the condition is worded after "if": a count of the cards of a hand or a graveyard, whose wording a static
/// ability shares.
pub(super) fn phrases(condition: &Condition) -> Vec<String> {
    if let Some(state) = own_state(condition) {
        return vec![state];
    }
    let Condition::Compare {
        left: Amount::Count(Selector(preds)) | Amount::CardTypes(Selector(preds)),
        ..
    } = condition
    else {
        return Vec::new();
    };
    let cards = preds
        .iter()
        .any(|pred| matches!(pred, Pred::In(ZoneKind::Hand | ZoneKind::Graveyard)));
    if !cards {
        return Vec::new();
    }
    let mut found = as_long_as::texts(condition);
    found.extend(player_hand(condition));
    found
}

/// "~ is untapped": the state of the card the trigger belongs to.
fn own_state(condition: &Condition) -> Option<String> {
    let Condition::Matches {
        subject: Ref::This,
        selector,
    } = condition
    else {
        return None;
    };
    (selector.0 == [Pred::Untapped]).then(|| "~ is untapped".to_owned())
}

/// "that player has no cards in hand", "that player has seven or more cards in hand".
fn player_hand(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let hand = [Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::Triggering)];
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    if preds.as_slice() != hand {
        return Vec::new();
    }
    facts::quantity_texts(number)
        .iter()
        .filter_map(|words| match op {
            Cmp::Eq if number == 0 => Some("that player has no cards in hand".to_owned()),
            Cmp::Ge => Some(format!("that player has {words} or more cards in hand")),
            Cmp::Le => Some(format!("that player has {words} or fewer cards in hand")),
            _ => None,
        })
        .collect()
}
