//! Goal: a fight plan becomes a playable game at every party size: the boss's life, hand, boss cards, extra combats,
//! phases, scheme deck and minions follow the scaling table for the party and the Heat and replay from the seed; every
//! seat starts with its life, hand and emblems, the boss with its scheme deck and the minions bound to it.
use std::sync::Arc;

use mtg_campaign::scale::{ENRAGE_TURN, enrage_id};
use mtg_campaign::{Run, RunEvent, Scale, content};
use mtg_cards::CardDefinition;
use mtg_engine::format::Commander;
use mtg_engine::object::ObjectKind;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::fixture_deck;
use rstest::rstest;

/// A deck card by name: a basic land from the testkit, anything else from the corpus.
fn card(name: &str) -> Arc<CardDefinition> {
    use mtg_core::color::Color;
    let basic = [
        ("Plains", Color::White),
        ("Island", Color::Blue),
        ("Swamp", Color::Black),
        ("Mountain", Color::Red),
        ("Forest", Color::Green),
    ];
    match basic.iter().find(|(basic, _)| *basic == name) {
        Some((_, color)) => mtg_testkit::cards::land(*color),
        None => corpus::card(name).unwrap(),
    }
}

fn boss_deck(deck: &[(String, u32)]) -> Vec<Arc<CardDefinition>> {
    deck.iter()
        .flat_map(|(name, n)| (0..*n).map(move |_| card(name)))
        .collect()
}

fn run_at_first_boss(heroes: usize, heat: u8) -> Run {
    let content = content().unwrap();
    let mut events = Vec::new();
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("H{at}"),
            user: Some(format!("u{at}")),
            bot: None,
        });
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(at).unwrap(),
            deck: mtg_campaign::HeroDeck {
                id: "x".into(),
                name: "x".into(),
                colors: "G".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    Run::replay(content, "warrens", 21, heat, &events).unwrap()
}

/// How often each custom card appears in `ids`.
fn copies(ids: &[String], id: &str) -> usize {
    ids.iter().filter(|x| *x == id).count()
}

/// One row of the scaling table for the first boss: life in percent of the party-size base, Reserves, Reinforcements,
/// Warpaths, minions (one fewer below Heat 2), the Last Bulwark.
type Row = (i32, usize, usize, usize, usize, bool);

#[rstest]
#[case::solo(1, 0, (100, 0, 0, 0, 0, false))]
#[case::duo(2, 0, (100, 1, 0, 0, 0, false))]
#[case::three(3, 0, (100, 0, 1, 0, 0, true))]
#[case::four(4, 0, (85, 0, 1, 0, 0, true))]
#[case::five(5, 0, (100, 0, 2, 0, 0, true))]
#[case::six(6, 0, (90, 0, 2, 1, 1, true))]
#[case::four_at_heat_1(4, 1, (85, 0, 1, 0, 0, true))]
#[case::five_at_heat_2(5, 2, (100, 0, 2, 0, 1, true))]
#[case::six_at_heat_2(6, 2, (90, 0, 2, 1, 2, true))]
#[case::two_at_heat_2(2, 2, (100, 1, 0, 0, 0, false))]
fn the_first_boss_scales_with_the_party_and_the_heat(
    #[case] heroes: usize,
    #[case] heat: u8,
    #[case] row: Row,
) {
    let (life, reserves, reinforcements, warpaths, minions, bulwark) = row;
    let content = content().unwrap();
    let theme = &content.themes[0];
    let run = run_at_first_boss(heroes, heat);
    let plan = run.fight_plan(content).unwrap().unwrap();
    let boss = theme.boss(&plan.boss.boss).unwrap();
    let extra = i32::try_from(heroes - 1).unwrap();
    let base = (boss.life + boss.life_per_hero * extra) * life / 100;
    let tier = Scale::new(heroes, heat, 0).tier();
    assert_eq!(plan.boss.life, base * tier.life / 100);
    let hand = i32::try_from(boss.hand).unwrap() - 5 + extra + tier.hand;
    assert_eq!(plan.boss.hand, u32::try_from(hand.max(1)).unwrap());
    assert_eq!(
        plan.boss.brain,
        Scale::new(heroes, heat, 0).brain(boss.brain)
    );
    let rules = &plan.boss.rules;
    assert_eq!(copies(rules, "reinforcements"), reinforcements);
    assert_eq!(copies(rules, "reserves"), reserves);
    assert_eq!(copies(rules, "warpath"), warpaths);
    assert_eq!(
        (
            copies(rules, "escalation"),
            copies(
                rules,
                &enrage_id(&boss.enrage, ENRAGE_TURN[usize::from(heat)])
            )
        ),
        (1, 1)
    );
    // One modifier of the pool when the tier draws one; the first boss brings no relic.
    let modifiers = rules
        .iter()
        .filter(|id| theme.modifiers.contains(id))
        .count();
    assert_eq!(modifiers, usize::from(tier.modifier));
    assert_eq!(copies(rules, &boss.relic), 0);
    // The boss's own phases at their share of its life, the Last Bulwark with the last of them.
    for ((_, at), (_, percent)) in plan.boss.phases.iter().zip(&boss.phases) {
        assert_eq!(*at, plan.boss.life * i32::try_from(*percent).unwrap() / 100);
    }
    assert_eq!(
        plan.boss.phases.len(),
        boss.phases.len() + usize::from(bulwark)
    );
    if bulwark {
        let last = &plan.boss.phases[boss.phases.len() - 1];
        assert_eq!(
            plan.boss.phases.last().unwrap(),
            &("bulwark".to_owned(), last.1)
        );
    }
    // The whole scheme deck, in an order of its own.
    let mut schemes = plan.boss.schemes.clone();
    schemes.sort();
    let mut deck = theme.schemes.clone();
    deck.sort();
    assert_eq!(schemes, deck);
    // Minions with another lieutenant's deck, their life at the tier's share.
    assert_eq!(plan.minions.len(), minions);
    for minion in &plan.minions {
        let spec = theme
            .minions
            .iter()
            .find(|m| m.id == minion.minion)
            .unwrap();
        assert_ne!(spec.deck_of, boss.id);
        assert_eq!(minion.deck, theme.boss(&spec.deck_of).unwrap().deck);
        assert_eq!(minion.life, spec.life * tier.life / 100);
    }
    let house = plan.house_rules(&mtg_core::house::HouseRules::default());
    assert_eq!(house.handicaps.len(), heroes + 1 + minions);
    let seats = house.teams.unwrap().seats;
    assert_eq!(seats.iter().filter(|t| **t == 1).count(), 1 + minions);
    assert_eq!(seats.iter().filter(|t| **t == 0).count(), heroes);
}

