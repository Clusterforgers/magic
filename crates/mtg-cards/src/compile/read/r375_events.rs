//! r37-5 trigger events and their writers: "Whenever you pay life", "Whenever ~ crews a Vehicle, that Vehicle ..." (the
//! Vehicle becoming crewed, if ~ crewed it, is the triggering object), "Whenever a creature you control exploits a nontoken
//! creature", "Whenever an opponent gains control of a permanent from you" and "Whenever a creature you control
//! explores a land card" (or a nonland card).

use super::tokens::Reader;
use super::triggers::{subject, subject_text};
use crate::ability::Ability;
use crate::did::Action;
use crate::refs::{Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;
use mtg_core::card::Subtype;

/// The Vehicle a crew trigger's steps name, as a target of the twin they are read and written with.
const TARGET_VEHICLE: &str = "target Vehicle";
/// The Vehicle a crew trigger's steps name, as the card words it.
const THAT_VEHICLE: &str = "that Vehicle";
/// The head of the twin of a crew trigger.
const TWIN_HEAD: &str = "When ~ enters, ";
/// The head of a crew trigger of the card itself.
const CREW_HEAD: &str = "Whenever ~ crews a Vehicle, ";
/// The same head as the compiler meets it on a card that calls itself "this creature".
const CREW_HEAD_WORDED: &str = "Whenever this creature crews a Vehicle, ";

/// After "When" or "Whenever": "you pay life", "a creature you control exploits a nontoken
/// creature", "an opponent gains control of a permanent from you".
pub(super) fn r375_event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    if r.words("you pay life") {
        return Some(did(Action::PaidLife, Ref::You, Selector::default()));
    }
    if r.words("an opponent gains control of a permanent from you") {
        return Some(did(
            Action::ControlTakenByOpponent,
            Ref::You,
            Selector::default(),
        ));
    }
    let what = subject(r)?;
    if r.words("explores a land card") {
        return Some(did(Action::ExploredLand, Ref::EachPlayer, what));
    }
    if r.words("explores a nonland card") {
        return Some(did(Action::ExploredNonland, Ref::EachPlayer, what));
    }
    if r.words("exploits a non-Human creature") {
        return Some(did(Action::ExploitedNonHuman, Ref::EachPlayer, what));
    }
    r.need("exploits a nontoken creature")?;
    Some(did(Action::ExploitedNontoken, Ref::EachPlayer, what))
}

/// A `Did` pattern.
fn did(action: Action, who: Ref, what: Selector) -> TriggerPattern {
    TriggerPattern::Did { action, who, what }
}

/// The words of [`r375_event`] after "When"/"Whenever", with it.
pub(super) fn r375_did_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::Did { action, who, what } = pattern else {
        return None;
    };
    let words = match (action, who) {
        (Action::PaidLife, Ref::You) if what.is_empty() => "you pay life".to_owned(),
        (Action::ControlTakenByOpponent, Ref::You) if what.is_empty() => {
            "an opponent gains control of a permanent from you".to_owned()
        }
        (Action::ExploitedNontoken, Ref::EachPlayer) if !what.is_empty() => {
            format!("{} exploits a nontoken creature", subject_text(what)?)
        }
        (Action::ExploitedNonHuman, Ref::EachPlayer) if !what.is_empty() => {
            format!("{} exploits a non-Human creature", subject_text(what)?)
        }
        (Action::ExploredLand, Ref::EachPlayer) if !what.is_empty() => {
            format!("{} explores a land card", subject_text(what)?)
        }
        (Action::ExploredNonland, Ref::EachPlayer) if !what.is_empty() => {
            format!("{} explores a nonland card", subject_text(what)?)
        }
        _ => return None,
    };
    Some(format!("Whenever {words}"))
}

/// The JSON of the steps with every `from` reference written as `to`.
fn swapped<T: serde::Serialize + serde::de::DeserializeOwned>(
    value: &T,
    from: &str,
    to: &str,
) -> Option<T> {
    let text = serde_json::to_string(value).ok()?;
    serde_json::from_str(&text.replace(from, to)).ok()
}

/// The Vehicle selector.
fn vehicles() -> Selector {
    Selector(vec![Pred::Subtype(Subtype("Vehicle".to_owned()))])
}

/// A Vehicle becomes crewed.
fn crewed_vehicle() -> TriggerPattern {
    TriggerPattern::BecomesCrewed { what: vehicles() }
}

/// The card itself crewed the Vehicle that triggered (CR 702.122).
fn crewed_by_this() -> Condition {
    Condition::Matches {
        subject: Ref::This,
        selector: Selector(vec![Pred::Crewed(Ref::Triggering)]),
    }
}

/// The target spec of the twin: one Vehicle.
fn vehicle() -> TargetSpec {
    TargetSpec {
        what: Targetable::Object(vehicles()),
        bounds: crate::refs::Bounds::default(),
        divide: None,
    }
}

/// "Whenever ~ crews a Vehicle, that Vehicle gets +1/+1 until end of turn.": read from the twin "When ~ enters, target
/// Vehicle gets +1/+1 until end of turn." whose target becomes the Vehicle crewed.
pub(super) fn r375_crew_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let rest = text
        .strip_prefix(CREW_HEAD)
        .or_else(|| text.strip_prefix(CREW_HEAD_WORDED))?;
    (rest.matches(THAT_VEHICLE).count() >= 1 && !rest.contains(TARGET_VEHICLE)).then_some(())?;
    let twin = read(&format!(
        "{TWIN_HEAD}{}",
        rest.replace(THAT_VEHICLE, TARGET_VEHICLE)
    ))?;
    let Ability::Triggered {
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = &twin
    else {
        return None;
    };
    (targets.as_slice() == [vehicle()]).then_some(())?;
    Some(Ability::Triggered {
        when: crewed_vehicle(),
        intervening_if: Some(crewed_by_this()),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: swapped(effects, r#"{"Target":0}"#, r#""Triggering""#)?,
    })
}

/// The wordings of a crew trigger of the card itself whose steps name the Vehicle: the twin's steps with "that Vehicle".
pub(super) fn r375_crew_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Ability::Triggered {
        when,
        intervening_if: Some(condition),
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return;
    };
    if *when != crewed_vehicle() || *condition != crewed_by_this() || !targets.is_empty() {
        return;
    }
    let text = serde_json::to_string(effects).unwrap_or_default();
    if !text.contains(r#""Triggering""#) || text.contains(r#"{"Target":"#) {
        return;
    }
    let Some(effects) = swapped(effects, r#""Triggering""#, r#"{"Target":0}"#) else {
        return;
    };
    let twin = Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: vec![vehicle()],
        effects,
    };
    for twin_text in written(&twin) {
        let Some(rest) = twin_text.strip_prefix(TWIN_HEAD) else {
            continue;
        };
        if rest.matches(TARGET_VEHICLE).count() != 1 {
            continue;
        }
        let text = format!("{CREW_HEAD}{}", rest.replace(TARGET_VEHICLE, THAT_VEHICLE));
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
