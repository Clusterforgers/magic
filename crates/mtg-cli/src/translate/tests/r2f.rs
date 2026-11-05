//! Goal: trigger lines whose event the engine already has read as that event with their state words as an intervening condition, and each such trigger fires in a played game exactly when its words say.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::trigger::TriggerPattern;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, creature, forest, instant, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const GAIN: &str = "SVar:Gain:DB$ GainLife | Defined$ You | LifeAmount$ 1\n";

/// The `when` and `if` of the first triggered ability of a creature that holds the trigger line.
fn trigger_of(line: &str) -> Value {
    let script = format!("Name:Subject\nTypes:Creature Elf\nPT:1/1\n{line}\n{GAIN}");
    assert_eq!(gaps(&script), Vec::<String>::new(), "{line}");
    let ability = abilities(&script)
        .into_iter()
        .find(|ability| ability.get("Triggered").is_some())
        .unwrap();
    let inner = &ability["Triggered"];
    let mut out = json!({ "when": inner["when"] });
    if let Some(condition) = inner.get("if") {
        out["if"] = condition.clone();
    }
    // Every pattern the translator writes is one the DSL parses.
    serde_json::from_value::<TriggerPattern>(inner["when"].clone()).unwrap();
    out
}

#[rstest]
#[case::exploit(
    "T:Mode$ Exploited | ValidCard$ Creature | ValidSource$ Card.Self | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Exploits": {"what": ["This"]}}})
)]
#[case::expend(
    "T:Mode$ ManaExpend | Amount$ 4 | Player$ You | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Expend": {"who": "You", "n": 4}}})
)]
#[case::opponent_second_draw(
    "T:Mode$ Drawn | ValidPlayer$ Opponent | Number$ 2 | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Nth": {"event": {"Drew": {"who": "Opponents"}}, "nth": 2}}})
)]
#[case::you_draw(
    "T:Mode$ Drawn | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Drew": {"who": "You"}}})
)]
#[case::life_total(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | LifeTotal$ You | LifeAmount$ LE10 | Execute$ Gain",
    json!({"when": {"StepBegins": {"step": "Upkeep", "whose": "You"}},
        "if": {"Compare": {"left": {"Life": "You"}, "op": "Le", "right": 10}}})
)]
#[case::monarch_end_step(
    "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | CheckDefinedPlayer$ You.isMonarch | Execute$ Gain",
    json!({"when": {"StepBegins": {"step": "End", "whose": "You"}}, "if": {"Monarch": "You"}})
)]
#[case::initiative_end_step(
    "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | CheckDefinedPlayer$ You.hasInitiative | Execute$ Gain",
    json!({"when": {"StepBegins": {"step": "End", "whose": "You"}}, "if": {"Initiative": "You"}})
)]
#[case::mana_not_spent(
    "T:Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Card.Self | ManaNotSpent$ W | Execute$ Gain",
    json!({"when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "if": {"Not": {"ManaSpent": "White"}}})
)]
#[case::become_monarch(
    "T:Mode$ BecomeMonarch | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"BecomesMonarch": {"who": "You"}}})
)]
#[case::day_night(
    "T:Mode$ DayTimeChanges | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"DayNightChanges": {}}})
)]
#[case::class_level(
    "T:Mode$ ClassLevelGained | ClassLevel$ 2 | ValidCard$ Card.Self | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"BecomesLevel": {"what": ["This"], "level": 2}}})
)]
#[case::phases_in(
    "T:Mode$ PhaseIn | ValidCard$ Card.Self | Execute$ Gain",
    json!({"when": {"PhasedIn": {"what": ["This"]}}})
)]
#[case::phases_out(
    "T:Mode$ PhaseOut | ValidCard$ Card.phasedOutSelf | Execute$ Gain",
    json!({"when": {"PhasedOut": {"what": ["This"]}}})
)]
#[case::copy_spell(
    "T:Mode$ SpellCopy | ValidCard$ Instant | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"SpellCopied": {"what": [{"Type": "Instant"}], "by": "You"}}})
)]
#[case::dice(
    "T:Mode$ RolledDieOnce | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"DiceRolled": {"who": "You"}}})
)]
#[case::coin(
    "T:Mode$ FlippedCoin | ValidPlayer$ You | ValidResult$ Win | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"CoinFlipWon": {"who": "You"}}})
)]
#[case::first_life_loss(
    "T:Mode$ LifeLost | ValidPlayer$ You | FirstTime$ True | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Nth": {"event": {"LifeLost": {"who": "You"}}, "nth": 1}}})
)]
#[case::first_attack(
    "T:Mode$ Attacks | ValidCard$ Card.Self | FirstAttack$ True | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Nth": {"event": {"Attacks": {"what": ["This"]}}, "nth": 1}}})
)]
#[case::sacrificed_token(
    "T:Mode$ Sacrificed | ValidPlayer$ You | ValidCard$ Permanent.token+YouCtrl | Origin$ Any | Destination$ Battlefield | TriggerZones$ Battlefield | Execute$ Gain",
    json!({"when": {"Sacrificed": {"what": ["Token", {"ControlledBy": "You"}], "by": "You"}}})
)]
fn the_line_reads_as_the_event_and_its_condition(#[case] line: &str, #[case] expected: Value) {
    let found = trigger_of(line);
    assert_eq!(found, expected, "{line}");
}

