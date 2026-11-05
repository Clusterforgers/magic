//! Goal: audit fixes of round 8 play right in real games: last known information of a sacrificed creature, exile-instead counters, bans on activation, the defending player of an Aura, face-down looks, copies, delayed triggers and hands.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::cast_possible_truncation)]

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_cli::forge::script::Script;
use mtg_cli::translate::card::translate_script;
use mtg_cli::translate::ctx::World;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{CardsChoice, NameChoice, Target, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, swamp};
use mtg_testkit::{Driver, Scenario, ready, with_counters};

/// The card a Forge script gives, with the rules the translator read; no gaps allowed.
fn card(text: &str, types: &[CardType], cost: &str, pt: Option<(i32, i32)>) -> Arc<CardDefinition> {
    let out = translate_script(&World::new(&[], BTreeMap::new()), &Script::parse(text));
    let face = &out.faces[0];
    assert!(face.gaps.is_empty(), "{:?}", face.gaps);
    let mut def = CardDefinition::new(&out.name).with_mana_cost(ManaCost::parse(cost).unwrap());
    for t in types {
        def = def.with_type(*t);
    }
    if let Some((p, t)) = pt {
        def = def.with_power_toughness(p, t);
    }
    for rules in &face.abilities {
        def = def.with_rules(serde_json::from_value::<Ability>(rules.clone()).unwrap());
    }
    Arc::new(def)
}

/// Plays until the stack is empty at priority: yes to questions, the names in `want` picked in cards and targets.
fn run(driver: &mut Driver, yes: bool, want: &[&str]) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::YesNo(o) if yes && !o.only_no => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(false))).unwrap();
            }
            DecisionKind::ChooseCards(o) => {
                let picked: Vec<_> = o
                    .from
                    .iter()
                    .copied()
                    .filter(|i| {
                        want.contains(&driver.state().object(*i).unwrap().card().name.as_str())
                    })
                    .take(o.max as usize)
                    .collect();
                if picked.len() as u32 >= o.min && !picked.is_empty() {
                    driver
                        .answer(Answer::ChooseCards(CardsChoice(picked)))
                        .unwrap();
                } else {
                    driver.answer_passively().unwrap();
                }
            }
            DecisionKind::ChooseTargets(o) => {
                let picked: Vec<Target> = want
                    .iter()
                    .filter_map(|n| driver.target_named(n).ok())
                    .filter(|t| o.legal.contains(t))
                    .take(o.max as usize)
                    .collect();
                if picked.len() as u32 >= o.min && !picked.is_empty() {
                    driver
                        .answer(Answer::ChooseTargets(TargetChoice::Targets(picked)))
                        .unwrap();
                } else {
                    driver.answer_passively().unwrap();
                }
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            DecisionKind::ChooseName(_) => {
                let name = want.first().copied().unwrap_or("");
                driver
                    .answer(Answer::ChooseName(NameChoice(name.into())))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the game did not settle");
}

/// Casts the named spell, paying automatically, and plays it out.
fn cast_and_run(driver: &mut Driver, name: &str, yes: bool, want: &[&str]) {
    driver.begin_cast(name).unwrap();
    run(driver, yes, want);
}

/// The names in a zone.
fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|i| driver.state().object(*i).unwrap().card().name.clone())
        .collect()
}

/// The current power and toughness of a battlefield permanent.
fn pt(driver: &Driver, name: &str) -> (Option<i32>, Option<i32>) {
    let id = driver.id_of(name).unwrap();
    let c = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (c.power, c.toughness)
}

const TIP: &str = "Name:Tip the Scales\nManaCost:2 B\nTypes:Sorcery\nA:SP$ Sacrifice | SacValid$ Creature | RememberSacrificed$ True | SubAbility$ DBImmediateTrigger | SpellDescription$ Sacrifice a creature.\nSVar:DBImmediateTrigger:DB$ ImmediateTrigger | Execute$ TrigPumpAll | ConditionDefined$ RememberedLKI | ConditionPresent$ Creature | RememberObjects$ RememberedLKI | SubAbility$ DBCleanup | SpellDescription$ When you do, all creatures get -X/-X until end of turn, where X is the sacrificed creature's toughness.\nSVar:TrigPumpAll:DB$ PumpAll | ValidCards$ Creature | NumAtt$ -X | NumDef$ -X\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\nSVar:X:TriggerRemembered$CardToughness\n";

