//! Goal: exert lines, mana of listed colors in any combination, mana triggers for the controller of a tapped permanent, creature spell lines and a card's own cast permission from its graveyard or exile translate to the engine's nodes, and a spell with a cost beyond its mana is castable exactly when that cost is payable.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::mountain;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const ONCE: &str = "IsPresent$ Creature.Self+notExertedThisTurn | ";

#[rstest]
#[case::plain("", "SVar:Go:DB$ Draw | NumCards$ 1", json!({"Exert": {}}), json!([{"Draw": {}}]))]
#[case::once_per_turn(ONCE, "SVar:Go:DB$ Draw | NumCards$ 1", json!({"Exert": {"once_per_turn": true}}), json!([{"Draw": {}}]))]
fn an_exert_line_is_the_keyword_and_the_trigger_on_the_exert(
    #[case] present: &str,
    #[case] svar: &str,
    #[case] keyword: Value,
    #[case] effects: Value,
) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nS:Mode$ OptionalAttackCost | ValidCard$ Card.Self | {present}Trigger$ Go | Cost$ Exert<1/CARDNAME>\n{svar}\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(found[0], json!({ "Keyword": keyword }));
    assert_eq!(found[1]["Triggered"]["effects"], effects);
    assert_eq!(
        found[1]["Triggered"]["when"],
        json!({"Did": {"action": "Exerted", "what": ["This"]}})
    );
}

#[rstest]
#[case::without_a_trigger("Cost$ Exert<1/CARDNAME>", json!([{"Keyword": {"Exert": {}}}]))]
fn an_exert_line_without_a_trigger_is_the_keyword(#[case] words: &str, #[case] expected: Value) {
    let text = format!(
        "Name:T\nTypes:Creature Elf\nPT:1/1\nS:Mode$ OptionalAttackCost | ValidCard$ Card.Self | {words}\n"
    );
    assert_eq!(abilities(&text), expected.as_array().unwrap().clone());
}

#[rstest]
#[case::other_cost(
    "ValidCard$ Card.Self | Cost$ Exert<2/CARDNAME>",
    "key:Static.OptionalAttackCost.Cost"
)]
#[case::other_card(
    "ValidCard$ Creature.YouCtrl | Cost$ Exert<1/CARDNAME>",
    "key:Static.OptionalAttackCost.ValidCard"
)]
#[case::other_condition(
    "ValidCard$ Card.Self | IsPresent$ Land.YouCtrl | Cost$ Exert<1/CARDNAME>",
    "key:Static.OptionalAttackCost.IsPresent"
)]
fn an_exert_line_of_another_shape_stays_a_gap(#[case] words: &str, #[case] gap: &str) {
    let text =
        format!("Name:T\nTypes:Creature Elf\nPT:1/1\nS:Mode$ OptionalAttackCost | {words}\n");
    assert_eq!(gaps(&text), vec![gap.to_owned()]);
}

fn artifact(line: &str) -> String {
    format!(
        "Name:Subject\nManaCost:2\nTypes:Artifact\n{line}\nSVar:X:Count$Valid Creature.YouCtrl\n"
    )
}

#[rstest]
#[case::two_of_two("Combo W B | Amount$ 2", json!({"count": 2, "among": ["White", "Black"]}))]
#[case::three_of_three("Combo R G | Amount$ 3", json!({"count": 3, "among": ["Red", "Green"]}))]
#[case::counted("Combo R G | Amount$ X", json!({"count": {"Count": [{"Type": "Creature"}, {"ControlledBy": "You"}]}, "among": ["Red", "Green"]}))]
#[case::three_colors("Combo U B R | Amount$ 2", json!({"count": 2, "among": ["Blue", "Black", "Red"]}))]
fn mana_of_listed_colors_is_a_counted_pick_among_them(#[case] produced: &str, #[case] mana: Value) {
    let text = artifact(&format!("A:AB$ Mana | Cost$ T | Produced$ {produced}"));
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    assert_eq!(
        found[0]["Activated"]["effects"][0]["AddMana"]["mana"]["AnyCombination"],
        mana
    );
}

#[rstest]
#[case::one_is_a_single_pick("Combo W B", "OneOf")]
#[case::unknown_word("Combo W Bogus | Amount$ 2", "")]
fn other_combinations_keep_their_reading(#[case] produced: &str, #[case] key: &str) {
    let text = artifact(&format!("A:AB$ Mana | Cost$ T | Produced$ {produced}"));
    if key.is_empty() {
        assert_eq!(gaps(&text), vec!["key:Mana.Produced=Combo".to_owned()]);
    } else {
        assert!(abilities(&text)[0]["Activated"]["effects"][0]["AddMana"]["mana"][key].is_array());
    }
}

