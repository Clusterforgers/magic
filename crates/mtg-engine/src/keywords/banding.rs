//! Banding and "bands with other" (CR 702.22): attackers declared in a band are blocked as a group (702.22h), and the
//! player whose banding creature fights a creature divides that creature's combat damage (702.22j, 702.22k).
//!
//! A band is named in the attack declaration (`Answer::DeclareExerting::bands`), checked here and folded from
//! `Event::BandFormed` into `CombatState::bands`; it lasts for the combat even if the keyword goes (702.22e). A block of one
//! member is declared as a block of every member (`blocks_with_bands`). Who divides is asked of the keywords at damage.

use mtg_cards::keyword::Keyword;
use mtg_cards::refs::Selector;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{KeywordRules, keywords_of};
use crate::characteristics::controller_of;
use crate::combat::{AttackTarget, Attacker};
use crate::error::IllegalAnswer;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::Scope;
use crate::state::GameState;

/// The rules of banding: it adds nothing; combat asks [`may_band`] and the damage step [`divides_for`].
#[derive(Debug)]
pub(super) struct Banding;

impl KeywordRules for Banding {}

/// The rules of "bands with other": as banding, among creatures of its quality.
#[derive(Debug)]
pub(super) struct BandsWithOther;

impl KeywordRules for BandsWithOther {}

/// Whether `id` has banding or "bands with other", so an attack declaration may put it in a band.
pub(crate) fn may_band(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Banding | Keyword::BandsWithOther(_)))
}

fn has_banding(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Banding))
}

/// The qualities of the "bands with other" abilities of `id`.
fn qualities(state: &GameState, id: ObjectId) -> Vec<Selector> {
    keywords_of(state, id)
        .into_iter()
        .filter_map(|keyword| match keyword {
            Keyword::BandsWithOther(quality) => Some(quality.clone()),
            _ => None,
        })
        .collect()
}

/// Whether `id` has the quality, judged for the player who controls `by`.
fn is_of(state: &GameState, quality: &Selector, by: ObjectId, id: ObjectId) -> bool {
    let (Some(player), Some(source), Some(object)) = (
        controller_of(state, by),
        state.object(by).map(GameObject::reference),
        state.object(id).map(GameObject::reference),
    ) else {
        return false;
    };
    Scope::announcing(state, player, source)
        .selects(quality, object)
        .unwrap_or(false)
}

/// Whether the creatures may form a band (CR 702.22c): some with banding and at most one without, or all of the quality of
/// a "bands with other" one among them.
pub(crate) fn may_form(state: &GameState, members: &[ObjectId]) -> bool {
    let banding = members.iter().filter(|id| has_banding(state, **id)).count();
    if banding >= 1 && members.len() - banding <= 1 {
        return true;
    }
    members.iter().any(|leader| {
        qualities(state, *leader).iter().any(|quality| {
            members
                .iter()
                .all(|member| is_of(state, quality, *leader, *member))
        })
    })
}

/// Checks the bands of an attack declaration: each of two or more declared attackers that may band together and attack
/// the same player, planeswalker or battle (CR 702.22c, 702.22d), and no creature in two bands.
pub(crate) fn check_bands(
    state: &GameState,
    attackers: &[(ObjectId, AttackTarget)],
    bands: &[Vec<ObjectId>],
) -> Result<(), IllegalAnswer> {
    let mut seen: Vec<ObjectId> = Vec::new();
    for band in bands {
        if band.len() < 2 {
            return Err(IllegalAnswer::NotOffered("band of one"));
        }
        let mut target = None;
        for member in band {
            if seen.contains(member) {
                return Err(IllegalAnswer::Duplicate("band member"));
            }
            seen.push(*member);
            let Some((_, at)) = attackers.iter().find(|(object, _)| object == member) else {
                return Err(IllegalAnswer::NotOffered("band member"));
            };
            if *target.get_or_insert(*at) != *at {
                return Err(IllegalAnswer::NotOffered("band split between defenders"));
            }
        }
        if !may_form(state, band) {
            return Err(IllegalAnswer::NotOffered("band"));
        }
    }
    Ok(())
}

/// The events that form the bands of a declaration, after its `AttackersDeclared`.
pub(crate) fn band_events(player: PlayerId, bands: &[Vec<ObjectId>]) -> Vec<Event> {
    bands
        .iter()
        .map(|band| Event::BandFormed {
            player,
            attackers: band.clone(),
        })
        .collect()
}

