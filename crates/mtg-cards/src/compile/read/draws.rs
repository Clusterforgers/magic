//! Draw wordings: "draw an additional card" and the wordings of a trigger that the renderer writes in more than one way.

use super::amounts::{cards, cards_counted, cards_text, count_word, number_word};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{Step, TriggerPattern};
use crate::value::Amount;

/// How many cards a player draws: "a card", "two cards for each creature", and in a draw step "an additional card" or
/// "two additional cards".
pub(super) fn drawn(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(additional).or_else(|| cards_counted(r))
}

/// The cards in a player's hand: what "discard your hand" discards.
pub(super) fn hand_of(who: &Ref) -> Amount {
    Amount::Count(Selector(vec![
        Pred::In(ZoneKind::Hand),
        Pred::OwnedBy(who.clone()),
    ]))
}

/// How many cards a player discards: "a card", "two cards", or every card of the hand when it is "your hand" (or
/// "their hand" for a player a target or the event names, or for each player or opponent: the count is every card in
/// those hands, more than any one of them holds, so each hand is taken whole).
pub(super) fn discarded(r: &mut Reader<'_>, who: &Ref) -> Option<Amount> {
    let own = match who {
        Ref::You => "your",
        Ref::Target(_) | Ref::Triggering | Ref::EachPlayer | Ref::Opponents => "their",
        _ => return cards(r),
    };
    let hand = r.attempt(|r| (r.word(own) && r.word("hand")).then(|| hand_of(who)));
    hand.or_else(|| cards(r))
}

/// "Each player discards their hand, then draws seven cards.": every player empties their hand and draws.
pub(super) fn wheel(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("each player discards their hand")?;
    r.punct(',').then_some(())?;
    r.need("then draws")?;
    let (bind, count) = if r.words(MOST_DISCARDED) {
        (Some(DISCARDED.to_owned()), most_discarded())
    } else {
        (None, drawn(r)?)
    };
    Some(vec![
        Effect::Discard {
            who: Ref::EachPlayer,
            count: hand_of(&Ref::EachPlayer),
            random: false,
            bind,
        },
        Effect::Draw {
            who: Ref::EachPlayer,
            count,
        },
    ])
}

/// The name the cards a sentence discards are bound to, for the sentence that counts them.
pub(super) const DISCARDED: &str = "discarded";

/// The count of "cards equal to the greatest number of cards a player discarded this way".
pub(super) const MOST_DISCARDED: &str =
    "cards equal to the greatest number of cards a player discarded this way";

/// The most cards one player discarded in the sentence before: the cards bound as `discarded`, counted for each player.
pub(super) fn most_discarded() -> Amount {
    Amount::Largest {
        who: Ref::EachPlayer,
        of: Selector(vec![
            Pred::In(ZoneKind::Graveyard),
            Pred::Is(Ref::Bound(DISCARDED.to_owned())),
            Pred::OwnedBy(Ref::You),
        ]),
    }
}

/// What a player discards, in words: the hand, or a number of cards.
pub(super) fn discarded_text(who: &Ref, count: &Amount) -> Option<String> {
    let own_hand = matches!(
        who,
        Ref::You | Ref::Target(_) | Ref::Triggering | Ref::EachPlayer | Ref::Opponents
    );
    if own_hand && *count == hand_of(who) {
        let own = if *who == Ref::You { "your" } else { "their" };
        return Some(format!("{own} hand"));
    }
    cards_text(count)
}

/// "an additional card", "three additional cards": the cards a draw step gives beyond the one it always draws.
fn additional(r: &mut Reader<'_>) -> Option<Amount> {
    let number = number_word(r.peek_word()?)?;
    r.skip();
    r.need("additional")?;
    (r.word("card") || r.word("cards")).then_some(())?;
    Some(Amount::Fixed(i32::try_from(number).ok()?))
}

/// The wordings of a trigger beyond those the renderer writes first: "each player's" before a step, "an additional"
/// card in a draw step and "you attack with two or more creatures" for the creatures you control that attack.
pub(super) fn wordings(ability: &Ability, texts: &[String]) -> Vec<String> {
    let Ability::Triggered { when, .. } = ability else {
        return Vec::new();
    };
    let mut found = Vec::new();
    match when {
        TriggerPattern::StepBegins {
            step,
            whose: Ref::EachPlayer,
        } if *step != Step::BeginCombat => {
            for text in texts {
                if let Some(rest) = text.strip_prefix("At the beginning of each ") {
                    found.push(format!("At the beginning of each player's {rest}"));
                }
            }
        }
        TriggerPattern::GroupAttacks { at_least, .. } => {
            found.extend(super::batches::attack_wordings(ability, texts));
            let Some(count) = count_word(*at_least) else {
                return found;
            };
            let from = format!("Whenever {count} or more creatures you control attack,");
            for text in texts {
                if let Some(rest) = text.strip_prefix(&from) {
                    found.push(format!(
                        "Whenever you attack with {count} or more creatures,{rest}"
                    ));
                }
            }
        }
        _ => {}
    }
    if matches!(
        when,
        TriggerPattern::StepBegins {
            step: Step::Draw,
            ..
        }
    ) {
        let both: Vec<&String> = texts.iter().chain(&found).collect();
        let extra: Vec<String> = both
            .into_iter()
            .filter_map(|text| additional_card(text))
            .collect();
        found.extend(extra);
    }
    found
}

/// The text of a draw-step trigger with its first draw worded "an additional card" ("two additional cards").
fn additional_card(text: &str) -> Option<String> {
    let (head, tail) = text.split_once(", ")?;
    for verb in ["draw", "draws"] {
        let plain = format!("{verb} a card");
        if tail.contains(&plain) {
            let extra = format!("{verb} an additional card");
            return Some(format!("{head}, {}", tail.replacen(&plain, &extra, 1)));
        }
        for number in 2..=10 {
            let word = count_word(number)?;
            let plain = format!("{verb} {word} cards");
            if tail.contains(&plain) {
                let extra = format!("{verb} {word} additional cards");
                return Some(format!("{head}, {}", tail.replacen(&plain, &extra, 1)));
            }
        }
    }
    None
}
