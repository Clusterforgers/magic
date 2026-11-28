//! Goal: bot allies temper at a campfire and buy commander upgrades at a shop by what each is worth to their
//! commander (rest when hurt, reforge when no upgrade beats a card pick, the best ware their gold allows), at every
//! party size, and a whole run of bots takes upgrades and replays to the same run from its seed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_campaign::content::Rarity;
use mtg_campaign::run::UPGRADE_PRICE;
use mtg_campaign::{
    Boon, Brain, CommanderFacts, Gift, HeroDeck, Offer, Pick, Run, RunEvent, ShopItem, Status,
    Upgrade, content,
};
use rstest::rstest;

fn facts(power: Option<i32>) -> CommanderFacts {
    CommanderFacts {
        name: "Ruric".into(),
        creature: power.is_some(),
        planeswalker: power.is_none(),
        power,
        generic: 3,
        keywords: Vec::new(),
    }
}

fn up(boon: Boon) -> Upgrade {
    Upgrade {
        commander: "Ruric".into(),
        boon,
    }
}

/// A started run of one player and `bots` bot allies, each deck led by `commander`.
fn started(seed: u64, bots: usize, commander: &CommanderFacts) -> (Run, Vec<RunEvent>) {
    let content = content().unwrap();
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
                commanders: vec![commander.clone()],
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    (
        Run::replay(content, "warrens", seed, 0, &events).unwrap(),
        events,
    )
}

/// The run in a camp with every bot holding `offer` with `gold` and `life` lost.
fn holding(bots: usize, commander: &CommanderFacts, offer: &Offer, gold: u32, hurt: i32) -> Run {
    let content = content().unwrap();
    let (mut run, _) = started(5, bots, commander);
    run.status = Status::Room;
    run.at = run
        .map
        .nodes
        .iter()
        .find(|n| n.boss.is_none())
        .map(|n| n.id);
    for h in run.heroes.iter_mut().skip(1) {
        h.gold = gold;
        h.life = h.max_life(content) - hurt;
        h.offers = vec![offer.clone()];
    }
    run
}

/// Each bot's pick among `moves`, in seat order.
fn picks(moves: &[RunEvent]) -> Vec<Pick> {
    moves
        .iter()
        .filter_map(|m| match m {
            RunEvent::Picked { pick, .. } => Some(pick.clone()),
            _ => None,
        })
        .collect()
}

#[rstest]
#[case::the_bigger_pump(
    Some(3),
    vec![up(Boon::Pump(1)), up(Boon::Pump(2)), up(Boon::Counter)],
    0,
    Pick::Take(1)
)]
#[case::flying_on_a_big_commander(
    Some(6),
    vec![up(Boon::Pump(2)), up(Boon::Keyword(Gift::Flying))],
    0,
    Pick::Take(1)
)]
#[case::the_first_of_equals(
    Some(3),
    vec![up(Boon::Cheaper(2)), up(Boon::Pump(2))],
    0,
    Pick::Take(0)
)]
#[case::loyalty_for_a_planeswalker(None, vec![up(Boon::Counter), up(Boon::Loyalty)], 0, Pick::Take(1))]
#[case::reforges_when_nothing_beats_a_card(
    Some(1),
    vec![up(Boon::Keyword(Gift::Vigilance)), up(Boon::Counter)],
    0,
    Pick::Reforge
)]
#[case::reforges_with_no_upgrade(Some(3), vec![], 0, Pick::Reforge)]
#[case::rests_when_hurt(Some(3), vec![up(Boon::Pump(2))], 11, Pick::Rest)]
fn a_bot_tempers_with_the_best_upgrade(
    #[values(1, 2, 3)] bots: usize,
    #[case] power: Option<i32>,
    #[case] upgrades: Vec<Upgrade>,
    #[case] hurt: i32,
    #[case] expected: Pick,
) {
    let content = content().unwrap();
    let offer = Offer::Campfire { upgrades };
    let mut run = holding(bots, &facts(power), &offer, 0, hurt);
    let moves = run.bot_moves(content);
    assert_eq!(picks(&moves), vec![expected; bots]);
    assert_eq!(run.bot_moves(content), moves, "the same picks again");
    for event in moves {
        run.accepts(content, &event).unwrap();
        run.apply(content, &event).unwrap();
    }
}

