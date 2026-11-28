//! Goal: the lc4 cards play whole at two to four seats: Agent Maria Hill gets a +1/+1 counter and draws a card only when
//! she is tapped to pay a teamwork cost, never when the cost taps other creatures or is declined; Captain America, First
//! Avenger unattaches an Equipment to deal damage equal to its mana value divided among one to three targets, and
//! attaches an Equipment at the beginning of combat; Knowledge Vault exiles the top card face down, gives its cards back
//! once sacrificed and buries them when it leaves another way; Swirl the Mists changes every color word of spells and
//! permanents to the chosen one while it stays; each game replays and each card is Verified.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, CostsChoice, DivideChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

pub(crate) fn lc4_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

pub(crate) fn lc4_libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, vec![island(); 12])
    })
}

pub(crate) fn lc4_counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

pub(crate) fn lc4_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if driver.state().stack().is_empty() && driver.state().pending_triggers().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    Ok(())
}

/// An instant for {1} with teamwork 2 that gains 2 life.
fn lc4_huddle() -> Arc<CardDefinition> {
    let effects = r#"{"Spell":{"effects":[{"GainLife":{"amount":2}}]}}"#;
    Arc::new(
        CardDefinition::new("Huddle")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(Ability::Keyword(Keyword::Macro {
                name: "teamwork".to_owned(),
                amount: 2,
            }))
            .with_rules(serde_json::from_str(effects).unwrap()),
    )
}

#[rstest]
#[case::maria_and_a_mate(true, &["Agent Maria Hill", "Mate"], 1)]
#[case::a_mate_alone(true, &["Mate"], 0)]
#[case::declined(false, &[], 0)]
fn lc4_maria_answers_a_tap_for_teamwork(
    #[values(2, 3, 4)] seats: usize,
    #[case] pays: bool,
    #[case] tapped: &[&str],
    #[case] grown: u32,
) -> Result<(), ScriptError> {
    let scenario = lc4_libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Agent Maria Hill").unwrap()),
                    ready(free_creature("Mate", 2, 2)),
                    ready(forest()),
                ],
            )
            .hand(0, [lc4_huddle()]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let maria = driver.id_of("Agent Maria Hill")?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Huddle")?;
    if let DecisionKind::ChooseCosts(_) = &driver.decision()?.kind {
        let chosen = if pays { vec![0] } else { vec![] };
        driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
    }
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let picked: Vec<_> = options
            .from
            .iter()
            .copied()
            .filter(|id| tapped.contains(&driver.state().object(*id).unwrap().card().name.as_str()))
            .collect();
        driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    lc4_settle(&mut driver)?;
    assert_eq!(lc4_counters(&driver, maria), grown);
    let now = driver.state().zone(Zone::Hand(pid(0))).len();
    // The Huddle left the hand; Maria's draw replaces it.
    assert_eq!(now + 1, hand + grown as usize);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 22);
    lc4_replays(&driver);
    Ok(())
}

#[rstest]
#[case("Agent Maria Hill")]
#[case("Captain America, First Avenger")]
#[case("Knowledge Vault")]
#[case("Swirl the Mists")]
fn lc4_the_cards_are_verified(#[case] name: &str) {
    let support = mtg_testkit::corpus::corpus()
        .unwrap()
        .support(name)
        .unwrap();
    assert_eq!(
        support.support,
        mtg_cards::support::Support::Verified,
        "{name}: {support:?}"
    );
}

