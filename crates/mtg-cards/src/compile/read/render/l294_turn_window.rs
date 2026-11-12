//! A restriction that applies only during the next turn of the controller of what it names: "that creature can't
//! attack during its controller's next turn" (`Duration::DuringItsControllersNextTurn`).

use super::Writer;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Ref, Targetable};

impl Writer<'_> {
    /// "that creature can't attack during its controller's next turn", "target creature can't attack during its
    /// controller's next turn": one creature, the attacker of the event or the one target.
    pub(super) fn l294_turn_window(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Continuous {
            affected,
            mods,
            duration: Duration::DuringItsControllersNextTurn,
        } = effect
        else {
            return None;
        };
        if mods.as_slice() != [Modification::CantAttack] {
            return None;
        }
        let subject = match affected {
            Ref::Triggering => "that creature".to_owned(),
            Ref::Target(0)
                if self.target_is(|what| matches!(what, Targetable::Object(_)))
                    && self.at_most_one(affected) =>
            {
                if self.named && self.single(affected) {
                    "it".to_owned()
                } else {
                    self.name_target()?
                }
            }
            _ => return None,
        };
        Some(format!(
            "{subject} can't attack during its controller's next turn"
        ))
    }
}
