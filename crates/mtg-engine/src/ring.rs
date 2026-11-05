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

use crate::effects::{ContinuousEffect, Selector};
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

/// The abilities the levels 2 to 4 give, parsed once.
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

/// The effects the Rings of the game have on their bearers, for the layers to apply with the others.
pub(crate) fn effects(state: &GameState) -> Vec<ContinuousEffect> {
    let mut found = Vec::new();
    for (player, ring) in &state.ring {
        let Some(bearer) = ring.bearer.filter(|_| ring.level > 0) else {
            continue;
        };
        // An id past any the game hands out keeps these apart from the registered effects and from each other.
        let mut id = u64::MAX - player.0 * 8;
        let mut give = |modification: Modification| {
            id -= 1;
            // The bearer's controller loses the bearer by control (the layers drop it with the source) or by leaving.
            let mut effect = ContinuousEffect::new(
                EffectId(id),
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
            give(Modification::CantBeBlockedBy {
                by,
                controller: *player,
                source: bearer,
            });
        }
        let granted = usize::from(ring.level.min(MAX_LEVEL)).saturating_sub(1);
        for ability in level_abilities().iter().take(granted) {
            give(Modification::GrantAbility(Arc::clone(ability)));
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
}
