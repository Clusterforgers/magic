//! Continuous effects: what they change, which objects they touch and how long they last.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
pub use mtg_cards::modify::Duration;
use mtg_core::card::{CardType, CardTypes, Subtype};
use mtg_core::color::ColorSet;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::characteristics::Characteristics;
use crate::object::{GameObject, ObjectRef};
use crate::state::GameState;
use crate::turn::Step;

/// A layer of the continuous-effect system (CR 613.1); effects apply in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Layer {
    /// Layer 1: copy effects.
    Copy,
    /// Layer 2: control-changing effects.
    Control,
    /// Layer 3: text-changing effects.
    Text,
    /// Layer 4: type-changing effects.
    Type,
    /// Layer 5: color-changing effects.
    Color,
    /// Layer 6: ability adding and removing effects.
    Ability,
    /// Layer 7a: characteristic-defining power and toughness.
    PtCharacteristicDefining,
    /// Layer 7b: effects that set power and toughness.
    PtSet,
    /// Layer 7c: effects that modify power and toughness (and counters).
    PtModify,
    /// Layer 7d: effects that switch power and toughness.
    PtSwitch,
}

/// Which objects an effect applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Selector {
    /// Exactly these objects, named when the effect began (CR 611.2c). One that has changed zones since is a new
    /// object (CR 400.7) and is not among them.
    Objects(Vec<ObjectRef>),
    /// Every creature, judged by the characteristics built up to the effect's layer.
    AllCreatures,
    /// Every creature a player controls.
    CreaturesControlledBy(PlayerId),
}

impl Selector {
    /// The objects with these ids as they are now; an id no object answers to is left out.
    pub fn objects(state: &GameState, ids: impl IntoIterator<Item = ObjectId>) -> Selector {
        Selector::Objects(
            ids.into_iter()
                .filter_map(|id| state.object(id).map(GameObject::reference))
                .collect(),
        )
    }

    /// Whether the effect touches `object` given its characteristics and its controller so far.
    pub(crate) fn matches(
        &self,
        object: &GameObject,
        so_far: &Characteristics,
        controller: PlayerId,
    ) -> bool {
        match self {
            Selector::Objects(named) => named.contains(&object.reference()),
            Selector::AllCreatures => so_far.types.contains(CardType::Creature),
            Selector::CreaturesControlledBy(player) => {
                so_far.types.contains(CardType::Creature) && controller == *player
            }
        }
    }
}