#[rstest]
#[case::enchanted_land("Card.AttachedBy", json!([{"Is": "Attached"}]))]
#[case::lands("Land.YouCtrl", json!([{"Type": "Land"}, {"ControlledBy": "You"}]))]
fn the_controller_of_the_tapped_permanent_adds_the_extra_mana(
    #[case] valid: &str,
    #[case] what: Value,
) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ TapsForMana | ValidCard$ {valid} | Execute$ Mana | Static$ True | TriggerZones$ Battlefield\nSVar:Mana:DB$ Mana | Produced$ G | Amount$ 1 | Defined$ TriggeredCardController\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    let found = abilities(&text);
    let trigger = found.last().unwrap();
    assert_eq!(trigger["Triggered"]["when"]["TappedForMana"]["what"], what);
    assert_eq!(
        trigger["Triggered"]["effects"],
        json!([{"AddMana": {"mana": {"Fixed": ["Green"]}, "who": "Triggering"}}])
    );
}

#[rstest]
#[case::the_permanent_itself("TriggeredCard")]
#[case::its_owner("TriggeredCardOwner")]
fn another_word_for_the_tapped_permanent_stays_a_gap(#[case] word: &str) {
    let text = format!(
        "Name:T\nTypes:Enchantment\nT:Mode$ TapsForMana | ValidCard$ Forest | Execute$ Mana | Static$ True | TriggerZones$ Battlefield\nSVar:Mana:DB$ Mana | Produced$ G | Amount$ 1 | Defined$ {word}\n"
    );
    assert!(!gaps(&text).is_empty());
}

fn creature_line(cost: &str) -> String {
    format!(
        "Name:Subject\nManaCost:1 R\nTypes:Creature Zombie\nPT:2/2\nA:SP$ PermanentCreature | Cost$ {cost}\n"
    )
}

#[rstest]
#[case::exile_from_graveyard("1 R ExileFromGrave<1/Creature>", json!({"AdditionalCost": {"cost": [{"ExileFromGraveyard": {"filter": [{"Type": "Creature"}]}}], "required": true}}))]
#[case::discard("1 R Discard<1/Card>", json!({"AdditionalCost": {"cost": [{"Discard": {}}], "required": true}}))]
fn a_creature_spell_line_is_its_additional_cost_alone(#[case] cost: &str, #[case] expected: Value) {
    let text = creature_line(cost);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text), vec![expected]);
}

#[rstest]
#[case::no_extra_cost("1 R", 0)]
fn a_creature_spell_line_with_no_extra_cost_is_no_ability(
    #[case] cost: &str,
    #[case] count: usize,
) {
    assert_eq!(abilities(&creature_line(cost)).len(), count);
}

#[rstest]
#[case::other_mana("2 R ExileFromGrave<1/Creature>", "cost:spell-different-mana")]
fn a_creature_spell_line_paying_other_mana_stays_a_gap(#[case] cost: &str, #[case] gap: &str) {
    assert_eq!(gaps(&creature_line(cost)), vec![gap.to_owned()]);
}

fn card_of(text: &str, mana: &str) -> Arc<CardDefinition> {
    let card = CardDefinition::new("Subject")
        .with_type(CardType::Creature)
        .with_power_toughness(2, 2)
        .with_mana_cost(ManaCost::parse(mana).unwrap());
    Arc::new(abilities(text).into_iter().fold(card, |card, rule| {
        card.with_rules(serde_json::from_value::<Ability>(rule).unwrap())
    }))
}

fn filler(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

fn castable(cost: &str, graveyard: usize, hand: usize) -> bool {
    let subject = card_of(&creature_line(cost), "{1}{R}");
    let mut cards = vec![subject];
    cards.extend((0..hand).map(|_| mountain()));
    let scenario = Scenario::new(2)
        .at(1, 0, Step::Main1)
        .hand(0, cards)
        .graveyard(0, (0..graveyard).map(|n| filler(&format!("Dead {n}"))))
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .library(0, (0..12).map(|_| mountain()))
        .library(1, (0..12).map(|_| mountain()));
    let driver = Driver::from_scenario(scenario).unwrap();
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority was expected");
    };
    !options.castable.is_empty()
}

