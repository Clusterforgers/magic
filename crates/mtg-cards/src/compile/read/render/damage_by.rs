//! Damage dealt by an object other than the ability's own source: "it deals 3 damage to any target" for the creature an
//! event is about, "equipped creature deals" for the creature the Equipment is attached to.

use super::{Writer, points_text};
use crate::effect::Effect;
use crate::refs::Ref;

impl Writer<'_> {
    /// "it deals 2 damage to any target" (the creature of the event), "~ deals 2 damage to you" (the card named) and
    /// "equipped creature deals that much damage" (the creature attached to).
    pub(super) fn damage_by(&mut self, effect: &Effect) -> Option<String> {
        let Effect::DealDamage {
            source: Some(source),
            to,
            amount,
            ..
        } = effect
        else {
            return None;
        };
        let subject = match source {
            Ref::This => "~",
            Ref::Triggering => "it",
            Ref::Attached => "equipped creature",
            _ => return None,
        };
        Some(format!(
            "{subject} deals {} damage to {}",
            points_text(amount)?,
            self.recipient(to)?
        ))
    }
}
