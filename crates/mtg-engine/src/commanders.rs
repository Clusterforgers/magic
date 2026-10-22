//! Commanders (CR 903): which objects are commanders, how often each was cast from the command zone (the tax of CR
//! 903.8), the combat damage each has dealt to each player (CR 903.10a), and the owner's choice to put a commander that
//! is in a graveyard or exile into the command zone (CR 903.9a). The replacement of a move to a hand or a library
//! (CR 903.9b) is in `replace.rs`.
//!
//! A commander is a card, so all of it is kept by object id, which a zone change leaves as it is. A token and a copy
//! of a commander are other objects and so no commanders. The tallies are folded from events and hashed only once they
//! hold something.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use mtg_core::ids::{ObjectId, PlayerId};

use crate::answers::Outcome;
use crate::choose::{YesNoChoice, YesNoOptions};
use crate::decision::{Decision, DecisionKind};
use crate::error::EngineError;
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The commanders of a game and what is counted for them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Commanders {
    /// The commander cards, fixed when the game is set up.
    ids: BTreeSet<ObjectId>,
    /// Times each was cast from the command zone.
    casts: BTreeMap<ObjectId, u32>,
    /// Combat damage each has dealt to each player.
    damage: BTreeMap<(ObjectId, PlayerId), u32>,
    /// The commanders their owner left in a graveyard or in exile when asked, until they move again.
    stays: BTreeSet<ObjectId>,
}

impl Hash for Commanders {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.ids.is_empty() {
            return;
        }
        self.ids.hash(state);
        if !self.casts.is_empty() {
            1u8.hash(state);
            self.casts.hash(state);
        }
        if !self.damage.is_empty() {
            2u8.hash(state);
            self.damage.hash(state);
        }
        if !self.stays.is_empty() {
            3u8.hash(state);
            self.stays.hash(state);
        }
    }
}

impl Commanders {
    /// The commanders of a game set up with these cards.
    pub(crate) fn of(ids: impl IntoIterator<Item = ObjectId>) -> Commanders {
        Commanders {
            ids: ids.into_iter().collect(),
            ..Commanders::default()
        }
    }

    /// Whether the game has a commander.
    pub(crate) fn any(&self) -> bool {
        !self.ids.is_empty()
    }

    /// Whether `id` is a commander.
    pub(crate) fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    /// The commanders in id order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.ids.iter().copied()
    }

    /// A commander moved: its owner is asked again the next time it lies in a graveyard or exile.
    pub(crate) fn moved(&mut self, id: ObjectId) {
        self.stays.remove(&id);
    }

    /// A card was cast from `from`: from the command zone it counts for the tax.
    pub(crate) fn cast(&mut self, id: ObjectId, from: Zone) {
        if matches!(from, Zone::Command(_)) && self.ids.contains(&id) {
            let count = self.casts.entry(id).or_insert(0);
            *count = count.saturating_add(1);
        }
    }

    /// Combat damage `amount` from `source` reached `player`.
    pub(crate) fn dealt(&mut self, source: ObjectId, player: PlayerId, amount: u32) {
        if self.ids.contains(&source) && amount > 0 {
            let total = self.damage.entry((source, player)).or_insert(0);
            *total = total.saturating_add(amount);
        }
    }

    /// The owner of `id` left it in a graveyard or exile.
    pub(crate) fn stay(&mut self, id: ObjectId) {
        if self.ids.contains(&id) {
            self.stays.insert(id);
        }
    }
}

impl GameState {
    /// Whether `id` is a player's commander.
    pub fn is_commander(&self, id: ObjectId) -> bool {
        self.commanders.contains(id)
    }

    /// Every commander of the game, in object order.
    pub fn commanders(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.commanders.ids()
    }

    /// How many times the commander `id` was cast from the command zone this game.
    pub fn commander_casts(&self, id: ObjectId) -> u32 {
        self.commanders.casts.get(&id).copied().unwrap_or(0)
    }

    /// The generic mana the commander `id` costs more to cast from the command zone now (CR 903.8).
    pub fn commander_tax(&self, id: ObjectId) -> u32 {
        self.commander_casts(id)
            .saturating_mul(self.ruleset().commander_tax())
    }

    /// The combat damage the commander `id` has dealt to `player` this game.
    pub fn commander_damage(&self, id: ObjectId, player: PlayerId) -> u32 {
        self.commanders
            .damage
            .get(&(id, player))
            .copied()
            .unwrap_or(0)
    }
}

