//! Goal: each pattern of an event about players and permanents (damage, life, draws, discards, cycling, taps, untaps, mana, counters, blocks, attackers, attachments) triggers on the event it names for the players and permanents it names, never on the same event for anyone else, and hands the ability the subjects it reads.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaType;
use rstest::rstest;

use super::detect;
use crate::choose::Target;
use crate::combat::{AttackTarget, DamageTarget};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::fixtures::{creature, state};
use crate::fold::apply_event;
use crate::state::GameState;
use crate::triggers::Triggered;
use crate::turn::Step;
use crate::zones::Zone;

pub(super) const ME: PlayerId = PlayerId(0);
pub(super) const FOE: PlayerId = PlayerId(1);

pub(super) fn pattern(json: &str) -> TriggerPattern {
    serde_json::from_str(json).unwrap()
}

/// A 1/1 creature named `name` with "when `when`, gain 1 life", which works from `from`.
fn watching(name: &str, when: &str, from: ZoneKind) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Triggered {
                when: pattern(when),
                intervening_if: None,
                limit: None,
                from,
                targets: Vec::new(),
                effects: vec![Effect::GainLife {
                    who: Ref::You,
                    amount: Amount::Fixed(1),
                }],
            }),
    )
}

fn named(name: &str, kind: CardType) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(kind))
}

fn aura(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_subtype("Aura"),
    )
}

/// Seat 0 holds a Spell and controls the Watcher of `when`, a Bear, a Forest and a Cloak (an Aura); seat 1 holds a
/// Their Spell and controls an Ogre, a Wolf and a Their Forest.
pub(super) fn board(when: &str) -> GameState {
    state(
        vec![
            vec![named("Spell", CardType::Instant)],
            vec![named("Their Spell", CardType::Instant)],
        ],
        vec![
            vec![
                watching("Watcher", when, ZoneKind::Battlefield),
                creature("Bear", 2, 2),
                named("Forest", CardType::Land),
                aura("Cloak"),
            ],
            vec![
                creature("Ogre", 3, 4),
                creature("Wolf", 2, 2),
                named("Their Forest", CardType::Land),
            ],
        ],
    )
}

pub(super) fn id(state: &GameState, name: &str) -> ObjectId {
    state
        .objects()
        .find(|o| o.card().name == name)
        .map(crate::object::GameObject::id)
        .unwrap()
}

pub(super) fn target(state: &GameState, name: &str) -> Target {
    Target::Object(state.object(id(state, name)).unwrap().reference())
}

pub(super) fn fold(state: &GameState, events: &[Event]) -> GameState {
    let mut next = state.clone();
    for event in events {
        apply_event(&mut next, event).unwrap();
    }
    next
}

/// The abilities `events` trigger on top of `pre`, once they are folded.
pub(super) fn fires(pre: &GameState, events: &[Event]) -> Vec<Triggered> {
    detect(pre, events, &fold(pre, events))
}

/// How many abilities `events` trigger on top of `pre`.
pub(super) fn fired(pre: &GameState, events: &[Event]) -> usize {
    fires(pre, events).len()
}

/// The same, for events that cannot be folded without the rest of a combat or of a game around them.
fn fired_unfolded(pre: &GameState, events: &[Event]) -> usize {
    detect(pre, events, pre).len()
}

pub(super) fn damage(state: &GameState, source: &str, to: DamageTarget) -> Event {
    Event::DamageDealt {
        source: id(state, source),
        target: to,
        amount: 2,
        deathtouch: false,
        form: crate::combat::DamageForm::Normal,
        toxic: 0,
    }
}

pub(super) fn in_combat_damage(state: &GameState) -> GameState {
    fold(
        state,
        &[Event::StepBegan {
            step: Step::CombatDamage,
        }],
    )
}