/// The id of a relic of `rarity` in the content.
fn relic(rarity: Rarity) -> String {
    let content = content().unwrap();
    let mut ids: Vec<&String> = content
        .cards
        .iter()
        .filter(|(_, c)| c.rarity == rarity)
        .map(|(id, _)| id)
        .collect();
    ids.sort();
    ids[0].clone()
}

#[rstest]
#[case::the_upgrade_over_a_common_relic(Rarity::Common, Boon::Pump(2), 200, Pick::Take(1))]
#[case::a_rare_relic_over_the_upgrade(Rarity::Rare, Boon::Pump(2), 200, Pick::Take(0))]
#[case::the_upgrade_it_can_afford(Rarity::Rare, Boon::Counter, UPGRADE_PRICE, Pick::Take(1))]
#[case::leaves_when_poor(Rarity::Common, Boon::Pump(2), UPGRADE_PRICE - 1, Pick::Skip)]
fn a_bot_buys_the_best_ware_it_can_afford(
    #[values(1, 2, 3)] bots: usize,
    #[case] rarity: Rarity,
    #[case] boon: Boon,
    #[case] gold: u32,
    #[case] expected: Pick,
) {
    let content = content().unwrap();
    let price = mtg_campaign::relics::price(rarity);
    let items = vec![
        ShopItem::Relic {
            id: relic(rarity),
            price,
            sold: false,
        },
        ShopItem::Upgrade {
            upgrade: up(boon.clone()),
            sold: false,
        },
    ];
    let mut run = holding(bots, &facts(Some(3)), &Offer::Shop { items }, gold, 0);
    let moves = run.bot_moves(content);
    assert_eq!(picks(&moves), vec![expected.clone(); bots]);
    for event in moves {
        run.apply(content, &event).unwrap();
    }
    for h in run.heroes.iter().skip(1) {
        assert_eq!(
            h.upgrades.contains(&up(boon.clone())),
            expected == Pick::Take(1)
        );
    }
}

/// Plays one floor of a run with `heroes` heroes, every hero answering like a bot and every fight won; the upgrades
/// each hero holds and the events.
fn play(seed: u64, heroes: usize) -> (Vec<Vec<Upgrade>>, Vec<RunEvent>) {
    let content = content().unwrap();
    let (mut run, mut events) = started(seed, heroes - 1, &facts(Some(4)));
    for _ in 0..400 {
        let next: Vec<RunEvent> = match &run.status {
            Status::Fight { game: None } => vec![RunEvent::FightCreated {
                game: format!("g{}", run.fights.len()),
            }],
            Status::Fight { game: Some(_) } => vec![RunEvent::FightEnded {
                won: true,
                lives: run.heroes.iter().map(|h| h.life).collect(),
                spirits: Vec::new(),
            }],
            Status::Map | Status::Room => {
                let mut as_bots = run.clone();
                for h in &mut as_bots.heroes {
                    h.bot = Some(Brain::Normal);
                    h.user = None;
                }
                as_bots.bot_moves(content)
            }
            _ => break,
        };
        if next.is_empty() {
            break;
        }
        for event in next {
            run.apply(content, &event).unwrap();
            events.push(event);
        }
    }
    assert!(
        matches!(run.status, Status::FloorCleared | Status::Victory),
        "seed {seed}: {:?}",
        run.status
    );
    let replayed = Run::replay(content, "warrens", seed, 0, &events).unwrap();
    assert_eq!(replayed, run, "seed {seed} replays");
    (
        run.heroes.iter().map(|h| h.upgrades.clone()).collect(),
        events,
    )
}

#[rstest]
fn a_party_of_bots_takes_upgrades_the_same_way_from_a_seed(#[values(1, 2, 3, 4)] heroes: usize) {
    let mut taken = 0;
    for seed in 1..=6 {
        let (upgrades, events) = play(seed, heroes);
        assert_eq!(play(seed, heroes).1, events, "seed {seed} plays the same");
        taken += upgrades.iter().map(Vec::len).sum::<usize>();
    }
    assert!(taken > 0, "the bots took no upgrade in six runs");
}
