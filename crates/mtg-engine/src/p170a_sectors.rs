//! The sectors of space sculptor (Space Beleren and its release notes; the CR has no rule for it): while a permanent with
//! space sculptor is on the battlefield, every creature there is in the alpha, beta or gamma sector. A creature not in
//! one is assigned by its controller as a state-based action: first the players who control no permanent with space
//! sculptor, then those who do, each group in turn order from the active player. A sector is a designation of the
//! permanent: a copy doesn't copy it, a change of control keeps it, and every sector is lost once no permanent with space
//! sculptor is left. Only the abilities that read sectors give them meaning: `Rule::P170aSectorBlocks` and
//! `Pred::P170aInSector`.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::decision::{Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// A sector of space sculptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Sector {
    /// The alpha sector.
    Alpha,
    /// The beta sector.
    Beta,
    /// The gamma sector.
    Gamma,
}

impl Sector {
    /// The three sectors, in order.
    pub const ALL: [Sector; 3] = [Sector::Alpha, Sector::Beta, Sector::Gamma];

    /// The word for the sector, as a choice of it is remembered (`ValueKind::P170aSector`).
    pub const fn word(self) -> &'static str {
        match self {
            Sector::Alpha => "alpha",
            Sector::Beta => "beta",
            Sector::Gamma => "gamma",
        }
    }
}

/// The question of a player who assigns their creatures that are in no sector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct P170aSectorsOptions {
    /// The creatures to assign, each to exactly one sector.
    pub creatures: Vec<ObjectId>,
    /// The question in words.
    pub text: String,
}

impl P170aSectorsOptions {
    /// The answer that spreads the creatures over the sectors in turn: the first to alpha, the next to beta, and so on.
    pub fn spread(&self) -> Vec<(ObjectId, Sector)> {
        self.creatures
            .iter()
            .zip(Sector::ALL.iter().cycle())
            .map(|(creature, sector)| (*creature, *sector))
            .collect()
    }
}

/// The sector of the permanent `id`, while it is on the battlefield in one.
pub(crate) fn p170a_sector_of(state: &GameState, id: ObjectId) -> Option<Sector> {
    state
        .object(id)
        .and_then(|object| object.permanent())
        .and_then(crate::object::PermanentState::sector)
}

/// The creatures on the battlefield in no sector, by controller.
fn p170a_unassigned(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| crate::characteristics::controller_of(state, *id) == Some(player))
        .filter(|id| p170a_sector_of(state, *id).is_none())
        .filter(|id| {
            crate::characteristics::characteristics(state, *id)
                .is_some_and(|chars| chars.types.contains(mtg_core::card::CardType::Creature))
        })
        .collect()
}

/// The players in the order they assign: those who control no permanent with space sculptor in turn order from the
/// active player, then those who do. Empty while no permanent with space sculptor is on the battlefield.
fn p170a_assigners(state: &GameState) -> Vec<PlayerId> {
    let sculptors = crate::keywords::space_sculptor::p170a_sculptors(state);
    if sculptors.is_empty() {
        return Vec::new();
    }
    let order: Vec<PlayerId> = state
        .turn_order_from(state.turn().active())
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    let (with, without): (Vec<PlayerId>, Vec<PlayerId>) = order
        .into_iter()
        .partition(|player| sculptors.contains(player));
    without.into_iter().chain(with).collect()
}

/// The question the sectors ask now: the first player in assigning order with a creature in no sector assigns all of
/// theirs. `None` while every creature is in one or no permanent with space sculptor is on the battlefield.
pub(crate) fn p170a_question(state: &GameState) -> Option<Decision> {
    if !state.possible.sectors {
        return None;
    }
    p170a_assigners(state).into_iter().find_map(|player| {
        let creatures = p170a_unassigned(state, player);
        (!creatures.is_empty()).then(|| Decision {
            player,
            kind: DecisionKind::P170aAssignSectors(P170aSectorsOptions {
                text: if creatures.len() == 1 {
                    "Assign your creature to the alpha, beta or gamma sector.".to_owned()
                } else {
                    "Assign each of your creatures to the alpha, beta or gamma sector.".to_owned()
                },
                creatures,
            }),
        })
    })
}

/// The event that ends every sector once no permanent with space sculptor is on the battlefield; `None` while one is or
/// no permanent is in a sector.
pub(crate) fn p170a_lost(state: &GameState) -> Option<Event> {
    if !state.possible.sectors
        || !crate::keywords::space_sculptor::p170a_sculptors(state).is_empty()
    {
        return None;
    }
    state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| p170a_sector_of(state, *id).is_some())
        .then_some(Event::P170aSectorsLost)
}