#[rstest]
#[case::any_source_any_recipient(r#"{"DealsDamage":{"source":[]}}"#, false, "Bear", 1, 1)]
#[case::this_source_misses_another(r#"{"DealsDamage":{"source":["This"]}}"#, false, "Bear", 1, 0)]
#[case::a_creature_you_control(
    r#"{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    false,
    "Bear",
    1,
    1
)]
#[case::a_creature_you_control_not_theirs(
    r#"{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    false,
    "Ogre",
    1,
    0
)]
#[case::to_an_opponent(
    r#"{"DealsDamage":{"source":[],"to":"Opponents"}}"#,
    false,
    "Bear",
    1,
    1
)]
#[case::to_an_opponent_not_to_me(
    r#"{"DealsDamage":{"source":[],"to":"Opponents"}}"#,
    false,
    "Ogre",
    0,
    0
)]
#[case::to_a_player_in_combat(
    r#"{"DealsDamage":{"source":[],"to":"EachPlayer","combat":true}}"#,
    true,
    "Bear",
    1,
    1
)]
#[case::combat_damage_outside_combat_misses(
    r#"{"DealsDamage":{"source":[],"to":"EachPlayer","combat":true}}"#,
    false,
    "Bear",
    1,
    0
)]
#[case::noncombat_damage_in_combat_misses(
    r#"{"DealsDamage":{"source":[],"combat":false}}"#,
    true,
    "Bear",
    1,
    0
)]
#[case::noncombat_damage(r#"{"DealsDamage":{"source":[],"combat":false}}"#, false, "Bear", 1, 1)]
fn damage_to_a_player_triggers_what_names_its_source_recipient_and_kind(
    #[case] when: &str,
    #[case] combat: bool,
    #[case] source: &str,
    #[case] victim: usize,
    #[case] expected: usize,
) {
    let mut pre = board(when);
    if combat {
        pre = in_combat_damage(&pre);
    }
    let hit = damage(
        &pre,
        source,
        DamageTarget::Player(PlayerId::from_index(victim)),
    );
    assert_eq!(fired(&pre, &[hit]), expected);
}

#[rstest]
#[case::to_a_creature(
    r#"{"DealsDamage":{"source":[],"to":{"Each":[{"Type":"Creature"}]}}}"#,
    "Ogre",
    1
)]
#[case::to_a_creature_not_a_player(r#"{"DealsDamage":{"source":[],"to":"EachPlayer"}}"#, "Ogre", 0)]
#[case::to_one_creature_only(
    r#"{"DealsDamage":{"source":[],"to":{"Each":[{"Subtype":"Wolf"}]}}}"#,
    "Ogre",
    0
)]
#[case::to_anything(r#"{"DealsDamage":{"source":[]}}"#, "Ogre", 1)]
fn damage_to_a_permanent_triggers_what_names_its_recipient(
    #[case] when: &str,
    #[case] hurt: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let hit = damage(&pre, "Bear", DamageTarget::Object(id(&pre, hurt)));
    assert_eq!(fired(&pre, &[hit]), expected);
}

#[test]
fn damage_hands_the_ability_its_recipient_alone_so_that_damage_to_it_hits_no_one_else() {
    let pre = board(r#"{"DealsDamage":{"source":[]}}"#);
    let hit = damage(&pre, "Bear", DamageTarget::Player(FOE));
    assert_eq!(fires(&pre, &[hit])[0].cause.subjects, [Target::Player(FOE)]);
    let hit = damage(&pre, "Bear", DamageTarget::Object(id(&pre, "Ogre")));
    assert_eq!(
        fires(&pre, &[hit])[0].cause.subjects,
        [target(&pre, "Ogre")]
    );
}

#[rstest]
#[case::my_gain(r#"{"LifeGained":{"who":"You"}}"#, "gained", ME, 1)]
#[case::their_gain_is_not_mine(r#"{"LifeGained":{"who":"You"}}"#, "gained", FOE, 0)]
#[case::an_opponents_gain(r#"{"LifeGained":{"who":"Opponents"}}"#, "gained", FOE, 1)]
#[case::my_gain_is_not_an_opponents(r#"{"LifeGained":{"who":"Opponents"}}"#, "gained", ME, 0)]
#[case::any_gain(r#"{"LifeGained":{}}"#, "gained", FOE, 1)]
#[case::a_gain_is_not_a_loss(r#"{"LifeLost":{}}"#, "gained", ME, 0)]
#[case::my_loss(r#"{"LifeLost":{"who":"You"}}"#, "lost", ME, 1)]
#[case::their_loss_is_not_mine(r#"{"LifeLost":{"who":"You"}}"#, "lost", FOE, 0)]
#[case::an_opponents_loss(r#"{"LifeLost":{"who":"Opponents"}}"#, "lost", FOE, 1)]
#[case::life_paid_is_life_lost(r#"{"LifeLost":{"who":"You"}}"#, "paid", ME, 1)]
#[case::damage_is_life_lost(r#"{"LifeLost":{"who":"Opponents"}}"#, "damaged", FOE, 1)]
#[case::damage_to_me_is_not_an_opponents_loss(
    r#"{"LifeLost":{"who":"Opponents"}}"#,
    "damaged",
    ME,
    0
)]
#[case::a_loss_is_not_a_gain(r#"{"LifeGained":{}}"#, "lost", ME, 0)]
fn life_gained_and_lost_trigger_for_the_players_the_pattern_names(
    #[case] when: &str,
    #[case] what: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let event = match what {
        "gained" => Event::LifeGained { player, amount: 3 },
        "lost" => Event::LifeLost { player, amount: 3 },
        "paid" => Event::LifePaid { player, amount: 3 },
        _ => damage(&pre, "Bear", DamageTarget::Player(player)),
    };
    assert_eq!(fired(&pre, &[event]), expected);
}