/// The blocks a declaration makes: each pair declared, and a block of an attacker in a band as a block of every other
/// member still attacking (CR 702.22h).
pub(crate) fn blocks_with_bands(
    state: &GameState,
    blocks: &[(ObjectId, ObjectId)],
) -> Vec<(ObjectId, ObjectId)> {
    let mut all = blocks.to_vec();
    for (blocker, attacker) in blocks {
        let Some(band) = state.combat().band_of(*attacker) else {
            continue;
        };
        for member in band {
            if !all.contains(&(*blocker, *member)) {
                all.push((*blocker, *member));
            }
        }
    }
    all
}

/// Whether one of the `fighters` makes their controller divide the combat damage of the creature they fight (CR 702.22j,
/// 702.22k): one has banding, or one has "bands with other" and another of them shares its quality.
fn fighters_band(state: &GameState, fighters: &[ObjectId]) -> bool {
    fighters.iter().any(|id| has_banding(state, *id))
        || fighters.iter().any(|leader| {
            qualities(state, *leader).iter().any(|quality| {
                fighters
                    .iter()
                    .any(|other| other != leader && is_of(state, quality, *leader, *other))
            })
        })
}

/// The player who divides the combat damage of the attacker: the defending player when a creature blocking it bands
/// (CR 702.22j), else nobody but its controller.
pub(crate) fn divides_attacker(state: &GameState, attacker: &Attacker) -> Option<PlayerId> {
    if !fighters_band(state, attacker.blockers()) {
        return None;
    }
    attacker
        .blockers()
        .first()
        .and_then(|blocker| controller_of(state, *blocker))
}

/// The player who divides the combat damage of a blocker among the attackers it blocks: the active player when one of
/// them bands (CR 702.22k).
pub(crate) fn divides_blocker(state: &GameState, blocked: &[ObjectId]) -> Option<PlayerId> {
    fighters_band(state, blocked).then(|| state.turn().active())
}

#[cfg(test)]
mod tests {
    //! Goal: bands form exactly from banding creatures with at most one other, or from creatures of a "bands with other" quality; a declaration's bands are refused when a member is missing, repeated, alone or aimed elsewhere; a block of one member blocks the band; and the damage of a creature fighting a banding creature is divided by the banding side.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_cards::ability::Ability;
    use mtg_cards::refs::Pred;
    use mtg_core::card::{CardType, Subtype};
    use rstest::rstest;

    use super::*;
    use crate::combat::{Attacker, CombatState};
    use crate::fixtures::state;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn creature(name: &str, subtype: &str, keyword: Option<Keyword>) -> Arc<CardDefinition> {
        let card = CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(2, 2);
        Arc::new(match keyword {
            Some(keyword) => card.with_rules(Ability::Keyword(keyword)),
            None => card,
        })
    }

    fn dinosaurs() -> Keyword {
        Keyword::BandsWithOther(Selector(vec![Pred::Subtype(Subtype(
            "Dinosaur".to_owned(),
        ))]))
    }

