//! The Ring (CR 701.54): each player's Ring level and Ring-bearer, and the abilities the levels give the bearer.
//!
//! The Ring tempts a player: their level rises (to 4 at most) and they choose a creature they control as their
//! Ring-bearer. The bearer keeps the status until another creature is chosen, it leaves the battlefield or another
//! player controls it. The levels are not objects: the layers read them as effects that give the bearer what the Ring
//! says, so a copy, a control change or a new bearer needs no bookkeeping.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use crate::effects::Modification;
use mtg_cards::ability::Ability;
use mtg_cards::modify::Duration;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use crate::abilities::Grant;
use crate::effects::{ContinuousEffect, Selector};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// The highest Ring level.
pub(crate) const MAX_LEVEL: u8 = 4;

/// One player's Ring: its level and the creature chosen as the Ring-bearer, as it was chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct Ring {
    pub(crate) level: u8,
    pub(crate) bearer: Option<ObjectRef>,
}

/// The Ring of every player the Ring has tempted.
pub(crate) type Rings = BTreeMap<PlayerId, Ring>;

/// Level 1: your Ring-bearer (legendary, see `is_legendary_bearer`) can't be blocked by creatures with greater power.
const EVASION: &str =
    r#"[{"Compare":{"stat":"Power","op":"Gt","to":{"Stat":{"of":"This","stat":"Power"}}}}]"#;
/// Level 2: whenever your Ring-bearer attacks, draw a card, then discard a card.
const LOOT: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Draw":{}},{"Discard":{}}]}}"#;
/// Level 3: whenever your Ring-bearer becomes blocked by a creature, that creature's controller sacrifices it at end of combat.
const SACRIFICE: &str = r#"{"Triggered":{"when":{"BecomesBlocked":{"what":["This"],"by":[{"Type":"Creature"}]}},"effects":[{"Bind":{"what":"Dealer","as":"blocker"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Sacrifice":{"who":{"ControllerOf":{"Bound":"blocker"}},"filter":[{"Is":{"Bound":"blocker"}}]}}]}}]}}"#;
/// Level 4: whenever your Ring-bearer deals combat damage to a player, each opponent loses 3 life.
const DRAIN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"LoseLife":{"who":"Opponents","amount":3}}]}}"#;

/// The abilities the levels 2 to 4 give, parsed once, in the order of the levels.
fn level_abilities() -> &'static [Arc<Ability>] {
    static PARSED: OnceLock<Vec<Arc<Ability>>> = OnceLock::new();
    PARSED.get_or_init(|| {
        [LOOT, SACRIFICE, DRAIN]
            .iter()
            .filter_map(|text| serde_json::from_str(text).ok())
            .map(Arc::new)
            .collect()
    })
}

/// The id of the effect that the Ring of `player` has at `level`: past any id the game hands out, which keeps these
/// apart from the registered effects and from each other.
fn effect_id(player: PlayerId, level: u8) -> EffectId {
    EffectId(u64::MAX - player.0 * 8 - u64::from(level))
}

/// What a layers pass names the effect `id` by when it is one of a Ring's, so that what it gives is the Ring's.
pub(crate) fn grant_of(state: &GameState, id: EffectId) -> Option<Grant> {
    let below = u64::MAX - id.0;
    let (player, level) = (PlayerId(below / 8), u8::try_from(below % 8).ok()?);
    ((1..=MAX_LEVEL).contains(&level) && state.ring.contains_key(&player))
        .then_some(Grant::Ring { player, level })
}

/// The ability the Ring of `player` gives its bearer at `level` while it gives it: the Ring has a bearer and has reached
/// the level.
pub(crate) fn ability_given(
    state: &GameState,
    player: PlayerId,
    level: u8,
) -> Option<Arc<Ability>> {
    let ring = state.ring.get(&player)?;
    if ring.bearer.is_none() || ring.level < level {
        return None;
    }
    level_abilities()
        .get(usize::from(level.checked_sub(2)?))
        .map(Arc::clone)
}

impl GameState {
    /// The level of the player's Ring: 0 until the Ring has tempted them.
    pub fn ring_level(&self, player: PlayerId) -> u8 {
        self.ring.get(&player).map_or(0, |ring| ring.level)
    }

    /// The player's Ring-bearer: the creature they chose, while it is still that object, on the battlefield, and
    /// controlled by them.
    pub fn ring_bearer(&self, player: PlayerId) -> Option<ObjectId> {
        let chosen = self.ring.get(&player)?.bearer?;
        let object = self.object(chosen.id)?;
        (object.reference() == chosen
            && object.zone() == Zone::Battlefield
            && crate::characteristics::controller_of(self, chosen.id) == Some(player))
        .then_some(chosen.id)
    }
}

/// The Ring-bearers another player has gained control of: the status ends then and does not return with the control
/// (CR 701.54c).
pub(crate) fn state_based(state: &GameState) -> Vec<Event> {
    state
        .ring
        .iter()
        .filter(|(player, ring)| {
            ring.bearer.is_some_and(|chosen| {
                state.object(chosen.id).is_some_and(|object| {
                    object.reference() == chosen
                        && object.zone() == Zone::Battlefield
                        && crate::characteristics::controller_of(state, chosen.id)
                            .is_some_and(|now| now != **player)
                })
            })
        })
        .map(|(player, _)| Event::RingBearerLost { player: *player })
        .collect()
}

