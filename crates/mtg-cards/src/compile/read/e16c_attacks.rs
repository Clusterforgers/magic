//! e16c: declarations of attackers at one player, written back: "Whenever you attack a player", "Whenever one or more
//! creatures attack you", "Whenever an opponent attacks you" and "Whenever two or more Warriors you control attack a
//! player". No template reads them: only the proposal checker writes them.

use mtg_core::card::CardType;

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

use super::selectors::{Form, phrase_text};

/// Wordings of one attack trigger: a player attacks a player when one or more creatures attack that player, and the
/// creatures you control attack only your opponents (CR 508.1b).
const SAME: [(&str, &str); 3] = [
    (
        "Whenever a player attacks you,",
        "Whenever one or more creatures attack you,",
    ),
    (
        "Whenever a player attacks one of your opponents,",
        "Whenever one or more creatures attack one of your opponents,",
    ),
    (
        " you control attack one of your opponents,",
        " you control attack a player,",
    ),
];

/// The text with each wording of `SAME` written the second way; both sides of a comparison are normalized.
pub(super) fn normal(text: &str) -> String {
    SAME.iter()
        .fold(text.to_owned(), |text, (from, to)| text.replace(from, to))
}

/// The attackers of one player change until end of turn: "it gets -1/-0" is "those creatures get -1/-0 until end of
/// turn" on a card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::GroupAttacksAt { .. },
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::Continuous {
            affected: Ref::Triggering,
            duration: Duration::UntilEndOfTurn,
            ..
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (head, change) = text.split_once(", it ")?;
            let change = change.strip_suffix(" until end of turn.")?;
            let (verb, rest) = change.split_once(' ')?;
            let plural = match verb {
                "gets" => "get",
                "gains" => "gain",
                _ => return None,
            };
            Some(format!(
                "{head}, those creatures {plural} {rest} until end of turn."
            ))
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// The player attacked, as the trigger words it.
fn attacked(at: &Ref) -> Option<&'static str> {
    Some(match at {
        Ref::EachPlayer => "a player",
        Ref::You => "you",
        Ref::Opponents => "one of your opponents",
        _ => return None,
    })
}

/// The wording of `GroupAttacksAt`; `None` for a shape it does not say.
pub(super) fn e16c_group_at_text(what: &Selector, at: &Ref, at_least: u32) -> Option<String> {
    let player = attacked(at)?;
    let creature = Pred::Type(CardType::Creature);
    if at_least == 1 {
        match what.0.as_slice() {
            // Only you attack with the creatures you control, so you attack someone else.
            [kind, Pred::ControlledBy(Ref::You)] if *kind == creature && *at == Ref::EachPlayer => {
                return Some("Whenever you attack a player".to_owned());
            }
            [kind, Pred::ControlledBy(Ref::Opponents)] if *kind == creature && *at == Ref::You => {
                return Some("Whenever an opponent attacks you".to_owned());
            }
            // "Whenever you attack a player with one or more equipped creatures": a quality of the creatures.
            [kind, Pred::ControlledBy(Ref::You), quality]
                if *kind == creature && *at == Ref::EachPlayer && *quality != Pred::Token =>
            {
                let group = Selector(vec![creature, quality.clone()]);
                return Some(format!(
                    "Whenever you attack a player with one or more {}",
                    phrase_text(&group, Form::All)?
                ));
            }
            _ => {}
        }
    }
    let group = super::triggers::group_attacks_text(what, at_least)?;
    Some(format!("{group} {player}"))
}