/// Seat 0 with Knowledge Vault and two Forests for each activation, a hand of two Islands and a library of named cards; Vault activations
/// `exiled` times, then the {0} ability when `sacrificed`, else the Vault destroyed by hand.
fn lc4_vault(seats: usize, exiled: usize, sacrificed: bool) -> Result<Driver, ScriptError> {
    let library: Vec<_> = (0..6)
        .map(|at| free_creature(&format!("Page {at}"), 1, 1))
        .collect();
    let mut field = vec![ready(card("Knowledge Vault").unwrap())];
    field.extend((0..exiled * 2).map(|_| ready(forest())));
    let scenario = lc4_libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, field)
            .hand(0, [island(), island()]),
        seats,
    )
    .library(0, library);
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..exiled {
        driver.begin_ability("Knowledge Vault", 0)?;
        if let Ok(DecisionKind::PayMana(_)) = driver.decision().map(|decision| &decision.kind) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
        lc4_settle(&mut driver)?;
        // Untapped again for the next activation, as a new turn would.
        let vault = driver.id_of("Knowledge Vault")?;
        driver.manual(mtg_engine::manual::ManualAction::Untap { object: vault })?;
    }
    let vault = driver.id_of("Knowledge Vault")?;
    if sacrificed {
        driver.begin_ability("Knowledge Vault", 1)?;
        if let Ok(DecisionKind::PayMana(_)) = driver.decision().map(|decision| &decision.kind) {
            driver.answer(Answer::PayMana(PayChoice::Auto))?;
        }
    } else {
        driver.manual(mtg_engine::manual::ManualAction::Move {
            object: vault,
            to: mtg_engine::manual::ManualZone::Graveyard,
        })?;
    }
    lc4_settle(&mut driver)?;
    Ok(driver)
}

fn lc4_pages(driver: &Driver, zone: Zone) -> usize {
    crate::common::names(driver.state(), zone)
        .iter()
        .filter(|name| name.starts_with("Page "))
        .count()
}

#[rstest]
#[case::two_pages_sacrificed(2, true, (2, 0, 2))]
#[case::none_sacrificed(0, true, (0, 0, 2))]
#[case::two_pages_buried(2, false, (0, 2, 0))]
fn lc4_the_vault_keeps_its_pages(
    #[values(2, 3, 4)] seats: usize,
    #[case] exiled: usize,
    #[case] sacrificed: bool,
    #[case] after: (usize, usize, usize),
) -> Result<(), ScriptError> {
    let driver = lc4_vault(seats, exiled, sacrificed)?;
    assert_eq!(lc4_pages(&driver, Zone::Exile), 0);
    // Pages in hand, pages in the graveyard, Islands discarded.
    let islands = crate::common::names(driver.state(), Zone::Graveyard(pid(0)))
        .iter()
        .filter(|name| name.as_str() == "Island")
        .count();
    assert_eq!(
        (
            lc4_pages(&driver, Zone::Hand(pid(0))),
            lc4_pages(&driver, Zone::Graveyard(pid(0))),
            islands
        ),
        after
    );
    lc4_replays(&driver);
    Ok(())
}

#[rstest]
fn lc4_the_vault_exiles_face_down(#[values(2, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        lc4_libraries(
            Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
                0,
                [
                    ready(card("Knowledge Vault").unwrap()),
                    ready(forest()),
                    ready(forest()),
                ],
            ),
            seats,
        )
        .library(0, [free_creature("Page", 1, 1)]),
    )?;
    driver.begin_ability("Knowledge Vault", 0)?;
    if let Ok(DecisionKind::PayMana(_)) = driver.decision().map(|decision| &decision.kind) {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    lc4_settle(&mut driver)?;
    let exiled = driver.state().zone(Zone::Exile);
    assert_eq!(exiled.len(), 1);
    for seat in 0..seats {
        assert!(
            !driver
                .state()
                .may_look_at(mtg_core::ids::PlayerId::from_index(seat), exiled[0]),
            "no player looks at the page"
        );
    }
    lc4_replays(&driver);
    Ok(())
}

