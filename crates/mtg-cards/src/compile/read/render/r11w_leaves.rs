//! A delayed trigger for the rest of the turn on the target leaving: "When that creature leaves the battlefield this
//! turn, sacrifice this creature." (Kjeldoran Elite Guard, Heart Wolf).

use super::{Effect, Pred, Ref, Selector, Targetable, Writer};
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

impl Writer<'_> {
    /// "when that creature leaves the battlefield this turn, sacrifice ~": the one target creature leaving, and the
    /// card of the ability sacrificed by its controller.
    pub(super) fn r11w_leaves_this_turn(&mut self, effect: &Effect) -> Option<String> {
        let Effect::DelayedTrigger {
            when:
                TriggerPattern::ZoneChange {
                    what: Selector(what),
                    from: Some(ZoneKind::Battlefield),
                    to: None,
                },
            this_turn: true,
            effects,
            ..
        } = effect
        else {
            return None;
        };
        let target = Pred::Is(Ref::Target(0));
        let creature = Pred::Type(CardType::Creature);
        // "the targeted creature" names a creature target; the creature type in the trigger adds nothing to it.
        let leaving = what.as_slice() == [target.clone()]
            || what.as_slice() == [creature.clone(), target.clone()];
        let creature_target = self.single(&Ref::Target(0))
            && self.target_is(|kind| {
                matches!(kind, Targetable::Object(Selector(preds)) if preds.contains(&creature))
            });
        (leaving && creature_target).then_some(())?;
        let [
            Effect::Sacrifice {
                who: Ref::You,
                filter,
                count,
                ..
            },
        ] = effects.as_slice()
        else {
            return None;
        };
        (filter.0 == [Pred::This] && count.is_one()).then_some(())?;
        Some("when that creature leaves the battlefield this turn, sacrifice ~".to_owned())
    }
}
