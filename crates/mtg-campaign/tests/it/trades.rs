//! Goal: from the rarity generation on every event is a known trade-off: no option gambles, each states its cost and
//! reward, the reward grows with the cost and every event keeps a free option with a small reward; each cost is applied
//! exactly as written (life, gold, a cut to the life cap that lasts the run, a card or relic given up, a commander bane
//! that goes into each fight as an emblem the engine applies); the same answer does the same on every seed, no option
//! can leave a hero dead or under the least life cap, and a bot ally takes the best trade it can.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use mtg_campaign::content::MIN_LIFE_CAP;
use mtg_campaign::relics::RARITY_FROM;
use mtg_campaign::run::CONTENT;
use mtg_campaign::{
    Bane, Boon, Brain, CommanderFacts, Content, EventOption, HeroDeck, Offer, Outcome, Pick,
    Rarity, RoomKind, Run, RunEvent, Status, Upgrade, content,
};
use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The events of the trade generation.
fn trades(content: &Content) -> Vec<&mtg_campaign::content::EventSpec> {
    content
        .theme("warrens")
        .unwrap()
        .events
        .iter()
        .filter(|e| e.in_generation(CONTENT))
        .collect()
}

/// How heavy a cost is, in the points the content's rule of thumb reads: life 1, life cap 2, a card 5, a relic 10, a bane
/// 8 per step, gold 1 per 5.
fn weight(option: &EventOption) -> i32 {
    option
        .outcomes
        .iter()
        .map(|outcome| match outcome {
            Outcome::Gold(n) if *n < 0 => -n / 5,
            Outcome::Life(n) if *n < 0 => -n,
            Outcome::MaxLife(n) if *n < 0 => -2 * n,
            Outcome::LoseCard => 5,
            Outcome::LoseRelic => 10,
            Outcome::Bane(Bane::Tax(n)) => 8 * i32::try_from(*n).unwrap(),
            Outcome::Bane(Bane::Shrink(n)) => 8 * n,
            _ => 0,
        })
        .sum()
}

fn relic_reward(option: &EventOption) -> Option<Rarity> {
    option.outcomes.iter().find_map(|o| match o {
        Outcome::RelicOf(rarity) => Some(*rarity),
        _ => None,
    })
}

#[test]
fn every_trade_event_is_known_up_front_and_keeps_a_free_option() {
    let content = content().unwrap();
    let events = trades(content);
    assert!(events.len() >= 10, "{} events", events.len());
    for event in events {
        assert_eq!(event.since, RARITY_FROM, "{}", event.id);
        let free = event
            .options
            .iter()
            .any(|o| o.outcomes.iter().all(|x| !x.is_cost()) && !o.outcomes.is_empty());
        assert!(free, "{} has no free option with a reward", event.id);
        for option in &event.options {
            assert!(
                option
                    .outcomes
                    .iter()
                    .all(|o| !matches!(o, Outcome::Gamble { .. } | Outcome::Relic)),
                "{}: {} gambles or offers an untiered relic",
                event.id,
                option.label
            );
            assert_ne!(option.reward(), "nothing", "{}: {}", event.id, option.label);
            // The cost and reward split the summary between them.
            let parts = option.outcomes.len();
            let words = |s: String| {
                if s == "nothing" {
                    0
                } else {
                    s.split(", ").count()
                }
            };
            assert_eq!(words(option.cost()) + words(option.reward()), parts);
        }
    }
}

#[test]
fn a_relic_reward_grows_with_its_cost() {
    let content = content().unwrap();
    let options: Vec<&EventOption> = trades(content)
        .into_iter()
        .flat_map(|e| e.options.iter())
        .collect();
    let least = |tier: Rarity| {
        options
            .iter()
            .filter(|o| relic_reward(o) == Some(tier))
            .map(|o| weight(o))
            .min()
            .unwrap()
    };
    let most = |tier: Rarity| {
        options
            .iter()
            .filter(|o| relic_reward(o) == Some(tier))
            .map(|o| weight(o))
            .max()
            .unwrap()
    };
    assert!(least(Rarity::Common) >= 3, "a Common relic is never free");
    assert!(least(Rarity::Rare) > most(Rarity::Common));
    assert!(least(Rarity::Mythic) > most(Rarity::Common));
    assert!(
        least(Rarity::Mythic) >= 15,
        "a Mythic relic demands a heavy cost"
    );
    assert!(
        options
            .iter()
            .filter(|o| relic_reward(o) == Some(Rarity::Mythic))
            .count()
            >= 4,
        "the events are where Mythic relics come from"
    );
}

