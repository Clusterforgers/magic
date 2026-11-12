//! A permanent that can't phase out for a while: "until your next upkeep, target permanent can't phase out" and "until
//! your next upkeep, ~ can't phase out".

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, GameRule, Prohibited};
use crate::refs::{Ref, Targetable};

impl Writer<'_> {
    /// "until your next upkeep, target permanent can't phase out", of the target or of the source itself.
    pub(super) fn r301_cant_phase_out(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule:
                GameRule::Cant {
                    who,
                    action: Prohibited::PhaseOut,
                },
            duration: Duration::UntilYourNextUpkeep,
        } = effect
        else {
            return None;
        };
        let subject = match who {
            Ref::This => "~".to_owned(),
            Ref::Target(0)
                if self.single(who)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                self.name_target()?
            }
            _ => return None,
        };
        Some(format!("until your next upkeep, {subject} can't phase out"))
    }
}
