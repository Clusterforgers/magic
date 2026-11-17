//! Goal: every event option reads plainly from its effect data (a summary, the gold and life it needs, a chance with
//! its odds); a hero who lacks the gold or life for a sure cost cannot take the option, yet a stored log still
//! replays; an answer records what really happened (gambles rolled, life and gold as they moved) in the run's results and
//! chronicle, and a bot ally takes the first option it can afford.
use mtg_campaign::{
    Brain, Content, EventOption, HeroDeck, Needs, Offer, Outcome, Pick, Resolved, RoomKind, Run,
    RunEvent, Status, content,
};
use rstest::rstest;

use Outcome::{CardPick, Duplicate, Gamble, Gold, Life, Relic};

fn option(outcomes: Vec<Outcome>) -> EventOption {
    EventOption {
        label: "Option".into(),
        outcomes,
    }
}

#[test]
fn every_event_option_of_the_content_has_a_summary_and_every_event_an_option_anyone_can_take() {
    let content = content().unwrap();
    for theme in &content.themes {
        assert!(!theme.events.is_empty(), "{} has events", theme.id);
        for event in &theme.events {
            assert!(!event.options.is_empty(), "{} has options", event.id);
            for option in &event.options {
                assert!(
                    !option.summary().trim().is_empty(),
                    "{}: {} has no summary",
                    event.id,
                    option.label
                );
                for outcome in &option.outcomes {
                    assert!(!outcome.words().is_empty(), "{}: {outcome:?}", event.id);
                }
                // The summary is the label's job done by data: a label never repeats an amount.
                assert!(
                    !option.label.chars().any(|c| c.is_ascii_digit()),
                    "{}: label {:?} states an amount its effects already show",
                    event.id,
                    option.label
                );
            }
            assert!(
                event.options.iter().any(|o| o.needs() == Needs::default()),
                "{} leaves a penniless hero with no way out",
                event.id
            );
        }
    }
}

#[rstest]
#[case::gold_gain(Gold(40), "+40 gold")]
#[case::gold_cost(Gold(-25), "−25 gold")]
#[case::life_cost(Life(-8), "−8 life")]
#[case::life_gain(Life(8), "+8 life")]
#[case::relic(Relic, "a relic")]
#[case::card(CardPick, "a card for your Satchel")]
#[case::duplicate(Duplicate, "a copy of a card from your deck or Satchel")]
#[case::chance(
    Gamble { odds: 50, win: vec![Relic], lose: vec![Life(-8)] },
    "50%: a relic · 50%: −8 life"
)]
#[case::chance_with_nothing_on_a_win(
    Gamble { odds: 60, win: vec![], lose: vec![Life(-8), Gold(-5)] },
    "60%: nothing · 40%: −8 life, −5 gold"
)]
fn an_outcome_reads_in_plain_words(#[case] outcome: Outcome, #[case] words: &str) {
    assert_eq!(outcome.words(), words);
}

#[rstest]
#[case::nothing(vec![], "nothing", Needs::default())]
#[case::a_payment(vec![Gold(-25), CardPick], "−25 gold, a card for your Satchel", Needs { gold: 25, life: 0, ..Needs::default() })]
#[case::a_sure_life_cost(vec![Life(-8), Gold(40)], "−8 life, +40 gold", Needs { gold: 0, life: 8, ..Needs::default() })]
#[case::a_gain(vec![Gold(30)], "+30 gold", Needs::default())]
#[case::a_gamble_is_a_risk_not_a_cost(
    vec![Gamble { odds: 50, win: vec![Relic], lose: vec![Life(-8), Gold(-9)] }],
    "50%: a relic · 50%: −8 life, −9 gold",
    Needs::default()
)]
fn an_option_summarises_its_outcomes_and_needs_its_sure_costs(
    #[case] outcomes: Vec<Outcome>,
    #[case] summary: &str,
    #[case] needs: Needs,
) {
    let option = option(outcomes);
    assert_eq!(option.summary(), summary);
    assert_eq!(option.needs(), needs);
}

