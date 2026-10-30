//! A change of control: "Gain control of target Equipment.", "Gain control of target creature until end of turn.",
//! "target opponent gains control of ~", and the sweep "Untap all creatures and gain control of them until end of turn.
//! They gain haste until end of turn."

use super::Writer;
use crate::compile::read::selectors::{Form, keyword_text, phrase_text};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Ref, Targetable};

impl Writer<'_> {
    /// The sentence for the control of the target or of the card itself changing, for the turn or for good.
    pub(super) fn control(
        &mut self,
        affected: &Ref,
        mods: &[Modification],
        duration: Duration,
    ) -> Option<String> {
        let [Modification::SetController(new)] = mods else {
            return None;
        };
        let until = match duration {
            Duration::UntilEndOfTurn => " until end of turn",
            Duration::Forever => "",
            Duration::WhileYouControlSource => " for as long as you control ~",
            Duration::WhileSourceOnBattlefield => " for as long as ~ remains on the battlefield",
            _ => return None,
        };
        match (affected, new) {
            (Ref::Target(0), Ref::You)
                if self.single(affected)
                    && self.target_is(|what| matches!(what, Targetable::Object(_))) =>
            {
                Some(format!("gain control of {}{until}", self.name_target()?))
            }
            (Ref::This, Ref::Target(0))
                if duration == Duration::Forever
                    && self.single(new)
                    && self.target_is(|what| {
                        matches!(what, Targetable::Player | Targetable::Opponent)
                    }) =>
            {
                Some(format!("{} gains control of ~", self.name_target()?))
            }
            (_, Ref::You) => Some(format!(
                "gain control of {}{until}",
                self.w6e_taken(affected)?
            )),
            _ => None,
        }
    }

    /// A change of control for good, which is no pump.
    pub(super) fn control_for_good(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Continuous {
            affected,
            mods,
            duration:
                duration @ (Duration::Forever
                | Duration::WhileYouControlSource
                | Duration::WhileSourceOnBattlefield),
        } = effect
        else {
            return None;
        };
        self.control(affected, mods, duration.clone())
    }

    /// "untap all creatures and gain control of them until end of turn. They gain haste until end of turn": the untap
    /// of a group and the control of the same group, with at most one keyword.
    pub(super) fn untapped_and_controlled(effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Untap {
                what: Ref::Each(first),
            },
            Effect::Continuous {
                affected: Ref::Each(second),
                mods,
                duration: Duration::UntilEndOfTurn,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (first == second).then_some(())?;
        let [Modification::SetController(Ref::You), rest @ ..] = mods.as_slice() else {
            return None;
        };
        let group = phrase_text(first, Form::All)?;
        let mut text = format!("untap all {group} and gain control of them until end of turn");
        match rest {
            [] => {}
            [Modification::AddKeyword(keyword)] => {
                text = format!(
                    "{text}. They gain {} until end of turn",
                    keyword_text(keyword)?
                );
            }
            _ => return None,
        }
        Some((text, 2))
    }
}
