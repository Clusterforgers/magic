//! Power-up abilities: "Power-up — {5}{R}{R}: Put two +1/+1 counters on ~." is an activated ability with
//! `Limit::PowerUp` (activated once for its object, its cost reduced by the object's mana cost while it entered this
//! turn); the reminder text says so and is no part of the rules. The writer words the ability without the limit and puts
//! the keyword before it.

use crate::ability::{Ability, Limit, Timing};
use crate::modify::GameRule;
use crate::refs::ZoneKind;

/// The keyword and its dash that open a power-up line.
const R5515_POWER_UP: &str = "Power-up \u{2014} ";

/// The cost and effect of a power-up line, without the keyword.
pub(super) fn r5515_power_up_body(text: &str) -> Option<&str> {
    text.strip_prefix(R5515_POWER_UP)
}

/// The activated ability read from the body of a power-up line, as a power-up ability: one with no other limit,
/// condition or timing.
pub(super) fn r5515_power_up(read: Ability) -> Option<Ability> {
    match read {
        Ability::Activated {
            cost,
            timing: Timing::Instant,
            condition: None,
            limit: None,
            by,
            from,
            targets,
            effects,
        } => Some(Ability::Activated {
            cost,
            timing: Timing::Instant,
            condition: None,
            limit: Some(Limit::PowerUp),
            by,
            from,
            targets,
            effects,
        }),
        _ => None,
    }
}

/// The wordings of a power-up ability: each wording of the ability without its limit, after the keyword.
pub(crate) fn r5515_power_up_texts(
    ability: &Ability,
    words: impl Fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let Ability::Activated {
        limit: Some(Limit::PowerUp),
        timing: Timing::Instant,
        condition: None,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let mut plain = ability.clone();
    if let Ability::Activated { limit, .. } = &mut plain {
        *limit = None;
    }
    words(&plain)
        .into_iter()
        .map(|text| format!("{R5515_POWER_UP}{text}"))
        .collect()
}

/// "Each power-up ability of permanents you control can be activated an additional time.": `GameRule::PowerUpAgain`
/// over the permanents the phrase names.
pub(super) fn r5515_power_up_again(text: &str) -> Option<Ability> {
    let body = text
        .strip_prefix("Each power-up ability of ")?
        .strip_suffix(" can be activated an additional time.")?;
    let tokens = super::tokens::tokenize(body);
    let mut reader = super::tokens::Reader::new(&tokens, false);
    let which = super::selectors::phrase(&mut reader, super::selectors::Form::All)?;
    reader.end().then_some(Ability::Rule {
        rule: GameRule::PowerUpAgain(which),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The words of a `GameRule::PowerUpAgain` ability [`r5515_power_up_again`] reads.
pub(crate) fn r5515_power_up_again_texts(ability: &Ability) -> Vec<String> {
    let Ability::Rule {
        rule: GameRule::PowerUpAgain(which),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    super::selectors::phrase_text(which, super::selectors::Form::All)
        .map(|what| {
            vec![format!(
                "Each power-up ability of {what} can be activated an additional time."
            )]
        })
        .unwrap_or_default()
}
