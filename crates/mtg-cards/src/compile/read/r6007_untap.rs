//! A rule that holds every creature down through a target player's next untap step, whoever controls it: "Creatures
//! don't untap during target player's next untap step." (Blinding Beam, CR 502.3).

use mtg_core::card::CardType;

use super::tokens::Reader;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};

/// The sentence, lowercase.
pub(super) const R6007_HELD: &str = "creatures don't untap during target player's next untap step";

/// The rule the sentence makes, over the first target.
pub(super) fn r6007_held_rule() -> Effect {
    Effect::Rule {
        rule: GameRule::R6007NoUntapDuring {
            what: Selector(vec![Pred::Type(CardType::Creature)]),
            whose: Ref::Target(0),
        },
        duration: Duration::NextUntapStep,
    }
}

/// "creatures don't untap during target player's next untap step": the rule over the target player's next untap step.
pub(super) fn r6007_creatures_held(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.targets.is_empty().then_some(())?;
    r.need(R6007_HELD)?;
    r.targets.push(TargetSpec {
        what: Targetable::Player,
        bounds: Bounds::default(),
        divide: None,
    });
    Some(vec![r6007_held_rule()])
}

/// The sentence of the rule `r6007_creatures_held` reads, or nothing for any other effect.
pub(super) fn r6007_held_text(effects: &[Effect]) -> Option<(String, usize)> {
    (effects.first()? == &r6007_held_rule()).then(|| (R6007_HELD.to_owned(), 1))
}
