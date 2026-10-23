//! Goal: "creatures can't attack you unless their controller pays {N} for each creature attacking you" (Propaganda, Ghostly Prison, Windborn Muse, Baird, Sphere of Safety, Norn's Annex, Forbidding Spirit) is paid as the attack is declared, at 2 to 4 seats: the declaration asks for the payment of the total (two attackers, two taxers that stack, an amount counted when the attack is declared, a Phyrexian symbol paid with life) and is made only with it; an attack its player cannot pay for is refused whole while a smaller one is made; backing out of the payment returns to the declaration with nothing paid; an attack on another player or on a planeswalker the tax does not protect costs nothing; the attackers that tap to attack cannot pay, a vigilant one with a mana ability can; a creature that must attack is not required to when attacking costs mana; and the options name the cost of each attack.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::effects::Duration;
use mtg_engine::rules::{AttackTax, Rule};
use mtg_engine::turn::Step;
use mtg_engine::{Event, PresetPermanent};
use mtg_testkit::cards::{free_creature, plains, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

const MANA: &str =
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#;

/// Names of creatures with no abilities, each a 1/1.
fn creatures(names: &[&str]) -> Vec<PresetPermanent> {
    names
        .iter()
        .map(|name| ready(free_creature(name, 1, 1)))
        .collect()
}

/// A permanent of the corpus card called `name`.
fn corpus(name: &str) -> PresetPermanent {
    ready(card(name).unwrap())
}

/// Seat 0 is about to declare `attackers` with `count` Plains in play; seat 1 holds `held`.
fn table(
    seats: usize,
    held: Vec<PresetPermanent>,
    attackers: &[&str],
    count: usize,
) -> Result<Driver, ScriptError> {
    let mut mine = creatures(attackers);
    mine.extend(lands(count, plains));
    Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, mine)
            .battlefield(1, held),
    )
}

fn taxers(names: &[&str]) -> Vec<PresetPermanent> {
    names.iter().map(|name| corpus(name)).collect()
}

fn declared(driver: &Driver) -> bool {
    driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::AttackersDeclared { .. }))
}

fn tapped(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == name)
        .filter(|object| {
            object
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count()
}

fn pay_auto(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.answer(Answer::PayMana(PayChoice::Auto))
}

fn attacks(names: &[&str], defender: usize) -> Vec<(String, usize)> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), defender))
        .collect()
}

fn declare(
    driver: &mut Driver,
    names: &[&str],
    defender: usize,
) -> Result<Vec<Event>, ScriptError> {
    let pairs = attacks(names, defender);
    let refs: Vec<(&str, usize)> = pairs.iter().map(|(n, d)| (n.as_str(), *d)).collect();
    driver.attack(&refs)
}

#[rstest]
#[case::propaganda_one_attacker(&["Propaganda"], &["A"], 2, Some(2))]
#[case::propaganda_two_attackers(&["Propaganda"], &["A", "B"], 4, Some(4))]
#[case::ghostly_prison_two_attackers(&["Ghostly Prison"], &["A", "B"], 5, Some(4))]
#[case::windborn_muse(&["Windborn Muse"], &["A", "B", "C"], 6, Some(6))]
#[case::baird_costs_one_each(&["Baird, Steward of Argive"], &["A", "B", "C"], 3, Some(3))]
#[case::two_taxers_stack(&["Propaganda", "Ghostly Prison"], &["A", "B"], 8, Some(8))]
#[case::two_taxers_one_mana_short(&["Propaganda", "Ghostly Prison"], &["A", "B"], 7, None)]
#[case::three_taxers_stack(&["Propaganda", "Ghostly Prison", "Windborn Muse"], &["A"], 6, Some(6))]
#[case::no_mana_at_all(&["Propaganda"], &["A"], 0, None)]
#[case::one_mana_short(&["Propaganda"], &["A"], 1, None)]
#[case::an_attack_of_three_is_not_paid_by_the_mana_for_two(&["Propaganda"], &["A", "B", "C"], 4, None)]
fn the_attack_is_declared_with_the_payment_of_its_total_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &[&str],
    #[case] attackers: &[&str],
    #[case] mana: usize,
    #[case] paid: Option<usize>,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(held), attackers, mana)?;
    let attempt = declare(&mut driver, attackers, 1);
    let Some(paid) = paid else {
        assert!(attempt.is_err(), "an attack that cannot be paid is refused");
        driver.expect_decision(0, "declare-attackers")?;
        assert!(!declared(&driver));
        assert_eq!(tapped(&driver, "Plains"), 0, "nothing was paid");
        return Ok(());
    };
    attempt?;
    driver.expect_decision(0, "pay-mana")?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        unreachable!();
    };
    assert_eq!(options.cost.mana_value() as usize, paid);
    assert!(options.can_cancel);
    assert!(!declared(&driver), "the declaration waits for the payment");
    let events = pay_auto(&mut driver)?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::AttackersDeclared { .. }))
    );
    assert_eq!(tapped(&driver, "Plains"), paid);
    assert_eq!(driver.state().combat().attackers().len(), attackers.len());
    Ok(())
}

