//! Phase-out durations and counts: "... phases out until ~ leaves the battlefield", "Tap that creature as it phases in
//! this way", "untap all creatures, then those creatures phase out until ~ leaves the battlefield" and "for each
//! creature that phased out this way"; the writer's side of each is beside its reader.

use super::selectors::{Form, all, phrase, phrase_text};
use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;
use mtg_core::card::CardType;

/// The words of the duration, as the writer gives them.
const W11B_UNTIL: &str = "until ~ leaves the battlefield";

/// What follows the verb of a phase-out: " until ~ leaves the battlefield" and, for one permanent, ". Tap that creature
/// as it phases in this way": the duration and whether it phases in tapped.
pub(super) fn w11b_phase_tail(r: &mut Reader<'_>, one: Option<&str>) -> (Option<Duration>, bool) {
    let Some(duration) = r.attempt(|r| {
        r.need("until")?;
        r.source().then_some(())?;
        r.need("leaves the battlefield")?;
        Some(Duration::WhileSourceOnBattlefield)
    }) else {
        return (None, false);
    };
    let tapped = one.is_some_and(|noun| {
        r.attempt(|r| {
            r.punct('.').then_some(())?;
            r.need(&format!("tap that {noun} as it phases in this way"))
        })
        .is_some()
    });
    (Some(duration), tapped)
}

/// The writer of `w11b_phase_tail`: the words after the verb, or nothing for a phase-out until the next untap step.
pub(super) fn w11b_phase_tail_text(
    duration: Option<Duration>,
    tapped: bool,
    one: Option<&str>,
) -> Option<String> {
    match (duration, tapped) {
        (None, false) => Some(String::new()),
        (Some(Duration::WhileSourceOnBattlefield), false) => Some(format!(" {W11B_UNTIL}")),
        (Some(Duration::WhileSourceOnBattlefield), true) => Some(format!(
            " {W11B_UNTIL}. Tap that {} as it phases in this way",
            one?
        )),
        _ => None,
    }
}

/// The noun "that ..." names a single target by: "creature" or "permanent".
pub(super) fn w11b_noun(selector: &Selector) -> Option<&'static str> {
    if selector.0.contains(&Pred::Type(CardType::Creature)) {
        Some("creature")
    } else {
        selector.0.is_empty().then_some("permanent")
    }
}

/// "Untap all creatures, then those creatures phase out until ~ leaves the battlefield." (Out of Time): the group
/// untaps, then the same group phases out for as long as the source stays.
pub fn w11b_untap_then_phase_out(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("untap")?;
    let what = all(r)?;
    let Ref::Each(selector) = &what else {
        return None;
    };
    r.punct(',').then_some(())?;
    r.need("then those")?;
    let again = phrase(r, Form::All)?;
    (again == *selector).then_some(())?;
    r.need("phase out")?;
    let (duration, _) = w11b_phase_tail(r, None);
    duration.is_some().then_some(())?;
    Some(vec![
        Effect::Untap { what: what.clone() },
        Effect::PhaseOut {
            what,
            duration,
            tapped: false,
        },
    ])
}

/// The writer of `w11b_untap_then_phase_out`: the sentence and the two effects it takes.
pub fn w11b_untap_then_phase_out_text(effects: &[Effect]) -> Option<(String, usize)> {
    let [
        Effect::Untap {
            what: Ref::Each(untapped),
        },
        Effect::PhaseOut {
            what: Ref::Each(phased),
            duration: duration @ Some(_),
            tapped: false,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    (untapped == phased).then_some(())?;
    let group = phrase_text(untapped, Form::All)?;
    let tail = w11b_phase_tail_text(duration.clone(), false, None)?;
    Some((
        format!("untap all {group}, then those {group} phase out{tail}"),
        2,
    ))
}

/// "creature that phased out this way" after "for each": the permanents of a kind the resolving effect phased out.
pub(super) fn w11b_phased_each(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        let selector = phrase(r, Form::Count)?;
        r.need("that phased out this way")?;
        Some(Amount::PhasedOut(selector))
    })
}

/// The writer of `w11b_phased_each`; `None` for any other amount.
pub(super) fn w11b_phased_each_text(amount: &Amount) -> Option<String> {
    let Amount::PhasedOut(selector) = amount else {
        return None;
    };
    Some(format!(
        "{} that phased out this way",
        phrase_text(selector, Form::Count)?
    ))
}
