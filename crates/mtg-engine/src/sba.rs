//! State-based actions (CR 704), applied in passes until nothing changes.

use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::house::EmptyLibrary;
use mtg_core::ids::ObjectId;

use crate::characteristics::{characteristics, controller_of_object};
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::{DeathCause, Event, ZoneCause};
use crate::keywords;
use crate::mulligan::shuffled;
use crate::object::PermanentState;
use crate::player::{LossReason, Player};
use crate::state::GameState;
use crate::zones::Zone;

/// Whether a player who failed to draw shuffles their graveyard into the library in place of losing: the house
/// rule is set and there is a graveyard to shuffle.
fn refills(state: &GameState, player: &Player) -> bool {
    player.failed_draws > 0
        && state.ruleset().empty_library() == EmptyLibrary::ReshuffleGraveyard
        && !state.zone(Zone::Graveyard(player.id())).is_empty()
}

fn loss_reason(state: &GameState, player: &Player) -> Option<LossReason> {
    if player.life() <= 0 {
        Some(LossReason::ZeroLife)
    } else if player.failed_draws > 0 && !refills(state, player) {
        Some(LossReason::EmptyLibrary)
    } else if player.counters_of(&CounterKind::Poison) >= state.ruleset().poison_threshold() {
        Some(LossReason::Poison)
    } else {
        state.format().loss_condition(state, player.id())
    }
}

/// The players who lose in this pass and why. A player at the life total that wins the game, a house rule, wins by
/// the others losing to it; when several are at it, the game is left to the game-end check, which draws it.
fn losses(state: &GameState) -> Vec<Event> {
    let own: Vec<(&Player, Option<LossReason>)> = state
        .players()
        .iter()
        .filter(|p| p.is_in_game())
        .map(|p| (p, loss_reason(state, p)))
        .collect();
    let goal = state.ruleset().winning_life();
    let at_goal = |p: &Player| goal.is_some_and(|goal| p.life() >= goal);
    let reached = own.iter().any(|(p, reason)| reason.is_none() && at_goal(p));
    own.into_iter()
        .filter_map(|(p, reason)| {
            let reason = reason
                .or_else(|| (reached && !at_goal(p)).then_some(LossReason::OpponentReachedLife))?;
            Some(Event::PlayerLost {
                player: p.id(),
                reason,
            })
        })
        .collect()
}

/// The refills of this pass: the graveyard of each player who failed to draw and loses for nothing else is shuffled
/// into their library and the cards they could not draw are drawn. A shortfall is failed again, so the player loses
/// at the next check. The k-th refill of a pass uses the k-th substream after those already used.
fn refills_of(state: &GameState, lost: &[Event]) -> Vec<Event> {
    let mut events = Vec::new();
    let mut shuffles = 0u64;
    for player in state.players().iter().filter(|p| p.is_in_game()) {
        let id = player.id();
        let leaving = lost
            .iter()
            .any(|e| matches!(e, Event::PlayerLost { player, .. } if *player == id));
        if leaving || !refills(state, player) {
            continue;
        }
        let mut cards = state.zone(Zone::Library(id)).to_vec();
        cards.extend_from_slice(state.zone(Zone::Graveyard(id)));
        let order = shuffled(state, shuffles, cards);
        shuffles += 1;
        let wanted = usize::try_from(player.failed_draws).unwrap_or(usize::MAX);
        let drawn = wanted.min(order.len());
        let top: Vec<ObjectId> = order.iter().rev().take(drawn).copied().collect();
        events.push(Event::LibraryRefilled { player: id, order });
        events.extend(
            top.into_iter()
                .map(|object| Event::CardDrawn { player: id, object }),
        );
        events.extend((drawn..wanted).map(|_| Event::DrewFromEmptyLibrary { player: id }));
    }
    events
}

