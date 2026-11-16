//! Goal: every card a hero earns goes to their Satchel and their deck stays as chosen (loot, shop); the Mirror Pool
//! copies a card of the deck or Satchel into the Satchel and only runs of its content generation meet it; commander
//! upgrades are offered only where they do something for that commander (creature, planeswalker, generic mana, keywords
//! it lacks, power for combat keywords, each commander of a pair on its own); a tempered or bought upgrade goes into
//! the fight as an emblem, and the engine applies it to the commander at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_campaign::run::{CONTENT, UPGRADE_PRICE};
use mtg_campaign::upgrade::fitting;
use mtg_campaign::{
    Boon, Brain, CommanderFacts, Gift, HeroDeck, Offer, Pick, Run, RunEvent, ShopItem, Status,
    Upgrade, content,
};
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest, walker, wastes};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn facts(name: &str) -> CommanderFacts {
    CommanderFacts {
        name: name.into(),
        creature: true,
        planeswalker: false,
        power: Some(3),
        generic: 2,
        keywords: Vec::new(),
    }
}

fn deck(commanders: Vec<CommanderFacts>) -> HeroDeck {
    HeroDeck {
        id: "precon:RG".into(),
        name: "Deck".into(),
        colors: "RG".into(),
        commanders,
        makeup: None,
    }
}

/// A started run of one player and `bots` bot allies, every deck led by `commanders`, of content generation
/// `version` (none stored for 0).
fn started(
    seed: u64,
    bots: usize,
    version: u32,
    commanders: &[CommanderFacts],
) -> (Run, Vec<RunEvent>) {
    let content = content().unwrap();
    let mut events = Vec::new();
    if version > 0 {
        events.push(RunEvent::Content { version });
    }
    events.push(RunEvent::Joined {
        name: "Ana".into(),
        user: Some("u1".into()),
        bot: None,
    });
    for n in 0..bots {
        events.push(RunEvent::Joined {
            name: format!("Bot {n}"),
            user: None,
            bot: Some(Brain::Normal),
        });
    }
    for hero in 0..=bots {
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(hero).unwrap(),
            deck: deck(commanders.to_vec()),
        });
    }
    events.push(RunEvent::Started);
    (
        Run::replay(content, "warrens", seed, 0, &events).unwrap(),
        events,
    )
}

fn apply(run: &mut Run, events: &mut Vec<RunEvent>, event: RunEvent) {
    let content = content().unwrap();
    run.accepts(content, &event).unwrap();
    run.apply(content, &event).unwrap();
    events.push(event);
}

fn win(run: &mut Run, events: &mut Vec<RunEvent>) {
    apply(run, events, RunEvent::FightCreated { game: "g".into() });
    let lives = run.heroes.iter().map(|h| h.life).collect();
    apply(
        run,
        events,
        RunEvent::FightEnded {
            won: true,
            lives,
            spirits: vec![],
        },
    );
}

#[test]
fn a_loot_card_goes_to_the_satchel_and_the_deck_stays() {
    let (mut run, mut events) = started(3, 0, CONTENT, &[facts("Ruric")]);
    win(&mut run, &mut events);
    apply(
        &mut run,
        &mut events,
        RunEvent::Picked {
            hero: 0,
            pick: Pick::Skip,
        },
    );
    let Some(Offer::Cards { cards }) = run.heroes[0].offers.first().cloned() else {
        panic!("a card pick follows the relic");
    };
    apply(
        &mut run,
        &mut events,
        RunEvent::Picked {
            hero: 0,
            pick: Pick::Take(1),
        },
    );
    assert_eq!(run.heroes[0].satchel, vec![cards[1].clone()]);
    assert_eq!(run.heroes[0].deck, Some(deck(vec![facts("Ruric")])));
    let replayed = Run::replay(content().unwrap(), "warrens", 3, 0, &events).unwrap();
    assert_eq!(replayed, run);
}

