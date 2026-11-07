//! e10c: the rules of the game, replacements and cost modifiers a permanent is given in quotes ("Enchanted land has
//! \"Untap this land during each other player's untap step.\""), two quoted abilities given together, and the untap rule
//! of a permanent during the other players' untap steps.

use super::render;
use super::selectors::{Form, granted_text, phrase_text};
use crate::ability::Ability;
use crate::modify::{GameRule, Modification};
use crate::refs::{Pred, Ref, Selector};

/// Whether the ability is of a kind that is given in quotes and written as its own sentence.
fn quotable(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Activated { .. }
            | Ability::Triggered { .. }
            | Ability::Rule { .. }
            | Ability::Replacement { .. }
            | Ability::CostModifier { .. }
    )
}

/// Whether the ability states a rule rather than an action: the kinds a card names first when it gives two.
fn stated(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Rule { .. } | Ability::Replacement { .. } | Ability::CostModifier { .. }
    )
}

/// One given ability in quotes.
fn quoted(ability: &Ability) -> Option<String> {
    if let Some(text) = super::dk1a_attached::granted_static_text(ability) {
        return Some(text);
    }
    quotable(ability).then(|| render::ability(ability).map(|text| format!("\"{text}\"")))?
}

/// The abilities a static gives when they are a rule, a replacement or a cost modifier, or two quoted abilities: "\"You
/// have no maximum hand size\" and \"Whenever this creature attacks, ...\"" (the rule first, as cards word it). `None`
/// leaves the modifications to the keyword writer.
pub(super) fn e10c_given_text(mods: &[Modification]) -> Option<String> {
    let given = match mods {
        [Modification::GrantAbility(ability)] if stated(ability) => quoted(ability),
        [
            Modification::GrantAbility(first),
            Modification::GrantAbility(second),
        ] => {
            let (first, second) = if stated(second) && !stated(first) {
                (second, first)
            } else {
                (first, second)
            };
            quoted(first)
                .zip(quoted(second))
                .map(|(first, second)| format!("{first} and {second}"))
        }
        _ => None,
    };
    given.or_else(|| granted_text(mods))
}

/// "Untap ~ during each other player's untap step", "Untap all permanents you control during each other player's untap
/// step": the untap rule for the other players' untap steps. Any other player is not written.
pub(super) fn e10c_untap_during_texts(rule: &GameRule) -> Vec<String> {
    let GameRule::UntapDuring {
        what,
        whose: Ref::Opponents,
    } = rule
    else {
        return Vec::new();
    };
    let subject = if *what == Selector::this() {
        "~".to_owned()
    } else if what.0 == [Pred::ControlledBy(Ref::You)] {
        "all permanents you control".to_owned()
    } else if what.0.last() == Some(&Pred::ControlledBy(Ref::You)) {
        match phrase_text(what, Form::All) {
            Some(phrase) => format!("all {phrase}"),
            None => return Vec::new(),
        }
    } else {
        return Vec::new();
    };
    vec![format!(
        "Untap {subject} during each other player's untap step"
    )]
}
