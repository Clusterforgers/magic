//! Goal: a replacement effect whose steps ask a question (CR 614.1, 616) applies: "you may ... instead" asks the player its `May` names (the replacement's controller by default, the affected player for "that player may") and a no leaves the draw, damage, death or entering as it was, a choice of one way ("choose land or nonland and reveal until") asks and runs the way chosen, the questions are the step's own decisions of the batch, and the log replays to the live state; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::{OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, forest, island, mountain};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// An enchantment named `name` with the replacement `rule` (JSON).
fn enchantment(name: &str, rule: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(rule).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

/// A free sorcery that draws a card.
fn peek() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(r#"{"Spell":{"effects":[{"Draw":{}}]}}"#).unwrap();
    Arc::new(
        CardDefinition::new("Peek")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_core::mana::ManaCost::default())
            .with_rules(rule),
    )
}

/// "If you would draw a card, you may gain 3 life instead."
const DRAW_MAY: &str = r#"{"Replacement":{"when":{"Draw":{}},"by":{"Instead":[{"May":{"effects":[{"GainLife":{"amount":3}}]}}]}}}"#;
/// "If you would draw a card, choose one: gain 1 life; or lose 1 life."
const DRAW_ONE_OF: &str = r#"{"Replacement":{"when":{"Draw":{}},"by":{"Instead":[{"OneOf":{"options":[[{"GainLife":{"amount":1}}],[{"LoseLife":{"amount":1}}]]}}]}}}"#;
/// Abundance: "you may instead choose land or nonland and reveal cards from the top of your library until you reveal a
/// card of the chosen kind. Put that card into your hand and put all other cards revealed this way on the bottom of
/// your library in any order."
const ABUNDANCE: &str = r#"{"Replacement":{"when":{"Draw":{}},"by":{"Instead":[{"May":{"effects":[
    {"ChooseValue":{"kind":{"Options":["land","nonland"]},"as":"kind"}},
    {"If":{"condition":{"ChosenWord":{"name":"kind","word":"land"}},
      "then":[{"RevealUntil":{"filter":[{"Type":"Land"}],"as":"found","revealed_as":"revealed"}},
              {"Move":{"what":{"Bound":"found"},"to":"Hand"}},
              {"Move":{"what":{"Bound":"revealed"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}],
      "otherwise":[{"RevealUntil":{"filter":[{"Not":{"Type":"Land"}}],"as":"found","revealed_as":"revealed"}},
              {"Move":{"what":{"Bound":"found"},"to":"Hand"}},
              {"Move":{"what":{"Bound":"revealed"},"to":{"Library":{"place":"Bottom","order":"Chosen"}}}}]}}]}}]}}}"#;
/// "If a source would deal damage to you, you may gain that much life instead."
const DAMAGE_MAY: &str = r#"{"Replacement":{"when":{"Damage":{"to":"You"}},"by":{"Instead":[{"May":{"effects":[{"GainLife":{"amount":"Triggering"}}]}}]}}}"#;
/// "If a source would deal damage to an opponent, that player may lose 1 life instead."
const DAMAGE_THAT_PLAYER: &str = r#"{"Replacement":{"when":{"Damage":{"to":"Opponents"}},"by":{"Instead":[{"May":{"who":"Triggering","effects":[{"LoseLife":{"who":"Triggering","amount":1}}]}}]}}}"#;
/// "If a creature you control would die, you may exile it instead."
const DIES_MAY: &str = r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"by":{"Instead":[{"May":{"effects":[{"Exile":{"what":"Triggering"}}]}}]}}}"#;
/// "If a creature would enter, you may exile it instead."
const ENTERS_MAY: &str = r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"}]}},"by":{"Instead":[{"May":{"effects":[{"Exile":{"what":"Triggering"}}]}}]}}}"#;

/// Answers the questions of the replacement with `script` in order (yes or no, an option) and every other question
/// passively, passing priority while the stack holds something. The players asked by the script, in order.
fn settle(driver: &mut Driver, script: &[Answer]) -> Result<Vec<PlayerId>, ScriptError> {
    let mut script = script.iter();
    let mut asked = Vec::new();
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        let answer = match &decision.kind {
            DecisionKind::YesNo(_) | DecisionKind::ChooseOption(_) => {
                asked.push(decision.player);
                script.next().cloned().expect("a scripted answer")
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    assert!(
        script.next().is_none(),
        "every scripted answer was asked for"
    );
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(asked)
}

fn yes(said: bool) -> Answer {
    Answer::YesNo(YesNoChoice(said))
}

fn option(index: usize) -> Answer {
    Answer::ChooseOption(OptionChoice(index))
}

/// Seat 0 controls the enchantment and holds Peek; seat 0's library is two Bears over Forests.
fn drawing(seats: usize, rule: &str) -> Scenario {
    let library = [bear(), bear(), forest(), forest(), island(), forest()];
    (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, [ready(enchantment("Rule", rule))])
            .hand(0, [peek()])
            .library(0, library),
        |scenario, seat| {
            if seat == 0 {
                scenario
            } else {
                scenario.library(seat, vec![forest(); 6])
            }
        },
    )
}

#[rstest]
#[case::may_yes(DRAW_MAY, vec![yes(true)], 0, 23)]
#[case::may_no_draws(DRAW_MAY, vec![yes(false)], 1, 20)]
#[case::one_of_first(DRAW_ONE_OF, vec![option(0)], 0, 21)]
#[case::one_of_second(DRAW_ONE_OF, vec![option(1)], 0, 19)]
fn a_draw_replaced_by_a_question_asks_its_controller(
    #[case] rule: &str,
    #[case] script: Vec<Answer>,
    #[case] drawn: usize,
    #[case] life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(drawing(seats, rule))?;
    driver.cast("Peek")?;
    let asked = settle(&mut driver, &script)?;
    assert_eq!(asked, vec![pid(0); script.len()]);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), drawn);
    driver.expect_life(0, life)
}

