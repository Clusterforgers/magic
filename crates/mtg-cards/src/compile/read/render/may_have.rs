//! A choice: "you may have other attacking creatures get +X/+X until end of turn", "you may draw a card and lose 1 life".

use super::Writer;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::Ref;

impl Writer<'_> {
    /// "you may have target creature get +2/+2 until end of turn": the pump the player may choose to give.
    pub(super) fn may_have(&mut self, effects: &[Effect]) -> Option<String> {
        let [
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::UntilEndOfTurn,
            },
        ] = effects
        else {
            return None;
        };
        let pump = self.pump(affected, mods, Duration::UntilEndOfTurn)?;
        Some(format!(
            "you may have {}",
            pump.replacen(" gets ", " get ", 1)
                .replacen(" gains ", " gain ", 1)
                .replacen(" becomes ", " become ", 1)
        ))
    }

    /// "you may have that player lose 1 life": the controller of the creature that entered loses life by the player's choice.
    pub(super) fn may_have_lose(&mut self, effects: &[Effect]) -> Option<String> {
        let [Effect::LoseLife { who, amount }] = effects else {
            return None;
        };
        (*who == Ref::ControllerOf(Box::new(Ref::Triggering))).then_some(())?;
        Some(format!(
            "you may have that player lose {}",
            self.life(amount)?
        ))
    }

    /// "you may gain 3 life", "you may draw a card and lose 1 life": the steps of one choice, each with "you" understood.
    pub(super) fn may_do(&mut self, effects: &[Effect]) -> Option<String> {
        let mut steps = Vec::with_capacity(effects.len());
        for effect in effects {
            let text = self.effect(effect)?;
            steps.push(
                text.strip_prefix("you ")
                    .map_or(text.clone(), str::to_owned),
            );
        }
        (!steps.is_empty()).then(|| format!("you may {}", super::join_list(&steps)))
    }
}