#[rstest]
#[case::my_draw(r#"{"Drew":{"who":"You"}}"#, ME, 1)]
#[case::their_draw_is_not_mine(r#"{"Drew":{"who":"You"}}"#, FOE, 0)]
#[case::an_opponents_draw(r#"{"Drew":{"who":"Opponents"}}"#, FOE, 1)]
#[case::any_draw(r#"{"Drew":{}}"#, ME, 1)]
fn a_draw_triggers_for_the_player_who_draws(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let top = *pre.zone(Zone::Library(player)).last().unwrap();
    let drawn = Event::CardDrawn {
        player,
        object: top,
    };
    let found = fires(&pre, &[drawn]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, [Target::Player(player)]);
    }
}

#[rstest]
#[case::my_discard(r#"{"Discarded":{"by":"You"}}"#, ME, "Spell", 1)]
#[case::their_discard_is_not_mine(r#"{"Discarded":{"by":"You"}}"#, FOE, "Their Spell", 0)]
#[case::an_opponents_discard(r#"{"Discarded":{"by":"Opponents"}}"#, FOE, "Their Spell", 1)]
#[case::any_discard(r#"{"Discarded":{}}"#, FOE, "Their Spell", 1)]
#[case::an_instant(r#"{"Discarded":{"what":[{"Type":"Instant"}]}}"#, ME, "Spell", 1)]
#[case::not_a_creature(r#"{"Discarded":{"what":[{"Type":"Creature"}]}}"#, ME, "Spell", 0)]
fn a_discard_triggers_for_the_player_and_the_card_the_pattern_names(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] card: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let discarded = Event::CardDiscarded {
        player,
        object: id(&pre, card),
    };
    let found = fires(&pre, &[discarded]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects[0], Target::Player(player));
        assert_eq!(found[0].cause.snapshots[0].zone, Zone::Graveyard(player));
    }
}

#[rstest]
#[case::my_cycling(r#"{"Cycled":{"by":"You"}}"#, ME, "Spell", 1)]
#[case::their_cycling_is_not_mine(r#"{"Cycled":{"by":"You"}}"#, FOE, "Their Spell", 0)]
#[case::an_opponents_cycling(r#"{"Cycled":{"by":"Opponents"}}"#, FOE, "Their Spell", 1)]
#[case::any_cycling(r#"{"Cycled":{}}"#, FOE, "Their Spell", 1)]
#[case::an_instant_cycled(r#"{"Cycled":{"what":[{"Type":"Instant"}]}}"#, ME, "Spell", 1)]
#[case::not_a_creature_cycled(r#"{"Cycled":{"what":[{"Type":"Creature"}]}}"#, ME, "Spell", 0)]
fn cycling_triggers_for_the_player_and_the_card_the_pattern_names(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] card: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let object = id(&pre, card);
    let cycled = [
        Event::CardDiscarded { player, object },
        Event::Cycled { player, object },
    ];
    assert_eq!(fired(&pre, &cycled), expected);
}

