//! Goal: damage shields that multiply or redirect damage are played (and a shape the engine lacks stays by hand), a rule the engine cannot play never stops a game, and the fixes of the round 7 audit run in real games with the case where they must not apply.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::card::Card;
use mtg_cards::support::{PrintedCard, PrintedFace, oracle_fingerprint};
use mtg_cards::{CardDefinition, builtin};
use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use crate::r3p3_runs::settle;

/// The definition the engine runs for these abilities, or why it would not run them.
pub(crate) fn defined(
    abilities: &Value,
    name: &str,
    types: &str,
    cost: &str,
) -> Result<Arc<CardDefinition>, String> {
    let oracle_id = format!("00000000-0000-4000-8000-{:012}", name.len());
    let card: Card = serde_json::from_value(json!({
        "oracle_id": oracle_id, "name": name, "impl_status": "Full",
        "faces": [{"name": name, "oracle_fingerprint": oracle_fingerprint(name), "abilities": abilities}]}))
    .unwrap();
    let printed = PrintedCard {
        oracle_id,
        name: name.to_owned(),
        keywords: vec![],
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: cost.to_owned(),
            type_line: types.to_owned(),
            oracle_text: name.to_owned(),
            power: types.contains("Creature").then(|| "1".to_owned()),
            toughness: types.contains("Creature").then(|| "1".to_owned()),
            loyalty: None,
            defense: None,
            colors: vec![],
        }],
    };
    mtg_cli::drafts::read_definitions(
        builtin::library().unwrap(),
        &mtg_engine::caps(),
        &[(&printed, &card)],
    )
    .into_iter()
    .next()
    .unwrap()
    .map(Arc::new)
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver
        .state()
        .player(PlayerId::from_index(seat))
        .unwrap()
        .life()
}

#[rstest]
#[case::double(json!({"Multiply": 2}), Some(14))]
#[case::redirected(
    json!({"Instead": [{"DealDamage": {"amount": "Triggering", "source": "Dealer", "to": "You"}}]}),
    Some(20)
)]
#[case::half_prevented_waits(json!("PreventHalf"), None)]
fn a_damage_shield_is_played_or_left_by_hand(#[case] by: Value, #[case] opponent: Option<i32>) {
    let rules = json!([{"Spell": {"effects": [
        {"Shield": {"when": {"Damage": {"source": [{"ControlledBy": "You"}], "to": "Opponents"}},
            "by": by, "duration": "UntilEndOfTurn"}},
        {"DealDamage": {"to": "Opponents", "amount": 3}}]}}]);
    let spell = defined(&rules, "Bolt Shield", "Instant", "{R}");
    let Some(opponent) = opponent else {
        assert!(spell.is_err(), "the engine lacks this shield");
        return;
    };
    let scenario = Scenario::new(2)
        .hand(0, [spell.unwrap()])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Bolt Shield").unwrap();
    settle(&mut driver);
    assert_eq!(life(&driver, 1), opponent);
}

#[rstest]
#[case::fixed(json!(1), true)]
#[case::half_of_a_stat(json!({"Half": {"up": true, "of": {"Stat": {"of": {"Bound": "d"}, "stat": "Power"}}}}), false)]
fn a_copy_with_a_computed_size_stays_by_hand(#[case] power: Value, #[case] runs: bool) {
    let rules = json!([{"Spell": {"effects": [
        {"Destroy": {"what": "This", "as": "d"}},
        {"CreateToken": {"token": {"CopyOf": {"of": {"Bound": "d"}, "except": [
            {"SetPT": {"power": power, "toughness": 1}}]}}}}]}}]);
    assert_eq!(defined(&rules, "Saw Half", "Instant", "{R}").is_ok(), runs);
}

use crate::r3p3_runs::{translated, translated_in};
use mtg_cli::translate::ctx::World;

#[rstest]
#[case::the_third_counter_one_at_a_time(1, 3, 5)]
#[case::two_is_not_the_third(1, 2, 0)]
#[case::three_at_once_is_a_total_of_three(3, 1, 5)]
#[case::steps_over_three(2, 2, 0)]
fn the_nth_counter_is_the_total_on_the_object(
    #[case] per_step: u32,
    #[case] steps: usize,
    #[case] gained: i32,
) {
    let script = format!(
        "Name:Plan Board\nTypes:Enchantment\nA:AB$ PutCounter | Cost$ 0 | CounterType$ PLAN | CounterNum$ {per_step}\nT:Mode$ CounterAdded | ValidCard$ Card.Self | TriggerZones$ Battlefield | CounterType$ PLAN | CounterAmount$ EQ3 | Execute$ TrigGain | TriggerDescription$ When the third plan counter is put on this enchantment, you gain 5 life.\nSVar:TrigGain:DB$ GainLife | LifeAmount$ 5\n"
    );
    let board = translated(&script, "Plan Board", "Enchantment", "{0}");
    let scenario = Scenario::new(2).battlefield(0, [ready(board)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..steps {
        driver.begin_activation("Plan Board").unwrap();
        driver.pay_tapping(&[], 0).unwrap();
        settle(&mut driver);
    }
    assert_eq!(life(&driver, 0), 20 + gained);
}

fn zombie() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Zed")
            .with_type(mtg_core::card::CardType::Creature)
            .with_subtype("Zombie")
            .with_power_toughness(1, 1),
    )
}