#[rstest]
#[case::enough_gold(25, 40, None)]
#[case::short_of_gold(24, 40, Some("needs 25 gold, you have 24"))]
fn a_gold_cost_needs_the_gold(#[case] gold: u32, #[case] life: i32, #[case] unmet: Option<&str>) {
    let needs = Needs {
        gold: 25,
        life: 0,
        ..Needs::default()
    };
    assert_eq!(needs.unmet(gold, life).as_deref(), unmet);
}

#[rstest]
#[case::spare_life(9, None)]
#[case::exactly_the_cost(8, Some("needs more than 8 life, you have 8"))]
#[case::less(3, Some("needs more than 8 life, you have 3"))]
fn a_life_cost_leaves_the_hero_standing(#[case] life: i32, #[case] unmet: Option<&str>) {
    let needs = Needs {
        gold: 0,
        life: 8,
        ..Needs::default()
    };
    assert_eq!(needs.unmet(500, life).as_deref(), unmet);
}

/// A run at an event with one hero (a player) and `bots` bot allies, each holding the event's offer.
fn at_event(content: &Content, event: &str, gold: u32, life: i32, bots: usize) -> Run {
    let mut events = vec![RunEvent::Joined {
        name: "Ana".into(),
        user: Some("u1".into()),
        bot: None,
    }];
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
                id: "precon:RG".into(),
                name: "Deck".into(),
                colors: "RG".into(),
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    let mut run = Run::replay(content, "warrens", 5, 0, &events).unwrap();
    run.status = Status::Room;
    // A camp: its only way on is the next boss.
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.kind != RoomKind::Boss)
        .map(|n| n.id);
    for hero in &mut run.heroes {
        hero.gold = gold;
        hero.life = life;
        hero.offers = vec![Offer::Event {
            event: event.into(),
        }];
    }
    run
}

fn take(run: &mut Run, hero: u8, option: u8) -> Result<(), String> {
    let content = content().unwrap();
    let event = RunEvent::Picked {
        hero,
        pick: Pick::Take(option),
    };
    run.accepts(content, &event)
        .and_then(|()| run.apply(content, &event))
        .map_err(|e| e.to_string())
}

#[rstest]
#[case::enough_gold("goblin_toll", 0, 30, 40, true)]
#[case::exact_gold("goblin_toll", 0, 25, 40, true)]
#[case::short_of_gold("goblin_toll", 0, 24, 40, false)]
#[case::life_to_spare("goblin_toll", 1, 0, 9, true)]
#[case::life_just_the_cost("goblin_toll", 1, 0, 8, false)]
#[case::no_cost_needs_nothing("goblin_toll", 2, 0, 1, true)]
#[case::a_gamble_needs_nothing("rat_kings_hoard", 0, 0, 1, true)]
fn a_sure_cost_blocks_an_option_the_hero_cannot_pay(
    #[case] event: &str,
    #[case] option: u8,
    #[case] gold: u32,
    #[case] life: i32,
    #[case] allowed: bool,
) {
    let content = content().unwrap();
    let mut run = at_event(content, event, gold, life, 0);
    let answer = take(&mut run, 0, option);
    assert_eq!(answer.is_ok(), allowed, "{answer:?}");
    if !allowed {
        assert!(answer.unwrap_err().contains("needs"));
        assert!(run.results.is_empty(), "a refused answer records nothing");
    }
}

#[test]
fn a_stored_log_with_an_unaffordable_payment_still_replays() {
    let content = content().unwrap();
    let mut run = at_event(content, "goblin_toll", 10, 40, 0);
    let event = RunEvent::Picked {
        hero: 0,
        pick: Pick::Take(0),
    };
    assert!(run.accepts(content, &event).is_err());
    run.apply(content, &event).unwrap();
    assert_eq!(run.heroes[0].gold, 0);
    assert_eq!(
        run.results[0].outcomes,
        vec![Gold(-10), CardPick],
        "the result shows the gold that really went"
    );
}

#[test]
fn an_answer_records_what_it_did_in_the_results_and_the_chronicle() {
    let content = content().unwrap();
    let mut run = at_event(content, "goblin_toll", 30, 40, 1);
    take(&mut run, 0, 1).unwrap();
    assert_eq!(
        run.results,
        vec![Resolved {
            hero: 0,
            what: "chose “Push past”".into(),
            outcomes: vec![Life(-8), Gold(40)],
            rolls: vec![],
        }]
    );
    assert_eq!(run.log.last().unwrap(), "Ana chose “Push past”.");
    assert_eq!((run.heroes[0].life, run.heroes[0].gold), (32, 70));
}