#[rstest]
#[case::i_become_it(r#"{"BecomesMonarch":{"who":"You"}}"#, ME, 1)]
#[case::they_become_it_not_me(r#"{"BecomesMonarch":{"who":"You"}}"#, FOE, 0)]
#[case::an_opponent_becomes_it(r#"{"BecomesMonarch":{"who":"Opponents"}}"#, FOE, 1)]
#[case::i_am_not_an_opponent(r#"{"BecomesMonarch":{"who":"Opponents"}}"#, ME, 0)]
#[case::anyone_becomes_it_me(r#"{"BecomesMonarch":{}}"#, ME, 1)]
#[case::anyone_becomes_it_them(r#"{"BecomesMonarch":{}}"#, FOE, 1)]
fn becoming_the_monarch_triggers_for_the_players_the_pattern_names_and_names_the_player(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let found = fires(&pre, &[Event::MonarchChanged { player }]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, vec![Target::Player(player)]);
    }
}

#[rstest]
#[case::i_proliferate(r#"{"Proliferated":{"who":"You"}}"#, ME, 1)]
#[case::they_proliferate_not_me(r#"{"Proliferated":{"who":"You"}}"#, FOE, 0)]
#[case::an_opponent_proliferates(r#"{"Proliferated":{"who":"Opponents"}}"#, FOE, 1)]
#[case::anyone_proliferates(r#"{"Proliferated":{}}"#, FOE, 1)]
fn proliferating_triggers_for_the_players_the_pattern_names_and_names_the_player(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let found = fires(&pre, &[Event::Proliferated { player }]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, vec![Target::Player(player)]);
    }
}

#[rstest]
#[case::i_amass(r#"{"Amassed":{"who":"You"}}"#, ME, 1)]
#[case::they_amass_not_me(r#"{"Amassed":{"who":"You"}}"#, FOE, 0)]
#[case::an_opponent_amasses(r#"{"Amassed":{"who":"Opponents"}}"#, FOE, 1)]
#[case::anyone_amasses(r#"{"Amassed":{}}"#, FOE, 1)]
fn amassing_triggers_for_the_players_the_pattern_names_and_names_the_player(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let army = ObjectId(1);
    let found = fires(
        &pre,
        &[Event::Amassed {
            object: army,
            player,
        }],
    );
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, vec![Target::Player(player)]);
    }
}

#[rstest]
#[case::my_creature(
    r#"{"Explored":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Bear",
    ME,
    1
)]
#[case::their_creature_is_not_mine(
    r#"{"Explored":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Ogre",
    FOE,
    0
)]
#[case::any_creature(r#"{"Explored":{"what":[{"Type":"Creature"}]}}"#, "Ogre", FOE, 1)]
#[case::the_watcher_itself(r#"{"Explored":{"what":["This"]}}"#, "Watcher", ME, 1)]
#[case::another_creature_is_not_the_watcher(r#"{"Explored":{"what":["This"]}}"#, "Bear", ME, 0)]
fn exploring_triggers_for_the_creatures_the_pattern_names_and_names_the_creature_and_its_controller(
    #[case] when: &str,
    #[case] creature: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let object = pre
        .objects()
        .find(|o| o.card().name == creature)
        .map(crate::object::GameObject::id)
        .unwrap();
    let found = fires(&pre, &[Event::Explored { object, player }]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(
            found[0].cause.subjects.first(),
            Some(&Target::Player(player))
        );
        assert_eq!(found[0].cause.subjects.len(), 2);
    }
}

#[rstest]
#[case::i_become_monstrous(r#"{"BecomesMonstrous":{"what":["This"]}}"#, "Watcher", true, ME, 1)]
#[case::another_creature_is_not_the_watcher(r#"{"BecomesMonstrous":{"what":["This"]}}"#, "Bear", true, ME, 0)]
#[case::my_creature(r#"{"BecomesMonstrous":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#, "Bear", true, ME, 1)]
#[case::their_creature_is_not_mine(r#"{"BecomesMonstrous":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#, "Ogre", true, FOE, 0)]
#[case::i_adapt(r#"{"Adapted":{"what":["This"]}}"#, "Watcher", false, ME, 1)]
#[case::another_creature_adapts_not_me(r#"{"Adapted":{"what":["This"]}}"#, "Bear", false, ME, 0)]
#[case::any_creature_adapts(r#"{"Adapted":{"what":[{"Type":"Creature"}]}}"#, "Ogre", false, FOE, 1)]
fn becoming_monstrous_and_adapting_trigger_for_the_creatures_the_pattern_names_and_name_the_creature_and_its_controller(
    #[case] when: &str,
    #[case] creature: &str,
    #[case] monstrous: bool,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let object = pre
        .objects()
        .find(|o| o.card().name == creature)
        .map(crate::object::GameObject::id)
        .unwrap();
    let event = if monstrous {
        Event::Monstrous { object, player }
    } else {
        Event::Adapted { object, player }
    };
    // Neither pattern answers the other action.
    let other = if monstrous {
        Event::Adapted { object, player }
    } else {
        Event::Monstrous { object, player }
    };
    assert_eq!(fires(&pre, &[other]).len(), 0);
    let found = fires(&pre, &[event]);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(
            found[0].cause.subjects.first(),
            Some(&Target::Player(player))
        );
        assert_eq!(found[0].cause.subjects.len(), 2);
    }
}