/// The most combat damage any one commander has dealt to `player`.
pub(crate) fn damage_dealt_to(state: &GameState, player: PlayerId) -> u32 {
    state
        .commanders
        .damage
        .iter()
        .filter(|((_, hit), _)| *hit == player)
        .map(|(_, amount)| *amount)
        .max()
        .unwrap_or(0)
}

/// The generic mana the spell `spell` costs more because it is a commander cast from the command zone.
pub(crate) fn tax_for(state: &GameState, spell: ObjectId) -> u32 {
    let in_command = state
        .object(spell)
        .is_some_and(|card| matches!(card.zone(), Zone::Command(_)));
    if in_command {
        state.commander_tax(spell)
    } else {
        0
    }
}

/// The commander in a graveyard or exile that its owner has not been asked about yet, owners in turn order from the
/// active player (CR 903.9a).
fn waiting(state: &GameState) -> Option<ObjectId> {
    if !state.commanders.any() {
        return None;
    }
    let order = state.turn_order_from(state.turn().active());
    let place = |id: ObjectId| {
        let owner = state.object(id)?.owner();
        order.iter().position(|seat| *seat == owner)
    };
    state
        .commanders
        .ids()
        .filter(|id| !state.commanders.stays.contains(id))
        .filter(|id| {
            state.object(*id).is_some_and(|card| {
                matches!(card.zone(), Zone::Graveyard(_) | Zone::Exile)
                    && state.is_alive(card.owner())
            })
        })
        .min_by_key(|id| (place(*id), *id))
}

/// The question the owner of a commander in a graveyard or exile is asked.
pub(crate) fn decision(state: &GameState) -> Option<Decision> {
    let id = waiting(state)?;
    let card = state.object(id)?;
    Some(Decision {
        player: card.owner(),
        kind: DecisionKind::YesNo(YesNoOptions::new(
            id,
            format!("Put {} into the command zone?", card.card().name),
        )),
    })
}

/// Whether a yes-or-no question of the state-based actions is the one about a commander.
pub(crate) fn asks(state: &GameState, options: &YesNoOptions) -> bool {
    waiting(state) == Some(options.source)
}

/// The answer: yes puts the commander into the command zone, no leaves it where it is until it moves again.
pub(crate) fn answer(
    state: &GameState,
    options: &YesNoOptions,
    choice: YesNoChoice,
) -> Result<Outcome, EngineError> {
    let Some(card) = state.object(options.source) else {
        return Ok(Outcome::Events(Vec::new()));
    };
    let event = if choice.0 {
        Event::ZoneChanged {
            object: card.id(),
            from: card.zone(),
            to: Zone::Command(card.owner()),
            cause: ZoneCause::Moved,
        }
    } else {
        Event::CommanderStayed { object: card.id() }
    };
    Ok(Outcome::Events(vec![event]))
}

#[cfg(test)]
mod tests {
    //! Goal: commander damage is counted per commander and per player from combat damage only and survives the commander's zone changes, the tax counts casts from the command zone only, the owner of a commander in a graveyard or exile is asked once for each time it got there and a refusal is remembered until it moves, and a game with commanders hashes what is counted only once it is nonzero.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::card::{CardType, Supertype};
    use rstest::rstest;

    use super::*;
    use crate::combat::DamageTarget;
    use crate::fixtures::{creature, setup};
    use crate::fold::apply_event;
    use crate::format::Commander;
    use crate::player::LossReason;
    use crate::turn::Step;

    fn legend(name: &str) -> Arc<CardDefinition> {
        Arc::new(
            CardDefinition::new(name)
                .with_type(CardType::Creature)
                .with_supertype(Supertype::Legendary)
                .with_power_toughness(3, 3),
        )
    }

    /// A game of `seats` seats at the first main step where seat `i` has the commanders `commanders[i]`; ids of the
    /// commanders are read back with `ids`.
    fn game(seats: usize, commanders: &[&[&str]]) -> GameState {
        let mut config = setup(vec![Vec::new(); seats], vec![Vec::new(); seats]);
        config.format = Arc::new(Commander);
        for (player, names) in config.players.iter_mut().zip(commanders) {
            player.commanders = names.iter().map(|name| legend(name)).collect();
        }
        GameState::from_setup(&config).unwrap()
    }

    fn ids(game: &GameState) -> Vec<ObjectId> {
        game.commanders().collect()
    }

    fn hit(game: &mut GameState, source: ObjectId, seat: usize, amount: u32) {
        let event = Event::DamageDealt {
            source,
            target: DamageTarget::Player(PlayerId::from_index(seat)),
            amount,
            deathtouch: false,
        };
        apply_event(game, &event).unwrap();
    }

