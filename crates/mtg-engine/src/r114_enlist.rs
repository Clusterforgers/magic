//! Enlist as the attack is declared (CR 702.154a): "As this creature attacks, you may tap a nonattacking creature you
//! control without summoning sickness. When you do, add its power to this creature's until end of turn." The choice is
//! part of the declaration (`AttackerOption::r114_enlist`, `Answer::DeclareExerting`): the creature is tapped in the
//! declaration's batch and the attacker has enlisted it (`Event::Enlisted`), which the "when you do" trigger and "it
//! enlisted a creature this combat" (`Pred::R114Enlisted`) read; the combat keeps who enlisted whom until it ends.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::combat::AttackTarget;
use crate::decision::AttackOptions;
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The keyword name of enlist (`Keyword::Macro`).
const ENLIST: &str = "enlist";

/// Whether the creature has enlist.
fn has_enlist(state: &GameState, object: ObjectId) -> bool {
    crate::keywords::keywords_of(state, object)
        .iter()
        .any(|keyword| matches!(keyword, Keyword::Macro { name, .. } if name == ENLIST))
}

/// The creatures `attacker` may enlist as `player` declares it: untapped creatures they control other than it without
/// summoning sickness (haste excuses it, CR 702.154a); none when it has no enlist. Which of them attack is checked with
/// the declaration.
pub(crate) fn r114_may_enlist(
    state: &GameState,
    player: PlayerId,
    attacker: ObjectId,
) -> Vec<ObjectId> {
    if !has_enlist(state, attacker) {
        return Vec::new();
    }
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| *id != attacker)
        .filter(|id| {
            state.object(*id).is_some_and(|object| {
                crate::characteristics::controller_of_object(state, object) == player
                    && crate::characteristics::characteristics(state, *id)
                        .is_some_and(|chars| chars.is_creature())
                    && object
                        .permanent()
                        .is_some_and(|permanent| !permanent.tapped())
            }) && !crate::keywords::summoning_sick(state, *id)
        })
        .collect()
}

/// Checks the enlisting of a declaration: each attacker enlists once a creature it was offered, which does not attack,
/// and no creature is enlisted twice.
pub(crate) fn r114_check_enlist(
    options: &AttackOptions,
    attackers: &[(ObjectId, AttackTarget)],
    enlist: &[(ObjectId, ObjectId)],
) -> Result<(), IllegalAnswer> {
    for (i, (attacker, enlisted)) in enlist.iter().enumerate() {
        if enlist[..i]
            .iter()
            .any(|(other, tapped)| other == attacker || tapped == enlisted)
        {
            return Err(IllegalAnswer::Duplicate("enlisted creature"));
        }
        let declared = attackers.iter().any(|(object, _)| object == attacker);
        let attacking = attackers.iter().any(|(object, _)| object == enlisted);
        let offered = options
            .attackers
            .iter()
            .any(|option| option.object == *attacker && option.r114_enlist.contains(enlisted));
        if !declared || attacking || !offered {
            return Err(IllegalAnswer::NotOffered("enlist"));
        }
    }
    Ok(())
}

/// The events of the enlisting of a declaration: each creature enlisted becomes tapped, and its attacker has enlisted it.
pub(crate) fn r114_enlist_events(player: PlayerId, enlist: &[(ObjectId, ObjectId)]) -> Vec<Event> {
    enlist
        .iter()
        .flat_map(|(attacker, enlisted)| {
            [
                Event::PermanentTapped { object: *enlisted },
                Event::Enlisted {
                    object: *attacker,
                    enlisted: *enlisted,
                    player,
                },
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a declaration enlists only a creature it was offered for an attacker it declares, never an attacker, never
    //! one creature twice and never twice for one attacker.
    use super::*;
    use crate::decision::AttackerOption;
    use rstest::rstest;

    fn id(n: u64) -> ObjectId {
        ObjectId(n)
    }

    fn options() -> AttackOptions {
        let defender = AttackTarget::Player(PlayerId(1));
        AttackOptions {
            attackers: vec![
                AttackerOption {
                    r114_enlist: vec![id(3), id(4)],
                    ..AttackerOption::free(id(1), vec![defender])
                },
                AttackerOption {
                    r114_enlist: vec![id(3), id(4)],
                    ..AttackerOption::free(id(2), vec![defender])
                },
                AttackerOption::free(id(4), vec![defender]),
            ],
            required: Vec::new(),
            mana: 0,
            most: None,
            most_at: Vec::new(),
            cant_alone: Vec::new(),
            joins: Vec::new(),
            at_least_one: None,
            party: None,
        }
    }

    #[rstest]
    #[case::one_offered(&[1], &[(1, 3)], true)]
    #[case::two_attackers_two_creatures(&[1, 2], &[(1, 3), (2, 4)], true)]
    #[case::a_creature_twice(&[1, 2], &[(1, 3), (2, 3)], false)]
    #[case::one_attacker_twice(&[1], &[(1, 3), (1, 4)], false)]
    #[case::an_attacker_enlisted(&[1, 4], &[(1, 4)], false)]
    #[case::not_declared(&[2], &[(1, 3)], false)]
    #[case::not_offered(&[4], &[(4, 3)], false)]
    fn r114_a_declaration_enlists_what_it_was_offered(
        #[case] declared: &[u64],
        #[case] enlist: &[(u64, u64)],
        #[case] ok: bool,
    ) {
        let attackers: Vec<_> = declared
            .iter()
            .map(|n| (id(*n), AttackTarget::Player(PlayerId(1))))
            .collect();
        let enlist: Vec<_> = enlist.iter().map(|(a, b)| (id(*a), id(*b))).collect();
        assert_eq!(
            r114_check_enlist(&options(), &attackers, &enlist).is_ok(),
            ok
        );
    }
}