/// Seat 0 with Swirl the Mists in hand, four Islands and a Banner (black creatures get +1/+1); seat 1 with a green Bear
/// and a white Knight with protection from black.
fn lc4_swirl_table(seats: usize) -> Scenario {
    let banner = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Color":"Black"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
    let banner = Arc::new(
        CardDefinition::new("Banner")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str(banner).unwrap()),
    );
    let knight = Arc::new(
        CardDefinition::new("Knight")
            .with_type(CardType::Creature)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[Color::White]))
            .with_power_toughness(2, 2)
            .with_rules(
                serde_json::from_str(r#"{"Keyword":{"Protection":[{"Color":"Black"}]}}"#).unwrap(),
            ),
    );
    lc4_libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(banner),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ],
            )
            .battlefield(
                1,
                [
                    ready(mtg_testkit::cards::creature(
                        "Bear",
                        1,
                        &[Color::Green],
                        2,
                        2,
                    )),
                    ready(knight),
                ],
            )
            .hand(0, [card("Swirl the Mists").unwrap()]),
        seats,
    )
}

fn lc4_power(driver: &Driver, name: &str) -> i32 {
    let id = driver.id_of(name).unwrap();
    mtg_engine::characteristics::characteristics(driver.state(), id)
        .unwrap()
        .power
        .unwrap()
}

fn lc4_keywords(driver: &Driver, name: &str) -> Vec<Keyword> {
    let id = driver.id_of(name).unwrap();
    mtg_engine::keywords::keywords_of(driver.state(), id)
        .into_iter()
        .cloned()
        .collect()
}

#[rstest]
#[case::green(Color::Green, 3, Color::Green)]
#[case::red(Color::Red, 2, Color::Red)]
#[case::black(Color::Black, 2, Color::Black)]
fn lc4_swirl_changes_every_color_word(
    #[values(2, 3, 4)] seats: usize,
    #[case] chosen: Color,
    #[case] bear: i32,
    #[case] protection: Color,
) -> Result<(), ScriptError> {
    use mtg_engine::choose::{ChoiceOption, OptionChoice};
    let mut driver = Driver::from_scenario(lc4_swirl_table(seats))?;
    assert_eq!(lc4_power(&driver, "Bear"), 2);
    driver.cast("Swirl the Mists")?;
    for _ in 0..20 {
        match &driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Color(chosen))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    assert!(
        crate::common::names(driver.state(), Zone::Battlefield)
            .contains(&"Swirl the Mists".to_owned())
    );
    // The Banner pumps creatures of the chosen color, and the Knight is protected from it.
    assert_eq!(lc4_power(&driver, "Bear"), bear);
    assert!(
        lc4_keywords(&driver, "Knight").contains(&Keyword::Protection(mtg_cards::refs::Selector(
            vec![mtg_cards::refs::Pred::Color(protection)]
        ))),
        "{:?}",
        lc4_keywords(&driver, "Knight")
    );
    // Once Swirl leaves, the words are as printed.
    let swirl = driver.id_of("Swirl the Mists")?;
    driver.manual(mtg_engine::manual::ManualAction::Move {
        object: swirl,
        to: mtg_engine::manual::ManualZone::Graveyard,
    })?;
    assert_eq!(lc4_power(&driver, "Bear"), 2);
    lc4_replays(&driver);
    Ok(())
}

/// Seat 0 with Captain America, the Equipment `gear` attached to him and three Islands; seat 1 with two 2/2 Bears.
fn lc4_captain_table(seats: usize, gear: &str) -> Scenario {
    lc4_libraries(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(
                0,
                [
                    ready(card("Captain America, First Avenger").unwrap()),
                    mtg_testkit::attached_to(ready(card(gear).unwrap()), 0, 0),
                    ready(island()),
                    ready(island()),
                    ready(island()),
                ],
            )
            .battlefield(
                1,
                [
                    ready(free_creature("Bear", 2, 2)),
                    ready(free_creature("Cub", 2, 2)),
                ],
            ),
        seats,
    )
}