#[test]
fn life_and_gold_are_shown_as_they_moved() {
    let content = content().unwrap();
    // Pray heals 8 life but only 2 fit under the cap; a pool gamble's loss never takes a hero below 1 life.
    let mut run = at_event(content, "old_shrine", 0, 38, 0);
    take(&mut run, 0, 0).unwrap();
    assert_eq!(run.results[0].outcomes, vec![Life(2)]);
    let mut run = at_event(content, "glittering_pool", 0, 5, 0);
    run.seed = (0..64)
        .find(|seed| {
            let mut probe = run.clone();
            probe.seed = *seed;
            take(&mut probe, 0, 0).unwrap();
            probe.results[0].rolls.iter().any(|r| !r.won)
        })
        .unwrap();
    take(&mut run, 0, 0).unwrap();
    assert_eq!(run.results[0].outcomes, vec![Life(-4)]);
    assert_eq!(run.heroes[0].life, 1);
}

#[test]
fn a_gamble_records_its_odds_and_how_it_fell_and_both_ways_occur() {
    let content = content().unwrap();
    let mut seen = [false, false];
    for seed in 0..64 {
        let mut run = at_event(content, "glittering_pool", 0, 40, 0);
        run.seed = seed;
        take(&mut run, 0, 0).unwrap();
        let result = &run.results[0];
        let [roll] = result.rolls.as_slice() else {
            panic!("one gamble, one roll: {result:?}")
        };
        assert_eq!(roll.odds, 60);
        let expected = if roll.won { Gold(60) } else { Life(-8) };
        assert_eq!(result.outcomes, vec![expected]);
        seen[usize::from(roll.won)] = true;
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn a_relic_or_card_from_an_event_is_answered_by_a_later_pick_that_is_recorded_too() {
    let content = content().unwrap();
    let mut run = at_event(content, "trapped_miner", 0, 40, 0);
    take(&mut run, 0, 0).unwrap();
    assert_eq!(run.results[0].outcomes, vec![Gold(20), Relic]);
    let Some(Offer::Relics { relics }) = run.heroes[0].offers.first().cloned() else {
        panic!("a relic is offered")
    };
    take(&mut run, 0, 0).unwrap();
    let name = &content.cards[&relics[0]].name;
    assert_eq!(run.results[1].what, format!("took the relic {name}"));
    assert!(run.log.contains(&format!("Ana took the relic {name}.")));
}

#[test]
fn a_bot_ally_takes_the_first_option_it_can_afford() {
    let content = content().unwrap();
    let mut run = at_event(content, "goblin_toll", 0, 40, 1);
    // The bot cannot pay the toll and pushes past instead.
    let moves = run.bot_moves(content);
    assert_eq!(
        moves,
        vec![RunEvent::Picked {
            hero: 1,
            pick: Pick::Take(1)
        }]
    );
    run.heroes[1].gold = 25;
    let moves = run.bot_moves(content);
    assert_eq!(
        moves,
        vec![RunEvent::Picked {
            hero: 1,
            pick: Pick::Take(0)
        }]
    );
    for event in moves {
        run.accepts(content, &event).unwrap();
    }
}

#[test]
fn the_results_stay_through_the_next_fight_and_clear_when_it_is_won() {
    let content = content().unwrap();
    let mut run = at_event(content, "old_shrine", 0, 30, 0);
    take(&mut run, 0, 0).unwrap();
    assert_eq!(run.results.len(), 1);
    // The only hero answered and a camp's single way on is the next boss: the party is in that fight.
    assert!(
        matches!(run.status, Status::Fight { .. }),
        "{:?}",
        run.status
    );
    assert_eq!(run.results.len(), 1, "the camp's results are still news");
    let events = [
        RunEvent::FightCreated { game: "g".into() },
        RunEvent::FightEnded {
            won: true,
            lives: vec![30],
            spirits: vec![],
        },
    ];
    for event in &events {
        run.apply(content, event).unwrap();
    }
    assert_eq!(run.results, vec![], "a won fight ends the camp's news");
}
