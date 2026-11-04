//! Goal: `CantBlockBy` lines with any attacker filter translate to a static on the attackers they name, and an attacker
//! the line covers cannot be blocked while one it does not cover can.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{abilities, gaps};

const EQUIPMENT: &str = "Name:Cloak\nTypes:Artifact Equipment\nS:Mode$ CantBlockBy | ValidAttacker$ Creature.EquippedBy | Description$ x\n";
const BANNER: &str = "Name:Cloak\nTypes:Enchantment\nS:Mode$ CantBlockBy | ValidAttacker$ Creature.YouCtrl | Description$ x\n";
const WARRIORS: &str = "Name:Cloak\nTypes:Enchantment\nS:Mode$ CantBlockBy | ValidAttacker$ Creature.Elf | ValidBlocker$ Creature.powerLE2 | Description$ x\n";

fn cloak(script: &str) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Cloak").with_type(CardType::Enchantment);
    if script.contains("Equipment") {
        card = CardDefinition::new("Cloak")
            .with_type(CardType::Artifact)
            .with_subtype("Equipment");
    }
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

/// Whether the wall of seat 1 can block the creature "Runner" that seat 0 attacks with.
fn wall_can_block(script: &str, attached: bool) -> bool {
    let cloak = if attached {
        attached_to(ready(cloak(script)), 0, 0)
    } else {
        ready(cloak(script))
    };
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(free_creature("Runner", 3, 3)), cloak])
        .battlefield(1, [ready(free_creature("Wall", 0, 9))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Runner", 1)]).unwrap();
    for _ in 0..40 {
        match &driver.decision().unwrap().kind {
            DecisionKind::DeclareBlockers(_) => {
                return driver.block(&[("Wall", "Runner")]).is_ok();
            }
            _ if driver.state().turn().step() == Step::EndCombat => return false,
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("combat did not reach blocks");
}

#[rstest]
#[case::equipped(EQUIPMENT, true, false)]
#[case::your_creatures(BANNER, false, false)]
#[case::small_blockers_only_cant_block_an_elf(WARRIORS, false, true)]
fn a_translated_cant_block_by_line_decides_who_can_block(
    #[case] script: &str,
    #[case] attached: bool,
    #[case] blockable: bool,
) {
    assert_eq!(wall_can_block(script, attached), blockable);
}

#[rstest]
#[case::remembered("S:Mode$ CantBlockBy | ValidAttacker$ Creature.IsRemembered")]
#[case::no_player("S:Mode$ CantBlockBy | ValidAttacker$ Player")]
fn an_attacker_filter_it_cannot_read_stays_a_gap(#[case] line: &str) {
    let text = format!("Name:Cloak\nTypes:Enchantment\n{line}\n");
    assert_eq!(gaps(&text).len(), 1);
}
