//! Derived characteristics: base values from the card plus continuous effects, layer by layer.

use mtg_cards::AbilityDef;
use mtg_cards::modify::Attacked;
use mtg_core::card::{CardType, CardTypes, Subtype};
use mtg_core::color::ColorSet;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::effects::Modification;
use crate::layers::{Held, outside};
use crate::object::{GameObject, ObjectKind};
use crate::resolve::Scope;
use crate::state::GameState;
use crate::zones::Zone;

/// The characteristics of an object right now; never stored, always recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Characteristics {
    /// Card types.
    pub types: CardTypes,
    /// The subtypes effects gave it; the ones printed on the card are the card's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<Subtype>>", optional))]
    pub subtypes: Vec<Subtype>,
    /// An effect replaced the creature types: the printed ones no longer count, only those in `subtypes` do.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub creature_types_set: bool,
    /// An effect made it every creature type (changeling, CR 702.73): it has each of them, unless another effect sets the types.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub all_creature_types: bool,
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

    /// Whether the object is `subtype`, given the subtypes its card prints: one an effect gave, or a printed one that
    /// no effect replaced (CR 205.1b).
    pub fn has_subtype(&self, printed: &[Subtype], subtype: &Subtype) -> bool {
        if let Some((first, second)) = subtype.two_word_land() {
            return self.has_subtype(printed, &first) && self.has_subtype(printed, &second);
        }
        self.subtypes.contains(subtype)
            || (self.all_creature_types && subtype.is_creature_type())
            || (printed.contains(subtype)
                && !(self.creature_types_set && subtype.is_creature_type()))
    }

    /// What nothing has: no types, colors, power, toughness or abilities.
    pub(crate) fn empty() -> Characteristics {
        Characteristics {
            types: CardTypes::default(),
            subtypes: Vec::new(),
            creature_types_set: false,
            all_creature_types: false,
            colors: ColorSet::default(),
            power: None,
            toughness: None,
            abilities: Vec::new(),
        }
    }
}

/// The characteristics of `id` from its card and the effects that apply to it, the static abilities of the permanents
/// among them (CR 613).
///
/// The layers are worked through once for the whole state (`layers.rs`): effects apply layer by layer, the
/// characteristic-defining abilities first and then by timestamp, an effect that depends on others after them (CR
/// 613.8). Only a permanent is affected by effects; an object elsewhere has the characteristics its card prints, with
/// those its characteristic-defining abilities define (CR 604.3). An ability on the stack has none.
pub fn characteristics(state: &GameState, id: ObjectId) -> Option<Characteristics> {
    seen_as(state, id).map(|(chars, _)| chars)
}

/// The characteristics and the controller of `id` from one reading of the layers: what the value evaluator and the
/// snapshots read an object as.
pub(crate) fn seen_as(state: &GameState, id: ObjectId) -> Option<(Characteristics, PlayerId)> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability {
        return Some((Characteristics::empty(), object.controller()));
    }
    if object.zone() == Zone::Battlefield {
        return Some(match state.layers().get(id) {
            Some(held) => (held.chars.clone(), held.controller),
            None => (base(object), object.controller()),
        });
    }
    let mut chars = outside(state, object, None);
    // What static abilities give to a card outside the battlefield (`layers::outside_zones`).
    if let Some(held) = state.layers().get(id) {
        held.replay(&mut chars);
    }
    Some((chars, object.controller()))
}

/// The creature types of `id` as the game sees it now (printed ones no effect replaced and those effects gave), and whether it
/// has every one (changeling, CR 702.73).
pub(crate) fn creature_types(state: &GameState, id: ObjectId) -> (Vec<Subtype>, bool) {
    let (Some(object), Some(chars)) = (state.object(id), characteristics(state, id)) else {
        return (Vec::new(), false);
    };
    let printed = &object.card().subtypes;
    let mut types: Vec<Subtype> = printed
        .iter()
        .filter(|subtype| subtype.is_creature_type() && chars.has_subtype(printed, subtype))
        .cloned()
        .collect();
    for subtype in &chars.subtypes {
        if subtype.is_creature_type() && !types.contains(subtype) {
            types.push(subtype.clone());
        }
    }
    (types, chars.all_creature_types)
}

/// Who controls `id` now: for a permanent the controller after the control-changing effects of layer 2 (CR 613.1b), for
/// anything else the one the object has. `None` for an object that is not in the game.
pub fn controller_of(state: &GameState, id: ObjectId) -> Option<PlayerId> {
    state
        .object(id)
        .map(|object| controller_of_object(state, object))
}