fn ruric() -> CommanderFacts {
    CommanderFacts {
        name: "Ruric".into(),
        creature: true,
        planeswalker: false,
        power: Some(2),
        generic: 2,
        keywords: Vec::new(),
    }
}

/// A run of the trade generation at `event` for a player and `bots` bot allies; each holds 100 gold, `life` life, two
/// relics, a Satchel card and Ruric as commander.
fn at_trade(content: &Content, event: &str, life: i32, bots: usize, seed: u64) -> Run {
    let mut events = vec![
        RunEvent::Content { version: CONTENT },
        RunEvent::Joined {
            name: "Ana".into(),
            user: Some("u1".into()),
            bot: None,
        },
    ];
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
            deck: HeroDeck {
                id: "precon:x".into(),
                name: "Deck".into(),
                colors: "RG".into(),
                commanders: vec![ruric()],
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    let mut run = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    run.status = Status::Room;
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.kind != RoomKind::Boss)
        .map(|n| n.id);
    for hero in &mut run.heroes {
        hero.gold = 100;
        hero.life = life;
        hero.relics = vec!["hearthstone".into(), "seers_lantern".into()];
        hero.satchel = vec!["Llanowar Elves".into()];
        hero.offers = vec![Offer::Event {
            event: event.into(),
        }];
    }
    run
}

fn answer(run: &mut Run, hero: u8, pick: Pick) -> Result<(), String> {
    let content = content().unwrap();
    let event = RunEvent::Picked { hero, pick };
    run.accepts(content, &event)
        .and_then(|()| run.apply(content, &event))
        .map_err(|e| e.to_string())
}

/// Answers the follow-ups an option left the hero: the first card or relic offered, the Satchel card to give up, the
/// first relic to give up.
fn answer_follow_ups(run: &mut Run) {
    while let Some(offer) = run.heroes[0].offers.first().cloned() {
        let pick = match offer {
            Offer::Discard => Pick::Discard(run.heroes[0].satchel[0].clone()),
            Offer::Surrender { .. } | Offer::Relics { .. } | Offer::Cards { .. } => Pick::Take(0),
            Offer::Duplicate => Pick::Duplicate("Llanowar Elves".into()),
            other => panic!("{other:?}"),
        };
        answer(run, 0, pick).unwrap();
        if run.status != Status::Room {
            break;
        }
    }
}

