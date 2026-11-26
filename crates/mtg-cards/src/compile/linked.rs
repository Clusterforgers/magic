//! The two linked triggers of Oblivion Ring, Journey to Nowhere and Fiend Hunter: "When ~ enters, exile target ... When ~
//! leaves the battlefield, return the exiled card to the battlefield under its owner's control." stay two abilities
//! (CR 607.2a). The second returns what the source exiled while it was on the battlefield, so a source that leaves before
//! its enter trigger resolves exiles the card for good (CR 610.3c and the rulings); "until ~ leaves" is another card.

use super::Pending;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// What the trigger that returns the exiled card says after it names the card.
const RETURN_TAIL: &str =
    " leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

/// Whether the clause is "When ~ leaves the battlefield, return the exiled card ...", the card named by its name or as
/// "this enchantment".
fn is_return_trigger(text: &str) -> bool {
    text.strip_prefix("When ")
        .and_then(|rest| rest.strip_suffix(RETURN_TAIL))
        .is_some_and(|subject| subject == "~" || subject.starts_with("this "))
}

/// Whether the ability triggers when the card itself enters the battlefield.
fn enters_itself(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::ZoneChange { what, from: None, to: Some(ZoneKind::Battlefield) },
            intervening_if: None,
            ..
        } if what.0 == [Pred::This]
    )
}

/// Whether the effects exile for good, by themselves or inside a `May`: the exile the return trigger is linked to.
fn exiles_for_good(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::Exile {
            bind: None,
            duration: None,
            ..
        } => true,
        Effect::May { effects, .. } => exiles_for_good(effects),
        _ => false,
    })
}

/// "When ~ leaves the battlefield, return the exiled card to the battlefield under its owner's control.": the cards the
/// source exiled while it was on the battlefield return (`Pred::ExiledWith`).
fn return_trigger() -> Ability {
    Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::This]),
            from: Some(ZoneKind::Battlefield),
            to: None,
        },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Move {
            what: Ref::Each(Selector(vec![
                Pred::In(ZoneKind::Exile),
                Pred::ExiledWith(Ref::This),
            ])),
            to: Destination::Battlefield {
                tapped: false,
                under: None,
                transformed: false,
                counters: Vec::new(),
            },
            bind: None,
        }],
    }
}

/// Reads the trigger that returns the exiled card as the second of two linked abilities, when an enter trigger of the same
/// card before it exiles for good.
pub(super) fn exile_pair(pending: &mut [Pending]) {
    let Some(second) = pending.iter().position(|clause| {
        clause.ability.is_none() && !clause.read && is_return_trigger(&clause.text)
    }) else {
        return;
    };
    let linked = pending[..second]
        .iter()
        .filter_map(|clause| clause.ability.as_ref())
        .filter(|ability| enters_itself(ability))
        .any(|ability| exiles_for_good(ability.effects()));
    if linked {
        pending[second].ability = Some(return_trigger());
        pending[second].read = true;
    }
}
