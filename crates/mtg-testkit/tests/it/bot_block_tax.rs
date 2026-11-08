//! Goal: a bot's random block that its player cannot pay for (CR 509.1d, a tax in mana or in life) is cut back to one
//! the engine accepts, so a game of bots never stops on the cost of blocking, at tables of two, three and four seats.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::turn::Step;
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{RandomBot, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// A 2/2 creature taxed by `tax`, the fields of a `BlockTax` rule.
fn taxed(name: &str, tax: &str) -> Arc<CardDefinition> {
    let rule = format!(r#"{{"Rule":{{"rule":{{"BlockTax":{{{tax}}}}}}}}}"#);
    let rules: Ability = serde_json::from_str(&rule).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(rules),
    )
}

const SELF_TWO: &str = r#""blockers":["This"],"mana":"{2}""#;
const EVERY_ONE: &str = r#""mana":"{1}""#;
const LIFE_ONE: &str = r#""blockers":["This"],"life":1"#;
const LIFE_TWO: &str = r#""blockers":["This"],"life":2"#;

/// Seat 0 attacks with four 1/1s; seat 1 holds three taxed creatures, a free one, `count` Plains and `life`. Bots
/// answer every decision through combat. Returns whether seat 1 declared a block with a taxed creature.
fn combat(seats: usize, seed: u64, tax: &str, count: usize, life: i32) -> bool {
    let attackers: Vec<_> = (0..4)
        .map(|slot| ready(free_creature(&format!("Attacker {slot}"), 1, 1)))
        .collect();
    let mut held: Vec<_> = (0..3)
        .map(|slot| ready(taxed(&format!("Taxed {slot}"), tax)))
        .collect();
    held.push(ready(free_creature("Free", 2, 2)));
    held.extend(lands(count, plains));
    let mut engine = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers)
        .battlefield(1, held)
        .life(1, life)
        .build()
        .unwrap();
    let mut bot = RandomBot::new(seed, 0);
    for _ in 0..200 {
        let step = engine.state().turn().step();
        if !matches!(step, Step::DeclareAttackers | Step::DeclareBlockers) {
            break;
        }
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = bot.choose_in(&decision, engine.state());
        if let Err(error) = apply_answer(&mut engine, &decision, answer) {
            panic!("seed {seed}: {error}");
        }
    }
    let state = engine.state();
    engine.events().iter().any(|event| {
        matches!(event, Event::BlockersDeclared { player, blocks } if *player == pid(1)
            && blocks.iter().any(|(blocker, _)| state
                .object(*blocker)
                .is_some_and(|object| object.card().name.starts_with("Taxed"))))
    })
}

#[rstest]
#[case::mana_never_enough(SELF_TWO, 1, 20, false)]
// Three creatures each tax every blocker, so one blocker costs {3}.
#[case::three_taxes_short(EVERY_ONE, 2, 20, false)]
#[case::three_taxes_for_one_blocker(EVERY_ONE, 3, 20, true)]
#[case::mana_for_some(SELF_TWO, 4, 20, true)]
#[case::life_never_enough(LIFE_TWO, 0, 1, false)]
#[case::life_for_some(LIFE_ONE, 0, 3, true)]
fn a_bot_cuts_back_a_block_it_cannot_pay_for(
    #[values(2, 3, 4)] seats: usize,
    #[case] tax: &str,
    #[case] count: usize,
    #[case] life: i32,
    #[case] some_taxed: bool,
) {
    let declared = (0..30)
        .filter(|seed| combat(seats, *seed, tax, count, life))
        .count();
    assert_eq!(
        declared > 0,
        some_taxed,
        "taxed creatures blocked in {declared} of 30 games"
    );
}