/// Throws the Equipment at the first `among` of the Bear, the Cub and seat 1, the damage split as evenly as it goes
/// with the remainder to the first.
fn lc4_throw(driver: &mut Driver, among: usize) -> Result<(), ScriptError> {
    let ability = driver.offered_abilities("Captain America, First Avenger")?;
    driver.begin_ability("Captain America, First Avenger", ability[0])?;
    for _ in 0..12 {
        let Ok(decision) = driver.decision() else {
            break;
        };
        match &decision.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let first = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let all = [
                    driver.target_named("Bear")?,
                    driver.target_named("Cub")?,
                    driver.target_player(1),
                ];
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(
                    all[..among].to_vec(),
                )))?;
            }
            DecisionKind::DivideAmount(options) => {
                let count = u32::try_from(options.among.len()).unwrap();
                let split = options
                    .among
                    .iter()
                    .enumerate()
                    .map(|(at, target)| {
                        let extra = if at == 0 { options.total % count } else { 0 };
                        (*target, options.total / count + extra)
                    })
                    .collect();
                driver.answer(Answer::DivideAmount(DivideChoice::Division(split)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) => break,
            other => panic!("unexpected question {}", other.name()),
        }
    }
    lc4_settle(driver)
}

/// The damage marked on the named permanent, `None` once it has left the battlefield.
fn lc4_damage(driver: &Driver, name: &str) -> Option<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| state.object(**id).unwrap().card().name == name)
        .map(|id| state.object(*id).unwrap().permanent().unwrap().damage())
}

#[rstest]
#[case::at_one_target(1, (None, Some(0), 20))]
#[case::at_two_targets(2, (None, Some(1), 20))]
#[case::at_three_targets(3, (Some(1), Some(1), 19))]
fn lc4_the_captain_throws_his_equipment(
    #[values(2, 3, 4)] seats: usize,
    #[case] among: usize,
    #[case] after: (Option<u32>, Option<u32>, i32),
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(lc4_captain_table(seats, "Loxodon Warhammer"))?;
    let hammer = driver.id_of("Loxodon Warhammer")?;
    lc4_throw(&mut driver, among)?;
    let held = driver.state().object(hammer).unwrap().permanent().unwrap();
    assert!(held.attached_to().is_none(), "the Warhammer let go");
    // 3 damage (the Warhammer's mana value): all to the Bear, 2 and 1, or 1 each.
    assert_eq!(
        (
            lc4_damage(&driver, "Bear"),
            lc4_damage(&driver, "Cub"),
            driver.state().player(pid(1)).unwrap().life()
        ),
        after
    );
    lc4_replays(&driver);
    Ok(())
}

#[rstest]
fn lc4_the_captain_throws_only_attached_equipment(#[values(2, 4)] seats: usize) {
    let scenario = lc4_libraries(
        Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(card("Captain America, First Avenger").unwrap()),
                ready(card("Loxodon Warhammer").unwrap()),
                ready(island()),
                ready(island()),
                ready(island()),
            ],
        ),
        seats,
    );
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(
        driver
            .offered_abilities("Captain America, First Avenger")
            .unwrap_or_default(),
        Vec::<usize>::new()
    );
}

#[rstest]
fn lc4_the_captain_catches_an_equipment_at_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = lc4_libraries(
        Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
            0,
            [
                ready(card("Captain America, First Avenger").unwrap()),
                ready(card("Bonesplitter").unwrap()),
            ],
        ),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let captain = driver.id_of("Captain America, First Avenger")?;
    let splitter = driver.id_of("Bonesplitter")?;
    let mut caught = false;
    for _ in 0..30 {
        match &driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                let target = driver.target_named("Bonesplitter")?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))?;
                caught = true;
            }
            _ if caught
                && driver.state().stack().is_empty()
                && driver.state().pending_triggers().is_empty() =>
            {
                break;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let attached = driver
        .state()
        .object(splitter)
        .unwrap()
        .permanent()
        .unwrap()
        .attached_to()
        .map(|host| host.id);
    assert_eq!(attached, Some(captain));
    lc4_replays(&driver);
    Ok(())
}
