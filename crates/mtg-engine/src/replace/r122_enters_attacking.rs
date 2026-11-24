//! A creature that enters tapped and attacking (`ReplaceBy::R122TappedAndAttacking`, CR 508.4): "If a creature you
//! control would enter while you control an attacking creature, instead it enters tapped and attacking." Its controller
//! chooses the player or planeswalker it attacks as it enters, asked unless there is one; it was never declared as an
//! attacker, so nothing that triggers on attacking sees it.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Item, Pass, Slot};
use crate::choose::{Choice, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;

impl Pass<'_> {
    /// The creature `object` entering under `controller` joins the attack right after it enters, against what its
    /// controller chooses; with no attack declared or nothing to attack it only enters.
    pub(super) fn r122_attacking(
        &mut self,
        at: usize,
        source: ObjectId,
        object: ObjectId,
        controller: PlayerId,
    ) -> Result<Asked<()>, EngineError> {
        let targets = crate::resolve::r122_attackable(self.state, controller);
        let picked = match targets.as_slice() {
            [] => return Ok(Asked::Got(())),
            [_] => 0,
            _ => match self.answer() {
                None => {
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            options: targets.iter().map(|(_, option)| option.clone()).collect(),
                            text: "Choose a player or planeswalker for it to attack.".into(),
                        }),
                    }));
                }
                Some((who, Choice::Option(index)))
                    if *who == controller && *index < targets.len() =>
                {
                    *index
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to what {controller:?} attacks with {object:?}"
                    )));
                }
            },
        };
        let Some((target, _)) = targets.get(picked) else {
            return Ok(Asked::Got(()));
        };
        let entered = self.entered_at(at, object, controller);
        self.slots.insert(
            entered + 1,
            Slot {
                item: Item::One(Event::JoinedAttack {
                    object,
                    target: *target,
                }),
                applied: Vec::new(),
                frozen: false,
            },
        );
        Ok(Asked::Got(()))
    }
}