#[rstest]
#[case::exile_with_one_card("1 R ExileFromGrave<1/Creature>", 1, 0, true)]
#[case::exile_with_none("1 R ExileFromGrave<1/Creature>", 0, 0, false)]
#[case::exile_six_with_five("1 R ExileFromGrave<6/Card>", 5, 0, false)]
#[case::exile_six_with_six("1 R ExileFromGrave<6/Card>", 6, 0, true)]
#[case::discard_with_a_card("1 R Discard<1/Card>", 0, 1, true)]
#[case::discard_with_no_other_card("1 R Discard<1/Card>", 0, 0, false)]
fn a_creature_spell_is_castable_exactly_when_its_extra_cost_is_payable(
    #[case] cost: &str,
    #[case] graveyard: usize,
    #[case] hand: usize,
    #[case] expected: bool,
) {
    assert_eq!(castable(cost, graveyard, hand), expected);
}

#[rstest]
#[case::graveyard("Graveyard")]
#[case::exile("Exile")]
fn a_card_that_may_cast_itself_from_its_own_zone_has_the_rule_work_from_there(#[case] zone: &str) {
    let text = format!(
        "Name:T\nTypes:Creature Bird\nPT:1/1\nS:Mode$ Continuous | Affected$ Card.Self | MayPlay$ True | AffectedZone$ {zone} | EffectZone$ {zone} | Description$ You may cast CARDNAME from {zone}.\n"
    );
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text),
        vec![
            json!({"Rule": {"from": zone, "rule": {"MayCast": {"from": zone, "what": ["This"]}}}})
        ]
    );
}

#[rstest]
#[case::other_zone(
    "Affected$ Card.Self | MayPlay$ True | AffectedZone$ Graveyard | EffectZone$ Exile"
)]
#[case::all_zones("Affected$ Card.Self | MayPlay$ True | AffectedZone$ Hand | EffectZone$ All")]
#[case::other_cards(
    "Affected$ Creature.YouOwn | MayPlay$ True | AffectedZone$ Graveyard | EffectZone$ Graveyard"
)]
fn a_cast_permission_from_another_zone_than_the_cards_own_stays_a_gap(#[case] words: &str) {
    let text = format!("Name:T\nTypes:Creature Bird\nPT:1/1\nS:Mode$ Continuous | {words}\n");
    assert!(
        gaps(&text)
            .iter()
            .any(|gap| gap == "key:Static.MayPlay.EffectZone"),
        "{:?}",
        gaps(&text)
    );
}

#[test]
fn a_card_in_exile_that_may_be_cast_from_there_is_castable_and_one_without_the_rule_is_not() {
    let with_rule = "Name:Subject\nManaCost:1 R\nTypes:Creature Bird\nPT:1/1\nS:Mode$ Continuous | Affected$ Card.Self | MayPlay$ True | AffectedZone$ Exile | EffectZone$ Exile\n";
    let banish = "Name:Banish\nTypes:Instant\nA:SP$ ChangeZone | Origin$ Graveyard | Destination$ Exile | ValidTgts$ Card.YouOwn | TgtZone$ Graveyard\n";
    assert_eq!(gaps(banish), Vec::<String>::new());
    let mut banisher = CardDefinition::new("Banish")
        .with_type(CardType::Instant)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    for rule in abilities(banish) {
        banisher = banisher.with_rules(serde_json::from_value::<Ability>(rule).unwrap());
    }
    for (script, expected) in [
        (with_rule, true),
        (
            "Name:Subject\nManaCost:1 R\nTypes:Creature Bird\nPT:1/1\n",
            false,
        ),
    ] {
        let scenario = Scenario::new(2)
            .at(1, 0, Step::Main1)
            .hand(0, [Arc::new(banisher.clone())])
            .graveyard(0, [card_of(script, "{1}{R}")])
            .battlefield(0, [ready(mountain()), ready(mountain())])
            .library(0, (0..12).map(|_| mountain()))
            .library(1, (0..12).map(|_| mountain()));
        let mut driver = Driver::from_scenario(scenario).unwrap();
        let id = *driver
            .state()
            .zone(Zone::Graveyard(PlayerId::from_index(0)))
            .first()
            .unwrap();
        let target = Target::Object(driver.state().object(id).unwrap().reference());
        driver.cast_at("Banish", &[target]).unwrap();
        driver.pass().unwrap();
        driver.pass().unwrap();
        let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
            panic!("priority was expected");
        };
        assert_eq!(!options.castable.is_empty(), expected);
    }
}
