//! Derived characteristics: base values from the card plus continuous effects, layer by layer.

use mtg_cards::AbilityDef;
use mtg_core::card::{CardType, CardTypes, Subtype};
use mtg_core::color::ColorSet;
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::abilities::Grant;
use crate::effects::{ContinuousEffect, Layer, Modification, Selector};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::statics::StaticEffect;
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

    /// What nothing has: no types, colors, power, toughness or abilities.
    fn empty() -> Characteristics {
        Characteristics {
            types: CardTypes::default(),
            subtypes: Vec::new(),
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
/// Effects apply layer by layer, and by timestamp within a layer; dependencies (CR 613.8) are not modeled yet. Selectors
/// are evaluated against the characteristics built up to their own layer. Only objects on the battlefield are
/// affected by effects. An ability on the stack has none.
pub fn characteristics(state: &GameState, id: ObjectId) -> Option<Characteristics> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability {
        return Some(Characteristics::empty());
    }
    Some(derive(state, object).chars)
}

/// The characteristics and the controller of `id` from one walk of the layers: what the value evaluator and the snapshots
/// read an object as.
pub(crate) fn seen_as(state: &GameState, id: ObjectId) -> Option<(Characteristics, PlayerId)> {
    let object = state.object(id)?;
    if object.kind() == ObjectKind::Ability {
        return Some((Characteristics::empty(), object.controller()));
    }
    let derived = derive(state, object);
    Some((derived.chars, derived.controller))
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
    // Most games change no control, and this is asked for every permanent on every decision.
    if object.zone() != Zone::Battlefield || (state.effects().is_empty() && !state.statics_possible)
    {
        return object.controller();
    }
    derive_to(state, object, Layer::Control).controller
}

/// Whether an effect that applies to `id` says it can't attack: the one place the attack rules read this restriction.
pub(crate) fn cant_attack(state: &GameState, id: ObjectId) -> bool {
    restricted(state, id, |modification| {
        matches!(modification, Modification::CantAttack)
    })
}

/// Whether an effect that applies to `id` says it can't block: the one place the block rules read this restriction.
pub(crate) fn cant_block(state: &GameState, id: ObjectId) -> bool {
    restricted(state, id, |modification| {
        matches!(modification, Modification::CantBlock)
    })
}

/// Whether a modification that `is` applies to the permanent `id` once the layers are through with it.
fn restricted(state: &GameState, id: ObjectId, is: impl Fn(&Modification) -> bool) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    if object.zone() != Zone::Battlefield
        || (state.effects().is_empty() && state.statics().is_empty())
    {
        return false;
    }
    derive(state, object)
        .applied
        .iter()
        .any(|applied| is(applied.modification))
}

/// One effect that changed an object, in the order the layers applied them.
#[derive(Debug, Clone, Copy)]
// `layer` and `timestamp` are what the dependency pass of the derivation reads; the tests read them today.
#[allow(dead_code)]
pub(crate) struct Applied<'a> {
    /// What the effect is: a resolved spell or ability's, or one modification of a static ability.
    pub(crate) origin: Grant,
    /// The layer it applies in, or none for a restriction.
    pub(crate) layer: Option<Layer>,
    /// Its place in the order of the layer.
    pub(crate) timestamp: u64,
    /// The change.
    pub(crate) modification: &'a Modification,
}

/// What the layers made of an object: its characteristics, its controller and the effects that changed it.
#[derive(Debug, Clone)]
pub(crate) struct Derived<'a> {
    pub(crate) chars: Characteristics,
    pub(crate) controller: PlayerId,
    pub(crate) applied: Vec<Applied<'a>>,
}

/// An effect waiting its turn in the walk: a registered one or a static ability's.
struct Candidate<'a> {
    origin: Grant,
    layer: Option<Layer>,
    timestamp: u64,
    chosen: Chosen<'a>,
    modification: &'a Modification,
}