/// The run with hero 0 holding `offer` in a camp, with `gold`.
fn holding(offer: Offer, gold: u32, commanders: &[CommanderFacts]) -> Run {
    let (mut run, _) = started(5, 1, CONTENT, commanders);
    run.status = Status::Room;
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.boss.is_none())
        .map(|n| n.id);
    run.heroes[0].gold = gold;
    run.heroes[0].offers = vec![offer];
    run.heroes[1].offers = vec![Offer::Campfire {
        upgrades: Vec::new(),
    }];
    run
}

fn pick(run: &mut Run, hero: u8, pick: Pick) -> Result<(), String> {
    let content = content().unwrap();
    let event = RunEvent::Picked { hero, pick };
    run.accepts(content, &event)
        .and_then(|()| run.apply(content, &event))
        .map_err(|e| e.to_string())
}

#[test]
fn a_bought_card_goes_to_the_satchel() {
    let items = vec![ShopItem::Card {
        name: "Sol Ring".into(),
        sold: false,
    }];
    let mut run = holding(Offer::Shop { items }, 100, &[]);
    pick(&mut run, 0, Pick::Take(0)).unwrap();
    assert_eq!(run.heroes[0].satchel, ["Sol Ring"]);
    assert_eq!(run.heroes[0].deck, Some(deck(Vec::new())));
}

#[test]
fn the_mirror_pool_copies_a_card_into_the_satchel() {
    let mut run = holding(
        Offer::Event {
            event: "mirror_pool".into(),
        },
        0,
        &[],
    );
    let content = content().unwrap();
    let spec = content
        .theme("warrens")
        .unwrap()
        .event("mirror_pool")
        .unwrap();
    assert_eq!(
        spec.options[0].summary(),
        "a copy of a card from your deck or Satchel"
    );
    pick(&mut run, 0, Pick::Take(0)).unwrap();
    assert_eq!(run.heroes[0].offers.first(), Some(&Offer::Duplicate));
    assert!(
        pick(&mut run, 0, Pick::Take(0)).is_err(),
        "the pool takes a card"
    );
    pick(&mut run, 0, Pick::Duplicate("Rampant Growth".into())).unwrap();
    assert_eq!(run.heroes[0].satchel, ["Rampant Growth"]);
    assert!(
        run.log
            .last()
            .unwrap()
            .contains("copied Rampant Growth into the Satchel")
    );
}

#[rstest]
#[case::copies_the_newest(vec!["Rancor".into(), "Opt".into()], Pick::Duplicate("Opt".into()))]
#[case::skips_with_nothing(vec![], Pick::Skip)]
fn a_bot_at_the_mirror_pool_copies_its_newest_satchel_card(
    #[case] satchel: Vec<String>,
    #[case] expected: Pick,
) {
    let mut run = holding(
        Offer::Campfire {
            upgrades: Vec::new(),
        },
        0,
        &[],
    );
    run.heroes[1].satchel = satchel;
    run.heroes[1].offers = vec![Offer::Duplicate];
    let moves = run.bot_moves(content().unwrap());
    assert!(moves.contains(&RunEvent::Picked {
        hero: 1,
        pick: expected
    }));
}

/// The first option of `event` hero 0 can afford.
fn affordable(run: &Run, event: &str) -> Pick {
    let content = content().unwrap();
    let h = &run.heroes[0];
    let at = content
        .theme("warrens")
        .unwrap()
        .event(event)
        .unwrap()
        .options
        .iter()
        .position(|o| o.needs().unmet(h.gold, h.life).is_none())
        .unwrap();
    Pick::Take(u8::try_from(at).unwrap())
}