#[rstest]
#[case::exploited_narrower(
    "T:Mode$ Exploited | ValidCard$ Creature.nonHuman | ValidSource$ Card.Self | Execute$ Gain",
    "key:Trigger.Exploited.ValidCard=Creature.nonHuman"
)]
#[case::lose_a_flip(
    "T:Mode$ FlippedCoin | ValidPlayer$ You | ValidResult$ Lose | Execute$ Gain",
    "key:Trigger.FlippedCoin.ValidResult=Lose"
)]
#[case::monarch_of_another(
    "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | CheckDefinedPlayer$ You.withMostLife | Execute$ Gain",
    "key:Trigger.CheckDefinedPlayer=You.withMostLife"
)]
#[case::life_without_amount(
    "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | LifeTotal$ You | Execute$ Gain",
    "key:Trigger.LifeTotal-without-LifeAmount"
)]
#[case::sacrificed_from_elsewhere(
    "T:Mode$ Sacrificed | ValidPlayer$ You | ValidCard$ Permanent | Origin$ Hand | Execute$ Gain",
    "key:Trigger.Sacrificed.Origin=Hand"
)]
#[case::first_attack_alone(
    "T:Mode$ Attacks | ValidCard$ Card.Self | Alone$ True | FirstAttack$ True | Execute$ Gain",
    "key:Trigger.Attacks.FirstAttack-with-more"
)]
fn a_word_the_engine_cannot_check_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = format!("Name:Subject\nTypes:Creature Elf\nPT:1/1\n{line}\n{GAIN}");
    assert_eq!(gaps(&script), vec![gap.to_owned()], "{line}");
}