fn main_phase() -> Scenario {
    Scenario::new(2).at(1, 0, mtg_engine::turn::Step::Main1)
}

#[test]
fn a_sacrificed_creature_gives_its_toughness_with_its_counters() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let fat = with_counters(ready(bear()), CounterKind::PlusOnePlusOne, 2);
    let scenario = main_phase()
        .hand(0, [tip])
        .battlefield(0, [fat, ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Tip the Scales", true, &["Grizzly Bears"]);
    assert!(
        names(
            &driver,
            Zone::Graveyard(mtg_core::ids::PlayerId::from_index(0))
        )
        .contains(&"Grizzly Bears".to_string())
    );
    assert_eq!(pt(&driver, "Big"), (Some(2), Some(2)));
}

/// A free sorcery with the rules given as JSON.
fn json_sorcery(name: &str, rules: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    )
}

#[test]
fn a_sacrificed_token_still_gives_its_toughness() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let make = json_sorcery(
        "Make Token",
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Tok","power":3,"toughness":3,"types":["Creature"]}}}}]}}"#,
    );
    let scenario = main_phase()
        .hand(0, [tip, make])
        .battlefield(0, [ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Make Token", true, &[]);
    cast_and_run(&mut driver, "Tip the Scales", true, &["Tok"]);
    assert_eq!(pt(&driver, "Big"), (Some(3), Some(3)));
}

#[test]
fn nothing_to_sacrifice_shrinks_nothing() {
    let tip = card(TIP, &[CardType::Sorcery], "{2}{B}", None);
    let scenario = main_phase()
        .hand(0, [tip])
        .battlefield(0, [ready(swamp()), ready(swamp()), ready(swamp())])
        .battlefield(1, [ready(free_creature("Big", 6, 6))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    cast_and_run(&mut driver, "Tip the Scales", true, &[]);
    assert_eq!(pt(&driver, "Big"), (Some(6), Some(6)));
}

const NO_MORE_LIES: &str = "Name:No More Lies\nManaCost:W U\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | UnlessCost$ 3 | TgtPrompt$ Select target spell | ValidTgts$ Card | Destination$ Exile | SpellDescription$ Counter target spell unless its controller pays {3}. If that spell is countered this way, exile it instead of putting it into its owner's graveyard.\n";

#[rstest::rstest]
#[case::countered_spell_is_exiled(false, true)]
#[case::uncounterable_spell_resolves_to_the_graveyard(true, false)]
fn a_spell_countered_this_way_is_exiled_and_one_that_is_not_is_not(
    #[case] uncounterable: bool,
    #[case] exiled: bool,
) {
    use mtg_testkit::cards::{island, plains};
    let lies = card(NO_MORE_LIES, &[CardType::Instant], "{W}{U}", None);
    let rules = if uncounterable {
        r#"{"Rule":{"rule":{"CantBeCountered":"This"},"from":"Stack"}}"#
    } else {
        r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#
    };
    let victim = Arc::new(
        CardDefinition::new("Victim")
            .with_type(CardType::Instant)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            )
            .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
    );
    let scenario = Scenario::new(2)
        .at(1, 0, mtg_engine::turn::Step::Main1)
        .hand(0, [victim])
        .hand(1, [lies])
        .battlefield(
            1,
            [
                ready(island()),
                ready(plains()),
                ready(island()),
                ready(plains()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Victim").unwrap();
    // The opponent answers with No More Lies on the victim.
    driver.pass().unwrap();
    let target = {
        let id = *driver
            .state()
            .stack()
            .iter()
            .find(|i| driver.state().object(**i).unwrap().card().name == "Victim")
            .unwrap();
        Target::Object(driver.state().object(id).unwrap().reference())
    };
    driver.begin_cast("No More Lies").unwrap();
    for _ in 0..4 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))
                    .unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => break,
        }
    }
    run(&mut driver, false, &[]);
    let seat = |i| mtg_core::ids::PlayerId::from_index(i);
    let in_exile = names(&driver, Zone::Exile).contains(&"Victim".to_string());
    let in_graveyard = names(&driver, Zone::Graveyard(seat(0))).contains(&"Victim".to_string());
    assert_eq!((in_exile, in_graveyard), (exiled, !exiled));
}
