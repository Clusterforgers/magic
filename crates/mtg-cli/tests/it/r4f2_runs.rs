//! Goal: round 4 audit faults play right in a game: "up to one target of each player" can be none or one side, and a
//! total over chosen creatures is the total of all of them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, Target, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::{settle, translated};

const SWEEP: &str = "Name:Sweep\nManaCost:W\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature | TargetMin$ 0 | TargetMax$ OneEach | TargetsForEachPlayer$ True | SpellDescription$ x\n";

fn alive(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap())
        .filter(|o| {
            o.card().name == "Grizzly Bears" && o.controller() == PlayerId::from_index(seat)
        })
        .count()
}

#[rstest]
#[case::none(false, false, 1, 1)]
#[case::only_theirs(false, true, 1, 0)]
#[case::only_mine(true, false, 0, 1)]
#[case::both(true, true, 0, 0)]
fn up_to_one_target_of_each_player_takes_none_or_either_side(
    #[case] mine: bool,
    #[case] theirs: bool,
    #[case] left_mine: usize,
    #[case] left_theirs: usize,
) {
    let card = translated(SWEEP, "Sweep", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(plains()), ready(bear())])
        .battlefield(1, [ready(bear())])
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Sweep").unwrap();
    let DecisionKind::ChooseTargets(options) = driver.decision().unwrap().kind.clone() else {
        panic!("no target decision");
    };
    assert_eq!((options.min, options.max), (0, 2));
    let mut chosen = Vec::new();
    for target in &options.legal {
        let Target::Object(object) = target else {
            continue;
        };
        let controller = driver
            .state()
            .object(object.id)
            .unwrap()
            .controller()
            .index();
        if (controller == 0 && mine) || (controller == 1 && theirs) {
            chosen.push(*target);
        }
    }
    driver.choose_targets(&chosen).unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    settle(&mut driver);
    assert_eq!(
        (alive(&driver, 0), alive(&driver, 1)),
        (left_mine, left_theirs)
    );
}

const DREADNOUGHT: &str = "Name:Phyrexian Dreadnought\nManaCost:1\nTypes:Artifact Creature Phyrexian Dreadnought\nPT:12/12\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ ChooseCreatures | TriggerDescription$ x\nSVar:ChooseCreatures:DB$ ChooseCard | Defined$ You | Amount$ X | MinAmount$ 0 | Choices$ Creature.YouCtrl | ChoiceZone$ Battlefield | Reveal$ True | RememberChosen$ True | SubAbility$ SacrificeSelf\nSVar:SacrificeSelf:DB$ Sacrifice | ConditionCheckSVar$ TotalPower | ConditionSVarCompare$ LT12 | SubAbility$ SacrificeCreatures\nSVar:SacrificeCreatures:DB$ SacrificeAll | Defined$ Remembered | ConditionCheckSVar$ TotalPower | ConditionSVarCompare$ GE12 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:TotalPower:Remembered$CardPower\nSVar:X:Count$Valid Creature.YouCtrl\n";

/// The battlefield names of seat 0 after casting the Dreadnought and choosing the creatures named `pick`.
fn dreadnought(pick: &[&str]) -> Vec<String> {
    let card = translated(
        DREADNOUGHT,
        "Phyrexian Dreadnought",
        "Artifact Creature",
        "{1}",
    );
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(free_creature("Six1", 6, 6)),
                ready(free_creature("Six2", 6, 6)),
                ready(free_creature("Small", 1, 1)),
            ],
        )
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Phyrexian Dreadnought").unwrap();
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseCards(o) => {
                let state = driver.state();
                let ids = o
                    .from
                    .iter()
                    .copied()
                    .filter(|id| pick.contains(&state.object(*id).unwrap().card().name.as_str()))
                    .collect();
                Answer::ChooseCards(CardsChoice(ids))
            }
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    let mut names: Vec<String> = state
        .zone(Zone::Battlefield)
        .iter()
        .map(|id| state.object(*id).unwrap().card().name.clone())
        .filter(|name| name != "Plains")
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::twelve_between_two(&["Six1", "Six2"], &["Phyrexian Dreadnought", "Small"])]
#[case::seven_is_short(&["Six1", "Small"], &["Six1", "Six2", "Small"])]
#[case::none(&[], &["Six1", "Six2", "Small"])]
fn the_dreadnought_stays_only_when_the_chosen_total_power_is_twelve(
    #[case] pick: &[&str],
    #[case] left: &[&str],
) {
    assert_eq!(dreadnought(pick), left);
}

const AURAS: &str = "Name:Unhook\nManaCost:W\nTypes:Instant\nA:SP$ DestroyAll | ValidCards$ Card.Aura+AttachedTo Creature.attacking+OppCtrl | SpellDescription$ x\n";

#[rstest]
#[case::on_the_attacker(0, false)]
#[case::on_one_that_stays_home(1, true)]
fn an_aura_on_an_attacking_creature_an_opponent_controls_is_destroyed(
    #[case] host: usize,
    #[case] stays: bool,
) {
    use mtg_core::card::CardType;
    use mtg_engine::turn::Step;
    use mtg_testkit::scenario::attached_to;
    let aura = std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Curse")
            .with_type(CardType::Enchantment)
            .with_subtype("Aura"),
    );
    let mut world = mtg_cli::translate::ctx::World::default();
    world.subtypes.insert("Aura".to_owned());
    let card = crate::r3p3_runs::translated_in(&world, AURAS, "Unhook", "Instant", "{W}");
    let scenario = Scenario::new(2)
        .at(2, 1, Step::Main1)
        .hand(0, [card])
        .battlefield(0, [ready(plains()), attached_to(ready(aura), 1, host)])
        .battlefield(
            1,
            [
                ready(free_creature("Raider", 2, 2)),
                ready(free_creature("Idle", 2, 2)),
            ],
        )
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Raider", 0)]).unwrap();
    driver.pass().unwrap();
    driver.cast("Unhook").unwrap();
    settle(&mut driver);
    let state = driver.state();
    let curse = state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).unwrap().card().name == "Curse");
    assert_eq!(curse, stays);
}