#[test]
fn a_one_off_trigger_of_an_effect_is_a_delayed_trigger_that_fires_once() {
    let script = "Name:Booster\nTypes:Instant\nA:SP$ Effect | Triggers$ Cast | Duration$ EOT\nSVar:Cast:Mode$ SpellCast | ValidCard$ Creature | ValidActivatingPlayer$ You | OneOff$ True | TriggerZones$ Command | Execute$ Gain\nSVar:Gain:DB$ GainLife | Defined$ You | LifeAmount$ 1\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let text = serde_json::to_string(&abilities(script)).unwrap();
    assert!(
        text.contains(r#""once":true"#) || !text.contains(r#""once":false"#),
        "{text}"
    );
    assert!(text.contains("DelayedTrigger"), "{text}");
}

// ---- play ----

fn named(
    name: &str,
    types: CardType,
    mana: u32,
    power: Option<(i32, i32)>,
    script: &str,
) -> Arc<CardDefinition> {
    let text = format!("Name:{name}\nTypes:Creature Elf\n{script}\n{GAIN}");
    assert_eq!(gaps(&text), Vec::<String>::new(), "{script}");
    let mut card = CardDefinition::new(name)
        .with_type(types)
        .with_subtype("Elf")
        .with_mana_cost(cost(mana, &[]));
    if let Some((power, toughness)) = power {
        card = card.with_power_toughness(power, toughness);
    }
    for value in abilities(&text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

fn subject(script: &str) -> Arc<CardDefinition> {
    named("Subject", CardType::Creature, 0, Some((1, 1)), script)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

/// Answers decisions until the stack is empty at `until`: yes to a question when `may`, the card named `pick` when asked.
fn settle(driver: &mut Driver, until: Step, may: bool, pick: Option<&str>) {
    for _ in 0..120 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().turn().step() == until && driver.state().stack().is_empty() =>
            {
                return;
            }
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(may))).unwrap();
            }
            DecisionKind::ChooseCards(_) if pick.is_some() => {
                let chosen = driver.id_of(pick.unwrap()).unwrap();
                driver
                    .answer(Answer::ChooseCards(CardsChoice(vec![chosen])))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the game did not settle at {until:?}");
}

#[rstest]
#[case::the_fourth_mana_triggers(4, 21)]
#[case::three_mana_do_not(3, 20)]
fn expend_four_triggers_on_the_fourth_mana_spent_on_spells(#[case] spend: u32, #[case] after: i32) {
    let line =
        "T:Mode$ ManaExpend | Amount$ 4 | Player$ You | TriggerZones$ Battlefield | Execute$ Gain";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [instant("Spell", spend, &[])])
        .battlefield(
            0,
            std::iter::once(ready(subject(line))).chain((0..4).map(|_| ready(forest()))),
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Spell").unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), after);
}

#[rstest]
#[case::the_second_card_of_an_opponent(1, true)]
#[case::your_own_second_card_does_not(0, false)]
fn an_opponents_second_draw_of_a_turn_triggers_once(#[case] drawer: usize, #[case] fires: bool) {
    let line = "T:Mode$ Drawn | ValidPlayer$ Opponent | Number$ 2 | TriggerZones$ Battlefield | Execute$ Gain";
    let library = || (0..8).map(|_| forest());
    let scenario = Scenario::new(2)
        .at(1, drawer, Step::Upkeep)
        .library(0, library())
        .library(1, library())
        .hand(drawer, [mtg_testkit::cards::study("Study")])
        .battlefield(0, std::iter::once(ready(subject(line))))
        .battlefield(drawer, (0..1).map(|_| ready(mtg_testkit::cards::island())));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // The draw step draws the first card; the sorcery draws the second and the third.
    driver.pass_until(Step::Main1).unwrap();
    driver.cast("Study").unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), if fires { 21 } else { 20 });
}

#[rstest]
#[case::low(8, 9)]
#[case::exactly_ten(10, 11)]
#[case::high(15, 15)]
fn an_upkeep_trigger_with_a_life_total_checks_the_total(#[case] total: i32, #[case] after: i32) {
    let line = "T:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | LifeTotal$ You | LifeAmount$ LE10 | Execute$ Gain";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Untap)
        .life(0, total)
        .battlefield(0, std::iter::once(ready(subject(line))));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::Draw).unwrap();
    assert_eq!(life(&driver, 0), after);
}

