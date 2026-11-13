//! A card name chosen as a permanent enters (CR 201.3): any name, a nonland, a noncreature nonland or a nonbasic land
//! name, or the name of a nonland card revealed from the opponents' hands ("As ~ enters, each opponent reveals their
//! hand. You choose the name of a nonland card revealed this way.").

use mtg_cards::definition::CardDefinition;
use mtg_cards::trigger::EntersChoice;
use mtg_core::card::{CardType, Supertype};
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Applied, Asked, Item, Origin, Pass, Slot};
use crate::choose::{Choice, ChoiceOption, NameOptions, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::Event;
use crate::library::Revealed;
use crate::object::{Chosen, GameObject};
use crate::zones::Zone;

/// Whether a card with this definition may be named for the choice.
pub(super) fn r4702_fits(card: &CardDefinition, kind: EntersChoice) -> bool {
    let land = card.types.contains(&CardType::Land);
    match kind {
        EntersChoice::NonlandCardName | EntersChoice::RevealedNonlandName => !land,
        EntersChoice::NoncreatureNonlandCardName => {
            !land && !card.types.contains(&CardType::Creature)
        }
        EntersChoice::NonbasicLandCardName => land && !card.supertypes.contains(&Supertype::Basic),
        _ => true,
    }
}

impl Pass<'_> {
    /// The card name `controller` names as the card enters; nothing when they name nothing. A name a card of the game
    /// bears that the choice excludes is refused (a creature for "noncreature, nonland").
    pub(super) fn r4702_chooses_name(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        kind: EntersChoice,
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        let (nonland, noncreature, nonbasic_land, text) = match kind {
            EntersChoice::NonlandCardName => (true, false, false, "Choose a nonland card name."),
            EntersChoice::NoncreatureNonlandCardName => (
                true,
                true,
                false,
                "Choose a noncreature, nonland card name.",
            ),
            EntersChoice::NonbasicLandCardName => {
                (false, false, true, "Choose a nonbasic land card name.")
            }
            _ => (false, false, false, "Choose a card name."),
        };
        let name = match self.answer() {
            None => {
                return Ok(Asked::Ask(Decision {
                    player: controller,
                    kind: DecisionKind::ChooseName(NameOptions {
                        source,
                        text: text.to_owned(),
                        nonland,
                        noncreature,
                        nonbasic_land,
                    }),
                }));
            }
            Some((who, Choice::Name(name))) if *who == controller => name.clone(),
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the choice of a name of {controller:?}"
                )));
            }
        };
        if name.is_empty() {
            return Ok(Asked::Got(None));
        }
        let excluded = self
            .state
            .objects()
            .map(GameObject::card)
            .any(|card| card.name == name && !r4702_fits(card, kind));
        if excluded {
            return Err(Self::invalid(format!(
                "{name} is no name {controller:?} may choose here: {text}"
            )));
        }
        Ok(Asked::Got(Some(Chosen::Name(name))))
    }

    /// Each opponent of `controller` reveals their hand and `controller` chooses the name of a nonland card revealed
    /// this way (asked unless one name or none was revealed); the reveals come before the card enters.
    pub(super) fn r4702_revealed_name(
        &mut self,
        at: usize,
        source: ObjectId,
        object: ObjectId,
        controller: PlayerId,
        applied: Vec<Origin>,
    ) -> Result<Applied, EngineError> {
        let hands: Vec<(PlayerId, Vec<ObjectId>)> = self
            .state
            .turn_order_from(controller)
            .into_iter()
            .filter(|player| self.state.is_opponent(controller, *player))
            .map(|player| (player, self.state.zone(Zone::Hand(player)).to_vec()))
            .filter(|(_, cards)| !cards.is_empty())
            .collect();
        let mut names: Vec<String> = hands
            .iter()
            .flat_map(|(_, cards)| cards.iter())
            .filter_map(|card| self.state.object(*card))
            .map(GameObject::card)
            .filter(|card| r4702_fits(card, EntersChoice::RevealedNonlandName))
            .map(|card| card.name.clone())
            .collect();
        names.sort();
        names.dedup();
        let picked = match names.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            _ => match self.answer() {
                None => {
                    return Ok(Applied::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            options: names.iter().cloned().map(ChoiceOption::Label).collect(),
                            text: "Choose the name of a nonland card revealed this way.".to_owned(),
                        }),
                    }));
                }
                Some((who, Choice::Option(index))) if *who == controller => {
                    let index = *index;
                    Some(names.get(index).cloned().ok_or_else(|| {
                        Self::invalid(format!("{index} is no name revealed to {controller:?}"))
                    })?)
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of a revealed name of {controller:?}"
                    )));
                }
            },
        };
        self.w2b_remember(at, object, controller, picked.map(Chosen::Name));
        let reveals: Vec<Slot> = hands
            .into_iter()
            .map(|(player, cards)| Slot {
                item: Item::One(Event::Revealed(Revealed { player, cards })),
                applied: Vec::new(),
                frozen: true,
            })
            .collect();
        let count = reveals.len();
        self.slots.splice(at..at, reveals);
        // The enter event moved on by the reveals.
        if let Some(slot) = self.slots.get_mut(at + count) {
            slot.applied = applied;
        }
        Ok(Applied::Done)
    }
}
