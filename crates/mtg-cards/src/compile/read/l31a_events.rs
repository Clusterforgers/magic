//! Events of pack l31-a: a player, or a player and the permanents they control, becoming the target of a spell or
//! ability ("Whenever you or a permanent you control becomes the target of a spell or ability an opponent controls"), and
//! the keyword actions a player performs ("Whenever you investigate", "Whenever you discover", "Whenever you manifest
//! dread", "Whenever you give a gift").

use super::tokens::Reader;
use super::triggers::player_verb;
use crate::ability::{Ability, Limit};
use crate::did::Action;
use crate::effect::{Destination, Effect, discover};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// What targets: "a spell or ability", "a spell", each with "an opponent controls" or "you control" after it.
fn targeting(r: &mut Reader<'_>) -> Option<Selector> {
    r.need("the target of")?;
    let mut by = Vec::new();
    if !r.words("a spell or ability") {
        r.need("a spell")?;
        by.push(Pred::Spell);
    }
    if r.words("an opponent controls") {
        by.push(Pred::ControlledBy(Ref::Opponents));
    } else if r.words("you control") {
        by.push(Pred::ControlledBy(Ref::You));
    }
    Some(Selector(by))
}

/// The words of [`targeting`].
fn targeting_text(by: &Selector) -> Option<&'static str> {
    Some(match by.0.as_slice() {
        [] => "a spell or ability",
        [Pred::ControlledBy(Ref::Opponents)] => "a spell or ability an opponent controls",
        [Pred::ControlledBy(Ref::You)] => "a spell or ability you control",
        [Pred::Spell] => "a spell",
        [Pred::Spell, Pred::ControlledBy(Ref::Opponents)] => "a spell an opponent controls",
        [Pred::Spell, Pred::ControlledBy(Ref::You)] => "a spell you control",
        _ => return None,
    })
}

/// After "When" or "Whenever": "you become the target of ..." and "you or a permanent you control becomes the target of
/// ...", the second one event for the player and one for each permanent (each target triggers it once).
pub(super) fn l31a_targeted(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.word("you").then_some(())?;
    if r.word("become") {
        let by = targeting(r)?;
        return Some(TriggerPattern::PlayerBecomesTarget { who: Ref::You, by });
    }
    r.word("or").then_some(())?;
    // "you or another permanent you control" leaves the source out (Valkmira, Protector's Shield).
    let another = r.word("another");
    if !another {
        r.word("a").then_some(())?;
    }
    r.need("permanent you control becomes")?;
    let by = targeting(r)?;
    let what = if another {
        vec![Pred::Other, Pred::ControlledBy(Ref::You)]
    } else {
        vec![Pred::ControlledBy(Ref::You)]
    };
    Some(TriggerPattern::AnyOf(vec![
        TriggerPattern::PlayerBecomesTarget {
            who: Ref::You,
            by: by.clone(),
        },
        TriggerPattern::BecomesTarget {
            what: Selector(what),
            by,
        },
    ]))
}

/// The words of [`l31a_targeted`]'s pair: "Whenever you or a permanent you control becomes the target of a spell or
/// ability an opponent controls".
pub(super) fn l31a_targeted_text(patterns: &[TriggerPattern]) -> Option<String> {
    let [
        TriggerPattern::PlayerBecomesTarget { who: Ref::You, by },
        TriggerPattern::BecomesTarget { what, by: also },
    ] = patterns
    else {
        return None;
    };
    (by == also).then_some(())?;
    let permanent = match what.0.as_slice() {
        [Pred::ControlledBy(Ref::You)] => "a permanent",
        [Pred::Other, Pred::ControlledBy(Ref::You)] => "another permanent",
        _ => return None,
    };
    Some(format!(
        "Whenever you or {permanent} you control becomes the target of {}",
        targeting_text(by)?
    ))
}

/// The keyword actions a player performs with no object, and the verb of each.
const ACTIONS: [(Action, &str); 5] = [
    (Action::Investigated, "investigate"),
    (Action::Discovered, "discover"),
    (Action::ManifestedDread, "manifest dread"),
    (Action::GaveGift, "give a gift"),
    (Action::LostCoinFlip, "lose a coin flip"),
];