/// A creature that makes its controller the monarch as it enters.
fn maker() -> Arc<CardDefinition> {
    let crown: Ability = serde_json::from_value(json!({"Triggered": {
        "when": {"ZoneChange": {"what": ["This"], "to": "Battlefield"}},
        "effects": [{"BecomeMonarch": {}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Maker")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[]))
            .with_power_toughness(1, 1)
            .with_rules(crown),
    )
}

#[test]
fn becoming_the_monarch_triggers_and_the_end_step_trigger_needs_the_crown() {
    let end = "T:Mode$ Phase | Phase$ End of Turn | ValidPlayer$ You | TriggerZones$ Battlefield | CheckDefinedPlayer$ You.isMonarch | Execute$ Gain";
    let crowned =
        "T:Mode$ BecomeMonarch | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ Gain";
    for (crowned_by_maker, expected) in [(true, 22), (false, 20)] {
        let hand: Vec<Arc<CardDefinition>> = if crowned_by_maker {
            vec![maker()]
        } else {
            Vec::new()
        };
        let scenario = Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, hand)
            .battlefield(
                0,
                [
                    ready(named("Watcher", CardType::Creature, 0, Some((1, 1)), end)),
                    ready(named(
                        "Herald",
                        CardType::Creature,
                        0,
                        Some((1, 1)),
                        crowned,
                    )),
                ],
            );
        let mut driver = Driver::from_scenario(scenario).unwrap();
        if crowned_by_maker {
            driver.cast("Maker").unwrap();
            settle(&mut driver, Step::Main1, false, None);
            // The crown itself draws a card at the end step; the watcher gains a life there too.
            assert_eq!(life(&driver, 0), 21);
        }
        driver.idle_until(1, Step::End).unwrap();
        settle(&mut driver, Step::End, false, None);
        assert_eq!(life(&driver, 0), expected, "maker: {crowned_by_maker}");
    }
}

#[rstest]
#[case::paid_with_the_colour(&["Plains"], 20)]
#[case::paid_without_it(&["Forest"], 21)]
fn a_permanent_not_cast_with_the_mana_is_still_there_only_when_it_was_spent(
    #[case] lands: &[&str],
    #[case] after: i32,
) {
    // "When this enters, you gain 1 life unless {W} was spent to cast it."
    let line = "T:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | ManaNotSpent$ W | Execute$ Gain";
    let card = named("Sleeper", CardType::Creature, 1, Some((1, 1)), line);
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [card])
        .battlefield(0, [ready(plains()), ready(forest())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast_tapping("Sleeper", lands).unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), after);
}

#[rstest]
#[case::exploiting(true, 21)]
#[case::declining(false, 20)]
fn exploiting_a_creature_triggers_the_exploiter(#[case] exploit: bool, #[case] after: i32) {
    let line = "K:Exploit\nT:Mode$ Exploited | ValidCard$ Creature | ValidSource$ Card.Self | TriggerZones$ Battlefield | Execute$ Gain";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [subject(line)])
        .battlefield(0, [ready(creature("Fodder", 0, &[], 1, 1))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Subject").unwrap();
    settle(&mut driver, Step::Main1, exploit, Some("Fodder"));
    assert_eq!(life(&driver, 0), after);
}

#[test]
fn losing_life_for_the_first_time_each_turn_triggers_once() {
    let line = "T:Mode$ LifeLost | ValidPlayer$ You | FirstTime$ True | TriggerZones$ Battlefield | Execute$ Gain";
    let drip = |name: &str| {
        let text =
            format!("Name:{name}\nTypes:Instant\nA:SP$ LoseLife | Defined$ You | LifeAmount$ 1\n");
        assert_eq!(gaps(&text), Vec::<String>::new());
        let mut card = CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]));
        for value in abilities(&text) {
            card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
        }
        Arc::new(card)
    };
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [drip("First"), drip("Second")])
        .battlefield(0, std::iter::once(ready(subject(line))));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for name in ["First", "Second"] {
        driver.cast(name).unwrap();
        settle(&mut driver, Step::Main1, false, None);
    }
    // Two points lost and one gained.
    assert_eq!(life(&driver, 0), 19);
}

#[test]
fn attacking_for_the_first_time_in_a_turn_triggers() {
    let line = "T:Mode$ Attacks | ValidCard$ Card.Self | FirstAttack$ True | TriggerZones$ Battlefield | Execute$ Gain";
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(subject(line))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.pass_until(Step::DeclareAttackers).unwrap();
    driver.attack(&[("Subject", 1)]).unwrap();
    settle(&mut driver, Step::DeclareAttackers, false, None);
    assert_eq!(life(&driver, 0), 21);
}

