//! Goal: statics and replacements translated in round 4 play right in a game, including the case where the rule must not apply.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

const WALL: &str = "Name:Cuts Wall\nManaCost:3 W W\nTypes:Creature Wall\nPT:1/1\nK:Defender\nA:AB$ Effect | Cost$ W | StaticAbilities$ CanAttack | Duration$ UntilHostLeavesPlayOrEOT | SpellDescription$ CARDNAME can attack this turn as though it didn't have defender.\nSVar:CanAttack:Mode$ CanAttackDefender | ValidCard$ Card.EffectSource | Description$ EFFECTSOURCE can attack this turn as though it didn't have defender.\n";

#[rstest]
#[case::without_the_ability(false, false)]
#[case::after_the_ability(true, true)]
fn a_defender_attacks_only_the_turn_its_ability_was_used(
    #[case] activate: bool,
    #[case] attacks: bool,
) {
    let wall = translated(WALL, "Cuts Wall", "Creature — Wall", "{3}{W}{W}");
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(wall), ready(plains())])
        .battlefield(1, [ready(free_creature("Target", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    if activate {
        driver.begin_activation("Cuts Wall").unwrap();
        driver.pay_tapping(&["Plains"], 0).unwrap();
        settle(&mut driver);
    }
    driver.pass_until(Step::DeclareAttackers).unwrap();
    assert_eq!(driver.attack(&[("Cuts Wall", 1)]).is_ok(), attacks);
}

const DISTURBED: &str = "Name:Spirit Side\nManaCost:1 W\nTypes:Creature Spirit\nPT:1/1\nR:Event$ Moved | ValidCard$ Card.Self | Destination$ Graveyard | ReplaceWith$ Exile | Description$ x\nSVar:Exile:DB$ ChangeZone | Hidden$ True | Origin$ All | Destination$ Exile | Defined$ ReplacedCard\n";
const PLAIN: &str = "Name:Spirit Side\nManaCost:1 W\nTypes:Creature Spirit\nPT:1/1\n";

#[rstest]
#[case::exiled_instead(DISTURBED, mtg_engine::zones::Zone::Exile)]
#[case::an_ordinary_creature(
    PLAIN,
    mtg_engine::zones::Zone::Graveyard(mtg_core::ids::PlayerId::from_index(0))
)]
fn a_creature_that_dies_goes_where_its_replacement_sends_it(
    #[case] script: &str,
    #[case] zone: mtg_engine::zones::Zone,
) {
    let card = translated(script, "Spirit Side", "Creature — Spirit", "{1}{W}");
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [mtg_testkit::damaged(card, 1)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass().unwrap();
    let found = driver.state().zone(zone).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Spirit Side")
    });
    assert!(found);
}