/// One simultaneous pass over players and creatures; empty once the state is stable.
pub(crate) fn check(state: &GameState) -> Vec<Event> {
    let mut events = losses(state);
    let refilled = refills_of(state, &events);
    events.extend(refilled);
    events.extend(crate::day_night::sync(state));
    events.extend(crate::citys_blessing::sync(state));
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        if !state.is_alive(object.controller()) {
            continue;
        }
        let Some(chars) = characteristics(state, id) else {
            continue;
        };
        // A planeswalker with no loyalty is put into its owner's graveyard (CR 704.5i); one that is a creature too
        // goes once, whatever else would happen to it.
        if chars.types.contains(CardType::Planeswalker)
            && object
                .permanent()
                .is_some_and(|permanent| permanent.counters_of(&CounterKind::Loyalty) == 0)
        {
            events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Battlefield,
                to: Zone::Graveyard(object.owner()),
                cause: ZoneCause::StateBased,
            });
            continue;
        }
        if !chars.is_creature() {
            continue;
        }
        let toughness = chars.toughness.unwrap_or(0);
        let damage = object.permanent().map_or(0, PermanentState::damage);
        let deathtouched = object.permanent().is_some_and(PermanentState::deathtouched);
        let Some(cause) = death_cause(toughness, damage, deathtouched, || {
            keywords::is_indestructible(state, id)
        }) else {
            continue;
        };
        events.push(Event::CreatureDied { object: id, cause });
    }
    let died: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            Event::CreatureDied { object, .. } | Event::ZoneChanged { object, .. } => Some(*object),
            _ => None,
        })
        .collect();
    events.extend(annihilation(state, &died));
    events.extend(crate::attach::state_based(state, &died));
    events
}

/// A permanent with both a +1/+1 and a -1/-1 counter on it loses as many of each as it has of the kind it has fewer of
/// (CR 704.5q); one that dies in the same pass takes its counters with it.
fn annihilation(state: &GameState, died: &[ObjectId]) -> Vec<Event> {
    let mut events = Vec::new();
    if !state.possible.counters {
        return events;
    }
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state
            .object(id)
            .filter(|o| state.is_alive(o.owner()) && !died.contains(&id))
        else {
            continue;
        };
        let Some(permanent) = object.permanent() else {
            continue;
        };
        let pairs = permanent
            .counters_of(&CounterKind::PlusOnePlusOne)
            .min(permanent.counters_of(&CounterKind::MinusOneMinusOne));
        let Ok(pairs) = i32::try_from(pairs) else {
            continue;
        };
        if pairs > 0 {
            for counter in [CounterKind::PlusOnePlusOne, CounterKind::MinusOneMinusOne] {
                events.push(Event::CountersChanged(CountersChanged {
                    on: Target::Object(object.reference()),
                    counter,
                    delta: -pairs,
                }));
            }
        }
    }
    events
}

/// Why a creature with this toughness, damage marked and deathtouch mark dies now, if it does.
///
/// Zero toughness kills whatever else is true (CR 704.5f). Lethal damage and a mark from deathtouch destroy
/// it (CR 704.5g, 704.5h) unless it is indestructible, which is asked only when one of them applies.
pub(crate) fn death_cause(
    toughness: i32,
    damage: u32,
    deathtouched: bool,
    indestructible: impl FnOnce() -> bool,
) -> Option<DeathCause> {
    if toughness <= 0 {
        return Some(DeathCause::ZeroToughness);
    }
    let lethal = i64::from(damage) >= i64::from(toughness);
    if !(lethal || deathtouched) || indestructible() {
        return None;
    }
    Some(if lethal {
        DeathCause::LethalDamage
    } else {
        DeathCause::Deathtouch
    })
}

/// The objects of players who left the game leave with them (CR 800.4a).
fn departures(state: &GameState) -> Vec<Event> {
    state
        .objects()
        .filter(|o| !state.is_alive(o.owner()))
        .map(|o| Event::ObjectLeftGame { object: o.id() })
        .collect()
}