/// Who controls `object`, as [`controller_of`] says, for a caller that has the object at hand.
pub(crate) fn controller_of_object(state: &GameState, object: &GameObject) -> PlayerId {
    if object.zone() == Zone::PhasedOut {
        // The layers leave it out; it is the controller it had when it phased out who it phases in for.
        return object
            .permanent()
            .and_then(crate::object::PermanentState::phasing)
            .map_or_else(|| object.controller(), |phasing| phasing.controller);
    }
    if object.zone() != Zone::Battlefield {
        return object.controller();
    }
    state
        .layers()
        .get(object.id())
        .map_or_else(|| object.controller(), |held| held.controller)
}

/// Whether an effect that applies to `id` says it can't attack: the one place the attack rules read this restriction.
pub(crate) fn cant_attack(state: &GameState, id: ObjectId) -> bool {
    restricted(state, id, |modification| {
        matches!(modification, Modification::CantAttack)
    })
}

/// Whether an effect that applies to `id` says it can't attack `defender`, the player attacked or the controller of the
/// planeswalker attacked (`walker`): the one place the attack rules read "can't attack you or planeswalkers you
/// control".
pub(crate) fn cant_attack_defender(
    state: &GameState,
    id: ObjectId,
    defender: PlayerId,
    walker: bool,
) -> bool {
    restricted(state, id, |modification| match modification {
        Modification::CantAttackYou { you, attacked } => {
            *you == defender
                && match attacked {
                    Attacked::Both => true,
                    Attacked::Player => !walker,
                    Attacked::Planeswalkers => walker,
                }
        }
        _ => false,
    })
}

/// The players that goad the creature `id` (CR 701.15), by an effect that applies to it or by a static ability, in the
/// order the effects applied: the one place the attack declaration reads this requirement.
pub(crate) fn goaded_by(state: &GameState, id: ObjectId) -> Vec<PlayerId> {
    state
        .object(id)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .map(|held| goaders(held).collect())
        .unwrap_or_default()
}

/// The players whose goad effects have applied to what is held of a permanent, in the order they applied: what a
/// selector of goaded creatures reads while the layers are worked out.
pub(crate) fn goaders(held: &Held) -> impl Iterator<Item = PlayerId> + '_ {
    held.applied
        .iter()
        .filter_map(|applied| match applied.modification {
            Modification::Goaded { by } => Some(by),
            _ => None,
        })
}

/// Whether an effect that applies to `id` says it does not untap during its controller's untap step: the one place the
/// untap step reads this restriction. An exert of the controller holds it too (CR 701.43c).
pub(crate) fn doesnt_untap(state: &GameState, id: ObjectId) -> bool {
    restricted(state, id, |modification| {
        matches!(modification, Modification::DoesntUntap)
    }) || crate::w11a_exert::w11a_held(state, id)
}

/// Whether an effect that applies to `id` says it can't block: the one place the block rules read this restriction.
pub(crate) fn cant_block(state: &GameState, id: ObjectId) -> bool {
    restricted(state, id, |modification| {
        matches!(modification, Modification::CantBlock)
    })
}

/// Whether an effect that applies to the attacker `attacker` says the creature `blocker` can't block it ("can't be
/// blocked", "can't be blocked by creatures with power 2 or less"): the one place the block rules read these restrictions.
/// The selector judges `blocker` as the layers leave it now, when blockers are declared.
pub(crate) fn cant_be_blocked_by(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    let Some(blocker) = state.object(blocker).map(GameObject::reference) else {
        return false;
    };
    state
        .object(attacker)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .is_some_and(|held| {
            held.applied
                .iter()
                .any(|applied| match &applied.modification {
                    Modification::CantBeBlocked => true,
                    Modification::CantBeBlockedBy {
                        by,
                        controller,
                        source,
                    } => Scope::announcing(state, *controller, *source)
                        .selects(by, blocker)
                        .unwrap_or(false),
                    _ => false,
                })
        })
}

/// Whether an effect that applies to the creature `blocker` says it can block only attackers `attacker` is not one of
/// ("can block only creatures with flying"): the one place the block rules read this restriction. The selector judges
/// `attacker` as the layers leave it now.
pub(crate) fn cant_block_attacker(
    state: &GameState,
    blocker: ObjectId,
    attacker: ObjectId,
) -> bool {
    let Some(attacker) = state.object(attacker).map(GameObject::reference) else {
        return false;
    };
    state
        .object(blocker)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .is_some_and(|held| {
            held.applied
                .iter()
                .any(|applied| match &applied.modification {
                    Modification::CanBlockOnly {
                        only,
                        controller,
                        source,
                    } => !Scope::announcing(state, *controller, *source)
                        .selects(only, attacker)
                        .unwrap_or(true),
                    _ => false,
                })
        })
}