/// After "When" or "Whenever": "you investigate", "an opponent discovers", "you manifest dread", "you give a gift".
pub(super) fn l31a_action(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    let who = if r.word("you") {
        Ref::You
    } else if r.words("an opponent") {
        Ref::Opponents
    } else if r.words("a player") {
        Ref::EachPlayer
    } else {
        return None;
    };
    let action = ACTIONS.iter().find_map(|(action, verb)| {
        let said = if who == Ref::You {
            (*verb).to_owned()
        } else {
            third_person(verb)
        };
        r.words(&said).then_some(*action)
    })?;
    let did = TriggerPattern::Did {
        action,
        who,
        what: Selector::default(),
    };
    Some(
        if l31a_counted(action) && r.words("for the first time each turn") {
            TriggerPattern::Nth {
                event: Box::new(did),
                nth: 1,
            }
        } else {
            did
        },
    )
}

/// "discovers", "manifests dread", "gives a gift": the verb of the first word for a third person.
fn third_person(verb: &str) -> String {
    match verb.split_once(' ') {
        Some((first, rest)) => format!("{first}s {rest}"),
        None => format!("{verb}s"),
    }
}

/// The verb phrase of [`l31a_action`] for a player: "you investigate", "an opponent gives a gift".
pub(super) fn l31a_action_text(action: Action, who: &Ref) -> Option<String> {
    let verb = ACTIONS
        .iter()
        .find_map(|(found, verb)| (*found == action).then_some(*verb))?;
    let (first, rest) = verb.split_once(' ').unwrap_or((verb, ""));
    let said = player_verb(who, first)?;
    Some(if rest.is_empty() {
        said
    } else {
        format!("{said} {rest}")
    })
}

/// The steps after "Whenever you <action>" that no other writer words, as the card says them: "investigate an additional
/// time" (an investigation's log is dropped before the wording), "discover again for the same value" (with the trigger
/// limited to once each turn), "put a card you put into your graveyard this way into your hand".
fn after_action(action: Action, effects: &[Effect], limit: Option<Limit>) -> Option<&'static str> {
    let clue = super::create::clue(Ref::You);
    match action {
        Action::Investigated if effects == [clue] && limit.is_none() => {
            Some("investigate an additional time")
        }
        Action::Discovered
            if effects == discover(Amount::Triggering).as_slice()
                && limit == Some(Limit::OncePerTurn) =>
        {
            Some("discover again for the same value. This ability triggers only once each turn")
        }
        Action::ManifestedDread if limit.is_none() => {
            let moved = Effect::Move {
                what: Ref::Triggering,
                to: Destination::Hand,
                bind: None,
            };
            (effects == [moved])
                .then_some("put a card you put into your graveyard this way into your hand")
        }
        _ => None,
    }
}

/// Whole triggered abilities on a keyword action a player performs whose steps only the card's own words say:
/// "Whenever you investigate for the first time each turn, investigate an additional time.", "Whenever you discover,
/// discover again for the same value. This ability triggers only once each turn.", "Whenever you manifest dread, put a
/// card you put into your graveyard this way into your hand."
pub(super) fn l31a_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if: None,
        limit,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return;
    };
    if !targets.is_empty() {
        return;
    }
    let (event, first) = match when {
        TriggerPattern::Nth { event, nth: 1 } => (&**event, true),
        other => (other, false),
    };
    let TriggerPattern::Did { action, who, what } = event else {
        return;
    };
    if !what.is_empty() {
        return;
    }
    let Some(head) = l31a_action_text(*action, who) else {
        return;
    };
    let Some(tail) = after_action(*action, effects, *limit) else {
        return;
    };
    let counted = if first {
        " for the first time each turn"
    } else {
        ""
    };
    let text = format!("Whenever {head}{counted}, {tail}.");
    if !texts.contains(&text) {
        texts.push(text);
    }
}

/// Whether "for the first time each turn" counts this action's times: investigations, which the engine counts.
pub(super) fn l31a_counted(action: Action) -> bool {
    action == Action::Investigated
}