#[rstest]
fn a_smaller_attack_is_made_when_the_larger_cannot_be_paid(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Propaganda"]), &["A", "B", "C"], 4)?;
    assert!(declare(&mut driver, &["A", "B", "C"], 1).is_err());
    declare(&mut driver, &["A", "B"], 1)?;
    pay_auto(&mut driver)?;
    assert_eq!(driver.state().combat().attackers().len(), 2);
    assert_eq!(tapped(&driver, "Plains"), 4);
    Ok(())
}

#[rstest]
fn backing_out_of_the_payment_returns_to_the_declaration_with_nothing_paid(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Propaganda"]), &["A", "B"], 4)?;
    let log = driver.engine().events().len();
    declare(&mut driver, &["A", "B"], 1)?;
    driver.answer(Answer::PayMana(PayChoice::Cancel))?;
    driver.expect_decision(0, "declare-attackers")?;
    assert_eq!(
        driver.engine().events().len(),
        log,
        "a cancelled attack leaves no trace"
    );
    assert_eq!(tapped(&driver, "Plains"), 0);
    // The same attack can be declared again, and paid.
    declare(&mut driver, &["A", "B"], 1)?;
    pay_auto(&mut driver)?;
    assert!(declared(&driver));
    Ok(())
}

#[rstest]
fn an_attack_on_a_player_the_tax_does_not_protect_costs_nothing(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Propaganda"]), &["A", "B"], 4)?;
    let events = declare(&mut driver, &["A", "B"], 2)?;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::AttackersDeclared { .. }))
    );
    assert_eq!(tapped(&driver, "Plains"), 0);
    Ok(())
}

#[rstest]
fn the_attackers_pay_only_for_the_creatures_that_attack_the_protected_player(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Propaganda"]), &["A", "B", "C"], 2)?;
    let pairs = [("A", 1usize), ("B", 2), ("C", 2)];
    driver.attack(&pairs)?;
    driver.expect_decision(0, "pay-mana")?;
    pay_auto(&mut driver)?;
    assert_eq!(
        tapped(&driver, "Plains"),
        2,
        "one creature attacks the taxer"
    );
    Ok(())
}

/// Seat 1 holds `held` and the planeswalker "Chandra"; seat 0 attacks it, or the player, with `A`.
#[rstest]
#[case::propaganda_protects_the_player_only(&["Propaganda"], true, 0)]
#[case::propaganda_and_the_player(&["Propaganda"], false, 2)]
#[case::baird_protects_the_planeswalker(&["Baird, Steward of Argive"], true, 1)]
#[case::baird_and_the_player(&["Baird, Steward of Argive"], false, 1)]
#[case::ghostly_prison_does_not(&["Ghostly Prison"], true, 0)]
#[case::norns_annex_with_mana_in_play(&["Norn's Annex"], true, 1)]
fn a_planeswalker_is_protected_only_by_the_taxes_that_name_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &[&str],
    #[case] walker_attacked: bool,
    #[case] paid: usize,
) -> Result<(), ScriptError> {
    let mut permanents = taxers(held);
    permanents.push(ready(walker("Chandra", 3)));
    let mut driver = table(seats, permanents, &["A"], 3)?;
    if walker_attacked {
        driver.attack_planeswalkers(&[("A", "Chandra")])?;
    } else {
        declare(&mut driver, &["A"], 1)?;
    }
    if paid > 0 {
        driver.expect_decision(0, "pay-mana")?;
        pay_auto(&mut driver)?;
    }
    assert!(declared(&driver));
    assert_eq!(tapped(&driver, "Plains"), paid);
    Ok(())
}

#[rstest]
#[case::one_enchantment(0, 1)]
#[case::three_enchantments(2, 3)]
fn an_amount_is_counted_when_the_attack_is_declared(
    #[values(2, 3, 4)] seats: usize,
    #[case] others: usize,
    #[case] each: usize,
) -> Result<(), ScriptError> {
    let trinket = Arc::new(CardDefinition::new("Trinket").with_type(CardType::Enchantment));
    let mut held = taxers(&["Sphere of Safety"]);
    held.extend((0..others).map(|_| ready(Arc::clone(&trinket))));
    let mut driver = table(seats, held, &["A", "B"], 2 * each)?;
    declare(&mut driver, &["A", "B"], 1)?;
    driver.expect_decision(0, "pay-mana")?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        unreachable!();
    };
    assert_eq!(options.cost.mana_value() as usize, 2 * each);
    pay_auto(&mut driver)?;
    assert_eq!(tapped(&driver, "Plains"), 2 * each);
    Ok(())
}

