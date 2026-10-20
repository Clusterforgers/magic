//! Derived characteristics: base values from the card plus continuous effects, layer by layer.

use mtg_cards::AbilityDef;
use mtg_core::card::{CardType, CardTypes};
use mtg_core::color::ColorSet;
use mtg_core::ids::ObjectId;
use serde::{Deserialize, Serialize};

use crate::effects::ContinuousEffect;
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::Zone;

/// The characteristics of an object right now; never stored, always recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Characteristics {
    /// Card types.
    pub types: CardTypes,
    /// Colors.
    pub colors: ColorSet,
    /// Power, if the object has one.
    pub power: Option<i32>,
    /// Toughness, if the object has one.
    pub toughness: Option<i32>,
    /// Abilities.
    pub abilities: Vec<AbilityDef>,
}

impl Characteristics {
    /// Whether the object is currently a creature.
    pub fn is_creature(&self) -> bool {
        self.types.contains(CardType::Creature)
    }
}

/// Computes the characteristics of `id` from its card and the active continuous effects (CR 613).
///
/// Effects apply layer by layer, and by timestamp within a layer; dependencies (CR 613.8) are not
/// modeled yet. Selectors are evaluated against the characteristics built up to their own layer.
/// Only objects on the battlefield are affected by effects. An ability on the stack has none.
pub fn characteristics(state: &GameState, id: ObjectId) -> Option<Characteristics> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability {
        return Some(Characteristics {
            types: CardTypes::default(),
            colors: ColorSet::default(),
            power: None,
            toughness: None,
            abilities: Vec::new(),
        });
    }
    Some(derive(state, object).0)
}

/// The characteristics of a card or token and the effects that changed them, in the order they applied.
///
/// This is the one place the layers are walked: `characteristics` keeps the first part and `keywords_of` the
/// second, so nothing reads an effect another way.
pub(crate) fn derive<'a>(
    state: &'a GameState,
    object: &'a GameObject,
) -> (Characteristics, Vec<&'a ContinuousEffect>) {
    let card = object.card();
    let mut chars = Characteristics {
        types: object.base_types(),
        colors: card.colors(),
        power: card.power,
        toughness: card.toughness,
        abilities: card.abilities.clone(),
    };
    let mut applied = Vec::new();
    if object.zone() != Zone::Battlefield || state.effects().is_empty() {
        return (chars, applied);
    }
    let mut active: Vec<&ContinuousEffect> = state
        .effects()
        .iter()
        .filter(|e| e.is_active(state))
        .collect();
    active.sort_by_key(|e| (e.layer, e.timestamp, e.id));
    for effect in active {
        if effect.selector.matches(object, &chars) {
            effect.modification.apply(&mut chars);
            applied.push(effect);
        }
    }
    (chars, applied)
}

#[cfg(test)]
mod tests {
    //! Goal: characteristics are the printed values with the active effects applied layer by layer and by timestamp within a layer, whatever order the effects are stored in; only permanents change, only the objects an effect names, and an effect that is over changes nothing.
    use mtg_core::ids::{EffectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::effects::{Duration, Layer, Modification, Selector};
    use crate::fixtures::{creature, state};
    use crate::object::ObjectRef;

    fn game() -> GameState {
        state(
            vec![vec![creature("Idle", 3, 3)], vec![]],
            vec![
                vec![creature("Bear", 2, 2), creature("Wolf", 4, 4)],
                vec![creature("Ogre", 3, 4)],
            ],
        )
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::reference)
            .unwrap()
    }

    fn stats(game: &GameState, name: &str) -> (Option<i32>, Option<i32>) {
        let chars = characteristics(game, named(game, name).id).unwrap();
        (chars.power, chars.toughness)
    }

    #[derive(Clone, Copy)]
    enum Change {
        Pump(i32, i32),
        Set(i32, i32),
    }
    use Change::{Pump, Set};

    /// The effects on the Bear, as (change, timestamp), stored in this order.
    fn on_the_bear(changes: &[(Change, u64)]) -> GameState {
        let mut game = game();
        let bear = named(&game, "Bear");
        for (n, (change, timestamp)) in (0..).zip(changes) {
            let modification = match *change {
                Pump(power, toughness) => Modification::ModifyPowerToughness { power, toughness },
                Set(power, toughness) => Modification::SetPowerToughness { power, toughness },
            };
            game.effects.push(ContinuousEffect::new(
                EffectId(n),
                Selector::Objects(vec![bear]),
                modification,
                Duration::UntilEndOfTurn,
                *timestamp,
            ));
        }
        game
    }