/// How many creatures may block the attacker `id` together, as the effects that apply to it say: at least the most any
/// `MinBlockers` asks (1 when none does) and at most the fewest any `MaxBlockers` allows (`None` when none does). The
/// keyword menace is read beside this by `keywords::min_blockers`.
pub(crate) fn blocker_limits(state: &GameState, id: ObjectId) -> (usize, Option<usize>) {
    let mut least = 1;
    let mut most: Option<usize> = None;
    let applied = state
        .object(id)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .map(|held| held.applied.as_slice())
        .unwrap_or_default();
    for applied in applied {
        match applied.modification {
            Modification::MinBlockers(count) => least = least.max(count as usize),
            Modification::MaxBlockers(count) => {
                most = Some(most.map_or(count as usize, |held| held.min(count as usize)));
            }
            _ => {}
        }
    }
    (least, most)
}

/// Whether a modification that `is` applies to the permanent `id` once the layers are through with it.
fn restricted(state: &GameState, id: ObjectId, is: impl Fn(&Modification) -> bool) -> bool {
    state
        .object(id)
        .filter(|object| object.zone() == Zone::Battlefield)
        .and_then(|object| state.layers().get(object.id()))
        .is_some_and(|held| held.applied.iter().any(|applied| is(&applied.modification)))
}