    fn lost(game: &GameState, seat: usize) -> bool {
        crate::sba::check(game).iter().any(|event| {
            matches!(event, Event::PlayerLost { player, reason: LossReason::CommanderDamage } if player.index() == seat)
        })
    }

    #[rstest]
    #[case::twenty_is_not_enough(&[(0, 1, 20)], false)]
    #[case::twenty_one_is(&[(0, 1, 21)], true)]
    #[case::over_two_hits(&[(0, 1, 11), (0, 1, 10)], true)]
    #[case::under_two_hits(&[(0, 1, 10), (0, 1, 10)], false)]
    #[case::two_commanders_of_two_seats_are_counted_apart(&[(0, 2, 12), (1, 2, 12)], false)]
    #[case::partners_are_counted_apart(&[(0, 2, 12), (1, 2, 12), (0, 2, 8)], false)]
    #[case::one_partner_alone_reaches_it(&[(0, 2, 12), (1, 2, 12), (1, 2, 9)], true)]
    #[case::each_player_has_a_count(&[(0, 1, 20), (0, 2, 20)], false)]
    fn twenty_one_combat_damage_from_one_commander_loses(
        #[case] hits: &[(usize, usize, u32)],
        #[case] loses: bool,
    ) {
        let mut game = game(3, &[&["A", "B"], &["C"], &[]]);
        game.turn.step = Step::CombatDamage;
        let commanders = ids(&game);
        for (commander, seat, amount) in hits {
            hit(&mut game, commanders[*commander], *seat, *amount);
        }
        let target = hits.last().map_or(1, |(_, seat, _)| *seat);
        assert_eq!(lost(&game, target), loses);
    }

    #[rstest]
    #[case::main_phase(Step::Main1)]
    #[case::end_step(Step::End)]
    fn damage_outside_the_combat_damage_steps_is_not_commander_damage(#[case] step: Step) {
        let mut game = game(2, &[&["A"], &[]]);
        game.turn.step = step;
        let commander = ids(&game)[0];
        hit(&mut game, commander, 1, 30);
        assert_eq!(game.commander_damage(commander, PlayerId(1)), 0);
        assert!(!lost(&game, 1));
        assert_eq!(game.player(PlayerId(1)).unwrap().life(), 10);
    }

    #[test]
    fn damage_from_other_creatures_is_not_commander_damage() {
        let mut game = game(2, &[&["A"], &[]]);
        game.turn.step = Step::CombatDamage;
        let commander = ids(&game)[0];
        let other = ObjectId(0);
        hit(&mut game, other, 1, 30);
        assert_eq!(game.commander_damage(commander, PlayerId(1)), 0);
        assert_eq!(game.commander_damage(other, PlayerId(1)), 0);
    }

    #[test]
    fn the_count_survives_the_commander_changing_zones() {
        let mut game = game(2, &[&["A"], &[]]);
        game.turn.step = Step::CombatDamage;
        let commander = ids(&game)[0];
        hit(&mut game, commander, 1, 11);
        for (from, to) in [
            (Zone::Command(PlayerId(0)), Zone::Graveyard(PlayerId(0))),
            (Zone::Graveyard(PlayerId(0)), Zone::Exile),
            (Zone::Exile, Zone::Command(PlayerId(0))),
        ] {
            let moved = Event::ZoneChanged {
                object: commander,
                from,
                to,
                cause: ZoneCause::Moved,
            };
            apply_event(&mut game, &moved).unwrap();
        }
        hit(&mut game, commander, 1, 10);
        assert_eq!(game.commander_damage(commander, PlayerId(1)), 21);
        assert!(lost(&game, 1));
    }

