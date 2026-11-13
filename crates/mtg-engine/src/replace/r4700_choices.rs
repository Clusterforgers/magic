//! Several choices made as a permanent enters (CR 614.12): "choose two colors" (`EntersChoice::Colors`), different
//! colours asked one after another, and "choose a color and a creature type" (`EntersChoice::ColorAndCreatureType`), a
//! colour then a creature type; the permanent remembers them together.

use mtg_cards::trigger::EntersChoice;
use mtg_core::card::Subtype;
use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};

use super::w5d_instead::Ran;
use super::{Applied, Asked, Candidate, Origin, Pass, Unit, slots_of};
use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::object::Chosen;

impl Pass<'_> {
    /// What `controller` chooses for a choice of several values as the card enters; nothing for any other kind.
    pub(super) fn r4700_chooses(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        kind: EntersChoice,
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        match kind {
            EntersChoice::Colors(count) => {
                let mut chosen: Vec<Color> = Vec::new();
                for _ in 0..count {
                    let left: Vec<Color> = Color::ALL
                        .iter()
                        .copied()
                        .filter(|color| !chosen.contains(color))
                        .collect();
                    let options = left.iter().copied().map(ChoiceOption::Color).collect();
                    let text = if chosen.is_empty() {
                        "Choose a color.".to_owned()
                    } else {
                        "Choose another color.".to_owned()
                    };
                    match self.r4700_pick(source, controller, options, text)? {
                        Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
                        Asked::Got(Some(at)) => chosen.extend(left.get(at).copied()),
                        Asked::Got(None) => break,
                    }
                }
                Ok(Asked::Got(Some(Chosen::Colors(chosen))))
            }
            EntersChoice::ColorAndCreatureType => {
                let colors = Color::ALL
                    .iter()
                    .copied()
                    .map(ChoiceOption::Color)
                    .collect();
                let color = match self.r4700_pick(
                    source,
                    controller,
                    colors,
                    "Choose a color.".to_owned(),
                )? {
                    Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
                    Asked::Got(at) => at.and_then(|at| Color::ALL.get(at).copied()),
                };
                let types = crate::choose::values::creature_types();
                let picked = match self.r4700_pick(
                    source,
                    controller,
                    types.clone(),
                    "Choose a creature type.".to_owned(),
                )? {
                    Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
                    Asked::Got(at) => at.and_then(|at| types.get(at).cloned()),
                };
                let subtype = match picked {
                    Some(ChoiceOption::Label(name)) => Some(Subtype(name)),
                    _ => None,
                };
                Ok(Asked::Got(color.zip(subtype).map(|(color, subtype)| {
                    Chosen::ColorAndType(color, subtype)
                })))
            }
            EntersChoice::Players(count) => {
                let mut chosen: Vec<PlayerId> = Vec::new();
                for _ in 0..count {
                    let left: Vec<PlayerId> = self
                        .state
                        .turn_order_from(controller)
                        .into_iter()
                        .filter(|player| !chosen.contains(player))
                        .collect();
                    let options = left.iter().copied().map(ChoiceOption::Player).collect();
                    let text = if chosen.is_empty() {
                        "Choose a player.".to_owned()
                    } else {
                        "Choose another player.".to_owned()
                    };
                    match self.r4700_pick(source, controller, options, text)? {
                        Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
                        Asked::Got(Some(at)) => chosen.extend(left.get(at).copied()),
                        Asked::Got(None) => break,
                    }
                }
                Ok(Asked::Got(Some(Chosen::Players(chosen))))
            }
            _ => Ok(Asked::Got(None)),
        }
    }

    /// Runs `effects` for the permanent entering at `at`, then asks its controller `choice` and has the permanent
    /// remember it ("As ~ enters, look at an opponent's hand, then choose a card type other than creature"); the
    /// effects' events come first in the batch.
    pub(super) fn r4700_then_choose(
        &mut self,
        at: usize,
        candidate: &Candidate,
        (effects, choice): (&[mtg_cards::effect::Effect], EntersChoice),
        (object, controller): (ObjectId, PlayerId),
        unit: &Unit,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let events = match self.w5d_run(at, candidate, effects, unit) {
            Ran::Events(events) => events,
            Ran::Ask(decision) => return Ok(Applied::Ask(decision)),
            Ran::Kept => Vec::new(),
        };
        let chosen = match self.chooses(candidate.object.id, controller, choice)? {
            Asked::Ask(decision) => return Ok(Applied::Ask(decision)),
            Asked::Got(chosen) => chosen,
        };
        let events = self.w9c_fresh_tokens(events);
        let mut entering = self.slots[at].clone();
        entering.applied = applied;
        let mut slots = slots_of(events);
        slots.push(entering);
        let entered = at + slots.len() - 1;
        self.slots.splice(at..=at, slots);
        self.w2b_remember(entered, object, controller, chosen);
        Ok(Applied::Done)
    }

    /// The index of the option `controller` picks: asked unless there is one option or none.
    fn r4700_pick(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        options: Vec<ChoiceOption>,
        text: String,
    ) -> Result<Asked<Option<usize>>, EngineError> {
        Ok(Asked::Got(match options.len() {
            0 => None,
            1 => Some(0),
            len => match self.answer() {
                None => {
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            options,
                            text,
                        }),
                    }));
                }
                Some((who, Choice::Option(index))) if *who == controller && *index < len => {
                    Some(*index)
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of {controller:?}"
                    )));
                }
            },
        }))
    }
}