#[rstest]
#[case::i_bolster(r#"{"Bolstered":{"who":"You"}}"#, ME, 1)]
#[case::they_bolster_not_me(r#"{"Bolstered":{"who":"You"}}"#, FOE, 0)]
#[case::an_opponent_bolsters(r#"{"Bolstered":{"who":"Opponents"}}"#, FOE, 1)]
#[case::anyone_bolsters(r#"{"Bolstered":{}}"#, FOE, 1)]
fn bolstering_triggers_for_the_players_the_pattern_names_and_names_the_player(
    #[case] when: &str,
    #[case] player: PlayerId,
    #[case] expected: usize,
) {
    let pre = board(when);
    let found = fires(
        &pre,
        &[Event::Bolstered {
            object: ObjectId(1),
            player,
        }],
    );
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, vec![Target::Player(player)]);
    }
}

#[test]
fn a_discard_that_is_not_cycling_does_not_trigger_a_cycling_pattern_and_the_cycling_of_a_card_also_discards_it()
 {
    let pre = board(r#"{"Cycled":{}}"#);
    let object = id(&pre, "Spell");
    let discard = Event::CardDiscarded { player: ME, object };
    assert_eq!(fired(&pre, &[discard]), 0);
    let pre = board(r#"{"Discarded":{}}"#);
    let cycled = [
        Event::CardDiscarded { player: ME, object },
        Event::Cycled { player: ME, object },
    ];
    assert_eq!(
        fired(&pre, &cycled),
        1,
        "the discard triggers once, the cycling not at all"
    );
}

/// A card with "when you cycle this card" that works from its graveyard, in the hand of seat 0, and another with
/// "whenever you cycle another card".
fn cyclers() -> GameState {
    let mut setup = crate::fixtures::setup(vec![vec![], vec![]], vec![vec![], vec![]]);
    setup.players[0].hand = vec![
        watching(
            "Cycler",
            r#"{"Cycled":{"what":["This"],"by":"You"}}"#,
            ZoneKind::Graveyard,
        ),
        watching(
            "Other",
            r#"{"Cycled":{"what":["This"],"by":"You"}}"#,
            ZoneKind::Graveyard,
        ),
    ];
    setup.players[0].battlefield = vec![crate::setup::PresetPermanent::new(watching(
        "Fox",
        r#"{"Cycled":{"what":["Other"],"by":"You"}}"#,
        ZoneKind::Battlefield,
    ))];
    GameState::from_setup(&setup).unwrap()
}

#[rstest]
#[case::the_card_itself("Cycler", &["Cycler", "Fox"])]
#[case::the_other_card("Other", &["Fox", "Other"])]
fn a_card_cycled_triggers_its_own_ability_from_the_graveyard_and_what_watches_for_another(
    #[case] cycled: &str,
    #[case] triggers: &[&str],
) {
    let pre = cyclers();
    let object = id(&pre, cycled);
    let events = [
        Event::CardDiscarded { player: ME, object },
        Event::Cycled { player: ME, object },
    ];
    let post = fold(&pre, &events);
    let mut names: Vec<_> = detect(&pre, &events, &post)
        .iter()
        .map(|t| post.object(t.source.id).unwrap().card().name.clone())
        .collect();
    names.sort();
    assert_eq!(names, triggers);
}

#[test]
fn a_card_in_a_hand_that_is_not_cycled_does_not_trigger_its_graveyard_ability() {
    let pre = cyclers();
    let object = id(&pre, "Cycler");
    let discard = [Event::CardDiscarded { player: ME, object }];
    assert_eq!(fired(&pre, &discard), 0);
}

#[rstest]
#[case::a_creature_of_mine(
    r#"{"Untapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Bear",
    1
)]
#[case::not_a_creature_of_theirs(
    r#"{"Untapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Ogre",
    0
)]
#[case::any_permanent(r#"{"Untapped":{"what":[]}}"#, "Their Forest", 1)]
#[case::tapping_is_not_untapping(r#"{"Tapped":{"what":[]}}"#, "Their Forest", 0)]
fn a_permanent_becoming_untapped_triggers_what_names_it(
    #[case] when: &str,
    #[case] permanent: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let object = id(&pre, permanent);
    let tapped = fold(&pre, &[Event::PermanentTapped { object }]);
    assert_eq!(
        fired(&tapped, &[Event::PermanentUntapped { object }]),
        expected
    );
}