    /// Seat 0: two banding Knights (0, 1), two plain Bears (2, 3), a Dinosaur that bands with other Dinosaurs (4) and a
    /// plain Dinosaur (5). Seat 1: a banding Wall (6) and a plain Wall (7).
    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![
                    creature("Knight A", "Knight", Some(Keyword::Banding)),
                    creature("Knight B", "Knight", Some(Keyword::Banding)),
                    creature("Bear A", "Bear", None),
                    creature("Bear B", "Bear", None),
                    creature("Raptor", "Dinosaur", Some(dinosaurs())),
                    creature("Brontos", "Dinosaur", None),
                ],
                vec![
                    creature("Banding Wall", "Wall", Some(Keyword::Banding)),
                    creature("Plain Wall", "Wall", None),
                ],
            ],
        )
    }

    fn ids(game: &GameState, at: &[usize]) -> Vec<ObjectId> {
        let all: Vec<ObjectId> = game.zone(crate::zones::Zone::Battlefield).to_vec();
        at.iter().map(|index| all[*index]).collect()
    }

    #[rstest]
    #[case::two_banding(&[0, 1], true)]
    #[case::banding_and_one_other(&[0, 2], true)]
    #[case::two_banding_and_one_other(&[0, 1, 2], true)]
    #[case::banding_and_two_others(&[0, 2, 3], false)]
    #[case::no_banding(&[2, 3], false)]
    #[case::dinosaurs_with_a_bander(&[4, 5], true)]
    #[case::bands_with_other_and_a_bear(&[4, 2], false)]
    #[case::bands_with_other_counts_as_one_without_banding(&[0, 4], true)]
    #[case::plain_dinosaur_with_a_bear(&[5, 2], false)]
    fn bands_form_from_banding_creatures(#[case] members: &[usize], #[case] forms: bool) {
        let game = game();
        assert_eq!(may_form(&game, &ids(&game, members)), forms);
    }

    #[rstest]
    #[case::one_band("0 2", &[0, 1, 2], None)]
    #[case::two_bands("0 2|1 3", &[0, 1, 2, 3], None)]
    #[case::alone("0", &[0, 1], Some("band of one"))]
    #[case::not_attacking("0 2", &[0, 1], Some("band member"))]
    #[case::twice("0 2|1 2", &[0, 1, 2], Some("band member"))]
    #[case::not_allowed("0 2 3", &[0, 2, 3], Some("band"))]
    fn a_declarations_bands_are_checked(
        #[case] bands: &str,
        #[case] attacking: &[usize],
        #[case] refused: Option<&str>,
    ) {
        let game = game();
        let attackers: Vec<(ObjectId, AttackTarget)> = ids(&game, attacking)
            .into_iter()
            .map(|id| (id, AttackTarget::Player(FOE)))
            .collect();
        let bands: Vec<Vec<ObjectId>> = bands
            .split('|')
            .map(|band| {
                let at: Vec<usize> = band.split(' ').map(|n| n.parse().unwrap()).collect();
                ids(&game, &at)
            })
            .collect();
        let got = check_bands(&game, &attackers, &bands)
            .err()
            .map(|err| match err {
                IllegalAnswer::NotOffered(what) | IllegalAnswer::Duplicate(what) => what,
                _ => "other",
            });
        assert_eq!(got, refused);
    }

    #[test]
    fn a_band_split_between_defenders_is_refused() {
        let mut game = state(
            vec![vec![], vec![], vec![]],
            vec![
                vec![
                    creature("Knight A", "Knight", Some(Keyword::Banding)),
                    creature("Bear A", "Bear", None),
                ],
                vec![],
                vec![],
            ],
        );
        game.turn.step = crate::turn::Step::DeclareAttackers;
        let band = ids(&game, &[0, 1]);
        let attackers = vec![
            (band[0], AttackTarget::Player(FOE)),
            (band[1], AttackTarget::Player(PlayerId(2))),
        ];
        assert_eq!(
            check_bands(&game, &attackers, &[band]),
            Err(IllegalAnswer::NotOffered("band split between defenders"))
        );
    }

    fn attacking(game: &mut GameState, attackers: &[ObjectId], band: &[ObjectId]) {
        game.combat = CombatState {
            attackers: attackers
                .iter()
                .map(|object| Attacker {
                    object: *object,
                    target: AttackTarget::Player(FOE),
                    blocked: false,
                    blockers: Vec::new(),
                    assignment: None,
                    target_left: false,
                })
                .collect(),
            attackers_declared: true,
            bands: if band.is_empty() {
                Vec::new()
            } else {
                vec![band.to_vec()]
            },
            ..CombatState::default()
        };
    }

    #[test]
    fn a_block_of_one_member_blocks_the_band() {
        let mut game = game();
        let [knight, _, bear, other] = ids(&game, &[0, 1, 2, 3])[..] else {
            panic!("four creatures")
        };
        let wall = ids(&game, &[7])[0];
        attacking(&mut game, &[knight, bear, other], &[knight, bear]);
        assert_eq!(
            blocks_with_bands(&game, &[(wall, bear)]),
            vec![(wall, bear), (wall, knight)]
        );
        assert_eq!(
            blocks_with_bands(&game, &[(wall, other)]),
            vec![(wall, other)]
        );
    }

    #[rstest]
    #[case::banding_blocker(&[6], Some(FOE))]
    #[case::plain_blocker(&[7], None)]
    #[case::one_banding_among_two(&[7, 6], Some(FOE))]
    fn the_defender_divides_an_attackers_damage_when_a_blocker_bands(
        #[case] blockers: &[usize],
        #[case] divider: Option<PlayerId>,
    ) {
        let game = game();
        let bear = ids(&game, &[2])[0];
        let attacker = Attacker {
            object: bear,
            target: AttackTarget::Player(FOE),
            blocked: true,
            blockers: ids(&game, blockers),
            assignment: None,
            target_left: false,
        };
        assert_eq!(divides_attacker(&game, &attacker), divider);
    }

    #[rstest]
    #[case::a_band_with_banding(&[0, 2], Some(ME))]
    #[case::two_dinosaurs(&[4, 5], Some(ME))]
    #[case::two_bears(&[2, 3], None)]
    #[case::a_lone_dinosaur_bander_with_a_bear(&[4, 2], None)]
    fn the_attacker_divides_a_blockers_damage_when_an_attacker_bands(
        #[case] blocked: &[usize],
        #[case] divider: Option<PlayerId>,
    ) {
        let game = game();
        assert_eq!(divides_blocker(&game, &ids(&game, blocked)), divider);
    }
}