/// What an effect changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Modification {
    /// Adds a card type (layer 4).
    AddType(CardType),
    /// Removes a card type (layer 4).
    RemoveType(CardType),
    /// Makes these the card types (layer 4); the creature types go when it is no longer a creature.
    SetTypes(CardTypes),
    /// Adds a subtype (layer 4).
    AddSubtype(Subtype),
    /// Replaces the creature types with these (layer 4); the other subtypes stay (CR 205.1b).
    SetCreatureTypes(Vec<Subtype>),
    /// Makes the object every creature type (layer 4): changeling (CR 702.73).
    AddAllCreatureTypes,
    /// Replaces the colors (layer 5).
    SetColors(ColorSet),
    /// Adds a keyword ability (layer 6); `keywords_of` reads it and `characteristics` leaves it alone.
    AddKeyword(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Keyword),
    /// Removes a keyword ability (layer 6), printed or granted before it; `keywords_of` reads it.
    RemoveKeyword(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Keyword),
    /// Gives an activated or a triggered ability (layer 6); `ability_reader` lists it under an address of its own.
    GrantAbility(#[cfg_attr(feature = "ts", ts(type = "unknown"))] Arc<Ability>),
    /// Takes away every ability the object has so far (layer 6): the printed ones, the plain mana abilities and
    /// those granted earlier. What is granted after it still counts.
    LoseAbilities,
    /// Changes the controller (layer 2).
    SetController(PlayerId),
    /// Sets power and toughness (layer 7b).
    SetPowerToughness {
        /// New power.
        power: i32,
        /// New toughness.
        toughness: i32,
    },
    /// Adds to power and toughness (layer 7c).
    ModifyPowerToughness {
        /// Power delta.
        power: i32,
        /// Toughness delta.
        toughness: i32,
    },
    /// Switches power and toughness (layer 7d).
    SwitchPowerToughness,
    /// The object can't attack: a restriction with no layer that `can_attack` reads.
    CantAttack,
    /// The object can't block: a restriction with no layer that `block_options` reads.
    CantBlock,
    /// The object can't be blocked: a restriction with no layer that `block_options` reads.
    CantBeBlocked,
    /// The object can't be blocked by the creatures the selector names, judged when blockers are declared as `controller`
    /// and `source` see them ("you", "this"): a restriction with no layer that `block_options` reads.
    CantBeBlockedBy {
        /// The blockers it keeps out.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        by: mtg_cards::refs::Selector,
        /// Who "you" is for the selector.
        controller: PlayerId,
        /// What "this" is for the selector.
        source: ObjectRef,
    },
    /// The object can block only the attackers the selector names, judged when blockers are declared as `controller`
    /// and `source` see them: a restriction with no layer that `block_options` reads.
    CanBlockOnly {
        /// The attackers it may block.
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        only: mtg_cards::refs::Selector,
        /// Who "you" is for the selector.
        controller: PlayerId,
        /// What "this" is for the selector.
        source: ObjectRef,
    },
    /// The object can't be blocked except by this many creatures together or more: a restriction with no layer that
    /// the block rules read beside menace.
    MinBlockers(u32),
    /// The object can't be blocked by more than this many creatures together: a restriction with no layer that the
    /// block rules read.
    MaxBlockers(u32),
    /// The object is goaded by `by` (CR 701.15): a restriction with no layer that the attack declaration reads; it attacks
    /// each combat if able and attacks a player other than the goader if able.
    Goaded {
        /// The goader.
        by: PlayerId,
    },
}

impl Modification {
    /// The layer this kind of modification belongs to, or `None` for a restriction, which changes no characteristic.
    pub const fn layer(&self) -> Option<Layer> {
        Some(match self {
            Modification::SetController(_) => Layer::Control,
            Modification::AddType(_)
            | Modification::RemoveType(_)
            | Modification::SetTypes(_)
            | Modification::AddSubtype(_)
            | Modification::SetCreatureTypes(_)
            | Modification::AddAllCreatureTypes => Layer::Type,
            Modification::SetColors(_) => Layer::Color,
            Modification::AddKeyword(_)
            | Modification::RemoveKeyword(_)
            | Modification::GrantAbility(_)
            | Modification::LoseAbilities => Layer::Ability,
            Modification::SetPowerToughness { .. } => Layer::PtSet,
            Modification::ModifyPowerToughness { .. } => Layer::PtModify,
            Modification::SwitchPowerToughness => Layer::PtSwitch,
            Modification::CantAttack
            | Modification::CantBlock
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy { .. }
            | Modification::CanBlockOnly { .. }
            | Modification::MinBlockers(_)
            | Modification::MaxBlockers(_)
            | Modification::Goaded { .. } => return None,
        })
    }