#[test]
fn every_option_costs_exactly_what_it_says() {
    let content = content().unwrap();
    for event in trades(content) {
        for (at, option) in event.options.iter().enumerate() {
            let mut run = at_trade(content, &event.id, 30, 0, 7);
            let before = run.heroes[0].clone();
            let cap = before.max_life(content);
            answer(&mut run, 0, Pick::Take(u8::try_from(at).unwrap())).unwrap();
            // The follow-ups wait in the order the outcomes are written.
            let follow: Vec<&Offer> = run.heroes[0].offers.iter().collect();
            let mut expected_follow = 0;
            let mut gold = i64::from(before.gold);
            let mut life = before.life;
            let mut life_cap = 0;
            let mut banes = Vec::new();
            for outcome in &option.outcomes {
                match outcome {
                    Outcome::Gold(n) => gold += i64::from(*n),
                    Outcome::Life(n) => life = (life + n).clamp(1, cap + life_cap),
                    Outcome::MaxLife(n) => {
                        life_cap += n;
                        life = life.min(cap + life_cap);
                    }
                    Outcome::Bane(bane) => banes.push(Upgrade {
                        commander: "Ruric".into(),
                        boon: bane.boon(),
                    }),
                    Outcome::RelicOf(tier) => {
                        let Offer::Relics { relics } = follow[expected_follow] else {
                            panic!("{}: a relic pick", event.id)
                        };
                        assert_eq!(relics.len(), 3, "{}", event.id);
                        for id in relics {
                            assert_eq!(content.card(id).unwrap().rarity, *tier, "{id}");
                        }
                        expected_follow += 1;
                    }
                    Outcome::LoseRelic => {
                        assert_eq!(
                            follow[expected_follow],
                            &Offer::Surrender {
                                relics: before.relics.clone()
                            }
                        );
                        expected_follow += 1;
                    }
                    Outcome::LoseCard => {
                        assert_eq!(follow[expected_follow], &Offer::Discard);
                        expected_follow += 1;
                    }
                    Outcome::CardPick | Outcome::Duplicate => expected_follow += 1,
                    other => panic!("{}: {other:?}", event.id),
                }
            }
            let hero = &run.heroes[0];
            let label = format!("{}: {}", event.id, option.label);
            assert_eq!(i64::from(hero.gold), gold, "{label}");
            assert_eq!(hero.life, life, "{label}");
            assert_eq!(hero.life_cap, life_cap, "{label}");
            assert_eq!(hero.upgrades, banes, "{label}");
            assert_eq!(follow.len(), expected_follow, "{label}");
            assert!(run.results[0].rolls.is_empty(), "{label}");
            // The costs that wait for a choice are paid as the hero answers them.
            answer_follow_ups(&mut run);
            let hero = &run.heroes[0];
            let lost_relics = option.outcomes.contains(&Outcome::LoseRelic);
            let gained_relics = relic_reward(option).is_some();
            assert_eq!(
                hero.relics.len(),
                2 - usize::from(lost_relics) + usize::from(gained_relics),
                "{label}"
            );
            if option.outcomes.contains(&Outcome::LoseCard) {
                assert!(
                    !hero.satchel.contains(&"Llanowar Elves".to_owned()),
                    "{label}"
                );
            }
            assert!(
                hero.life >= 1 && hero.max_life(content) >= MIN_LIFE_CAP,
                "{label}"
            );
        }
    }
}

#[test]
fn the_same_answer_does_the_same_on_every_seed() {
    let content = content().unwrap();
    for event in trades(content) {
        for at in 0..event.options.len() {
            let outcome = |seed: u64| {
                let mut run = at_trade(content, &event.id, 30, 0, seed);
                answer(&mut run, 0, Pick::Take(u8::try_from(at).unwrap())).unwrap();
                let hero = &run.heroes[0];
                (
                    run.results[0].outcomes.clone(),
                    hero.gold,
                    hero.life,
                    hero.life_cap,
                    hero.upgrades.clone(),
                )
            };
            let first = outcome(1);
            for seed in 2..12 {
                assert_eq!(outcome(seed), first, "{} option {at}", event.id);
            }
        }
    }
}

#[rstest]
#[case::the_cut_leaves_the_least_cap("bleeding_altar", 1, 40, true)]
#[case::the_cut_would_go_below_it("bleeding_altar", 1, 25, false)]
#[case::life_to_spare("bleeding_altar", 0, 40, true)]
fn a_cut_or_a_life_cost_never_leaves_a_hero_unable_to_go_on(
    #[case] event: &str,
    #[case] option: u8,
    #[case] cap: i32,
    #[case] allowed: bool,
) {
    let content = content().unwrap();
    let mut run = at_trade(content, event, 30, 0, 3);
    run.heroes[0].life_cap = cap - 40;
    run.heroes[0].life = cap.min(30);
    let got = answer(&mut run, 0, Pick::Take(option));
    assert_eq!(got.is_ok(), allowed, "{got:?}");
    // The life cost: 15 of 30 life leaves 15; with 15 life the option is shut.
    let mut low = at_trade(content, event, 15, 0, 3);
    assert!(
        answer(&mut low, 0, Pick::Take(0))
            .unwrap_err()
            .contains("life")
    );
}