    #[rstest]
    #[case::nothing(&[], (2, 2))]
    #[case::a_pump(&[(Pump(3, 3), 0)], (5, 5))]
    #[case::pumps_add_up(&[(Pump(1, 1), 0), (Pump(2, 0), 1), (Pump(0, 3), 2)], (5, 6))]
    #[case::a_pump_can_take_toughness_to_zero(&[(Pump(-1, -2), 0)], (1, 0))]
    #[case::a_pump_can_take_it_below_zero(&[(Pump(-5, -5), 0)], (-3, -3))]
    #[case::a_set_comes_before_a_pump_whatever_the_timestamps(&[(Pump(1, 1), 5), (Set(1, 1), 1)], (2, 2))]
    #[case::the_later_set_wins(&[(Set(4, 4), 9), (Set(1, 1), 2)], (4, 4))]
    #[case::stored_late_but_stamped_early(&[(Set(1, 1), 7), (Set(4, 4), 3)], (1, 1))]
    #[case::pump_set_pump(&[(Pump(1, 0), 1), (Set(0, 1), 2), (Pump(3, 0), 3)], (4, 1))]
    fn power_and_toughness_follow_layer_then_timestamp_not_storage_order(
        #[case] changes: &[(Change, u64)],
        #[case] expected: (i32, i32),
    ) {
        let game = on_the_bear(changes);
        assert_eq!(stats(&game, "Bear"), (Some(expected.0), Some(expected.1)));
        let (_, applied) = derive(&game, game.object(named(&game, "Bear").id).unwrap());
        let order: Vec<(Layer, u64)> = applied.iter().map(|e| (e.layer, e.timestamp)).collect();
        assert!(order.windows(2).all(|w| w[0] <= w[1]), "{order:?}");
        assert_eq!(applied.len(), changes.len());
    }

    #[test]
    fn an_effect_changes_the_objects_it_names_and_no_other() {
        let game = on_the_bear(&[(Pump(3, 3), 0)]);
        assert_eq!(stats(&game, "Bear"), (Some(5), Some(5)));
        assert_eq!(stats(&game, "Wolf"), (Some(4), Some(4)));
        assert_eq!(stats(&game, "Ogre"), (Some(3), Some(4)));
    }

    #[rstest]
    #[case::every_creature(Selector::AllCreatures, [(3, 3), (3, 3), (5, 5), (4, 5)])]
    #[case::one_seats_creatures(Selector::CreaturesControlledBy(PlayerId(0)), [(3, 3), (3, 3), (5, 5), (3, 4)])]
    #[case::the_other_seat(Selector::CreaturesControlledBy(PlayerId(1)), [(3, 3), (2, 2), (4, 4), (4, 5)])]
    fn a_selector_over_creatures_changes_permanents_and_not_cards_elsewhere(
        #[case] selector: Selector,
        #[case] expected: [(i32, i32); 4],
    ) {
        let mut game = game();
        game.effects.push(ContinuousEffect::new(
            EffectId(0),
            selector,
            Modification::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
            Duration::UntilEndOfTurn,
            0,
        ));
        // The card in a hand keeps its printed values; the three permanents are changed if the selector reaches them.
        let shown = ["Idle", "Bear", "Wolf", "Ogre"].map(|name| {
            let (power, toughness) = stats(&game, name);
            (power.unwrap(), toughness.unwrap())
        });
        assert_eq!(shown, expected);
    }

    #[test]
    fn an_effect_that_is_over_changes_nothing() {
        let mut game = on_the_bear(&[(Pump(3, 3), 0)]);
        assert_eq!(stats(&game, "Bear"), (Some(5), Some(5)));
        // The Bear becomes a new object: the effect still names the old one.
        game.objects
            .get_mut(&named(&game, "Bear").id)
            .unwrap()
            .counter += 1;
        assert_eq!(stats(&game, "Bear"), (Some(2), Some(2)));
        assert_eq!(stats(&game, "Wolf"), (Some(4), Some(4)));
    }
}
