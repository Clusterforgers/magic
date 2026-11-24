//! A creature that enters tapped and attacking (`ReplaceBy::R122TappedAndAttacking`, CR 508.4): "If a creature you
//! control would enter while you control an attacking creature, instead it enters tapped and attacking." Its controller
//! chooses what it attacks as it enters, among what they could attack; it is never declared as an attacker.

use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Item, Origin, Pass, Slot};
use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::combat::{AttackTarget, defenders_for};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;

impl Pass<'_> {
    /// What `controller` may have the entering creature attack: nothing unless they are the active player and
    /// attackers have been declared.
    fn r122_defenders(&self, controller: PlayerId) -> Vec<AttackTarget> {
        if !self.state.is_active(controller) || !self.state.combat().attackers_declared() {
            return Vec::new();
        }
        defenders_for(self.state, controller)
    }

    /// The creature `object` entering under `controller` enters tapped; with something to attack it then joins the
    /// attack as what its controller chooses (asked only between two or more).
    pub(super) fn r122_tapped_and_attacking(
        &mut self,
        at: usize,
        (object, controller): (ObjectId, PlayerId),
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let targets = self.r122_defenders(controller);
        let target = match targets.as_slice() {
            [] => None,
            [one] => Some(*one),
            _ => match self.answer() {
                None => {
                    let options = targets
                        .iter()
                        .map(|target| match target {
                            AttackTarget::Player(seat) => ChoiceOption::Player(*seat),
                            AttackTarget::Planeswalker(walker) => ChoiceOption::Label(
                                self.state
                                    .object(*walker)
                                    .map_or_else(String::new, |o| o.card().name.clone()),
                            ),
                        })
                        .collect();
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source: object,
                            options,
                            text: "Choose a player or planeswalker for it to attack.".to_owned(),
                        }),
                    }));
                }
                Some((who, Choice::Option(index)))
                    if *who == controller && *index < targets.len() =>
                {
                    targets.get(*index).copied()
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to what {object:?} attacks as it enters"
                    )));
                }
            },
        };
        self.slots[at].applied = applied;
        let entered = self.entered_at(at, object, controller);
        if let Item::One(Event::Entered { tapped, .. }) = &mut self.slots[entered].item {
            *tapped = true;
        }
        if let Some(target) = target {
            self.slots.insert(
                entered + 1,
                Slot {
                    item: Item::One(Event::JoinedAttack { object, target }),
                    applied: Vec::new(),
                    frozen: true,
                },
            );
        }
        Ok(Applied::Done)
    }
}
