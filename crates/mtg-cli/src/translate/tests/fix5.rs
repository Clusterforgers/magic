//! Goal: a `RepeatEach` that gathers its moves (`ChangeZoneTable`) or damage (`DamageMap`) and a damage map chain from its first line to its `DamageResolve` translate to one `Together` batch, so a "whenever one or more" trigger sees the moves and the damage of the whole effect once; lines outside the chain and chains without a map stay as they were.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

use super::abilities;

const RAMPAGE: &str = "Name:Rampage\nTypes:Sorcery\nManaCost:R\nA:SP$ DestroyAll | ValidCards$ Artifact,Enchantment | RememberDestroyed$ True | SubAbility$ DBRepeat\nSVar:DBRepeat:DB$ RepeatEach | UseImprinted$ True | DefinedCards$ DirectRemembered | RepeatSubAbility$ DBToken | SubAbility$ DBCleanup | ChangeZoneTable$ True\nSVar:DBToken:DB$ Token | TokenOwner$ ImprintedController | TokenScript$ w_1_1_soldier\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

const CHAIN: &str = "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Player | NumDmg$ 1 | DamageMap$ True | SubAbility$ D\nSVar:D:DB$ DamageAll | NumDmg$ 1 | ValidCards$ Creature | SubAbility$ R\nSVar:R:DB$ DamageResolve | SubAbility$ G\nSVar:G:DB$ GainLife | LifeAmount$ 1\n";

const NO_MAP: &str = "Name:T\nTypes:Sorcery\nA:SP$ DealDamage | ValidTgts$ Player | NumDmg$ 1 | SubAbility$ R\nSVar:R:DB$ DamageResolve\n";

fn text(script: &str) -> String {
    abilities(script)
        .iter()
        .map(Value::to_string)
        .collect::<String>()
}

#[rstest]
#[case::a_repeat_with_a_change_zone_table(RAMPAGE, true)]
#[case::a_damage_map_chain(CHAIN, true)]
#[case::a_chain_without_a_map(NO_MAP, false)]
fn the_batch_is_written_where_the_card_gathers_one(#[case] script: &str, #[case] together: bool) {
    let found = text(script);
    assert_eq!(found.contains("\"Together\""), together, "{found}");
    assert!(!found.contains("~DamageResolve"), "{found}");
}

#[test]
fn only_the_damage_steps_are_inside_the_batch() {
    let found = text(CHAIN);
    let batch = found.find("\"Together\"").unwrap();
    let gain = found.find("\"GainLife\"").unwrap();
    assert!(batch < gain, "{found}");
    assert_eq!(found.matches("\"DealDamage\"").count(), 2, "{found}");
    assert!(
        found[batch..gain].matches("\"DealDamage\"").count() == 2,
        "{found}"
    );
}

fn card(script: &str, kind: CardType) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new("Rampage")
        .with_type(kind)
        .with_mana_cost(cost(0, &[Color::Red]));
    for value in abilities(script) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn watcher() -> Arc<CardDefinition> {
    let rules = r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}}},"effects":[{"GainLife":{"amount":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Watcher")
            .with_type(CardType::Land)
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

fn relic(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(name).with_type(CardType::Artifact))
}

#[rstest]
fn the_tokens_of_the_passes_arrive_as_one_batch(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(RAMPAGE, CardType::Sorcery)])
        .battlefield(0, [ready(mountain())]);
    scenario = scenario.battlefield(
        1,
        [
            ready(relic("Relic")),
            ready(relic("Idol")),
            ready(relic("Orb")),
            ready(watcher()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let life = |driver: &Driver| {
        driver
            .state()
            .player(PlayerId::from_index(1))
            .unwrap()
            .life()
    };
    let before = life(&driver);
    driver.begin_cast("Rampage").unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle
            && matches!(
                driver.engine().decision().map(|d| &d.kind),
                Some(DecisionKind::Priority(_))
            )
        {
            break;
        }
        driver.answer_passively().unwrap();
    }
    assert_eq!(life(&driver) - before, 1, "three tokens, one trigger");
}