#[rstest]
#[case(1)]
#[case(4)]
#[case(6)]
fn a_plan_replays_from_the_seed(#[case] heroes: usize) {
    let content = content().unwrap();
    let first = run_at_first_boss(heroes, 0).fight_plan(content).unwrap();
    let again = run_at_first_boss(heroes, 0).fight_plan(content).unwrap();
    assert_eq!(first, again);
}

#[rstest]
fn a_planned_fight_starts_as_a_game_with_its_emblems(
    #[values(0, 2)] heat: u8,
    #[values(0, 3)] rank: usize,
) {
    let content = content().unwrap();
    let run = run_at_first_boss(6, heat);
    let plan = run.fight_plan_at(content, rank).unwrap().unwrap();
    let spec = content.themes[0].boss(&plan.boss.boss).unwrap();
    assert_eq!(plan.boss.life, Scale::new(6, heat, rank).life(spec));
    assert_eq!(plan.minions.len(), Scale::new(6, heat, rank).minion_count());
    let hero = fixture_deck("precon_tramplesaurus_rex").unwrap();
    let mut decks: Vec<Vec<Arc<CardDefinition>>> = (0..6).map(|_| hero.cards.clone()).collect();
    let mut commanders: Vec<Vec<Arc<CardDefinition>>> =
        (0..6).map(|_| hero.commanders.clone()).collect();
    for deck in plan.boss_decks() {
        decks.push(boss_deck(&deck));
        commanders.push(Vec::new());
    }
    let setup = GameSetup::new(Arc::new(Commander), 4, decks).with_commanders(commanders);
    let setup = plan.apply(content, setup).unwrap();
    let engine = Engine::new(setup).unwrap();
    let state = engine.state();
    let boss = plan.boss_seat();
    let boss_emblems: Vec<String> = state
        .zone(Zone::Command(boss))
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.kind() == ObjectKind::Emblem)
        .map(|o| o.card().name.clone())
        .collect();
    let expected: Vec<String> = plan
        .boss
        .rules
        .iter()
        .map(|id| content.card(id).unwrap().name.clone())
        .collect();
    assert_eq!(boss_emblems, expected);
    assert_eq!(state.player(boss).unwrap().life(), plan.boss.life);
    assert_eq!(
        state.player(boss).unwrap().phases().0,
        plan.boss.phases.len()
    );
    let first = content.card(&plan.boss.schemes[0]).unwrap();
    assert_eq!(
        state.player(boss).unwrap().schemes(),
        Some((None, first.name.as_str()))
    );
    for (at, minion) in plan.minions.iter().enumerate() {
        let seat = state.player(plan.minion_seat(at)).unwrap();
        assert_eq!(seat.leader(), Some(boss));
        assert_eq!(seat.life(), minion.life);
        assert_eq!(seat.schemes(), None);
    }
}

#[rstest]
fn each_boss_brings_its_relic_and_a_modifier_as_its_rank_and_heat_say(
    #[values(21, 5, 77)] seed: u64,
    #[values(0, 1, 2)] heat: u8,
) {
    let content = content().unwrap();
    let theme = &content.themes[0];
    let mut run = run_at_first_boss(2, heat);
    run.seed = seed;
    run.map = mtg_campaign::map::generate(theme, seed);
    let bosses: Vec<(u8, String)> = run
        .map
        .nodes
        .iter()
        .filter_map(|n| n.boss.clone().map(|b| (n.id, b)))
        .collect();
    assert_eq!(
        bosses.len(),
        theme.bosses.iter().filter(|b| b.since == 0).count()
    );
    for (rank, (node, id)) in bosses.iter().enumerate() {
        run.at = Some(*node);
        assert_eq!(run.boss_rank(*node), rank);
        let plan = run.fight_plan(content).unwrap().unwrap();
        let boss = theme.boss(id).unwrap();
        assert_eq!(
            copies(&plan.boss.rules, &boss.relic),
            usize::from(Scale::new(2, heat, rank).tier().relic),
            "{id}"
        );
        let drawn = plan
            .boss
            .rules
            .iter()
            .filter(|id| theme.modifiers.contains(id))
            .count();
        assert_eq!(
            drawn,
            usize::from(Scale::new(2, heat, rank).tier().modifier),
            "{id}"
        );
        assert_eq!(plan.boss.life, Scale::new(2, heat, rank).life(boss), "{id}");
        assert_eq!(plan.boss.scheme_every, theme.scheme_every);
    }
}
