//! Damage dealt by an object other than the ability's own source: "it deals 3 damage to any target" for the creature an
//! event is about.

use super::{Writer, points_text};
use crate::effect::Effect;
use crate::refs::Ref;

impl Writer<'_> {
    /// "it deals 2 damage to any target" (the creature of the event) and "~ deals 2 damage to you" (the card named).
    pub(super) fn damage_by(&mut self, effect: &Effect) -> Option<String> {
        let Effect::DealDamage {
            source: Some(source),
            to,
            amount,
        } = effect
        else {
            return None;
        };
        let subject = match source {
            Ref::This => "~",
            Ref::Triggering => "it",
            _ => return None,
        };
        Some(format!(
            "{subject} deals {} damage to {}",
            points_text(amount)?,
            self.recipient(to)?
        ))
    }
}
