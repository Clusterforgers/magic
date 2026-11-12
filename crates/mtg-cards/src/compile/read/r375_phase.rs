//! r37-5 phasing holds and their writers: a phase-out followed by the sentence that its permanents can't phase in for a
//! while ("Until the end of your next turn, they can't phase in.", "It can't phase in for as long as you control this
//! Saga.") is `Effect::PhaseOut` with `held`; "Permanents can't phase in." is a `Cant` rule of `PhaseIn` for every
//! player.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::Ref;

/// The static ability that keeps every permanent from phasing in.
const NONE_PHASE_IN: &str = "Permanents can't phase in.";

/// "Permanents can't phase in.": a rule over every player's permanents.
pub(super) fn r375_no_phase_in_read(text: &str) -> Option<Ability> {
    super::tokens::same_text(text, NONE_PHASE_IN).then(no_phase_in)
}

/// The rule [`r375_no_phase_in_read`] reads.
fn no_phase_in() -> Ability {
    Ability::Rule {
        rule: GameRule::Cant {
            who: Ref::EachPlayer,
            action: Prohibited::PhaseIn,
        },
        condition: None,
        from: crate::refs::ZoneKind::Battlefield,
    }
}

/// The words of the rule that no permanent phases in.
pub(super) fn r375_no_phase_in_wordings(ability: &Ability, texts: &mut Vec<String>) {
    if *ability == no_phase_in() && !texts.iter().any(|text| text == NONE_PHASE_IN) {
        texts.push(NONE_PHASE_IN.to_owned());
    }
}

/// The sentences that hold the permanents just phased out, and the duration each stands for.
const HELD: [(&str, Duration); 6] = [
    (
        " Until the end of your next turn, they can't phase in.",
        Duration::UntilEndOfYourNextTurn,
    ),
    (
        " Until the end of your next turn, it can't phase in.",
        Duration::UntilEndOfYourNextTurn,
    ),
    (
        " It can't phase in for as long as you control this Saga.",
        Duration::WhileYouControlSource,
    ),
    (
        " It can't phase in for as long as you control ~.",
        Duration::WhileYouControlSource,
    ),
    (
        " They can't phase in for as long as you control this Saga.",
        Duration::WhileYouControlSource,
    ),
    (
        " They can't phase in for as long as you control ~.",
        Duration::WhileYouControlSource,
    ),
];

/// The ability with the hold set on its last step, a phase-out with none; `None` when its last step is no such phase-out.
fn holding(ability: &Ability, held: Option<Duration>) -> Option<Ability> {
    let mut value = serde_json::to_value(ability).ok()?;
    let effects = value
        .as_object_mut()?
        .values_mut()
        .next()?
        .as_object_mut()?
        .get_mut("effects")?
        .as_array_mut()?;
    let last = effects.last_mut()?;
    let mut step: Effect = serde_json::from_value(last.clone()).ok()?;
    let Effect::PhaseOut { held: own, .. } = &mut step else {
        return None;
    };
    (own.is_some() != held.is_some()).then_some(())?;
    *own = held;
    *last = serde_json::to_value(step).ok()?;
    serde_json::from_value(value).ok()
}

/// "S. Until the end of your next turn, they can't phase in.": read from S, whose last step is the phase-out.
pub(super) fn r375_held_read(
    text: &str,
    read: impl Fn(&str) -> Option<Ability>,
) -> Option<Ability> {
    let (twin, held) = HELD.iter().find_map(|(sentence, held)| {
        text.strip_suffix(sentence)
            .map(|twin| (twin.to_owned(), held.clone()))
    })?;
    holding(&read(&twin)?, Some(held))
}

/// The wordings of an ability whose last step is a held phase-out: the ability without the hold, then the sentence.
pub(super) fn r375_held_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let Some(Effect::PhaseOut {
        what,
        held: Some(held),
        ..
    }) = ability.effects().last()
    else {
        return;
    };
    // "they" for a group, "it" for one target.
    let plural = matches!(what, Ref::Each(_));
    let Some(twin) = holding(ability, None) else {
        return;
    };
    let mut twins = written(&twin);
    // "Each creature target opponent controls phases out." beside "All creatures target opponent controls phase out.":
    // the nouns of the two forms differ only before the words they share, which the target makes the writer change.
    if let Ref::Each(selector) = what
        && let (Some(all), Some(each)) = (
            super::selectors::phrase_text(selector, super::selectors::Form::All),
            super::selectors::phrase_text(selector, super::selectors::Form::Each),
        )
    {
        let shared = all
            .chars()
            .rev()
            .zip(each.chars().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let (all_head, each_head) = (
            all.chars()
                .take(all.chars().count() - shared)
                .collect::<String>(),
            each.chars()
                .take(each.chars().count() - shared)
                .collect::<String>(),
        );
        let more: Vec<String> = twins
            .iter()
            .filter_map(|text| {
                let from = format!("All {all_head}");
                let at = text.find(&from)?;
                let rest = &text[at + from.len()..];
                let verb = rest.find(" phase out")?;
                Some(format!(
                    "{}Each {each_head}{} phases out{}",
                    &text[..at],
                    &rest[..verb],
                    &rest[verb + " phase out".len()..]
                ))
            })
            .collect();
        for text in more {
            if !twins.contains(&text) {
                twins.push(text);
            }
        }
    }
    for text in twins {
        for (sentence, own) in &HELD {
            let said = format!("{text}{sentence}");
            let says_they = sentence.contains(" they ") || sentence.contains(" They ");
            if own == held && says_they == plural && !texts.contains(&said) {
                texts.push(said);
            }
        }
    }
}

/// Every r37-5 wording of the ability: crew triggers, held phase-outs and the rule that nothing phases in.
pub(super) fn r375_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    super::r375_events::r375_crew_wordings(ability, written, texts);
    r375_held_wordings(ability, written, texts);
    r375_no_phase_in_wordings(ability, texts);
}
