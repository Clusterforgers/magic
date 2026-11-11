//! The Attraction wordings (CR 717): "open an Attraction", "open two Attractions", "roll to visit your Attractions",
//! "whenever you open an Attraction", "whenever you roll to visit your Attractions" and the visit ability
//! "Visit — [effect]" (CR 702.159a).

use super::amounts::{count_word, number_word};
use super::tokens::Reader;
use super::{Own, clause_with};
use crate::ability::Ability;
use crate::compile::ClauseKind;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Amount;

/// The head of a visit ability.
pub(crate) const VISIT: &str = "Visit \u{2014} ";

/// "an Attraction" or "two Attractions" after "open": how many.
fn attractions(r: &mut Reader<'_>) -> Option<u32> {
    let count = number_word(r.take_word()?)?;
    let noun = if count == 1 {
        "Attraction"
    } else {
        "Attractions"
    };
    (r.name() == Some(noun)).then_some(count)
}

/// The rest of "open an Attraction" (CR 701.51) after the verb.
pub(super) fn un33_open(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    let count = attractions(r)?;
    Some(Effect::OpenAttraction {
        who,
        count: Amount::Fixed(i32::try_from(count).ok()?),
    })
}

/// The rest of "roll to visit your Attractions" (CR 701.52) after the verb.
pub(super) fn un33_roll(r: &mut Reader<'_>, who: Ref) -> Option<Effect> {
    visit_roll(r, &who)?;
    Some(Effect::RollToVisit { who })
}

/// "to visit your Attractions" ("their Attractions" for another player).
fn visit_roll(r: &mut Reader<'_>, who: &Ref) -> Option<()> {
    r.need("to visit")?;
    r.need(if *who == Ref::You { "your" } else { "their" })?;
    (r.name() == Some("Attractions")).then_some(())
}

/// The two Attraction effects written back, for "you"; another player is not written.
pub(super) fn un33_effect_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::OpenAttraction {
            who: Ref::You,
            count: Amount::Fixed(1),
        } => Some("open an Attraction".to_owned()),
        Effect::OpenAttraction {
            who: Ref::You,
            count: Amount::Fixed(count),
        } => Some(format!(
            "open {} Attractions",
            count_word(u32::try_from(*count).ok()?)?
        )),
        Effect::RollToVisit { who: Ref::You } => Some("roll to visit your Attractions".to_owned()),
        _ => None,
    }
}

/// The event after the player and its verb: "you open an Attraction", "you roll to visit your Attractions".
pub(super) fn un33_event(r: &mut Reader<'_>, verb: &str, by: Ref) -> Option<TriggerPattern> {
    match verb {
        "open" | "opens" => {
            (attractions(r)? == 1).then_some(TriggerPattern::OpenedAttraction { who: by })
        }
        "roll" | "rolls" => {
            visit_roll(r, &by)?;
            Some(TriggerPattern::RolledToVisit { who: by })
        }
        _ => None,
    }
}

/// The patterns `un33_event` reads, written back for "you".
pub(super) fn un33_event_text(pattern: &TriggerPattern) -> Option<String> {
    match pattern {
        TriggerPattern::OpenedAttraction { who: Ref::You } => {
            Some("Whenever you open an Attraction".to_owned())
        }
        TriggerPattern::RolledToVisit { who: Ref::You } => {
            Some("Whenever you roll to visit your Attractions".to_owned())
        }
        _ => None,
    }
}

/// Whether the clause is a visit ability.
pub fn is_visit_line(text: &str) -> bool {
    text.starts_with(VISIT)
}

/// The visit ability of a "Visit — [effect]" line (and the bullets under it): a triggered ability whose effects are
/// the text after the dash read as a spell; `None` when the text is not read.
pub fn un33_visit_block(text: &str, x_free: bool, own: &Own) -> Option<Ability> {
    let body = text.strip_prefix(VISIT)?;
    let kind = if body.starts_with("Choose ") {
        ClauseKind::Modal
    } else {
        ClauseKind::Spell
    };
    let Ability::Spell { targets, effects } = clause_with(body, kind, x_free, own)? else {
        return None;
    };
    Some(Ability::Triggered {
        when: TriggerPattern::Visit {},
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    })
}