    pub(crate) fn apply(&self, chars: &mut Characteristics) {
        match self {
            Modification::AddType(ty) => chars.types = chars.types.with(*ty),
            Modification::RemoveType(ty) => chars.types = without(chars.types, *ty),
            Modification::SetTypes(types) => {
                chars.types = *types;
                // A creature type needs the type it is a type of (CR 205.3d): the printed ones stop counting too.
                if !types.contains(CardType::Creature) && !types.contains(CardType::Kindred) {
                    chars.subtypes.retain(|subtype| !subtype.is_creature_type());
                    chars.all_creature_types = false;
                    chars.creature_types_set = true;
                }
            }
            Modification::AddSubtype(subtype) => {
                if !chars.subtypes.contains(subtype) {
                    chars.subtypes.push(subtype.clone());
                }
            }
            Modification::SetCreatureTypes(types) => {
                chars.subtypes.retain(|subtype| !subtype.is_creature_type());
                chars.all_creature_types = false;
                chars.creature_types_set = true;
                for subtype in types {
                    if !chars.subtypes.contains(subtype) {
                        chars.subtypes.push(subtype.clone());
                    }
                }
            }
            Modification::AddAllCreatureTypes => chars.all_creature_types = true,
            Modification::SetColors(colors) => chars.colors = *colors,
            Modification::LoseAbilities => chars.abilities.clear(),
            Modification::SetPowerToughness { power, toughness } => {
                chars.power = Some(*power);
                chars.toughness = Some(*toughness);
            }
            Modification::ModifyPowerToughness { power, toughness } => {
                chars.power = chars.power.map(|p| p + power);
                chars.toughness = chars.toughness.map(|t| t + toughness);
            }
            Modification::SwitchPowerToughness => {
                std::mem::swap(&mut chars.power, &mut chars.toughness);
            }
            // Abilities are not part of `Characteristics` but for the plain mana abilities: `keywords_of` and
            // `ability_reader` read them off the order the effects applied in. The attack and block rules read the
            // restrictions the same way.
            // The controller is walked with the characteristics (`characteristics::walk`).
            Modification::SetController(_)
            | Modification::AddKeyword(_)
            | Modification::RemoveKeyword(_)
            | Modification::GrantAbility(_)
            | Modification::CantAttack
            | Modification::CantBlock
            | Modification::CantBeBlocked
            | Modification::CantBeBlockedBy { .. }
            | Modification::CanBlockOnly { .. }
            | Modification::MinBlockers(_)
            | Modification::MaxBlockers(_)
            | Modification::Goaded { .. } => {}
        }
    }
}

/// The card types, so one can be taken out of a set of them.
const CARD_TYPES: [CardType; 9] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Instant,
    CardType::Kindred,
    CardType::Land,
    CardType::Planeswalker,
    CardType::Sorcery,
];

/// `types` without `ty`.
pub(crate) fn without(types: CardTypes, ty: CardType) -> CardTypes {
    CARD_TYPES
        .into_iter()
        .filter(|kept| *kept != ty && types.contains(*kept))
        .collect()
}

/// Whether an effect of a spell or ability that `controller` controlled, made by `source` in turn `began`, still lasts
/// in `state` by its duration (CR 611.2): the one rule for the durations of continuous effects and of rules.
pub(crate) fn lasts(
    state: &GameState,
    duration: Duration,
    began: u32,
    controller: PlayerId,
    source: Option<ObjectRef>,
) -> bool {
    let turn = state.turn();
    match duration {
        // `UntilEndOfTurn` ends with the cleanup step, which logs the expiry (CR 514.2); `UntilEndOfYourNextTurn` ends
        // the same way, in the cleanup step of the next turn its controller has.
        Duration::UntilEndOfTurn | Duration::Forever => true,
        Duration::UntilEndOfYourNextTurn => state.is_alive(controller),
        Duration::UntilEndOfCombat => turn.number() == began && turn.step() <= Step::EndCombat,
        Duration::UntilYourNextTurn => {
            state.is_alive(controller) && !(turn.number() != began && turn.active() == controller)
        }
        Duration::WhileSourceOnBattlefield => source.is_some_and(|source| {
            state
                .object(source.id)
                .is_some_and(|o| o.counter() == source.counter && o.zone().is_permanent_zone())
        }),
        // Only an exile lasts until an opponent becomes the monarch (a card in exile waits, `exile_return.rs`); no continuous
        // effect does.
        Duration::UntilOpponentBecomesMonarch => false,
    }
}