/// Plays a run to its end, every answer the first that fits, and names every event the party met.
fn events_met(seed: u64, version: u32) -> Vec<String> {
    let content = content().unwrap();
    let (mut run, mut events) = started(seed, 0, version, &[facts("Ruric")]);
    let mut met = Vec::new();
    let mut steps = 0;
    while !run.is_over() && steps < 400 {
        steps += 1;
        let next = match &run.status {
            Status::Fight { game: None } => RunEvent::FightCreated { game: "g".into() },
            Status::Fight { game: Some(_) } => RunEvent::FightEnded {
                won: true,
                lives: vec![30],
                spirits: vec![],
            },
            Status::Map => RunEvent::Voted {
                hero: 0,
                node: run.map.choices(run.at)[0],
            },
            Status::Room => {
                let offer = run.heroes[0].offers[0].clone();
                if let Offer::Event { event } = &offer {
                    met.push(event.clone());
                }
                let pick = match offer {
                    Offer::Duplicate => Pick::Duplicate("Sol Ring".into()),
                    Offer::Campfire { upgrades } if !upgrades.is_empty() => Pick::Take(0),
                    Offer::Campfire { .. } => Pick::Rest,
                    Offer::Shop { .. } => Pick::Skip,
                    Offer::Cards { cards } if cards.is_empty() => Pick::Skip,
                    Offer::Relics { relics } if relics.is_empty() => Pick::Skip,
                    Offer::Event { event } => affordable(&run, &event),
                    Offer::Cards { .. } | Offer::Relics { .. } => Pick::Take(0),
                };
                RunEvent::Picked { hero: 0, pick }
            }
            _ => break,
        };
        apply(&mut run, &mut events, next);
    }
    assert_eq!(run.status, Status::Victory, "seed {seed}");
    assert_eq!(
        Run::replay(content, "warrens", seed, 0, &events).unwrap(),
        run
    );
    met
}

#[test]
fn only_a_run_of_the_mirror_pools_generation_meets_it() {
    let mut newer = 0;
    for seed in 0..48 {
        assert!(
            !events_met(seed, 0).contains(&"mirror_pool".to_owned()),
            "seed {seed}"
        );
        newer += events_met(seed, CONTENT)
            .iter()
            .filter(|e| *e == "mirror_pool")
            .count();
    }
    assert!(newer > 0, "a new run meets the Mirror Pool");
}

fn boons(commanders: &[CommanderFacts], taken: &[Upgrade]) -> Vec<(String, Boon)> {
    fitting(commanders, taken)
        .into_iter()
        .map(|u| (u.commander, u.boon))
        .collect()
}

const COMBAT: [Gift; 7] = [
    Gift::Flying,
    Gift::Trample,
    Gift::Vigilance,
    Gift::Lifelink,
    Gift::Menace,
    Gift::FirstStrike,
    Gift::Deathtouch,
];

#[rstest]
#[case::a_fighter(facts("A"), true, true, false)]
#[case::no_power(CommanderFacts { power: Some(0), ..facts("A") }, true, false, false)]
#[case::a_planeswalker(
    CommanderFacts { creature: false, planeswalker: true, power: None, ..facts("A") },
    false, false, true
)]
#[case::a_background(
    CommanderFacts { creature: false, power: None, generic: 1, ..facts("A") },
    false, false, false
)]
fn upgrades_follow_the_commanders_type(
    #[case] commander: CommanderFacts,
    #[case] bodies: bool,
    #[case] combat: bool,
    #[case] loyalty: bool,
) {
    let offered = boons(std::slice::from_ref(&commander), &[]);
    let has = |boon: &Boon| offered.iter().any(|(_, b)| b == boon);
    assert_eq!(has(&Boon::Pump(1)), bodies);
    assert_eq!(has(&Boon::Pump(2)), bodies);
    assert_eq!(has(&Boon::Counter), bodies);
    assert_eq!(has(&Boon::Keyword(Gift::Hexproof)), bodies);
    assert_eq!(has(&Boon::Keyword(Gift::Haste)), bodies);
    for gift in COMBAT {
        assert_eq!(has(&Boon::Keyword(gift)), combat, "{gift:?}");
    }
    assert_eq!(has(&Boon::Loyalty), loyalty);
    assert!(has(&Boon::Cheaper(1)));
    assert_eq!(has(&Boon::Cheaper(2)), commander.generic >= 2);
}