/// A tap of `Forest` and the mana it adds, `mana` of it, as a mana ability logs them.
fn tap_for_mana(state: &GameState, land: &str, player: PlayerId, mana: usize) -> Vec<Event> {
    let object = id(state, land);
    let mut events = vec![Event::PermanentTapped { object }];
    events.extend((0..mana).map(|_| Event::ManaAdded {
        player,
        mana: ManaType::Green,
        source: Some(object),
        only: Vec::new(),
    }));
    events
}

#[rstest]
#[case::a_land_of_mine(
    r#"{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}}"#,
    "Forest",
    ME,
    1,
    1
)]
#[case::two_mana_is_one_tap(
    r#"{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}}"#,
    "Forest",
    ME,
    2,
    1
)]
#[case::not_a_land_of_theirs(
    r#"{"TappedForMana":{"what":[{"Type":"Land"}],"by":"You"}}"#,
    "Their Forest",
    FOE,
    1,
    0
)]
#[case::an_opponents_land(
    r#"{"TappedForMana":{"what":[{"Type":"Land"}],"by":"Opponents"}}"#,
    "Their Forest",
    FOE,
    1,
    1
)]
#[case::any_player_any_permanent(r#"{"TappedForMana":{"what":[]}}"#, "Their Forest", FOE, 1, 1)]
#[case::a_creature_is_not_a_land(
    r#"{"TappedForMana":{"what":[{"Type":"Land"}]}}"#,
    "Bear",
    ME,
    1,
    0
)]
fn tapping_a_permanent_for_mana_triggers_once_for_the_tap(
    #[case] when: &str,
    #[case] land: &str,
    #[case] player: PlayerId,
    #[case] mana: usize,
    #[case] expected: usize,
) {
    let pre = board(when);
    let events = tap_for_mana(&pre, land, player, mana);
    let found = fires(&pre, &events);
    let mana_taps = found.len();
    assert_eq!(mana_taps, expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects, [Target::Player(player)]);
    }
}

