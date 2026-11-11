//! The wordings `tc34_control` reads: a player controlled during their next turn, a text change of the one target and
//! a spell taken, and a restart of the game.

use super::Writer;
use crate::compile::read::tc34_control::{FROM, TO, kept_out, spell_or_permanent};
use crate::effect::{Effect, NewTargets, ValueKind};
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Ref, Targetable};

impl Writer<'_> {
    /// "gain control of target spell. You may choose new targets for it": the spell taken for good and its targets
    /// chosen anew, one sentence group of two effects.
    pub(super) fn tc34_take_spell(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::Continuous {
                affected,
                mods,
                duration: Duration::Forever,
            },
            Effect::ChangeTargets {
                what,
                to: NewTargets::Choose,
                onto: None,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        let [Modification::SetController(Ref::You)] = mods.as_slice() else {
            return None;
        };
        (*affected == Ref::Target(0) && what == affected && self.single(affected)).then_some(())?;
        self.target_is(|target| {
            matches!(target, Targetable::Object(selector) if selector.0.contains(&Pred::Spell))
        })
        .then_some(())?;
        let target = self.name_target()?;
        Some((
            format!("gain control of {target}. You may choose new targets for it"),
            2,
        ))
    }

    /// "you control target player during that player's next turn", or the gain of control with the extra turn after.
    pub(super) fn tc34_control_player(&mut self, effect: &Effect) -> Option<String> {
        let Effect::ControlPlayer { who, extra_turn } = effect else {
            return None;
        };
        (*who == Ref::Target(0)
            && self.single(who)
            && self.target_is(|what| matches!(what, Targetable::Player | Targetable::Opponent)))
        .then_some(())?;
        let player = self.name_target()?;
        Some(if *extra_turn {
            format!(
                "you gain control of {player} during that player's next turn. After that turn, that player takes an extra turn"
            )
        } else {
            format!("you control {player} during that player's next turn")
        })
    }

    /// "change the text of target permanent by replacing all instances of one color word with another": the two words
    /// chosen and the change of the target's text, one sentence of three effects.
    pub(super) fn tc34_text_change(&mut self, effects: &[Effect]) -> Option<(String, usize)> {
        let [
            Effect::ChooseValue {
                who: Ref::You,
                kind: from,
                bind: from_name,
            },
            Effect::ChooseValue {
                who: Ref::You,
                kind: to,
                bind: to_name,
            },
            Effect::Continuous {
                affected,
                mods,
                duration,
            },
            ..,
        ] = effects
        else {
            return None;
        };
        (from_name == FROM && to_name == TO).then_some(())?;
        let [
            Modification::ChangeTextChosen {
                from: read,
                to: written,
            },
        ] = mods.as_slice()
        else {
            return None;
        };
        (read == FROM && written == TO).then_some(())?;
        let either = self.target_is(|what| *what == spell_or_permanent());
        (*affected == Ref::Target(0)
            && self.single(affected)
            && (either || self.target_is(|what| matches!(what, Targetable::Object(_)))))
        .then_some(())?;
        let words = match from {
            ValueKind::Color => "one color word with another",
            ValueKind::BasicLandType => "one basic land type with another",
            ValueKind::CreatureType => "one creature type with another",
            ValueKind::ColorOrBasicLandType => {
                "one color word with another or one basic land type with another"
            }
            _ => return None,
        };
        let until = match duration {
            Duration::Forever => "",
            Duration::UntilEndOfTurn => " until end of turn",
            _ => return None,
        };
        let tail = match to {
            ValueKind::LikeChosen(name) if name == FROM => String::new(),
            ValueKind::CreatureTypeExcept(left_out) if *from == ValueKind::CreatureType => {
                let [left_out] = left_out.as_slice() else {
                    return None;
                };
                format!(". The new creature type can't be {left_out}")
            }
            _ => return None,
        };
        let target = if either && !self.named {
            self.named = true;
            "target spell or permanent".to_owned()
        } else {
            self.name_target()?
        };
        Some((
            format!(
                "change the text of {target} by replacing all instances of {words}{until}{tail}"
            ),
            3,
        ))
    }

    /// "restart the game, leaving in exile all non-Aura permanent cards exiled with ~. Then put those cards onto the
    /// battlefield under your control".
    pub(super) fn tc34_restart(effect: &Effect) -> Option<String> {
        let Effect::RestartGame { keep } = effect else {
            return None;
        };
        (*keep == kept_out()).then(|| {
            "restart the game, leaving in exile all non-Aura permanent cards exiled with ~. Then put those cards onto the battlefield under your control".to_owned()
        })
    }
}