#[rstest]
#[case::no_generic(0, &[], &[])]
#[case::one_generic(1, &[], &[1])]
#[case::two_generic(2, &[], &[1, 2])]
#[case::one_taken_of_two(2, &[1], &[1])]
#[case::all_taken(2, &[2], &[])]
fn a_cost_reduction_never_takes_more_than_the_generic_mana(
    #[case] generic: u32,
    #[case] taken: &[u32],
    #[case] offered: &[u32],
) {
    let commander = CommanderFacts {
        generic,
        ..facts("A")
    };
    let taken: Vec<Upgrade> = taken
        .iter()
        .map(|n| Upgrade {
            commander: "A".into(),
            boon: Boon::Cheaper(*n),
        })
        .collect();
    let cheaper: Vec<u32> = boons(&[commander], &taken)
        .into_iter()
        .filter_map(|(_, b)| {
            if let Boon::Cheaper(n) = b {
                Some(n)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(cheaper, offered);
}

#[test]
fn a_keyword_it_has_or_an_upgrade_it_took_is_not_offered_again() {
    let commander = CommanderFacts {
        keywords: vec!["Flying".into(), "First strike".into()],
        ..facts("A")
    };
    let taken = [Upgrade {
        commander: "A".into(),
        boon: Boon::Pump(1),
    }];
    let offered = boons(&[commander], &taken);
    for boon in [
        Boon::Keyword(Gift::Flying),
        Boon::Keyword(Gift::FirstStrike),
        Boon::Pump(1),
    ] {
        assert!(!offered.iter().any(|(_, b)| *b == boon), "{boon:?}");
    }
    assert!(offered.iter().any(|(_, b)| *b == Boon::Pump(2)));
}

#[test]
fn partners_are_upgraded_each_for_itself() {
    let walker_facts = CommanderFacts {
        creature: false,
        planeswalker: true,
        power: None,
        ..facts("Walker")
    };
    let offered = boons(&[facts("Fighter"), walker_facts], &[]);
    assert!(offered.contains(&("Fighter".into(), Boon::Pump(1))));
    assert!(!offered.contains(&("Walker".into(), Boon::Pump(1))));
    assert!(offered.contains(&("Walker".into(), Boon::Loyalty)));
    assert!(!offered.contains(&("Fighter".into(), Boon::Loyalty)));
}

#[test]
fn a_hero_without_commander_facts_is_offered_no_upgrade() {
    assert_eq!(fitting(&[], &[]), Vec::new());
}

#[test]
fn a_tempered_and_a_bought_upgrade_go_into_the_fight_as_emblems() {
    let commanders = [facts("Ruric")];
    let upgrades = fitting(&commanders, &[]);
    let mut run = holding(
        Offer::Campfire {
            upgrades: upgrades[..3].to_vec(),
        },
        0,
        &commanders,
    );
    pick(&mut run, 0, Pick::Take(2)).unwrap();
    assert_eq!(run.heroes[0].upgrades, vec![upgrades[2].clone()]);
    let item = ShopItem::Upgrade {
        upgrade: upgrades[4].clone(),
        sold: false,
    };
    run.heroes[0].offers = vec![Offer::Shop { items: vec![item] }];
    assert!(pick(&mut run, 0, Pick::Take(0)).is_err(), "no gold");
    run.heroes[0].gold = UPGRADE_PRICE;
    pick(&mut run, 0, Pick::Take(0)).unwrap();
    assert_eq!(run.heroes[0].gold, 0);
    assert_eq!(run.heroes[0].upgrades.len(), 2);
    run.status = Status::Fight { game: None };
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.boss.is_some())
        .map(|n| n.id);
    let plan = run.fight_plan(content().unwrap()).unwrap().unwrap();
    assert_eq!(plan.heroes[0].upgrades, run.heroes[0].upgrades);
    for upgrade in &plan.heroes[0].upgrades {
        let emblem = upgrade.definition().unwrap();
        assert!(emblem.name.contains(&upgrade.words()));
    }
}

#[test]
fn a_camp_offers_only_upgrades_that_fit_and_a_deck_without_facts_gets_none() {
    for (commanders, any) in [(vec![facts("Ruric")], true), (Vec::new(), false)] {
        let mut seen = false;
        for seed in 0..24 {
            let (mut run, mut events) = started(seed, 0, CONTENT, &commanders);
            while !run.is_over() && run.cleared.len() < 6 {
                let next = match &run.status {
                    Status::Fight { game: None } => RunEvent::FightCreated { game: "g".into() },
                    Status::Fight { game: Some(_) } => RunEvent::FightEnded {
                        won: true,
                        lives: vec![30],
                        spirits: vec![],
                    },
                    Status::Map => RunEvent::Voted {
                        hero: 0,
                        node: run.map.choices(run.at)[0],
                    },
                    Status::Room => {
                        let fits = fitting(&commanders, &run.heroes[0].upgrades);
                        let pick = match &run.heroes[0].offers[0] {
                            Offer::Campfire { upgrades } => {
                                assert!(upgrades.iter().all(|u| fits.contains(u)));
                                seen |= !upgrades.is_empty();
                                Pick::Rest
                            }
                            Offer::Shop { items } => {
                                for item in items {
                                    if let ShopItem::Upgrade { upgrade, .. } = item {
                                        assert!(fits.contains(upgrade));
                                        seen = true;
                                    }
                                }
                                Pick::Skip
                            }
                            Offer::Duplicate => Pick::Skip,
                            Offer::Cards { cards } if cards.is_empty() => Pick::Skip,
                            Offer::Relics { relics } if relics.is_empty() => Pick::Skip,
                            Offer::Event { event } => affordable(&run, event),
                            Offer::Cards { .. } | Offer::Relics { .. } => Pick::Take(0),
                        };
                        RunEvent::Picked { hero: 0, pick }
                    }
                    _ => break,
                };
                apply(&mut run, &mut events, next);
            }
        }
        assert_eq!(seen, any);
    }
}

/// Seat 0's commander Ruric ({2}{G}, 2/2) with `upgrades` as emblems, two Forests, and a planeswalker commander for
/// seat 1 with the loyalty upgrade.
fn upgraded_table(seats: usize, upgrades: &[Boon]) -> Scenario {
    let emblems: Vec<_> = upgrades
        .iter()
        .map(|boon| {
            Upgrade {
                commander: "Ruric".into(),
                boon: boon.clone(),
            }
            .definition()
            .unwrap()
        })
        .collect();
    let loyalty = Upgrade {
        commander: "Tamiyo".into(),
        boon: Boon::Loyalty,
    }
    .definition()
    .unwrap();
    Scenario::new(seats)
        .format(Arc::new(mtg_engine::format::Commander))
        .commanders(0, [creature("Ruric", 2, &[Color::Green], 2, 2)])
        .emblems(0, emblems)
        .battlefield(0, [ready(forest()), ready(forest())])
        .commanders(1, [walker("Tamiyo", 1)])
        .emblems(1, [loyalty])
        .battlefield(1, [ready(wastes())])
}

#[rstest]
fn the_engine_applies_a_commanders_upgrades(#[values(2, 3, 4)] seats: usize) {
    let boons = [
        Boon::Cheaper(1),
        Boon::Pump(1),
        Boon::Counter,
        Boon::Keyword(Gift::Flying),
    ];
    let mut driver = Driver::from_scenario(upgraded_table(seats, &boons)).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    // {2}{G} less {1} is paid with the two Forests.
    driver.cast("Ruric").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let ruric = driver.id_of("Ruric").unwrap();
    let ch = characteristics(driver.state(), ruric).unwrap();
    // 2/2, +1/+1 from the upgrade and a +1/+1 counter.
    assert_eq!(ch.power, Some(4));
    assert!(has_keyword(driver.state(), ruric, &Keyword::Flying));
    let counters = driver
        .state()
        .object(ruric)
        .and_then(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne));
    assert_eq!(counters, Some(1));
    // Seat 1's planeswalker enters with one more loyalty than printed.
    driver.idle_until(2, Step::Main1).unwrap();
    driver.cast("Tamiyo").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let tamiyo = driver.id_of("Tamiyo").unwrap();
    let loyalty = driver
        .state()
        .object(tamiyo)
        .and_then(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::Loyalty));
    assert_eq!(loyalty, Some(4));
}

#[test]
fn without_the_upgrade_the_commander_costs_its_whole_cost() {
    let mut driver = Driver::from_scenario(upgraded_table(2, &[])).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    assert!(
        driver.begin_cast("Ruric").is_err(),
        "{{2}}{{G}} is more than two Forests"
    );
}
