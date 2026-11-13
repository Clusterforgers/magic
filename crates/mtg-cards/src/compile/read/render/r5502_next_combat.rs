//! "target creature attacks during its controller's next combat phase if able": a requirement to attack that lasts until
//! that creature's controller's next combat phase ends.

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule};
use crate::refs::{Ref, Targetable};

impl Writer<'_> {
    /// "target creature attacks during its controller's next combat phase if able", of the one target.
    pub(super) fn r5502_attacks_next_combat(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule: GameRule::MustAttack(who @ Ref::Target(0)),
            duration: Duration::UntilItsControllersNextCombatEnds,
        } = effect
        else {
            return None;
        };
        if !(self.single(who) && self.target_is(|what| matches!(what, Targetable::Object(_)))) {
            return None;
        }
        let subject = self.name_target()?;
        Some(format!(
            "{subject} attacks during its controller's next combat phase if able"
        ))
    }
}