#[rstest]
#[case::land(vec![yes(true), option(0)], "Forest", &["Forest", "Island", "Forest", "Grizzly Bears", "Grizzly Bears"][..])]
#[case::nonland(vec![yes(true), option(1)], "Grizzly Bears", &["Grizzly Bears", "Forest", "Forest", "Island", "Forest"][..])]
#[case::declined(vec![yes(false)], "Grizzly Bears", &["Grizzly Bears", "Forest", "Forest", "Island", "Forest"][..])]
fn abundance_reveals_until_the_kind_chosen(
    #[case] script: Vec<Answer>,
    #[case] got: &str,
    #[case] library: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(drawing(seats, ABUNDANCE))?;
    driver.cast("Peek")?;
    settle(&mut driver, &script)?;
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), [got]);
    // The Bears revealed on the way to a land go to the bottom; the card found leaves the library once.
    let top_first: Vec<String> = driver
        .state()
        .zone(Zone::Library(pid(0)))
        .iter()
        .rev()
        .map(|id| crate::common::name_of(driver.state(), *id))
        .collect();
    assert_eq!(top_first, library);
    Ok(())
}

#[rstest]
#[case::controller_yes(DAMAGE_MAY, 0, true, 0, 23)]
#[case::controller_no(DAMAGE_MAY, 0, false, 0, 17)]
#[case::that_player_yes(DAMAGE_THAT_PLAYER, 1, true, 0, 19)]
#[case::that_player_no(DAMAGE_THAT_PLAYER, 1, false, 0, 17)]
fn damage_replaced_by_a_question_asks_the_player_it_names(
    #[case] rule: &str,
    #[case] owner: usize,
    #[case] said: bool,
    #[case] asked_seat: usize,
    #[case] life: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Seat 0 aims its own Bolt at itself; the rule is seat 0's or an opponent's.
    let rule = ready(enchantment("Rule", rule));
    let scenario = if owner == 0 {
        Scenario::new(seats).battlefield(0, [rule, ready(mountain())])
    } else {
        Scenario::new(seats)
            .battlefield(0, [ready(mountain())])
            .battlefield(owner, [rule])
    }
    .hand(0, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let me = driver.target_player(0);
    driver.cast_at("Bolt", &[me])?;
    let asked = settle(&mut driver, &[yes(said)])?;
    assert_eq!(asked, [pid(asked_seat)]);
    driver.expect_life(0, life)
}

#[rstest]
#[case::exiled(true, Zone::Exile)]
#[case::dies(false, Zone::Graveyard(pid(0)))]
fn a_death_replaced_by_a_may_asks_the_controller(
    #[case] said: bool,
    #[case] zone: Zone,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(
            0,
            [
                ready(enchantment("Rule", DIES_MAY)),
                ready(bear()),
                ready(mountain()),
            ],
        )
        .hand(0, [burn("Bolt", 3)]);
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Bolt", &[target])?;
    let asked = settle(&mut driver, &[yes(said)])?;
    assert_eq!(asked, [pid(0)]);
    let there = names(driver.state(), zone);
    assert_eq!(
        there.iter().filter(|n| *n == "Grizzly Bears").count(),
        1,
        "{there:?}"
    );
    Ok(())
}

#[rstest]
#[case::exiled(true, Zone::Exile)]
#[case::enters(false, Zone::Battlefield)]
fn an_entering_replaced_by_a_may_asks_the_controller(
    #[case] said: bool,
    #[case] zone: Zone,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let free_bear = mtg_testkit::cards::free_creature("Grizzly Bears", 2, 2);
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(enchantment("Rule", ENTERS_MAY))])
        .hand(0, [free_bear]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Grizzly Bears")?;
    let asked = settle(&mut driver, &[yes(said)])?;
    assert_eq!(asked, [pid(0)]);
    let there = names(driver.state(), zone);
    assert!(there.contains(&"Grizzly Bears".to_owned()), "{there:?}");
    Ok(())
}