const PEEK: &str = "Name:Grave Peek\nManaCost:R\nTypes:Sorcery\nA:SP$ PeekAndReveal | PeekAmount$ 1 | RevealValid$ Zombie | RevealOptional$ True | RememberRevealed$ True | SubAbility$ DBToHand\nSVar:DBToHand:DB$ ChangeZone | Defined$ Remembered | Origin$ Library | Destination$ Hand | SubAbility$ DBToGrave\nSVar:DBToGrave:DB$ ChangeZone | Defined$ TopOfLibrary | Origin$ Library | Destination$ Graveyard | Optional$ True | ConditionDefined$ Remembered | ConditionPresent$ Card | ConditionCompare$ EQ0 | SubAbility$ DBCleanup\nSVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True\n";

#[rstest]
#[case::taken_so_nothing_else_is_asked(zombie(), 1, 1)]
#[case::not_a_zombie_so_the_top_card_may_go(forest(), 0, 2)]
fn if_you_dont_take_the_card_counts_the_card_in_its_new_zone(
    #[case] top: Arc<CardDefinition>,
    #[case] zombies_in_hand: usize,
    #[case] graveyard: usize,
) {
    let mut world = World::default();
    world.subtypes.insert("Zombie".to_owned());
    let spell = translated_in(&world, PEEK, "Grave Peek", "Sorcery", "{R}");
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .library(0, vec![top, mountain(), mountain(), mountain()])
        .battlefield(0, [ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Grave Peek").unwrap();
    settle(&mut driver);
    let me = PlayerId::from_index(0);
    let count = |zone, name: &str| {
        let state = driver.state();
        state
            .zone(zone)
            .iter()
            .filter(|id| state.object(**id).is_some_and(|o| o.card().name == name))
            .count()
    };
    assert_eq!(count(Zone::Hand(me), "Zed"), zombies_in_hand);
    // The spell is in the graveyard; the peeked card joins it only when nothing was taken.
    assert_eq!(driver.state().zone(Zone::Graveyard(me)).len(), graveyard);
    assert_eq!(
        driver.state().zone(Zone::Library(me)).len(),
        4 - zombies_in_hand - (graveyard - 1)
    );
}

const HYDRA: &str = "Name:Hydra Brood\nManaCost:4 G G\nTypes:Creature Hydra\nPT:7/7\nA:AB$ PutCounter | Cost$ X X G | Monstrosity$ X\nT:Mode$ BecomeMonstrous | ValidCard$ Card.Self | TriggerZones$ Battlefield | Execute$ TrigToken | TriggerDescription$ When CARDNAME becomes monstrous, create X X/X green Hydra creature tokens.\nSVar:TrigToken:DB$ Token | TokenAmount$ MonstrosityX | TokenScript$ g_x_x_hydra | TokenOwner$ You | TokenPower$ MonstrosityX | TokenToughness$ MonstrosityX\nSVar:X:Count$xPaid\nSVar:MonstrosityX:TriggerCount$Amount\n";

#[rstest]
#[case::one(1)]
#[case::two(2)]
fn monstrosity_x_makes_x_tokens_of_size_x(#[case] x: u32) {
    use mtg_cli::forge::script::Script;
    use mtg_engine::choose::NumberChoice;
    use mtg_engine::decision::{Answer, DecisionKind};
    let mut world = World::default();
    world.tokens.insert(
        "g_x_x_hydra".to_owned(),
        Script::parse("Name:Hydra Token\nColors:green\nTypes:Creature Hydra\nPT:*/*\n")
            .faces
            .into_iter()
            .next()
            .unwrap(),
    );
    let hydra = translated_in(
        &world,
        HYDRA,
        "Hydra Brood",
        "Creature — Hydra",
        "{4}{G}{G}",
    );
    let scenario = Scenario::new(2).battlefield(
        0,
        std::iter::once(ready(hydra)).chain((0..5).map(|_| ready(forest()))),
    );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Hydra Brood").unwrap();
    for _ in 0..40 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseNumber(_) => driver
                .answer(Answer::ChooseNumber(NumberChoice::Number(x)))
                .map(drop)
                .unwrap(),
            DecisionKind::PayMana(_) => driver
                .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
                .map(drop)
                .unwrap(),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                let next = match &decision.kind {
                    DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(true)),
                    _ => Driver::passive_answer(&decision),
                };
                driver.answer(next).map(drop).unwrap();
            }
        }
    }
    let state = driver.state();
    let tokens: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == "Hydra"))
        .map(|id| mtg_engine::characteristics::characteristics(state, *id).unwrap())
        .collect();
    assert_eq!(tokens.len(), x as usize);
    assert!(
        tokens
            .iter()
            .all(|c| c.power == Some(i32::try_from(x).unwrap()))
    );
}