#[rstest]
fn a_phyrexian_symbol_is_paid_with_life_when_there_is_no_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Norn's Annex"]), &["A", "B"], 0)?;
    declare(&mut driver, &["A", "B"], 1)?;
    driver.expect_decision(0, "pay-mana")?;
    pay_auto(&mut driver)?;
    assert!(declared(&driver));
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20 - 4);
    Ok(())
}

fn vigilant_mystic() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Vigilant Mystic")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::Vigilance))
            .with_rules(serde_json::from_str::<Ability>(MANA).unwrap()),
    )
}

fn plain_mystic() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Plain Mystic")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str::<Ability>(MANA).unwrap()),
    )
}

#[rstest]
#[case::a_vigilant_attacker_taps_for_the_tax(vigilant_mystic as fn() -> _, "Vigilant Mystic", 1, true)]
#[case::a_tapped_attacker_cannot(plain_mystic as fn() -> _, "Plain Mystic", 1, false)]
#[case::the_tapped_attacker_with_enough_other_mana(plain_mystic as fn() -> _, "Plain Mystic", 2, true)]
fn the_mana_of_an_attacking_creature_pays_only_if_the_attack_leaves_it_untapped(
    #[values(2, 3, 4)] seats: usize,
    #[case] mystic: fn() -> Arc<CardDefinition>,
    #[case] name: &str,
    #[case] mana: usize,
    #[case] works: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(mystic())];
    mine.extend(lands(mana, plains));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(0, mine)
            .battlefield(1, taxers(&["Propaganda"])),
    )?;
    let attempt = declare(&mut driver, &[name], 1);
    assert_eq!(attempt.is_ok(), works);
    if works {
        pay_auto(&mut driver)?;
        assert!(declared(&driver));
        assert_eq!(tapped(&driver, "Plains"), mana);
        assert_eq!(
            tapped(&driver, name),
            1,
            "it taps to attack or taps for mana"
        );
    }
    Ok(())
}

#[rstest]
fn a_rule_made_by_an_effect_taxes_until_it_ends(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, Vec::new(), &["A"], 2)?;
    let tax = AttackTax {
        players: vec![pid(1)],
        attacked: mtg_cards::modify::Attacked::Both,
        creatures: mtg_cards::refs::Selector::default(),
        mana: mtg_core::mana::ManaCost::parse("{2}").unwrap(),
    };
    driver.add_rule(Rule::AttackTax(tax), Duration::UntilEndOfTurn)?;
    declare(&mut driver, &["A"], 1)?;
    driver.expect_decision(0, "pay-mana")?;
    pay_auto(&mut driver)?;
    assert_eq!(tapped(&driver, "Plains"), 2);
    Ok(())
}

#[rstest]
fn a_creature_that_must_attack_is_not_required_to_when_attacking_costs_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, taxers(&["Propaganda"]), &["A"], 0)?;
    let a = driver.id_of("A")?;
    let named = driver.state().object(a).unwrap().reference();
    driver.add_rule(Rule::MustAttack(vec![named]), Duration::UntilEndOfTurn)?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("the declaration is asked");
    };
    if seats == 2 {
        assert!(options.required.is_empty(), "the only defender is taxed");
        assert_eq!(options.required_attack().len(), 0);
    } else {
        assert_eq!(
            options.required,
            [a],
            "a defender it can attack for nothing"
        );
        let free = options.required_attack();
        assert_eq!(free.len(), 1);
        assert_ne!(free[0].1, mtg_engine::combat::AttackTarget::Player(pid(1)));
        driver.answer(Answer::DeclareAttackers(free))?;
        assert!(declared(&driver));
    }
    Ok(())
}

#[rstest]
fn the_options_name_what_each_attack_costs(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = table(
        seats,
        taxers(&["Propaganda", "Baird, Steward of Argive"]),
        &["A", "B"],
        3,
    )?;
    let DecisionKind::DeclareAttackers(options) = &driver.decision()?.kind else {
        panic!("the declaration is asked");
    };
    assert_eq!(options.mana, 3, "the lands");
    for option in &options.attackers {
        assert_eq!(option.tax.len(), 1, "only the protected player costs mana");
        assert_eq!(
            option.tax[0].0,
            mtg_engine::combat::AttackTarget::Player(pid(1))
        );
        assert_eq!(option.tax[0].1.mana_value(), 3);
    }
    let json = serde_json::to_string(options).unwrap();
    assert!(json.contains("\"tax\""), "{json}");
    Ok(())
}
