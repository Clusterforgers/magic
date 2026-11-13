//! Goal: an Aura moved to "another permanent it can enchant" (`Pred::EnchantableBy` with `Choose` and `Attach`) goes only
//! to a permanent its enchant ability allows, other than its host and of the host's type for Enchantment Alteration,
//! and stays put when there is none; Aura Graft also takes control of it, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::characteristics::{characteristics, controller_of};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, attached_to, ready};
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

/// Seat 1 controls Grizzly Bears enchanted by Pacifism and a Forest enchanted by Wild Growth; seat 0 controls the
/// permanents given and islands, and holds the spell.
fn r5512_table(seats: usize, spell: &str, mine: Vec<&str>) -> Driver {
    let mut own = vec![ready(island()), ready(island()), ready(island())];
    own.extend(mine.into_iter().map(|name| ready(card(name).unwrap())));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, own)
        .battlefield(
            1,
            [
                ready(card("Grizzly Bears").unwrap()),
                ready(forest()),
                attached_to(ready(card("Pacifism").unwrap()), 1, 0),
                attached_to(ready(card("Wild Growth").unwrap()), 1, 1),
            ],
        )
        .hand(0, [card(spell).unwrap()]);
    Driver::from_scenario(scenario).unwrap()
}

/// The host of the Aura named, with its card types, and the seat that controls the Aura.
fn r5512_host(driver: &Driver, aura: &str) -> (Option<String>, bool, bool, usize) {
    let state = driver.state();
    let id = driver.id_of(aura).unwrap();
    let host = state.host_of(id);
    let chars = host.and_then(|host| characteristics(state, host));
    let controller = controller_of(state, id).unwrap();
    let seat = state
        .players()
        .iter()
        .position(|player| player.id() == controller)
        .unwrap();
    (
        host.and_then(|host| state.object(host))
            .map(|object| object.card().name.clone()),
        chars
            .as_ref()
            .is_some_and(|c| c.types.contains(CardType::Creature)),
        chars
            .as_ref()
            .is_some_and(|c| c.types.contains(CardType::Land)),
        seat,
    )
}

#[rstest]
#[case::graft_pacifism("Aura Graft", "Pacifism", vec!["Llanowar Elves"], Some("Llanowar Elves"), 0)]
#[case::graft_wild_growth("Aura Graft", "Wild Growth", vec!["Llanowar Elves"], Some("Island"), 0)]
#[case::graft_no_other_creature("Aura Graft", "Pacifism", vec![], Some("Grizzly Bears"), 0)]
#[case::alteration_pacifism("Enchantment Alteration", "Pacifism", vec!["Llanowar Elves"], Some("Llanowar Elves"), 1)]
#[case::alteration_wild_growth("Enchantment Alteration", "Wild Growth", vec!["Llanowar Elves"], Some("Island"), 1)]
#[case::alteration_no_other_creature("Enchantment Alteration", "Pacifism", vec![], Some("Grizzly Bears"), 1)]
fn r5512_auras_move_to_a_permanent_they_can_enchant(
    #[values(2, 3, 4)] seats: usize,
    #[case] spell: &str,
    #[case] aura: &str,
    #[case] mine: Vec<&str>,
    #[case] host: Option<&str>,
    #[case] controller: usize,
) {
    let mut driver = r5512_table(seats, spell, mine);
    let target = driver.target_named(aura).unwrap();
    driver.cast_at(spell, &[target]).unwrap();
    r5512_settle(&mut driver);
    let (now, creature, land, seat) = r5512_host(&driver, aura);
    assert_eq!(now.as_deref(), host);
    // Pacifism enchants a creature and Wild Growth a land, whatever the choice.
    assert_eq!(
        (creature, land),
        (aura == "Pacifism", aura == "Wild Growth")
    );
    assert_eq!(seat, controller);
}