/// Whether an effect of the given duration ends with the cleanup step of the turn that is ending (CR 514.2): "until end
/// of turn", or "until the end of your next turn" when this is the controller's first turn after the one it began in.
pub(crate) fn ends_with_turn(
    state: &GameState,
    duration: Duration,
    began: u32,
    controller: PlayerId,
) -> bool {
    let turn = state.turn();
    match duration {
        Duration::UntilEndOfTurn => true,
        Duration::UntilEndOfYourNextTurn => turn.number() > began && turn.active() == controller,
        _ => false,
    }
}

/// A continuous effect from a resolved spell or ability (CR 611): characteristics are always derived from these.
///
/// An effect that came from a spell or ability names the objects it changes when it begins (CR 611.2c) and keeps
/// its place in the timestamp order for as long as it lasts (CR 613.7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ContinuousEffect {
    /// Unique id.
    pub id: EffectId,
    /// Layer it applies in; always `modification.layer()`, and none for a restriction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub layer: Option<Layer>,
    /// Affected objects.
    pub selector: Selector,
    /// The change.
    pub modification: Modification,
    /// How long it lasts.
    #[cfg_attr(
        feature = "ts",
        ts(
            type = "\"UntilEndOfTurn\" | \"UntilEndOfCombat\" | \"UntilYourNextTurn\" | \"UntilEndOfYourNextTurn\" | \"WhileSourceOnBattlefield\" | \"Forever\""
        )
    )]
    pub duration: Duration,
    /// Order of creation; later effects apply after earlier ones within a layer.
    pub timestamp: u64,
    /// The object whose spell or ability created it, as it was then; `WhileSourceOnBattlefield` follows this object.
    pub source: Option<ObjectRef>,
    /// Who controlled that spell or ability: whose next turn `UntilYourNextTurn` waits for.
    pub controller: PlayerId,
    /// The number of the turn it began in, which `UntilEndOfCombat` and `UntilYourNextTurn` measure from.
    pub turn: u32,
}

impl ContinuousEffect {
    /// Builds an effect with no source, deriving its layer from the modification; it began in no turn and belongs to
    /// the first seat until [`begun`](Self::begun) says otherwise.
    pub fn new(
        id: EffectId,
        selector: Selector,
        modification: Modification,
        duration: Duration,
        timestamp: u64,
    ) -> Self {
        ContinuousEffect {
            id,
            layer: modification.layer(),
            selector,
            modification,
            duration,
            timestamp,
            source: None,
            controller: PlayerId(0),
            turn: 0,
        }
    }

    /// The same effect created by a spell or ability of `source`.
    #[must_use]
    pub fn from_source(mut self, source: ObjectRef) -> Self {
        self.source = Some(source);
        self
    }

    /// The same effect, begun in turn `turn` for `controller`.
    #[must_use]
    pub fn begun(mut self, controller: PlayerId, turn: u32) -> Self {
        self.controller = controller;
        self.turn = turn;
        self
    }

    /// Whether the effect still applies in `state`: its duration has not ended and an object it names is still the
    /// object it named. An effect that does not stops applying at once; the `EffectExpired` event only removes it.
    pub fn is_active(&self, state: &GameState) -> bool {
        lasts(
            state,
            self.duration,
            self.turn,
            self.controller,
            self.source,
        ) && !self.spent(state)
            && !self.gives_control_to_a_player_who_left(state)
    }

    /// Whether the effect gives control to a player who has left the game, which ends it (CR 800.4).
    fn gives_control_to_a_player_who_left(&self, state: &GameState) -> bool {
        matches!(self.modification, Modification::SetController(who) if !state.is_alive(who))
    }