#[test]
fn a_one_off_cast_trigger_fires_for_the_next_creature_spell_only() {
    let booster = "Name:Booster\nTypes:Instant\nA:SP$ Effect | Triggers$ Cast | Duration$ EOT\nSVar:Cast:Mode$ SpellCast | ValidCard$ Creature | ValidActivatingPlayer$ You | OneOff$ True | TriggerZones$ Command | Execute$ Gain\nSVar:Gain:DB$ GainLife | Defined$ You | LifeAmount$ 1\n";
    assert_eq!(gaps(booster), Vec::<String>::new());
    let mut card = CardDefinition::new("Booster")
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(booster) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    let scenario = Scenario::new(2).at(1, 0, Step::Main1).hand(
        0,
        [
            Arc::new(card),
            creature("One", 0, &[], 1, 1),
            creature("Two", 0, &[], 1, 1),
        ],
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for name in ["Booster", "One", "Two"] {
        driver.cast(name).unwrap();
        settle(&mut driver, Step::Main1, false, None);
    }
    assert_eq!(life(&driver, 0), 21);
}

const DYING: &str = "T:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard,Exile | ValidCard$ Card.Self | Execute$ Gain";

fn removal(name: &str, api: &str) -> Arc<CardDefinition> {
    let text = format!("Name:{name}\nTypes:Instant\nA:SP$ {api} | ValidTgts$ Creature\n");
    assert_eq!(gaps(&text), Vec::<String>::new(), "{api}");
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Instant)
        .with_mana_cost(cost(0, &[]));
    for value in abilities(&text) {
        card = card.with_rules(serde_json::from_value::<Ability>(value).unwrap());
    }
    Arc::new(card)
}

#[rstest]
#[case::dies("Destroy")]
#[case::is_exiled("ChangeZone | Origin$ Battlefield | Destination$ Exile")]
fn a_creature_that_dies_or_is_put_into_exile_triggers_once_either_way(#[case] api: &str) {
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, [removal("Remover", api)])
        .battlefield(0, std::iter::once(ready(subject(DYING))));
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Subject").unwrap();
    driver.cast_at("Remover", &[target]).unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), 21);
}

#[test]
fn the_two_lines_of_an_eminence_ability_work_from_the_command_zone_and_the_battlefield() {
    let eminence = "T:Mode$ SpellCast | ValidCard$ Card.Elf+Other | ValidActivatingPlayer$ You | TriggerZones$ Command | PresentZone$ Command | IsPresent$ Card.StrictlySelf | Execute$ Gain\nT:Mode$ SpellCast | ValidCard$ Card.Elf+Other | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | PresentZone$ Battlefield | IsPresent$ Card.StrictlySelf | Execute$ Gain | Secondary$ True";
    let script = format!("Name:Subject\nTypes:Legendary Creature Elf\nPT:1/1\n{eminence}\n{GAIN}");
    let found: Vec<Value> = abilities(&script)
        .into_iter()
        .filter(|ability| ability.get("Triggered").is_some())
        .collect();
    assert_eq!(gaps(&script), Vec::<String>::new());
    let zones: Vec<Value> = found
        .iter()
        .map(|ability| {
            ability["Triggered"]
                .get("from")
                .cloned()
                .unwrap_or(Value::Null)
        })
        .collect();
    assert_eq!(zones, vec![json!("Command"), Value::Null]);
}

#[test]
fn an_eminence_commander_triggers_from_the_command_zone_and_again_on_the_battlefield() {
    let eminence = "T:Mode$ SpellCast | ValidCard$ Card.Elf+Other | ValidActivatingPlayer$ You | TriggerZones$ Command | PresentZone$ Command | IsPresent$ Card.StrictlySelf | Execute$ Gain\nT:Mode$ SpellCast | ValidCard$ Card.Elf+Other | ValidActivatingPlayer$ You | TriggerZones$ Battlefield | PresentZone$ Battlefield | IsPresent$ Card.StrictlySelf | Execute$ Gain | Secondary$ True";
    let commander = subject(eminence);
    let elf = |name: &str| named(name, CardType::Creature, 0, Some((1, 1)), "");
    let scenario = Scenario::new(2)
        .format(Arc::new(mtg_engine::format::Commander))
        .commanders(0, [commander])
        .at(1, 0, Step::Main1)
        .hand(0, [elf("First"), elf("Second")]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    // A Commander game starts at 40 life.
    driver.cast("First").unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), 41);
    driver.cast("Subject").unwrap();
    settle(&mut driver, Step::Main1, false, None);
    assert_eq!(life(&driver, 0), 41);
    driver.cast("Second").unwrap();
    settle(&mut driver, Step::Main1, false, None);
    // Once from the command zone and once from the battlefield.
    assert_eq!(life(&driver, 0), 42);
}