    #[test]
    fn only_casts_from_the_command_zone_raise_the_tax() {
        let mut game = game(2, &[&["A"], &[]]);
        let commander = ids(&game)[0];
        let cast = Event::SpellCast {
            player: PlayerId(0),
            object: commander,
            choices: crate::choose::Choices::default(),
        };
        assert_eq!(
            (game.commander_tax(commander), tax_for(&game, commander)),
            (0, 0)
        );
        apply_event(&mut game, &cast).unwrap();
        // On the stack the commander owes nothing more, and it is counted once.
        assert_eq!(
            (game.commander_casts(commander), tax_for(&game, commander)),
            (1, 0)
        );
        assert_eq!(game.commander_tax(commander), 2);
        for (from, to) in [
            (Zone::Stack, Zone::Hand(PlayerId(0))),
            (Zone::Hand(PlayerId(0)), Zone::Stack),
        ] {
            let moved = Event::ZoneChanged {
                object: commander,
                from,
                to,
                cause: ZoneCause::Moved,
            };
            apply_event(&mut game, &moved).unwrap();
        }
        assert_eq!(
            game.commander_casts(commander),
            1,
            "a cast from a hand is not counted"
        );
        let back = Event::ZoneChanged {
            object: commander,
            from: Zone::Stack,
            to: Zone::Command(PlayerId(0)),
            cause: ZoneCause::Moved,
        };
        apply_event(&mut game, &back).unwrap();
        assert_eq!(tax_for(&game, commander), 2);
        apply_event(&mut game, &cast).unwrap();
        assert_eq!(game.commander_tax(commander), 4);
    }

    fn put_in(game: &mut GameState, commander: ObjectId, to: Zone) {
        let from = game.object(commander).unwrap().zone();
        let moved = Event::ZoneChanged {
            object: commander,
            from,
            to,
            cause: ZoneCause::Moved,
        };
        apply_event(game, &moved).unwrap();
    }

    #[rstest]
    #[case::graveyard(Zone::Graveyard(PlayerId(1)))]
    #[case::exile(Zone::Exile)]
    fn the_owner_of_a_commander_in_a_graveyard_or_exile_is_asked(#[case] zone: Zone) {
        let mut game = game(2, &[&[], &["A"]]);
        let commander = ids(&game)[0];
        assert!(
            decision(&game).is_none(),
            "in the command zone it is not asked"
        );
        put_in(&mut game, commander, zone);
        let asked = decision(&game).unwrap();
        assert_eq!(asked.player, PlayerId(1));
        let DecisionKind::YesNo(options) = &asked.kind else {
            panic!("a yes or no question");
        };
        assert!(asks(&game, options));
        let yes = answer(&game, options, YesNoChoice(true)).unwrap();
        assert!(matches!(
            yes,
            Outcome::Events(ref events) if matches!(events.as_slice(), [Event::ZoneChanged { to: Zone::Command(PlayerId(1)), .. }])
        ));
        let no = answer(&game, options, YesNoChoice(false)).unwrap();
        assert!(matches!(
            no,
            Outcome::Events(ref events) if matches!(events.as_slice(), [Event::CommanderStayed { .. }])
        ));
    }

    #[test]
    fn a_refusal_holds_until_the_commander_moves_again() {
        let mut game = game(2, &[&["A"], &[]]);
        let commander = ids(&game)[0];
        put_in(&mut game, commander, Zone::Graveyard(PlayerId(0)));
        apply_event(&mut game, &Event::CommanderStayed { object: commander }).unwrap();
        assert!(decision(&game).is_none(), "not asked again where it lies");
        put_in(&mut game, commander, Zone::Exile);
        assert!(
            decision(&game).is_some(),
            "asked again once it is in another zone"
        );
    }

    #[test]
    fn a_player_who_left_is_not_asked_and_a_plain_legend_never_is() {
        let mut game = game(3, &[&["A"], &[], &[]]);
        let commander = ids(&game)[0];
        put_in(&mut game, commander, Zone::Graveyard(PlayerId(0)));
        let left = Event::PlayerLost {
            player: PlayerId(0),
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &left).unwrap();
        assert!(decision(&game).is_none());
        let mut plain = setup(vec![Vec::new(); 2], vec![Vec::new(); 2]);
        plain.format = Arc::new(Commander);
        plain.players[0].graveyard = vec![creature("Bear", 2, 2)];
        let plain = GameState::from_setup(&plain).unwrap();
        assert!(decision(&plain).is_none());
    }

    #[test]
    fn what_is_counted_changes_the_hash_only_once_it_is_nonzero() {
        let mut game = game(2, &[&["A"], &[]]);
        game.turn.step = Step::CombatDamage;
        let commander = ids(&game)[0];
        let fresh = game.state_hash();
        assert_eq!(game.clone().state_hash(), fresh);
        let mut without = game.clone();
        without.commanders = Commanders::default();
        assert_ne!(
            without.state_hash(),
            fresh,
            "the commanders are part of the state"
        );
        hit(&mut game, commander, 1, 1);
        assert_ne!(game.state_hash(), fresh);
        // The life lost with the damage is the same in both, so the count alone tells them apart.
        let mut other = game.clone();
        other.commanders.damage.clear();
        assert_ne!(other.state_hash(), game.state_hash());
    }
}
