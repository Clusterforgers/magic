//! Blocks declared as piles (`Rule::R509BlockPiles`, Camouflage, CR 509.1): "instead of declaring blockers, each
//! defending player chooses any number of creatures they control and divides them into a number of piles equal to the
//! number of attacking creatures for whom that player is the defending player"; a creature that can block additional
//! creatures may go into as many more piles. The piles are assigned to the attackers at random, and each creature in a
//! pile that can block that pile's attacker does so, as far as the restrictions on blocks allow.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::answers::Outcome;
use crate::decision::{BlockOptions, Decision, DecisionKind};
use crate::error::{EngineError, IllegalAnswer};
use crate::events::Event;
use crate::rng::GameRng;
use crate::state::GameState;
use crate::zones::Zone;

/// A creature that may go into the piles, and into how many at most.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct R509PileCreature {
    /// The creature.
    pub object: ObjectId,
    /// The most piles it may go into: one, and one more for each additional creature it can block.
    pub piles: usize,
}

/// The question of a defending player who divides creatures into piles instead of declaring blockers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct R509PilesOptions {
    /// The attackers for whom the player is the defending player: one pile for each, in this order.
    pub attackers: Vec<ObjectId>,
    /// The creatures the player controls.
    pub creatures: Vec<R509PileCreature>,
    /// The question in words.
    pub text: String,
}

impl R509PilesOptions {
    /// The answer that blocks with nothing: every pile empty.
    pub fn empty_piles(&self) -> Vec<Vec<ObjectId>> {
        vec![Vec::new(); self.attackers.len()]
    }

    /// Piles of the creatures `chosen`, spread one after another over the piles, each as often as it may join.
    pub fn spread(&self, chosen: &[ObjectId]) -> Vec<Vec<ObjectId>> {
        let mut piles = self.empty_piles();
        if piles.is_empty() {
            return piles;
        }
        let mut at = 0;
        for creature in chosen {
            let most = self
                .creatures
                .iter()
                .find(|offered| offered.object == *creature)
                .map_or(0, |offered| offered.piles);
            for _ in 0..most.min(piles.len()) {
                piles[at % self.attackers.len()].push(*creature);
                at += 1;
            }
        }
        piles
    }
}

/// The question of `player`, who defends against `blocks.blockers`' attackers: every creature they control, a creature
/// that can block additional creatures offered for as many more piles.
pub(crate) fn r509_question(
    state: &GameState,
    player: PlayerId,
    decider: PlayerId,
    blocks: &BlockOptions,
) -> Decision {
    let attackers = crate::combat::r509_attackers_of(state, player);
    let creatures = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| crate::characteristics::controller_of(state, **id) == Some(player))
        .filter(|id| {
            crate::characteristics::characteristics(state, **id)
                .is_some_and(|chars| chars.types.contains(mtg_core::card::CardType::Creature))
        })
        .map(|object| R509PileCreature {
            object: *object,
            piles: blocks
                .blockers
                .iter()
                .find(|option| option.object == *object)
                .map_or(1, |option| option.extra + 1)
                .min(attackers.len().max(1)),
        })
        .collect();
    Decision {
        player: decider,
        kind: DecisionKind::R509BlockPiles(R509PilesOptions {
            text: format!(
                "Divide your creatures into {} piles. Each pile goes to an attacker at random, and each creature in it that can block that attacker does.",
                attackers.len()
            ),
            attackers,
            creatures,
        }),
    }
}

/// Checks the piles against the question: one pile for each attacker, each creature offered, in a pile at most once and
/// in no more piles than it may go into.
fn r509_check(options: &R509PilesOptions, piles: &[Vec<ObjectId>]) -> Result<(), IllegalAnswer> {
    if piles.len() != options.attackers.len() {
        return Err(IllegalAnswer::NotOffered("piles"));
    }
    for pile in piles {
        let mut seen = pile.clone();
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != pile.len() {
            return Err(IllegalAnswer::NotOffered("a creature twice in a pile"));
        }
    }
    for creature in piles.iter().flatten() {
        let most = options
            .creatures
            .iter()
            .find(|offered| offered.object == *creature)
            .ok_or(IllegalAnswer::NotOffered("pile creature"))?
            .piles;
        if piles.iter().filter(|pile| pile.contains(creature)).count() > most {
            return Err(IllegalAnswer::NotOffered("a creature in too many piles"));
        }
    }
    Ok(())
}

/// The blocks of the piles as assigned, kept to what the restrictions allow: each creature that can block its pile's
/// attacker with nothing to pay, no more on an attacker than it may have and none on one that needs more than it got
/// (menace), no more blockers than a limit allows, and none at all when what is left still breaks a restriction. The
/// requirements of CR 509.1c don't apply: nobody declares these blocks.
pub(crate) fn r509_blocks(
    state: &GameState,
    options: &BlockOptions,
    assigned: &[(ObjectId, Vec<ObjectId>)],
) -> Vec<(ObjectId, ObjectId)> {
    let mut blocks: Vec<(ObjectId, ObjectId)> = Vec::new();
    for (attacker, pile) in assigned {
        let mut on = 0;
        for creature in pile {
            let can = options
                .blockers
                .iter()
                .any(|option| option.object == *creature && option.can_block.contains(attacker));
            if can
                && on < options.limit_for(*attacker)
                && crate::block_tax::r509_costs_nothing(state, *creature, *attacker)
            {
                blocks.push((*creature, *attacker));
                on += 1;
            }
        }
    }
    loop {
        let before = blocks.len();
        if let Some(most) = options.most {
            while crate::w14c_block_capacity::w14c_distinct_blockers(&blocks).len() > most {
                blocks.pop();
            }
        }
        let short: Vec<ObjectId> = assigned
            .iter()
            .map(|(attacker, _)| *attacker)
            .filter(|attacker| {
                let got = blocks.iter().filter(|(_, a)| a == attacker).count();
                got > 0 && got < options.needed_for(*attacker)
            })
            .collect();
        blocks.retain(|(_, attacker)| !short.contains(attacker));
        if blocks.len() == before {
            break;
        }
    }
    let unrequired = BlockOptions {
        required: Vec::new(),
        must_be_blocked: Vec::new(),
        must_be_blocked_by: None,
        ..options.clone()
    };
    if crate::combat::check_blockers(&unrequired, &blocks).is_err() {
        blocks.clear();
    }
    blocks
}