#[test]
fn mana_that_was_not_made_by_a_tap_is_not_a_tap_for_mana() {
    let pre = board(r#"{"TappedForMana":{"what":[]}}"#);
    let mut events = tap_for_mana(&pre, "Forest", ME, 1);
    events.remove(0);
    assert_eq!(fired(&pre, &events), 0);
    let only_a_tap = [Event::PermanentTapped {
        object: id(&pre, "Forest"),
    }];
    assert_eq!(fired(&pre, &only_a_tap), 0);
}

fn counters(state: &GameState, on: &str, counter: CounterKind, delta: i32) -> Event {
    Event::CountersChanged(CountersChanged {
        on: target(state, on),
        counter,
        delta,
    })
}

#[rstest]
#[case::any_counters(r#"{"CountersPut":{}}"#, "Ogre", CounterKind::PlusOnePlusOne, 2, 1)]
#[case::on_a_creature_of_mine(
    r#"{"CountersPut":{"what":[{"ControlledBy":"You"}]}}"#,
    "Bear",
    CounterKind::PlusOnePlusOne,
    1,
    1
)]
#[case::not_on_a_creature_of_theirs(
    r#"{"CountersPut":{"what":[{"ControlledBy":"You"}]}}"#,
    "Ogre",
    CounterKind::PlusOnePlusOne,
    1,
    0
)]
#[case::this_permanent(
    r#"{"CountersPut":{"what":["This"]}}"#,
    "Watcher",
    CounterKind::PlusOnePlusOne,
    1,
    1
)]
#[case::not_another_permanent(
    r#"{"CountersPut":{"what":["This"]}}"#,
    "Bear",
    CounterKind::PlusOnePlusOne,
    1,
    0
)]
#[case::of_the_kind(
    r#"{"CountersPut":{"counter":"+1/+1"}}"#,
    "Bear",
    CounterKind::PlusOnePlusOne,
    1,
    1
)]
#[case::not_of_another_kind(
    r#"{"CountersPut":{"counter":"+1/+1"}}"#,
    "Bear",
    CounterKind::MinusOneMinusOne,
    1,
    0
)]
#[case::removal_is_not_putting(r#"{"CountersPut":{}}"#, "Bear", CounterKind::PlusOnePlusOne, -1, 0)]
#[case::removal(r#"{"CountersRemoved":{}}"#, "Bear", CounterKind::PlusOnePlusOne, -1, 1)]
#[case::removal_of_the_kind(r#"{"CountersRemoved":{"what":["This"],"counter":"+1/+1"}}"#, "Watcher", CounterKind::PlusOnePlusOne, -2, 1)]
#[case::removal_of_another_kind(r#"{"CountersRemoved":{"counter":"+1/+1"}}"#, "Bear", CounterKind::MinusOneMinusOne, -1, 0)]
#[case::putting_is_not_removal(
    r#"{"CountersRemoved":{}}"#,
    "Bear",
    CounterKind::PlusOnePlusOne,
    1,
    0
)]
fn counters_put_and_removed_trigger_for_the_permanent_and_the_kind_the_pattern_names(
    #[case] when: &str,
    #[case] on: &str,
    #[case] counter: CounterKind,
    #[case] delta: i32,
    #[case] expected: usize,
) {
    let pre = board(when);
    let event = counters(&pre, on, counter, delta);
    assert_eq!(fired_unfolded(&pre, &[event]), expected);
}

#[test]
fn counters_on_a_player_are_not_counters_on_a_permanent() {
    let pre = board(r#"{"CountersPut":{}}"#);
    let event = Event::CountersChanged(CountersChanged {
        on: Target::Player(FOE),
        counter: CounterKind::Poison,
        delta: 1,
    });
    assert_eq!(fired(&pre, &[event]), 0);
}

#[rstest]
#[case::a_blocker_of_mine(r#"{"Blocks":{"what":[{"ControlledBy":"You"}]}}"#, "Bear", "Ogre", 1)]
#[case::not_a_blocker_of_theirs(
    r#"{"Blocks":{"what":[{"ControlledBy":"You"}]}}"#,
    "Ogre",
    "Bear",
    0
)]
#[case::any_blocker(r#"{"Blocks":{"what":[]}}"#, "Ogre", "Bear", 1)]
#[case::an_attacker_blocked(
    r#"{"BecomesBlocked":{"what":[{"ControlledBy":"You"}]}}"#,
    "Ogre",
    "Bear",
    1
)]
#[case::not_an_attacker_of_theirs_blocked(
    r#"{"BecomesBlocked":{"what":[{"ControlledBy":"You"}]}}"#,
    "Bear",
    "Ogre",
    0
)]
#[case::the_blocker_is_not_blocked(
    r#"{"BecomesBlocked":{"what":[{"ControlledBy":"You"}]}}"#,
    "Bear",
    "Ogre",
    0
)]
fn blockers_trigger_what_blocks_and_attackers_what_becomes_blocked(
    #[case] when: &str,
    #[case] blocker: &str,
    #[case] attacker: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let declared = Event::BlockersDeclared {
        player: PlayerId(0),
        blocks: vec![(id(&pre, blocker), id(&pre, attacker))],
    };
    assert_eq!(fired_unfolded(&pre, &[declared]), expected);
}