#[test]
fn a_relic_to_give_up_and_a_creature_commander_to_weaken_are_needed() {
    let content = content().unwrap();
    let mut run = at_trade(content, "kings_forge", 30, 0, 3);
    run.heroes[0].relics.clear();
    assert!(
        answer(&mut run, 0, Pick::Take(0))
            .unwrap_err()
            .contains("relic")
    );
    let mut run = at_trade(content, "mirror_pool_trade", 30, 0, 3);
    if let Some(deck) = run.heroes[0].deck.as_mut() {
        deck.commanders[0].creature = false;
    }
    assert!(
        answer(&mut run, 0, Pick::Take(1))
            .unwrap_err()
            .contains("creature commander")
    );
    // A relic surrender and a card loss cannot be skipped.
    let mut run = at_trade(content, "kings_forge", 30, 0, 3);
    answer(&mut run, 0, Pick::Take(0)).unwrap();
    assert!(answer(&mut run, 0, Pick::Skip).is_err());
    let mut run = at_trade(content, "kings_forge", 30, 0, 3);
    answer(&mut run, 0, Pick::Take(1)).unwrap();
    assert!(answer(&mut run, 0, Pick::Skip).is_err());
}

#[test]
fn a_card_given_up_leaves_the_satchel_first_and_else_the_deck_for_the_run() {
    let content = content().unwrap();
    let mut run = at_trade(content, "kings_forge", 30, 0, 3);
    answer(&mut run, 0, Pick::Take(1)).unwrap();
    answer(&mut run, 0, Pick::Discard("Llanowar Elves".into())).unwrap();
    assert_eq!(run.heroes[0].satchel, Vec::<String>::new());
    assert_eq!(run.heroes[0].lost, Vec::<String>::new());
    let mut run = at_trade(content, "kings_forge", 30, 0, 3);
    answer(&mut run, 0, Pick::Take(1)).unwrap();
    answer(&mut run, 0, Pick::Discard("Sol Ring".into())).unwrap();
    assert_eq!(run.heroes[0].lost, ["Sol Ring"]);
    assert_eq!(
        run.log
            .iter()
            .filter(|l| l.contains("gave up the card Sol Ring"))
            .count(),
        1
    );
}

#[test]
fn a_cut_life_cap_lasts_through_heals_rests_and_regroups() {
    let content = content().unwrap();
    let mut run = at_trade(content, "bleeding_altar", 40, 0, 3);
    answer(&mut run, 0, Pick::Take(1)).unwrap();
    assert_eq!(run.heroes[0].max_life(content), 30);
    assert_eq!(run.heroes[0].life, 30);
    answer(&mut run, 0, Pick::Take(0)).unwrap();
    // The party walks into the next boss and wins: the heal stops at the cut cap.
    assert!(
        matches!(run.status, Status::Fight { .. }),
        "{:?}",
        run.status
    );
    run.apply(content, &RunEvent::FightCreated { game: "g".into() })
        .unwrap();
    run.apply(
        content,
        &RunEvent::FightEnded {
            won: true,
            lives: vec![20],
            spirits: vec![],
        },
    )
    .unwrap();
    assert_eq!(run.heroes[0].life, 30);
    assert_eq!(run.heroes[0].life_cap, -10);
    // A rest and a lost fight's regroup read the same cap.
    run.heroes[0].life = 5;
    run.heroes[0].offers = vec![Offer::Campfire {
        upgrades: Vec::new(),
    }];
    answer(&mut run, 0, Pick::Rest).unwrap();
    assert_eq!(run.heroes[0].life, 25);
}