/// Checks the assignments against the question: every creature offered once, and nothing else.
fn p170a_check(
    options: &P170aSectorsOptions,
    sectors: &[(ObjectId, Sector)],
) -> Result<(), IllegalAnswer> {
    for (creature, _) in sectors {
        if !options.creatures.contains(creature) {
            return Err(IllegalAnswer::NotOffered("sector creature"));
        }
        if sectors
            .iter()
            .filter(|(other, _)| other == creature)
            .count()
            > 1
        {
            return Err(IllegalAnswer::Duplicate("sector creature"));
        }
    }
    if sectors.len() != options.creatures.len() {
        return Err(IllegalAnswer::Required("sector of every creature"));
    }
    Ok(())
}

/// The answer: the assignments checked and made, in the order the question lists the creatures.
pub(crate) fn p170a_answer(
    player: PlayerId,
    options: &P170aSectorsOptions,
    sectors: &[(ObjectId, Sector)],
) -> Result<Outcome, EngineError> {
    p170a_check(options, sectors)?;
    let sectors = options
        .creatures
        .iter()
        .filter_map(|creature| sectors.iter().find(|(id, _)| id == creature).copied())
        .collect();
    Ok(Outcome::Events(vec![Event::P170aSectorsAssigned {
        player,
        sectors,
    }]))
}

/// Whether the sectors keep `blocker` from blocking `attacker` (`Rule::P170aSectorBlocks`): the attacker is in a sector
/// and the blocker is not in the same one.
pub(crate) fn p170a_blocks_apart(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    crate::rules::p170a_sector_blocks(state)
        && p170a_sector_of(state, attacker)
            .is_some_and(|sector| p170a_sector_of(state, blocker) != Some(sector))
}

/// Each creature named goes into its sector: it is a permanent in none.
pub(crate) fn p170a_fold_assigned(
    state: &mut GameState,
    sectors: &[(ObjectId, Sector)],
) -> Result<(), EngineError> {
    for (id, sector) in sectors {
        let perm = state
            .objects
            .get_mut(id)
            .and_then(|object| object.permanent.as_mut())
            .ok_or_else(|| crate::fold::invalid(format!("{id:?} is not a permanent")))?;
        if perm.designations.sector.is_some() {
            return Err(crate::fold::invalid(format!(
                "{id:?} is in a sector already"
            )));
        }
        perm.designations.sector = Some(*sector);
    }
    Ok(())
}

/// Every permanent leaves its sector.
pub(crate) fn p170a_fold_lost(state: &mut GameState) {
    for object in state.objects.values_mut() {
        if let Some(perm) = object.permanent.as_mut() {
            perm.designations.sector = None;
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an answer names every creature offered exactly once and nothing else, and is made in the order of the
    //! question; each sector has its own word.
    use rstest::rstest;

    use super::*;

    fn id(n: u64) -> ObjectId {
        ObjectId(n)
    }

    fn options() -> P170aSectorsOptions {
        P170aSectorsOptions {
            creatures: vec![id(1), id(2)],
            text: String::new(),
        }
    }

    #[rstest]
    #[case::each_once(vec![(id(1), Sector::Alpha), (id(2), Sector::Gamma)], true)]
    #[case::any_order(vec![(id(2), Sector::Beta), (id(1), Sector::Beta)], true)]
    #[case::one_left_out(vec![(id(1), Sector::Alpha)], false)]
    #[case::none(vec![], false)]
    #[case::twice(vec![(id(1), Sector::Alpha), (id(1), Sector::Beta)], false)]
    #[case::not_offered(vec![(id(1), Sector::Alpha), (id(2), Sector::Beta), (id(3), Sector::Beta)], false)]
    fn p170a_assignments_are_checked(#[case] sectors: Vec<(ObjectId, Sector)>, #[case] ok: bool) {
        assert_eq!(p170a_check(&options(), &sectors).is_ok(), ok, "{sectors:?}");
    }

    #[test]
    fn p170a_the_answer_follows_the_question() {
        let outcome = p170a_answer(
            PlayerId(0),
            &options(),
            &[(id(2), Sector::Beta), (id(1), Sector::Gamma)],
        );
        assert!(matches!(
            outcome,
            Ok(Outcome::Events(events)) if events == vec![Event::P170aSectorsAssigned {
                player: PlayerId(0),
                sectors: vec![(id(1), Sector::Gamma), (id(2), Sector::Beta)],
            }]
        ));
    }

    #[test]
    fn p170a_words_are_distinct() {
        let words: std::collections::BTreeSet<_> = Sector::ALL.iter().map(|s| s.word()).collect();
        assert_eq!(words.len(), 3);
    }
}