const SWALLOWER: &str = "Name:Sky Gulp\nManaCost:G\nTypes:Creature Dragon\nPT:6/6\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigGainControl | TriggerDescription$ When CARDNAME enters, target opponent gains control of all other permanents you control.\nSVar:TrigGainControl:DB$ GainControl | ValidTgts$ Opponent | AllValid$ Permanent.StrictlyOther+YouCtrl | NewController$ TargetedPlayer\n";

#[test]
fn an_opponent_gains_control_of_the_other_permanents_you_control_and_only_those() {
    use mtg_engine::characteristics::controller_of;
    let mut world = World::default();
    world.subtypes.insert("Dragon".to_owned());
    let gulp = translated_in(&world, SWALLOWER, "Sky Gulp", "Creature — Dragon", "{G}");
    let scenario = Scenario::new(2)
        .hand(0, [gulp])
        .battlefield(0, [ready(forest()), ready(mtg_testkit::cards::bear())])
        .battlefield(
            1,
            [ready(mtg_testkit::cards::free_creature("Theirs", 2, 2))],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Sky Gulp").unwrap();
    settle(&mut driver);
    let controller = |name: &str| {
        let state = driver.state();
        let id = state
            .zone(Zone::Battlefield)
            .iter()
            .copied()
            .find(|id| state.object(*id).is_some_and(|o| o.card().name == name))
            .unwrap();
        controller_of(state, id).unwrap().index()
    };
    assert_eq!(controller("Sky Gulp"), 0, "the swallower stays");
    assert_eq!(controller("Grizzly Bears"), 1);
    assert_eq!(controller("Forest"), 1);
    assert_eq!(controller("Theirs"), 1);
}

fn field() -> Arc<CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_value(json!({"CostModifier": {
        "activating": "Any", "applies_to": [], "change": "Increase", "mana": "{2}"}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Suppression")
            .with_type(mtg_core::card::CardType::Enchantment)
            .with_rules(rules),
    )
}

fn walker_with_plus_one() -> Arc<CardDefinition> {
    let rules: mtg_cards::ability::Ability = serde_json::from_value(json!({"Loyalty": {
        "cost": {"Plus": 1}, "effects": [{"GainLife": {"amount": 3}}]}}))
    .unwrap();
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(mtg_core::card::CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(rules),
    )
}

#[rstest]
#[case::no_field_no_mana(false, 0, true, 23)]
#[case::no_field_the_lands_stay_untapped(false, 2, true, 23)]
#[case::field_without_mana(true, 1, false, 20)]
#[case::field_with_mana(true, 2, true, 23)]
fn a_cost_increase_on_activated_abilities_applies_to_loyalty_abilities(
    #[case] field_in_play: bool,
    #[case] lands: usize,
    #[case] activated: bool,
    #[case] life_after: i32,
) {
    let mut board = vec![ready(walker_with_plus_one())];
    board.extend((0..lands).map(|_| ready(forest())));
    let mut scenario = Scenario::new(2).battlefield(0, board);
    if field_in_play {
        scenario = scenario.battlefield(1, [ready(field())]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let began = driver.begin_ability("Walker", 0).is_ok();
    assert_eq!(began, activated);
    if began {
        for _ in 0..6 {
            let decision = driver.decision().unwrap().clone();
            if let mtg_engine::decision::DecisionKind::PayMana(_) = decision.kind {
                driver
                    .answer(mtg_engine::decision::Answer::PayMana(
                        mtg_engine::decision::PayChoice::Auto,
                    ))
                    .unwrap();
            }
        }
        settle(&mut driver);
        let untapped = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .filter(|id| {
                driver.state().object(**id).is_some_and(|o| {
                    o.card().name == "Forest" && o.permanent().is_some_and(|p| !p.tapped())
                })
            })
            .count();
        let left = if field_in_play { 0 } else { lands };
        assert_eq!(untapped, left, "the {{2}} is paid only under the field");
    }
    assert_eq!(life(&driver, 0), life_after);
}