/// The answer: the piles checked, assigned to the attackers at random with the next substream of the generator, and the
/// blocks they make declared for the defending player whose question it was.
pub(crate) fn r509_answer(
    state: &GameState,
    decider: PlayerId,
    options: &R509PilesOptions,
    piles: &[Vec<ObjectId>],
) -> Result<Outcome, EngineError> {
    r509_check(options, piles)?;
    let player = crate::steps::r6400_declaring(state, decider);
    let mut order: Vec<usize> = (0..piles.len()).collect();
    GameRng::substream(state.seed(), state.rng_index()).shuffle(&mut order);
    let assigned: Vec<(ObjectId, Vec<ObjectId>)> = options
        .attackers
        .iter()
        .zip(&order)
        .map(|(attacker, pile)| (*attacker, piles[*pile].clone()))
        .collect();
    let blocks = r509_blocks(
        state,
        &crate::combat::block_options(state, player),
        &assigned,
    );
    Ok(Outcome::Events(vec![
        Event::R509PilesAssigned {
            player,
            piles: assigned,
        },
        Event::BlockersDeclared {
            player,
            blocks: crate::keywords::banding::blocks_with_bands(state, &blocks),
        },
    ]))
}

#[cfg(test)]
mod tests {
    //! Goal: an answer has exactly one pile per attacker, names only creatures offered, never one twice in a pile or in
    //! more piles than it may join; the blocks keep each creature that can block its pile's attacker, drop a lone
    //! blocker of an attacker that needs two, and keep no more than an attacker or a limit allows.
    use rstest::rstest;

    use super::*;
    use crate::decision::BlockerOption;

    fn id(n: u64) -> ObjectId {
        ObjectId(n)
    }

    fn piles_options() -> R509PilesOptions {
        R509PilesOptions {
            attackers: vec![id(1), id(2)],
            creatures: vec![
                R509PileCreature {
                    object: id(10),
                    piles: 1,
                },
                R509PileCreature {
                    object: id(11),
                    piles: 2,
                },
            ],
            text: String::new(),
        }
    }

    #[rstest]
    #[case::empty_piles(vec![vec![], vec![]], true)]
    #[case::one_each(vec![vec![id(10)], vec![id(11)]], true)]
    #[case::extra_blocker_twice(vec![vec![id(10), id(11)], vec![id(11)]], true)]
    #[case::too_few_piles(vec![vec![id(10)]], false)]
    #[case::too_many_piles(vec![vec![], vec![], vec![]], false)]
    #[case::not_offered(vec![vec![id(12)], vec![]], false)]
    #[case::twice_in_a_pile(vec![vec![id(10), id(10)], vec![]], false)]
    #[case::too_many_piles_for_it(vec![vec![id(10)], vec![id(10)]], false)]
    fn r509_piles_are_checked(#[case] piles: Vec<Vec<ObjectId>>, #[case] ok: bool) {
        assert_eq!(
            r509_check(&piles_options(), &piles).is_ok(),
            ok,
            "{piles:?}"
        );
    }

    fn block_options(
        min: Vec<(ObjectId, usize)>,
        max: Vec<(ObjectId, usize)>,
        most: Option<usize>,
    ) -> BlockOptions {
        BlockOptions {
            blockers: [10, 11, 12]
                .into_iter()
                .map(|n| BlockerOption {
                    object: id(n),
                    can_block: if n == 12 {
                        vec![id(2)]
                    } else {
                        vec![id(1), id(2)]
                    },
                    extra: 0,
                })
                .collect(),
            min_blockers: min,
            max_blockers: max,
            required: Vec::new(),
            must_be_blocked: Vec::new(),
            must_be_blocked_by: None,
            most,
            cant_alone: Vec::new(),
            joins: Vec::new(),
        }
    }

    #[rstest]
    #[case::each_that_can(
        block_options(vec![], vec![], None),
        vec![(id(10), id(1)), (id(11), id(2)), (id(12), id(2))]
    )]
    #[case::menace_needs_two(
        block_options(vec![(id(1), 2)], vec![], None),
        vec![(id(11), id(2)), (id(12), id(2))]
    )]
    #[case::at_most_one_on_two(
        block_options(vec![], vec![(id(2), 1)], None),
        vec![(id(10), id(1)), (id(11), id(2))]
    )]
    #[case::one_blocker_in_all(block_options(vec![], vec![], Some(1)), vec![(id(10), id(1))])]
    fn r509_blocks_keep_what_the_restrictions_allow(
        #[case] options: BlockOptions,
        #[case] expected: Vec<(ObjectId, ObjectId)>,
    ) {
        let state = crate::fixtures::state(vec![vec![], vec![]], vec![vec![], vec![]]);
        // Creature 12 can block only attacker 2, so in attacker 1's pile it does nothing.
        let assigned = vec![(id(1), vec![id(10), id(12)]), (id(2), vec![id(11), id(12)])];
        assert_eq!(r509_blocks(&state, &options, &assigned), expected);
    }
}