/// The departures the control loop runs before everything else, a resolution in progress and every state-based action
/// included, so nothing is asked while an object of a seat that left remains. A game that is decided keeps its final
/// board: its objects stay once nothing resolves, because the game-end check is next.
pub(crate) fn departures_first(state: &GameState) -> Vec<Event> {
    if state.players().iter().all(Player::is_in_game)
        || (state.resolving().is_none() && state.ruleset().game_result(state).is_some())
    {
        return Vec::new();
    }
    departures(state)
}

/// The end of the game once one player is left or none (CR 104.2a). The control loop checks it before the state-based
/// actions go on, so a pass after the last loss asks nothing while the board still holds what the losers left.
pub(crate) fn game_over(state: &GameState) -> Option<Event> {
    if state.alive_players().nth(1).is_some() {
        return None;
    }
    let result = state.ruleset().game_result(state)?;
    Some(Event::GameEnded { result })
}

/// Housekeeping that follows stable passes: remove eliminated players' objects, drop ended effects and delayed triggers.
pub(crate) fn housekeeping(state: &GameState) -> Vec<Event> {
    let departing = departures(state);
    if !departing.is_empty() {
        return departing;
    }
    let mut expired: Vec<Event> = state
        .effects()
        .iter()
        .filter(|e| !e.is_active(state))
        .map(|e| Event::EffectExpired { id: e.id })
        .collect();
    expired.extend(
        state
            .rule_effects()
            .iter()
            .filter(|e| !e.is_active(state))
            .map(|e| Event::RuleExpired { id: e.id }),
    );
    expired.extend(
        state
            .delayed()
            .iter()
            .filter(|d| d.lapsed(state))
            .map(|d| Event::DelayedTriggerExpired { id: d.id() }),
    );
    if !expired.is_empty() {
        return expired;
    }
    // CR 800.4: with their control effects over, what a player who left still controls is exiled.
    state
        .objects()
        .filter(|o| matches!(o.zone(), Zone::Battlefield | Zone::Stack))
        .filter(|o| !state.is_alive(controller_of_object(state, o)))
        .map(|o| Event::ZoneChanged {
            object: o.id(),
            from: o.zone(),
            to: Zone::Exile,
            cause: ZoneCause::Exiled,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a creature dies to state-based actions for zero toughness always, and for lethal damage or a deathtouch mark unless it is indestructible, and the cause named is the one that applies; a permanent with both +1/+1 and -1/-1 counters loses a pair of each until it has only one kind; a player with ten or more poison counters loses, and life is named first; the objects of a seat that left depart at once (their own first, then what they controlled is exiled) and none depart while every seat is in the game.
    use std::sync::Arc;

    use mtg_cards::CardDefinition;
    use mtg_core::ids::PlayerId;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, setup};

    #[rstest]
    #[case::healthy(2, 1, false, false, None)]
    #[case::exactly_lethal(2, 2, false, false, Some(DeathCause::LethalDamage))]
    #[case::more_than_lethal(2, 9, false, false, Some(DeathCause::LethalDamage))]
    #[case::deathtouch_with_one_damage(5, 1, true, false, Some(DeathCause::Deathtouch))]
    #[case::deathtouch_and_lethal_is_lethal_damage(
        2,
        3,
        true,
        false,
        Some(DeathCause::LethalDamage)
    )]
    #[case::zero_toughness(0, 0, false, false, Some(DeathCause::ZeroToughness))]
    #[case::negative_toughness(-1, 0, false, false, Some(DeathCause::ZeroToughness))]
    #[case::indestructible_shrugs_off_lethal_damage(2, 9, false, true, None)]
    #[case::indestructible_shrugs_off_deathtouch(2, 1, true, true, None)]
    #[case::indestructible_still_dies_to_zero_toughness(
        0,
        0,
        false,
        true,
        Some(DeathCause::ZeroToughness)
    )]
    #[case::indestructible_with_negative_toughness(
        -3,
        5,
        true,
        true,
        Some(DeathCause::ZeroToughness)
    )]
    fn the_cause_of_death(
        #[case] toughness: i32,
        #[case] damage: u32,
        #[case] deathtouched: bool,
        #[case] indestructible: bool,
        #[case] expected: Option<DeathCause>,
    ) {
        assert_eq!(
            death_cause(toughness, damage, deathtouched, || indestructible),
            expected
        );
    }

    #[test]
    fn indestructibility_is_asked_only_when_damage_would_destroy() {
        let ask = || -> bool { unreachable!("not asked") };
        assert_eq!(death_cause(3, 2, false, ask), None);
        assert_eq!(
            death_cause(0, 0, false, ask),
            Some(DeathCause::ZeroToughness)
        );
    }

    const PLUS: CounterKind = CounterKind::PlusOnePlusOne;
    const MINUS: CounterKind = CounterKind::MinusOneMinusOne;

    /// Seat 0 controls a Bear with these counters and has this many poison counters; seat 1 controls an Ogre.
    fn game_with(counters: &[(CounterKind, u32)], poison: u32) -> GameState {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 3)]],
        );
        game.players[0].battlefield[0].counters = counters.to_vec();
        game.players[0].counters = vec![(CounterKind::Poison, poison)];
        GameState::from_setup(&game).unwrap()
    }

    /// The counter changes the check asks for, as (kind, change).
    fn counter_changes(game: &GameState) -> Vec<(CounterKind, i32)> {
        check(game)
            .into_iter()
            .filter_map(|event| match event {
                Event::CountersChanged(changed) => Some((changed.counter, changed.delta)),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[case::no_counters(&[], &[])]
    #[case::only_plus_counters(&[(PLUS, 2)], &[])]
    #[case::only_minus_counters(&[(MINUS, 2)], &[])]
    #[case::one_pair(&[(PLUS, 1), (MINUS, 1)], &[(PLUS, -1), (MINUS, -1)])]
    #[case::more_plus_than_minus(&[(PLUS, 3), (MINUS, 1)], &[(PLUS, -1), (MINUS, -1)])]
    #[case::more_minus_than_plus(&[(PLUS, 3), (MINUS, 4)], &[(PLUS, -3), (MINUS, -3)])]
    #[case::as_many_of_each(&[(PLUS, 3), (MINUS, 3)], &[(PLUS, -3), (MINUS, -3)])]
    #[case::other_kinds_stay(&[(PLUS, 1), (CounterKind::Charge, 1)], &[])]
    fn a_permanent_with_both_kinds_loses_a_pair_of_each(
        #[case] counters: &[(CounterKind, u32)],
        #[case] expected: &[(CounterKind, i32)],
    ) {
        assert_eq!(counter_changes(&game_with(counters, 0)), expected);
    }

    #[test]
    fn the_pairs_of_every_permanent_go_together_and_only_while_a_game_has_such_counters() {
        let mut game = setup(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Ogre", 3, 3)]],
        );
        game.players[0].battlefield[0].counters = vec![(PLUS, 2), (MINUS, 1)];
        game.players[1].battlefield[0].counters = vec![(PLUS, 1), (MINUS, 2)];
        let game = GameState::from_setup(&game).unwrap();
        assert_eq!(
            counter_changes(&game),
            [(PLUS, -1), (MINUS, -1), (PLUS, -1), (MINUS, -1)]
        );
        assert!(game.possible.counters);
        let plain = game_with(&[], 0);
        assert!(
            !plain.possible.counters,
            "a game without such counters looks for none"
        );
        assert_eq!(check(&plain), []);
    }

    #[test]
    fn a_creature_that_dies_in_the_pass_takes_its_pairs_with_it() {
        // 2/2 with one +1/+1 and three -1/-1 counters: toughness -1.
        let events = check(&game_with(&[(PLUS, 1), (MINUS, 3)], 0));
        assert!(
            matches!(
                events.as_slice(),
                [Event::CreatureDied {
                    cause: DeathCause::ZeroToughness,
                    ..
                }]
            ),
            "{events:?}"
        );
    }

    #[rstest]
    #[case::none(0, false)]
    #[case::nine(9, false)]
    #[case::ten(10, true)]
    #[case::eleven(11, true)]
    fn ten_poison_counters_lose_the_game(#[case] poison: u32, #[case] loses: bool) {
        let events = check(&game_with(&[], poison));
        let lost = Event::PlayerLost {
            player: PlayerId(0),
            reason: LossReason::Poison,
        };
        assert_eq!(events.contains(&lost), loses, "{events:?}");
        assert_eq!(events.len(), usize::from(loses));
    }

    /// Seat 0 controls a Walker (printed loyalty 3) that has lost `lost` of its loyalty counters, and seat 1 a Bear; the
    /// Walker is also a 2/2 creature when `creature`, with `damage` damage marked on it.
    fn walker_game(lost: u32, creature: bool, damage: u32) -> GameState {
        let mut card = CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3);
        if creature {
            card = card
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2);
        }
        let mut setup = setup(
            vec![vec![], vec![]],
            vec![vec![Arc::new(card)], vec![creature_card("Bear")]],
        );
        setup.players[0].battlefield[0].damage = damage;
        let mut game = GameState::from_setup(&setup).unwrap();
        if lost > 0 {
            let walker = game.zone(Zone::Battlefield)[0];
            let event = Event::CountersChanged(CountersChanged {
                on: Target::Object(game.object(walker).unwrap().reference()),
                counter: CounterKind::Loyalty,
                delta: -i32::try_from(lost).unwrap(),
            });
            crate::fold::apply_event(&mut game, &event).unwrap();
        }
        game
    }

    fn creature_card(name: &str) -> Arc<CardDefinition> {
        creature(name, 2, 2)
    }

    #[rstest]
    #[case::three_left(0, false, 0, false)]
    #[case::one_left(2, false, 0, false)]
    #[case::none_left(3, false, 0, true)]
    #[case::damage_marked_on_a_planeswalker_kills_nothing(2, false, 9, false)]
    #[case::a_creature_and_planeswalker_with_loyalty_dies_as_a_creature(0, true, 9, false)]
    #[case::a_creature_and_planeswalker_with_no_loyalty_goes_once(3, true, 9, true)]
    fn a_planeswalker_with_no_loyalty_goes_to_the_graveyard_once(
        #[case] lost: u32,
        #[case] creature: bool,
        #[case] damage: u32,
        #[case] goes_for_loyalty: bool,
    ) {
        let game = walker_game(lost, creature, damage);
        let walker = game.zone(Zone::Battlefield)[0];
        let events = check(&game);
        let for_loyalty = Event::ZoneChanged {
            object: walker,
            from: Zone::Battlefield,
            to: Zone::Graveyard(PlayerId(0)),
            cause: ZoneCause::StateBased,
        };
        assert_eq!(
            events.contains(&for_loyalty),
            goes_for_loyalty,
            "{events:?}"
        );
        let about_the_walker = events
            .iter()
            .filter(|event| match event {
                Event::ZoneChanged { object, .. } | Event::CreatureDied { object, .. } => {
                    *object == walker
                }
                _ => false,
            })
            .count();
        let dies_as_a_creature = creature && damage >= 2 && !goes_for_loyalty;
        assert_eq!(
            about_the_walker,
            usize::from(goes_for_loyalty || dies_as_a_creature)
        );
    }

    #[test]
    fn a_starting_planeswalker_has_its_printed_loyalty_and_no_state_based_action_follows() {
        let game = walker_game(0, false, 0);
        let walker = game.zone(Zone::Battlefield)[0];
        let held = game
            .object(walker)
            .and_then(|o| o.permanent())
            .map(|p| p.counters_of(&CounterKind::Loyalty));
        assert_eq!(held, Some(3));
        assert_eq!(check(&game), []);
    }

    #[test]
    fn a_player_out_of_life_loses_for_that_reason_before_poison() {
        let mut game = setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        game.players[0].life = Some(0);
        game.players[0].counters = vec![(CounterKind::Poison, 12)];
        let events = check(&GameState::from_setup(&game).unwrap());
        assert_eq!(
            events,
            [Event::PlayerLost {
                player: PlayerId(0),
                reason: LossReason::ZeroLife
            }]
        );
    }

    #[rstest]
    #[case::two_seats_end_the_game_with_the_final_board(2, false)]
    #[case::three_seats(3, true)]
    #[case::four_seats(4, true)]
    fn the_objects_of_a_seat_that_left_depart_at_once_unless_that_decides_the_game_and_none_depart_while_all_are_in_it(
        #[case] seats: usize,
        #[case] departs: bool,
    ) {
        use crate::fold::apply_event;
        let mut game = GameState::from_setup(&setup(
            vec![vec![creature("Card", 1, 1)]; seats],
            vec![vec![creature("Wolf", 2, 2)]; seats],
        ))
        .unwrap();
        assert_eq!(departures_first(&game), [], "every seat is in the game");
        let leaver = PlayerId::from_index(seats - 1);
        let lost = Event::PlayerLost {
            player: leaver,
            reason: LossReason::Conceded,
        };
        assert_eq!(game_over(&game), None);
        apply_event(&mut game, &lost).unwrap();
        assert_eq!(
            game_over(&game),
            (!departs).then_some(Event::GameEnded {
                result: crate::state::GameResult::Winner(PlayerId(0))
            }),
            "the game ends at once when one player is left"
        );
        let owned: Vec<_> = game
            .objects()
            .filter(|o| o.owner() == leaver)
            .map(crate::object::GameObject::id)
            .collect();
        assert!(
            owned.len() >= 2,
            "a card in the hand and one in play at least"
        );
        let left = departures_first(&game);
        if !departs {
            assert_eq!(left, [], "a decided game keeps its final board");
            return;
        }
        let mut named: Vec<_> = left
            .iter()
            .map(|event| match event {
                Event::ObjectLeftGame { object } => *object,
                other => panic!("{other:?}"),
            })
            .collect();
        named.sort();
        assert_eq!(named, owned);
        for event in &left {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(departures_first(&game), []);
        assert!(game.objects().all(|o| o.owner() != leaver));
    }

    #[test]
    fn what_a_player_who_left_still_controls_is_exiled_once_their_own_objects_are_gone() {
        use crate::fold::apply_event;
        let mut game = GameState::from_setup(&setup(
            vec![vec![], vec![]],
            vec![vec![creature("Wolf", 2, 2)], vec![creature("Bear", 2, 2)]],
        ))
        .unwrap();
        let find = |game: &GameState, name: &str| {
            game.objects()
                .find(|o| o.card().name == name)
                .map(crate::object::GameObject::id)
                .unwrap()
        };
        let (wolf, bear) = (find(&game, "Wolf"), find(&game, "Bear"));
        // The Bear is the foe's but enters under the control of seat 0, who then leaves.
        let steal = Event::Entered {
            object: bear,
            tapped: false,
            controller: PlayerId(0),
            chosen: None,
        };
        let lost = Event::PlayerLost {
            player: PlayerId(0),
            reason: LossReason::Conceded,
        };
        apply_event(&mut game, &steal).unwrap();
        assert_eq!(housekeeping(&game), []);
        apply_event(&mut game, &lost).unwrap();
        let first = housekeeping(&game);
        assert!(
            first.contains(&Event::ObjectLeftGame { object: wolf }),
            "their own objects leave first"
        );
        assert!(
            first
                .iter()
                .all(|e| matches!(e, Event::ObjectLeftGame { .. })),
            "{first:?}"
        );
        for event in &first {
            apply_event(&mut game, event).unwrap();
        }
        assert_eq!(
            housekeeping(&game),
            [Event::ZoneChanged {
                object: bear,
                from: Zone::Battlefield,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            }]
        );
    }
}