/// How a candidate says which objects it applies to.
enum Chosen<'a> {
    Registered(&'a Selector),
    Static(&'a StaticEffect),
}

impl<'a> Candidate<'a> {
    fn registered(effect: &'a ContinuousEffect) -> Self {
        Candidate {
            origin: Grant::Effect(effect.id),
            layer: effect.layer,
            timestamp: effect.timestamp,
            chosen: Chosen::Registered(&effect.selector),
            modification: &effect.modification,
        }
    }

    fn of_static(effect: &'a StaticEffect) -> Self {
        Candidate {
            origin: effect.grant,
            layer: effect.layer,
            timestamp: effect.timestamp,
            chosen: Chosen::Static(effect),
            modification: &effect.modification,
        }
    }

    /// The order the walk applies effects in: layer (a restriction after them all), timestamp, then registered
    /// effects by id before static ones by source, ability and modification.
    fn key(&self) -> (bool, Option<Layer>, u64, (u8, u64, u64)) {
        let tie = match self.origin {
            Grant::Effect(id) => (0, id.0, 0),
            Grant::Static {
                source,
                rule,
                index,
            } => (1, source.get(), (rule as u64) << 32 | index as u64),
        };
        (self.layer.is_none(), self.layer, self.timestamp, tie)
    }

    fn affects(
        &self,
        state: &GameState,
        object: &GameObject,
        so_far: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        match self.chosen {
            Chosen::Registered(selector) => selector.matches(object, so_far, controller),
            Chosen::Static(effect) => effect.affects(state, object, so_far, controller),
        }
    }
}

/// The characteristics of a card or token on its own, before any effect.
fn base(object: &GameObject) -> Characteristics {
    let card = object.card();
    Characteristics {
        types: object.base_types(),
        subtypes: Vec::new(),
        colors: card.colors(),
        power: card.power,
        toughness: card.toughness,
        abilities: card.abilities.clone(),
    }
}

/// The characteristics of a card or token, its controller and the effects that changed them, in the order they applied.
///
/// This is the one place the layers are walked: `characteristics` keeps the first part, `controller_of` the second and
/// `keywords_of`, `ability_reader` and the attack and block rules the third, so nothing reads an effect another way.
pub(crate) fn derive<'a>(state: &'a GameState, object: &'a GameObject) -> Derived<'a> {
    walk(state, object, None, state.statics())
}

/// The same walk, stopped after `until`: what the layers up to it made of the object.
pub(crate) fn derive_to<'a>(
    state: &'a GameState,
    object: &'a GameObject,
    until: Layer,
) -> Derived<'a> {
    walk(state, object, Some(until), state.statics())
}

/// The walk, with the static effects to apply given: registered effects and `statics` in order of layer, timestamp
/// and id, each applying to the object when its selector matches the characteristics built so far.
pub(crate) fn walk<'a>(
    state: &'a GameState,
    object: &'a GameObject,
    until: Option<Layer>,
    statics: &'a [StaticEffect],
) -> Derived<'a> {
    let mut chars = base(object);
    let mut controller = object.controller();
    let mut applied = Vec::new();
    if object.zone() != Zone::Battlefield || (state.effects().is_empty() && statics.is_empty()) {
        return Derived {
            chars,
            controller,
            applied,
        };
    }
    let mut candidates: Vec<Candidate<'a>> = state
        .effects()
        .iter()
        .filter(|effect| effect.is_active(state))
        .map(Candidate::registered)
        .chain(statics.iter().map(Candidate::of_static))
        .collect();
    candidates.sort_unstable_by_key(Candidate::key);
    for candidate in candidates {
        // A restriction has no layer and comes after every layer, so the walk is over at the first one past `until`.
        if let Some(limit) = until
            && candidate.layer.is_none_or(|layer| layer > limit)
        {
            break;
        }
        if candidate.affects(state, object, &chars, controller) {
            candidate.modification.apply(&mut chars);
            if let Modification::SetController(player) = candidate.modification {
                controller = *player;
            }
            applied.push(Applied {
                origin: candidate.origin,
                layer: candidate.layer,
                timestamp: candidate.timestamp,
                modification: candidate.modification,
            });
        }
    }
    Derived {
        chars,
        controller,
        applied,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: characteristics are the printed values with the active effects applied layer by layer and by timestamp within a layer, whatever order the effects are stored in; only permanents change, only the objects an effect names, and an effect that is over changes nothing; every modification changes what its layer says, in the order of the layers (a type before a colour, a set before a pump before a switch), and a restriction changes no characteristic.
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
        let derived = derive(&game, game.object(named(&game, "Bear").id).unwrap());
        let order: Vec<(Option<Layer>, u64)> = derived
            .applied
            .iter()
            .map(|e| (e.layer, e.timestamp))
            .collect();
        assert!(order.windows(2).all(|w| w[0].0 <= w[1].0), "{order:?}");
        assert_eq!(derived.applied.len(), changes.len());
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
        let object = game.object(bear.id).unwrap();
        let derived = derive(&game, object);
        (derived.chars, derived.controller)
    }

    #[rstest]
    #[case::a_type(Modification::AddType(CardType::Artifact), &[CardType::Artifact, CardType::Creature], &[], 0, 0)]
    #[case::a_type_again_changes_nothing(Modification::AddType(CardType::Creature), &[CardType::Creature], &[], 0, 0)]
    #[case::a_type_taken_away(Modification::RemoveType(CardType::Creature), &[], &[], 0, 0)]
    #[case::a_type_it_does_not_have(Modification::RemoveType(CardType::Land), &[CardType::Creature], &[], 0, 0)]
    #[case::a_subtype(Modification::AddSubtype(Subtype("Elf".into())), &[CardType::Creature], &["Elf"], 0, 0)]
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
        let derived = derive(&game, game.object(bear.id).unwrap());
        let layers: Vec<Option<Layer>> = derived.applied.iter().map(|e| e.layer).collect();
        assert_eq!(
            layers,
            [Some(Layer::Type), Some(Layer::PtSwitch), None],
            "a restriction has no layer and applies last"
        );
        let until_abilities = derive_to(&game, game.object(bear.id).unwrap(), Layer::Ability);
        assert_eq!(
            until_abilities.applied.len(),
            1,
            "the walk stops after the layer asked for"
        );
    }
}
