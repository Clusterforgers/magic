//! Goal: an effect "until a player casts a creature spell" (`Duration::UntilCreatureSpellCast`) lasts through other
//! spells and turns and ends as soon as any player casts a creature spell after it began: Soul Sculptor's target is a
//! plain enchantment with no abilities until then, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Priority passes until the stack is empty, payments made automatically and other questions answered passively.
fn r5512_settle(driver: &mut Driver) {
    for _ in 0..80 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// Whether the permanent named is a creature and whether it is an enchantment.
fn r5512_types(driver: &Driver, name: &str) -> (bool, bool) {
    let id = driver.id_of(name).unwrap();
    let chars = characteristics(driver.state(), id).unwrap();
    (
        chars.types.contains(CardType::Creature),
        chars.types.contains(CardType::Enchantment),
    )
}

#[rstest]
#[case::creature_spell_ends_it("Grizzly Bears", true)]
#[case::noncreature_spell_keeps_it("Lightning Bolt", false)]
fn r5512_soul_sculptor_lasts_until_a_creature_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] ends: bool,
) {
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(card("Soul Sculptor").unwrap()),
                ready(plains()),
                ready(plains()),
                ready(forest()),
                ready(forest()),
                ready(mountain()),
            ],
        )
        .battlefield(1, [ready(card("Llanowar Elves").unwrap())])
        .hand(0, [card(spell).unwrap()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_activation("Soul Sculptor").unwrap();
    let elves = driver.target_named("Llanowar Elves").unwrap();
    driver.choose_targets(&[elves]).unwrap();
    r5512_settle(&mut driver);
    assert_eq!(r5512_types(&driver, "Llanowar Elves"), (false, true));
    assert!(
        driver
            .offered_abilities("Llanowar Elves")
            .unwrap_or_default()
            .is_empty(),
        "it has no abilities"
    );
    if spell == "Lightning Bolt" {
        let face = driver.target_player(1);
        driver.cast_at(spell, &[face]).unwrap();
    } else {
        driver.cast(spell).unwrap();
    }
    r5512_settle(&mut driver);
    let expected = if ends { (true, false) } else { (false, true) };
    assert_eq!(r5512_types(&driver, "Llanowar Elves"), expected);
    // The effect does not come back, and one that did not end still holds in a later turn.
    driver.idle_until(4, Step::Upkeep).unwrap();
    assert_eq!(r5512_types(&driver, "Llanowar Elves"), expected);
}
