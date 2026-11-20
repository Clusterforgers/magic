//! Card names chosen together with another value as a permanent enters (CR 614.12, 201.3): "choose an opponent and a card
//! name other than a basic land card name" (`EntersChoice::R111OpponentAndName`, kept as `Chosen::R111PlayerAndName`) and
//! "you and an opponent each choose a card name other than a basic land card name" (`EntersChoice::R111NamesWithOpponent`:
//! the controller chooses the opponent, then each names in turn; kept as `Chosen::R111Names`).

use mtg_cards::trigger::EntersChoice;
use mtg_core::card::Supertype;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{Asked, Pass};
use crate::choose::{Choice, ChoiceOption, NameOptions, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::object::{Chosen, GameObject};

/// The words of the question for a name other than a basic land card name.
const R111_NAME_TEXT: &str = "Choose a card name other than a basic land card name.";

impl Pass<'_> {
    /// What `controller` and the players they name choose for one of the two kinds; nothing for any other kind.
    pub(super) fn r111_chooses(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        kind: EntersChoice,
    ) -> Result<Asked<Option<Chosen>>, EngineError> {
        let opponent = match self.r111_opponent(source, controller)? {
            Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
            Asked::Got(opponent) => opponent,
        };
        let name = match self.r111_name(source, controller)? {
            Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
            Asked::Got(name) => name,
        };
        match kind {
            EntersChoice::R111OpponentAndName => {
                Ok(Asked::Got(opponent.zip(name).map(|(player, name)| {
                    Chosen::R111PlayerAndName(player, name)
                })))
            }
            EntersChoice::R111NamesWithOpponent => {
                let theirs = match opponent {
                    Some(opponent) => match self.r111_name(source, opponent)? {
                        Asked::Ask(decision) => return Ok(Asked::Ask(decision)),
                        Asked::Got(name) => name,
                    },
                    None => None,
                };
                let names: Vec<String> = name.into_iter().chain(theirs).collect();
                Ok(Asked::Got(
                    (!names.is_empty()).then_some(Chosen::R111Names(names)),
                ))
            }
            _ => Ok(Asked::Got(None)),
        }
    }

    /// The opponent `controller` chooses: asked unless there is one or none.
    fn r111_opponent(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
    ) -> Result<Asked<Option<PlayerId>>, EngineError> {
        let opponents: Vec<PlayerId> = self
            .state
            .turn_order_from(controller)
            .into_iter()
            .filter(|player| self.state.is_opponent(controller, *player))
            .collect();
        Ok(Asked::Got(match opponents.as_slice() {
            [] => None,
            [one] => Some(*one),
            _ => match self.answer() {
                None => {
                    return Ok(Asked::Ask(Decision {
                        player: controller,
                        kind: DecisionKind::ChooseOption(OptionOptions {
                            can_cancel: false,
                            source,
                            options: opponents
                                .iter()
                                .copied()
                                .map(ChoiceOption::Player)
                                .collect(),
                            text: "Choose an opponent.".to_owned(),
                        }),
                    }));
                }
                Some((who, Choice::Option(index))) if *who == controller => {
                    let index = *index;
                    Some(*opponents.get(index).ok_or_else(|| {
                        Self::invalid(format!("{index} is no opponent of {controller:?}"))
                    })?)
                }
                Some(other) => {
                    return Err(Self::invalid(format!(
                        "{other:?} is no answer to the choice of an opponent by {controller:?}"
                    )));
                }
            },
        }))
    }

    /// The card name `player` names, other than a basic land card name; nothing when they name nothing.
    fn r111_name(
        &mut self,
        source: ObjectId,
        player: PlayerId,
    ) -> Result<Asked<Option<String>>, EngineError> {
        let name = match self.answer() {
            None => {
                return Ok(Asked::Ask(Decision {
                    player,
                    kind: DecisionKind::ChooseName(NameOptions {
                        source,
                        text: R111_NAME_TEXT.to_owned(),
                        nonland: false,
                        noncreature: false,
                        nonbasic_land: false,
                        r111_not_basic_land: true,
                        looked: Vec::new(),
                    }),
                }));
            }
            Some((who, Choice::Name(name))) if *who == player => name.clone(),
            Some(other) => {
                return Err(Self::invalid(format!(
                    "{other:?} is no answer to the choice of a name of {player:?}"
                )));
            }
        };
        if name.is_empty() {
            return Ok(Asked::Got(None));
        }
        let basic = self.state.objects().map(GameObject::card).any(|card| {
            card.name == name
                && card.types.contains(&mtg_core::card::CardType::Land)
                && card.supertypes.contains(&Supertype::Basic)
        });
        if basic {
            return Err(Self::invalid(format!(
                "{name} is a basic land card name, which {player:?} may not choose here"
            )));
        }
        Ok(Asked::Got(Some(name)))
    }
}