/// The characteristics of a card or token on its own, before any effect.
pub(crate) fn base(object: &GameObject) -> Characteristics {
    let card = object.card();
    Characteristics {
        types: object.base_types(),
        // A bestowed creature card is an Aura while it is bestowed (CR 702.103b).
        subtypes: if object.bestowed() {
            vec![Subtype("Aura".to_owned())]
        } else {
            Vec::new()
        },
        creature_types_set: false,
        all_creature_types: false,
        colors: card.colors(),
        power: card.power,
        toughness: card.toughness,
        abilities: card.abilities.clone(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: characteristics are the printed values with the active effects applied layer by layer and by timestamp within a layer, whatever order the effects are stored in; only permanents change, only the objects an effect names, and an effect that is over changes nothing; every modification changes what its layer says, in the order of the layers (a type before a colour, a set before a pump before a switch), and a restriction changes no characteristic.
    use mtg_core::ids::{EffectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::effects::{ContinuousEffect, Duration, Layer, Modification, Selector};
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
        Switch,
    }
    use Change::{Pump, Set, Switch};

    fn modification(change: Change) -> Modification {
        match change {
            Pump(power, toughness) => Modification::ModifyPowerToughness { power, toughness },
            Set(power, toughness) => Modification::SetPowerToughness { power, toughness },
            Switch => Modification::SwitchPowerToughness,
        }
    }

    /// The effects on the Bear, as (change, timestamp), stored in this order.
    fn on_the_bear(changes: &[(Change, u64)]) -> GameState {
        let mut game = game();
        let bear = named(&game, "Bear");
        for (n, (change, timestamp)) in (0..).zip(changes) {
            game.effects.push(ContinuousEffect::new(
                EffectId(n),
                Selector::Objects(vec![bear]),
                modification(*change),
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
    #[case::a_switch_swaps_what_is_there(&[(Switch, 0)], (2, 2))]
    #[case::a_switch_after_a_set_and_a_pump_whatever_the_timestamps(&[(Switch, 1), (Pump(1, 0), 2), (Set(1, 5), 3)], (5, 2))]
    #[case::two_switches_cancel(&[(Switch, 1), (Switch, 2), (Set(1, 5), 3)], (1, 5))]
    fn power_and_toughness_follow_layer_then_timestamp_not_storage_order(
        #[case] changes: &[(Change, u64)],
        #[case] expected: (i32, i32),
    ) {
        let game = on_the_bear(changes);
        assert_eq!(stats(&game, "Bear"), (Some(expected.0), Some(expected.1)));
        // A state with no effect works out no layers.
        let applied = game
            .layers()
            .get(named(&game, "Bear").id)
            .map_or_else(Vec::new, |held| held.applied.clone());
        let order: Vec<(Option<Layer>, u64)> =
            applied.iter().map(|e| (e.layer, e.timestamp)).collect();
        assert!(order.windows(2).all(|w| w[0].0 <= w[1].0), "{order:?}");
        assert_eq!(applied.len(), changes.len());
    }

    #[test]
    fn a_switch_changes_nothing_for_an_object_with_no_power_or_toughness() {
        let mut chars = Characteristics::empty();
        Modification::SwitchPowerToughness.apply(&mut chars);
        assert_eq!((chars.power, chars.toughness), (None, None));
        chars.power = Some(3);
        Modification::SwitchPowerToughness.apply(&mut chars);
        assert_eq!((chars.power, chars.toughness), (None, Some(3)));
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
        game.layers.reset();
        assert_eq!(stats(&game, "Bear"), (Some(2), Some(2)));
        assert_eq!(stats(&game, "Wolf"), (Some(4), Some(4)));
    }

    fn on_a_bear(modification: Modification) -> (Characteristics, PlayerId) {
        let mut game = game();
        let bear = named(&game, "Bear");
        game.effects.push(ContinuousEffect::new(
            EffectId(0),
            Selector::Objects(vec![bear]),
            modification,
            Duration::UntilEndOfTurn,
            0,
        ));
        let held = game.layers().get(bear.id).unwrap();
        (held.chars.clone(), held.controller)
    }

    #[rstest]
    #[case::a_type(Modification::AddType(CardType::Artifact), &[CardType::Artifact, CardType::Creature], &[], 0, 0)]
    #[case::a_type_again_changes_nothing(Modification::AddType(CardType::Creature), &[CardType::Creature], &[], 0, 0)]
    #[case::a_type_taken_away(Modification::RemoveType(CardType::Creature), &[], &[], 0, 0)]
    #[case::a_type_it_does_not_have(Modification::RemoveType(CardType::Land), &[CardType::Creature], &[], 0, 0)]
    #[case::a_subtype(Modification::AddSubtype(Subtype("Elf".into())), &[CardType::Creature], &["Elf"], 0, 0)]
    #[case::creature_types(Modification::SetCreatureTypes(vec![Subtype("Weird".into())]), &[CardType::Creature], &["Weird"], 0, 0)]
    #[case::colours(Modification::SetColors(ColorSet::EMPTY.with(mtg_core::color::Color::Blue)), &[CardType::Creature], &[], 1, 0)]
    #[case::control(Modification::SetController(PlayerId(1)), &[CardType::Creature], &[], 0, 1)]
    fn a_modification_changes_what_its_layer_says(
        #[case] modification: Modification,
        #[case] types: &[CardType],
        #[case] subtypes: &[&str],
        #[case] colors: usize,
        #[case] controller: u64,
    ) {
        let (chars, who) = on_a_bear(modification);
        let held: Vec<CardType> = [
            CardType::Artifact,
            CardType::Creature,
            CardType::Land,
            CardType::Enchantment,
        ]
        .into_iter()
        .filter(|ty| chars.types.contains(*ty))
        .collect();
        let mut want = types.to_vec();
        want.sort();
        assert_eq!(held, want);
        let added: Vec<&str> = chars.subtypes.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(added, subtypes);
        assert_eq!(usize::from(chars.colors.iter().count() > 0), colors);
        assert_eq!(who, PlayerId(controller));
    }

    #[rstest]
    #[case::cant_attack(Modification::CantAttack)]
    #[case::cant_block(Modification::CantBlock)]
    #[case::a_keyword_granted(Modification::AddKeyword(mtg_cards::keyword::Keyword::Flying))]
    #[case::a_keyword_taken_away(Modification::RemoveKeyword(mtg_cards::keyword::Keyword::Flying))]
    fn a_restriction_and_the_keyword_modifications_change_no_characteristic(
        #[case] modification: Modification,
    ) {
        let (chars, who) = on_a_bear(modification);
        let base = characteristics(&game(), named(&game(), "Bear").id).unwrap();
        assert_eq!((chars, who), (base, PlayerId(0)));
    }

    #[test]
    fn a_restriction_comes_after_every_layer_in_the_walk() {
        let mut game = game();
        let bear = named(&game, "Bear");
        for (n, (modification, timestamp)) in [
            (Modification::CantAttack, 0),
            (Modification::SwitchPowerToughness, 1),
            (Modification::AddType(CardType::Artifact), 2),
        ]
        .into_iter()
        .enumerate()
        {
            game.effects.push(ContinuousEffect::new(
                EffectId(n as u64),
                Selector::Objects(vec![bear]),
                modification,
                Duration::UntilEndOfTurn,
                timestamp,
            ));
        }
        let derived = game.layers().get(bear.id).unwrap();
        let layers: Vec<Option<Layer>> = derived.applied.iter().map(|e| e.layer).collect();
        assert_eq!(
            layers,
            [Some(Layer::Type), Some(Layer::PtSwitch), None],
            "a restriction has no layer and applies last"
        );
    }
}