    /// Whether every object the effect names has become a new object or is gone, so it can never change anything again.
    fn spent(&self, state: &GameState) -> bool {
        match &self.selector {
            Selector::Objects(named) => named
                .iter()
                .all(|r| state.object(r.id).is_none_or(|o| o.counter() != r.counter)),
            Selector::AllCreatures | Selector::CreaturesControlledBy(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an effect applies while its duration lasts and no longer, each duration ends by its own rule (end of turn by the cleanup step's event, end of combat by the step, "your next turn" by the controller's turn, "while the source stays" by that very object, for ever not at all), and an effect that names objects is spent when none of them is the object it named.
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::events::{Event, ZoneCause};
    use crate::fixtures::{creature, state};
    use crate::fold::apply_event;
    use crate::player::LossReason;
    use crate::zones::Zone;

    const ME: PlayerId = PlayerId(0);
    const FOE: PlayerId = PlayerId(1);

    fn game() -> GameState {
        state(
            vec![vec![], vec![]],
            vec![
                vec![creature("Source", 1, 1), creature("Wolf", 2, 2)],
                vec![creature("Bear", 2, 2)],
            ],
        )
    }

    fn named(game: &GameState, name: &str) -> ObjectRef {
        game.objects()
            .find(|o| o.card().name == name)
            .map(GameObject::reference)
            .unwrap()
    }

    /// An effect of `controller` that began in turn 3, on the Wolf, with the Source as its source.
    fn effect(game: &GameState, duration: Duration, controller: PlayerId) -> ContinuousEffect {
        ContinuousEffect::new(
            EffectId(0),
            Selector::Objects(vec![named(game, "Wolf")]),
            Modification::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
            duration,
            0,
        )
        .from_source(named(game, "Source"))
        .begun(controller, 3)
    }

    fn at(game: &mut GameState, number: u32, active: PlayerId, step: Step) {
        game.turn.number = number;
        game.turn.active = active;
        game.turn.step = step;
    }

    fn leaves(game: &mut GameState, object: ObjectRef) {
        let owner = game.object(object.id).unwrap().owner();
        let event = Event::ZoneChanged {
            object: object.id,
            from: Zone::Battlefield,
            to: Zone::Graveyard(owner),
            cause: ZoneCause::Moved,
        };
        apply_event(game, &event).unwrap();
    }

    #[rstest]
    #[case::end_of_turn_ends_by_event_not_by_state(Duration::UntilEndOfTurn, (3, ME, Step::End), true)]
    #[case::end_of_turn_stays_until_the_cleanup_event(Duration::UntilEndOfTurn, (4, FOE, Step::Main1), true)]
    #[case::forever(Duration::Forever, (9, FOE, Step::Main2), true)]
    #[case::combat_before_it(Duration::UntilEndOfCombat, (3, ME, Step::Main1), true)]
    #[case::combat_through_damage(Duration::UntilEndOfCombat, (3, ME, Step::CombatDamage), true)]
    #[case::combat_to_its_last_step(Duration::UntilEndOfCombat, (3, ME, Step::EndCombat), true)]
    #[case::combat_after_it(Duration::UntilEndOfCombat, (3, ME, Step::Main2), false)]
    #[case::combat_of_another_turn(Duration::UntilEndOfCombat, (4, FOE, Step::Main1), false)]
    #[case::your_turn_still_going(Duration::UntilYourNextTurn, (3, ME, Step::End), true)]
    #[case::your_turn_when_another_seat_plays(Duration::UntilYourNextTurn, (4, FOE, Step::Main1), true)]
    #[case::your_turn_when_it_comes_again(Duration::UntilYourNextTurn, (5, ME, Step::Upkeep), false)]
    #[case::source_stays(Duration::WhileSourceOnBattlefield, (3, ME, Step::Main1), true)]
    #[case::the_end_of_your_next_turn_in_the_turn_it_began(Duration::UntilEndOfYourNextTurn, (3, ME, Step::End), true)]
    #[case::the_end_of_your_next_turn_in_another_seats_turn(Duration::UntilEndOfYourNextTurn, (4, FOE, Step::Main1), true)]
    #[case::the_end_of_your_next_turn_in_that_turn(Duration::UntilEndOfYourNextTurn, (5, ME, Step::Main2), true)]
    fn each_duration_lasts_as_long_as_its_rule_says(
        #[case] duration: Duration,
        #[case] now: (u32, PlayerId, Step),
        #[case] active: bool,
    ) {
        let mut game = game();
        let effect = effect(&game, duration, ME);
        at(&mut game, now.0, now.1, now.2);
        assert_eq!(effect.is_active(&game), active);
    }

    #[rstest]
    #[case::end_of_turn_in_the_turn(Duration::UntilEndOfTurn, (3, ME), true)]
    #[case::end_of_turn_in_a_later_turn(Duration::UntilEndOfTurn, (4, FOE), true)]
    #[case::forever(Duration::Forever, (3, ME), false)]
    #[case::your_next_turn_begins_ends_nothing_at_cleanup(Duration::UntilYourNextTurn, (5, ME), false)]
    #[case::the_turn_it_began_in(Duration::UntilEndOfYourNextTurn, (3, ME), false)]
    #[case::another_seats_turn(Duration::UntilEndOfYourNextTurn, (4, FOE), false)]
    #[case::your_next_turn(Duration::UntilEndOfYourNextTurn, (5, ME), true)]
    #[case::a_later_turn_of_yours(Duration::UntilEndOfYourNextTurn, (7, ME), true)]
    #[case::the_turn_after_yours(Duration::UntilEndOfYourNextTurn, (6, FOE), false)]
    fn the_cleanup_of_a_turn_ends_the_durations_that_end_with_it(
        #[case] duration: Duration,
        #[case] now: (u32, PlayerId),
        #[case] ends: bool,
    ) {
        let mut game = game();
        at(&mut game, now.0, now.1, Step::Cleanup);
        assert_eq!(ends_with_turn(&game, duration, 3, ME), ends);
    }

    #[test]
    fn an_effect_waiting_for_the_end_of_the_next_turn_of_a_player_who_left_is_over() {
        let mut game = game();
        let effect = effect(&game, Duration::UntilEndOfYourNextTurn, FOE);
        at(&mut game, 3, ME, Step::Main1);
        assert!(effect.is_active(&game));
        let lost = Event::PlayerLost {
            player: FOE,
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &lost).unwrap();
        assert!(!effect.is_active(&game));
    }

    #[test]
    fn an_effect_waiting_for_the_next_turn_of_a_player_who_left_is_over() {
        let mut game = game();
        let effect = effect(&game, Duration::UntilYourNextTurn, FOE);
        at(&mut game, 3, ME, Step::Main1);
        assert!(effect.is_active(&game));
        let lost = Event::PlayerLost {
            player: FOE,
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &lost).unwrap();
        assert!(!effect.is_active(&game));
    }

    #[rstest]
    #[case::control_to_a_player_who_left(Modification::SetController(FOE), false)]
    #[case::control_to_a_player_in_the_game(Modification::SetController(ME), true)]
    #[case::another_change_stays(Modification::SwitchPowerToughness, true)]
    fn an_effect_that_gives_control_to_a_player_who_left_ends(
        #[case] modification: Modification,
        #[case] active: bool,
    ) {
        let mut game = game();
        let mut effect = effect(&game, Duration::Forever, ME);
        effect.modification = modification;
        let lost = Event::PlayerLost {
            player: FOE,
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &lost).unwrap();
        assert_eq!(effect.is_active(&game), active);
    }

    #[rstest]
    #[case::stays_away(false)]
    #[case::returns_as_a_new_object(true)]
    fn an_effect_that_follows_its_source_ends_when_the_source_leaves(#[case] returns: bool) {
        let mut game = game();
        let effect = effect(&game, Duration::WhileSourceOnBattlefield, ME);
        let source = named(&game, "Source");
        leaves(&mut game, source);
        assert!(!effect.is_active(&game));
        if returns {
            let back = Event::ZoneChanged {
                object: source.id,
                from: Zone::Graveyard(ME),
                to: Zone::Battlefield,
                cause: ZoneCause::Moved,
            };
            apply_event(&mut game, &back).unwrap();
            assert_eq!(game.object(source.id).unwrap().zone(), Zone::Battlefield);
            assert!(
                !effect.is_active(&game),
                "the permanent that came back is a new object (CR 400.7)"
            );
        }
    }

    #[test]
    fn an_effect_without_a_source_does_not_follow_one() {
        let game = game();
        let mut effect = effect(&game, Duration::WhileSourceOnBattlefield, ME);
        effect.source = None;
        assert!(!effect.is_active(&game));
    }

    #[rstest]
    #[case::the_object_is_still_there(&[false], true)]
    #[case::it_changed_zones(&[true], false)]
    #[case::one_of_two_is_still_there(&[true, false], true)]
    #[case::both_changed_zones(&[true, true], false)]
    fn an_effect_is_spent_when_no_object_it_named_is_the_object_it_named(
        #[case] moved: &[bool],
        #[case] active: bool,
    ) {
        let mut game = game();
        let named_by_it = [named(&game, "Wolf"), named(&game, "Bear")];
        let effect = ContinuousEffect::new(
            EffectId(0),
            Selector::Objects(named_by_it[..moved.len()].to_vec()),
            Modification::ModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
            Duration::Forever,
            0,
        );
        for (object, gone) in named_by_it.iter().zip(moved) {
            if *gone {
                leaves(&mut game, *object);
            }
        }
        assert_eq!(effect.is_active(&game), active);
    }

    #[test]
    fn a_selector_names_the_incarnation_it_was_given() {
        let mut game = game();
        let wolf = named(&game, "Wolf");
        let selector = Selector::objects(&game, [wolf.id, ObjectId(u64::MAX)]);
        assert_eq!(selector, Selector::Objects(vec![wolf]));
        let chars = crate::characteristics(&game, wolf.id).unwrap();
        assert!(selector.matches(game.object(wolf.id).unwrap(), &chars, ME));
        game.objects.get_mut(&wolf.id).unwrap().counter += 1;
        assert!(!selector.matches(game.object(wolf.id).unwrap(), &chars, ME));
    }

    #[test]
    fn folding_an_effect_advances_the_counters_and_expiring_it_removes_it() {
        let mut game = game();
        let mut later = effect(&game, Duration::UntilEndOfTurn, ME);
        later.id = EffectId(6);
        later.timestamp = 20;
        apply_event(&mut game, &Event::EffectCreated { effect: later }).unwrap();
        assert_eq!((game.next_effect, game.next_timestamp), (7, 21));
        let earlier = effect(&game, Duration::UntilEndOfTurn, ME);
        apply_event(&mut game, &Event::EffectCreated { effect: earlier }).unwrap();
        assert_eq!(
            (game.next_effect, game.next_timestamp),
            (7, 21),
            "an older effect does not move the counters back"
        );
        assert_eq!(game.effects().len(), 2);
        apply_event(&mut game, &Event::EffectExpired { id: EffectId(6) }).unwrap();
        assert_eq!(game.effects().len(), 1);
        assert!(apply_event(&mut game, &Event::EffectExpired { id: EffectId(6) }).is_err());
    }

    #[test]
    fn an_effect_that_cannot_follow_from_the_state_is_refused() {
        let mut game = game();
        let made = effect(&game, Duration::UntilEndOfTurn, ME);
        apply_event(
            &mut game,
            &Event::EffectCreated {
                effect: made.clone(),
            },
        )
        .unwrap();
        let hash = game.state_hash();
        let mut wrong_layer = made.clone();
        wrong_layer.id = EffectId(1);
        wrong_layer.layer = Some(Layer::Type);
        let mut same_id = made.clone();
        same_id.timestamp = 5;
        let mut no_source = effect(&game, Duration::WhileSourceOnBattlefield, ME);
        no_source.id = EffectId(2);
        no_source.source = None;
        for refused in [wrong_layer, same_id, no_source] {
            assert!(apply_event(&mut game, &Event::EffectCreated { effect: refused }).is_err());
        }
        assert_eq!(game.state_hash(), hash, "a refused event changes nothing");
    }
}