/// The effects the Rings of the game have on their bearers, for the layers to apply with the others.
pub(crate) fn effects(state: &GameState) -> Vec<ContinuousEffect> {
    let mut found = Vec::new();
    for (player, ring) in &state.ring {
        let Some(bearer) = ring.bearer.filter(|_| ring.level > 0) else {
            continue;
        };
        let mut give = |level: u8, modification: Modification| {
            // The bearer's controller loses the bearer by control (the layers drop it with the source) or by leaving.
            let mut effect = ContinuousEffect::new(
                effect_id(*player, level),
                Selector::Objects(vec![bearer]),
                modification,
                Duration::WhileYouControlSource,
                0,
            )
            .begun(*player, 0);
            effect.source = Some(bearer);
            found.push(effect);
        };
        if let Ok(by) = serde_json::from_str(EVASION) {
            give(
                1,
                Modification::CantBeBlockedBy {
                    by,
                    controller: *player,
                    source: bearer,
                },
            );
        }
        for level in 2..=ring.level.min(MAX_LEVEL) {
            if let Some(ability) = ability_given(state, *player, level) {
                give(level, Modification::GrantAbility(ability));
            }
        }
    }
    found
}

/// Whether the object is the Ring-bearer of a player whose Ring has reached level 1: it is legendary (CR 701.54c).
pub(crate) fn is_legendary_bearer(state: &GameState, id: ObjectId) -> bool {
    !state.ring.is_empty()
        && state
            .ring
            .keys()
            .any(|player| state.ring_bearer(*player) == Some(id) && state.ring_level(*player) > 0)
}

#[cfg(test)]
mod tests {
    //! Goal: the text of the Ring's abilities is valid card DSL (a typo would silently drop a level), each level gives the bearer one effect more than the level before, and no Ring gives nothing.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};

    #[test]
    fn every_ability_text_is_valid() {
        assert_eq!(level_abilities().len(), 3);
        assert!(serde_json::from_str::<mtg_cards::refs::Selector>(EVASION).is_ok());
    }

    #[rstest]
    #[case::untouched(0, 0)]
    #[case::level_one(1, 1)]
    #[case::level_two(2, 2)]
    #[case::level_three(3, 3)]
    #[case::level_four(4, 4)]
    fn each_level_gives_one_effect_more(#[case] level: u8, #[case] effects: usize) {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Hero", 1, 1)], vec![]],
        );
        let hero = game.zone(Zone::Battlefield)[0];
        let bearer = game.object(hero).map(crate::object::GameObject::reference);
        game.ring.insert(PlayerId(0), Ring { level, bearer });
        assert_eq!(super::effects(&game).len(), effects);
    }

    /// A game in which both seats have a Ring at `level`, each with a creature as its bearer.
    fn ringed(level: u8) -> GameState {
        let mut game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Hero", 1, 1)], vec![creature("Foe", 1, 1)]],
        );
        for (seat, id) in game
            .zone(Zone::Battlefield)
            .to_vec()
            .into_iter()
            .enumerate()
        {
            let bearer = game.object(id).map(crate::object::GameObject::reference);
            game.ring
                .insert(PlayerId(seat as u64), Ring { level, bearer });
        }
        game
    }

    #[rstest]
    #[case::the_first_seat(0)]
    #[case::the_second_seat(1)]
    fn every_effect_of_a_ring_is_named_by_its_player_and_level(#[case] seat: u64) {
        let game = ringed(4);
        let mut named: Vec<_> = super::effects(&game)
            .iter()
            .filter(|effect| effect.controller == PlayerId(seat))
            .map(|effect| grant_of(&game, effect.id))
            .collect();
        named.sort();
        let player = PlayerId(seat);
        assert_eq!(
            named,
            (1..=4)
                .map(|level| Some(Grant::Ring { player, level }))
                .collect::<Vec<_>>()
        );
    }

    #[rstest]
    #[case::a_registered_effect(EffectId(7))]
    #[case::a_sticker_effect(EffectId(u64::MAX / 2 - 3))]
    #[case::a_ring_that_does_not_exist(EffectId(u64::MAX - 5 * 8 - 2))]
    #[case::past_the_levels(EffectId(u64::MAX - 5))]
    fn an_effect_that_is_no_ring_of_the_game_has_no_ring_name(#[case] id: EffectId) {
        assert_eq!(grant_of(&ringed(4), id), None);
    }

    #[rstest]
    #[case::level_two_at_level_two(2, 2, true)]
    #[case::level_two_above_it(4, 2, true)]
    #[case::level_four_at_level_four(4, 4, true)]
    #[case::level_three_below_it(2, 3, false)]
    #[case::level_one_gives_no_ability(4, 1, false)]
    #[case::no_such_level(4, 5, false)]
    fn the_ring_gives_the_ability_of_a_level_it_has_reached(
        #[case] reached: u8,
        #[case] asked: u8,
        #[case] given: bool,
    ) {
        assert_eq!(
            ability_given(&ringed(reached), PlayerId(0), asked).is_some(),
            given
        );
    }

    #[test]
    fn a_ring_without_a_bearer_gives_no_ability() {
        let mut game = ringed(4);
        game.ring.insert(
            PlayerId(0),
            Ring {
                level: 4,
                bearer: None,
            },
        );
        assert!(ability_given(&game, PlayerId(0), 2).is_none());
        assert!(ability_given(&game, PlayerId(1), 2).is_some());
    }
}