#[test]
fn an_attacker_blocked_by_two_becomes_blocked_once_and_each_blocker_blocks() {
    let blocks = |pre: &GameState| Event::BlockersDeclared {
        player: FOE,
        blocks: vec![
            (id(pre, "Ogre"), id(pre, "Bear")),
            (id(pre, "Wolf"), id(pre, "Bear")),
        ],
    };
    let pre = board(r#"{"BecomesBlocked":{"what":["Other"]}}"#);
    assert_eq!(fired_unfolded(&pre, &[blocks(&pre)]), 1);
    let pre = board(r#"{"Blocks":{"what":[]}}"#);
    assert_eq!(fired_unfolded(&pre, &[blocks(&pre)]), 2);
}

#[rstest]
#[case::two_of_mine(
    r#"{"GroupAttacks":{"what":[{"ControlledBy":"You"}],"at_least":2}}"#,
    2,
    1
)]
#[case::one_of_two_needed(
    r#"{"GroupAttacks":{"what":[{"ControlledBy":"You"}],"at_least":2}}"#,
    1,
    0
)]
#[case::one_will_do(r#"{"GroupAttacks":{"what":[{"ControlledBy":"You"}]}}"#, 1, 1)]
#[case::three_needed(
    r#"{"GroupAttacks":{"what":[{"ControlledBy":"You"}],"at_least":3}}"#,
    2,
    0
)]
fn attackers_declared_together_trigger_once_when_enough_of_them_match(
    #[case] when: &str,
    #[case] attacking: usize,
    #[case] expected: usize,
) {
    let pre = board(when);
    let attackers = ["Bear", "Watcher"][..attacking]
        .iter()
        .map(|name| (id(&pre, name), AttackTarget::Player(FOE)))
        .collect();
    let declared = Event::AttackersDeclared {
        player: ME,
        attackers,
    };
    let found = detect(&pre, &[declared], &pre);
    assert_eq!(found.len(), expected);
    if expected == 1 {
        assert_eq!(found[0].cause.subjects.len(), attacking);
    }
}

#[rstest]
#[case::an_aura_to_a_creature(
    r#"{"Attached":{"what":[{"Subtype":"Aura"}],"to":[{"Type":"Creature"}]}}"#,
    "Cloak",
    "Bear",
    1
)]
#[case::to_this_creature(
    r#"{"Attached":{"what":[{"Subtype":"Aura"}],"to":["This"]}}"#,
    "Cloak",
    "Watcher",
    1
)]
#[case::not_to_another(
    r#"{"Attached":{"what":[{"Subtype":"Aura"}],"to":["This"]}}"#,
    "Cloak",
    "Bear",
    0
)]
#[case::to_a_creature_i_control(
    r#"{"Attached":{"to":[{"ControlledBy":"You"}]}}"#,
    "Cloak",
    "Bear",
    1
)]
#[case::not_to_a_creature_i_do_not(
    r#"{"Attached":{"to":[{"ControlledBy":"You"}]}}"#,
    "Cloak",
    "Ogre",
    0
)]
#[case::not_a_creature_attaching(
    r#"{"Attached":{"what":[{"Subtype":"Aura"}]}}"#,
    "Bear",
    "Ogre",
    0
)]
fn an_attachment_becoming_attached_triggers_what_names_it_and_what_it_is_attached_to(
    #[case] when: &str,
    #[case] attachment: &str,
    #[case] host: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let attached = Event::Attached {
        object: id(&pre, attachment),
        to: id(&pre, host),
    };
    assert_eq!(fired_unfolded(&pre, &[attached]), expected);
}

#[rstest]
#[case::a_creature_of_mine(
    r#"{"Tapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Bear",
    1
)]
#[case::not_a_creature_of_theirs(
    r#"{"Tapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}"#,
    "Ogre",
    0
)]
#[case::any_permanent(r#"{"Tapped":{"what":[]}}"#, "Ogre", 1)]
#[case::a_land(r#"{"Tapped":{"what":[{"Type":"Land"}]}}"#, "Forest", 1)]
#[case::not_a_land(r#"{"Tapped":{"what":[{"Type":"Land"}]}}"#, "Bear", 0)]
fn a_permanent_becoming_tapped_triggers_what_names_it(
    #[case] when: &str,
    #[case] permanent: &str,
    #[case] expected: usize,
) {
    let pre = board(when);
    let tapped = Event::PermanentTapped {
        object: id(&pre, permanent),
    };
    assert_eq!(fired(&pre, &[tapped]), expected);
}