#[test]
fn a_bane_goes_into_the_fight_as_an_emblem() {
    let content = content().unwrap();
    let mut run = at_trade(content, "duelling_statue_trade", 30, 0, 3);
    answer(&mut run, 0, Pick::Take(1)).unwrap();
    let bane = Upgrade {
        commander: "Ruric".into(),
        boon: Boon::Tax(1),
    };
    assert_eq!(run.heroes[0].upgrades, std::slice::from_ref(&bane));
    assert!(bane.is_bane());
    assert_eq!(bane.words(), "Ruric costs {1} more to cast");
    answer(&mut run, 0, Pick::Take(0)).unwrap();
    let plan = run.fight_plan(content).unwrap().unwrap();
    assert_eq!(plan.heroes[0].upgrades, std::slice::from_ref(&bane));
    assert!(bane.definition().unwrap().name.starts_with("Bane: "));
}

/// Seat 0's commander Ruric ({2}{G}, 2/2) with `banes` as emblems over `forests` Forests.
fn baned_table(seats: usize, banes: &[Bane], forests: usize) -> Scenario {
    let emblems: Vec<_> = banes
        .iter()
        .map(|bane| {
            Upgrade {
                commander: "Ruric".into(),
                boon: bane.boon(),
            }
            .definition()
            .unwrap()
        })
        .collect();
    Scenario::new(seats)
        .format(Arc::new(mtg_engine::format::Commander))
        .commanders(0, [creature("Ruric", 2, &[Color::Green], 2, 2)])
        .emblems(0, emblems)
        .battlefield(0, (0..forests).map(|_| ready(forest())))
}

#[rstest]
fn the_engine_applies_a_commanders_banes(#[values(2, 3, 4)] seats: usize) {
    let banes = [Bane::Tax(1), Bane::Shrink(1)];
    // {2}{G} and {1} more is beyond three Forests.
    let mut short = Driver::from_scenario(baned_table(seats, &banes, 3)).unwrap();
    short.idle_until(1, Step::Main1).unwrap();
    assert!(short.begin_cast("Ruric").is_err());
    let mut driver = Driver::from_scenario(baned_table(seats, &banes, 4)).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    driver.cast("Ruric").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let ruric = driver.id_of("Ruric").unwrap();
    let ch = characteristics(driver.state(), ruric).unwrap();
    assert_eq!((ch.power, ch.toughness), (Some(1), Some(1)));
}

#[test]
fn a_bot_ally_takes_the_best_trade_it_can_make() {
    let content = content().unwrap();
    // Hale and with a relic to spare, the bot swaps a trinket for a Rare relic rather than ten gold.
    let run = at_trade(content, "rat_kings_hoard_trade", 40, 1, 3);
    let moves = run.bot_moves(content);
    let pick = |moves: &[RunEvent]| match moves {
        [RunEvent::Picked { hero: 1, pick }] => pick.clone(),
        other => panic!("{other:?}"),
    };
    let first = pick(&moves);
    assert!(matches!(first, Pick::Take(0 | 1)), "{first:?}");
    // Hurt, the bot prays rather than bleeds.
    let mut run = at_trade(content, "bleeding_altar", 12, 1, 3);
    run.heroes[1].life = 12;
    assert_eq!(pick(&run.bot_moves(content)), Pick::Take(2));
    // With no relic to give up and no Satchel card it cannot name, it never picks a shut option.
    let mut run = at_trade(content, "kings_forge", 30, 1, 3);
    run.heroes[1].relics.clear();
    run.heroes[1].satchel.clear();
    assert_eq!(pick(&run.bot_moves(content)), Pick::Take(2));
    for event in run.bot_moves(content) {
        run.accepts(content, &event).unwrap();
    }
    // Every bot answer of every event is one the run accepts, and a bot answers each follow-up it is left.
    for event in trades(content) {
        let mut run = at_trade(content, &event.id, 20, 1, 3);
        for _ in 0..6 {
            let moves = run.bot_moves(content);
            let Some(event) = moves
                .into_iter()
                .find(|m| matches!(m, RunEvent::Picked { hero: 1, .. }))
            else {
                break;
            };
            run.accepts(content, &event).unwrap();
            run.apply(content, &event).unwrap();
        }
        assert!(run.heroes[1].offers.is_empty(), "{}", event.id);
    }
}
