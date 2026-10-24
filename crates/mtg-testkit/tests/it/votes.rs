//! Goal: votes play whole through Council's Judgment, Magister of Worth, Custodi Squire and a secret ballot at two, three and four seats: the controller votes first and the others follow in turn order, each vote is one answer by one seat and is told to every seat, the choices with the most votes happen and a tie keeps every tied choice, the cards and the permanents to vote for are exactly those the wording names, the secret ballot counts every vote, and the game replays from its log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CardsChoice, Choice, OptionChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::{forest, free_creature, island, plains, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, secret_ballot};
use rstest::rstest;

use crate::common::pid;

/// Passes priority until the stack is empty again; every vote is answered by `vote`. Returns who was asked, in order.
fn play_out(
    driver: &mut Driver,
    mut vote: impl FnMut(&Decision, &Driver) -> Answer,
) -> Result<Vec<PlayerId>, ScriptError> {
    let mut asked = Vec::new();
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseCards(_) | DecisionKind::ChooseOption(_) => {
                asked.push(decision.player);
                let answer = vote(&decision, driver);
                driver.answer(answer)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

/// The name of the object.
fn name(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

/// The names of the cards in the zone, sorted.
fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    let mut found: Vec<String> = driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| name(driver, *id))
        .collect();
    found.sort();
    found
}

/// The seats that answered a vote, in the order the log tells it, whatever was chosen.
fn voted(driver: &Driver) -> Vec<PlayerId> {
    driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade { player, .. } => Some(*player),
            _ => None,
        })
        .collect()
}

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn seats_in_order(seats: usize) -> Vec<PlayerId> {
    (0..seats).map(pid).collect()
}

/// The answer that picks the card named `wanted` among the cards offered.
fn pick(decision: &Decision, driver: &Driver, wanted: &str) -> Answer {
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        panic!(
            "a vote on cards asks for cards, not {}",
            decision.kind.name()
        );
    };
    let chosen = options
        .from
        .iter()
        .copied()
        .find(|id| name(driver, *id) == wanted)
        .unwrap_or_else(|| panic!("{wanted} is not on offer"));
    Answer::ChooseCards(CardsChoice(vec![chosen]))
}

/// Council's Judgment ({1}{W}{W}, Will of the council): each player votes for a nonland permanent the caster does not
/// control; each permanent with the most votes, or tied for the most, is exiled.
#[rstest]
#[case::two_seats_tied(2, &["Bear 1a", "Bear 1b"], &["Bear 1a", "Bear 1b"])]
#[case::two_seats_agreeing(2, &["Bear 1a", "Bear 1a"], &["Bear 1a"])]
#[case::three_seats_majority(3, &["Bear 1a", "Bear 2a", "Bear 1a"], &["Bear 1a"])]
#[case::three_seats_all_different(3, &["Bear 1a", "Bear 2a", "Bear 2b"], &["Bear 1a", "Bear 2a", "Bear 2b"])]
#[case::four_seats_two_tied(4, &["Bear 1a", "Bear 1a", "Bear 2a", "Bear 2a"], &["Bear 1a", "Bear 2a"])]
#[case::four_seats_one_ahead(4, &["Bear 3b", "Bear 3b", "Bear 2a", "Bear 1a"], &["Bear 3b"])]
fn councils_judgment_exiles_what_has_the_most_votes_or_ties_for_it(
    #[case] seats: usize,
    #[case] votes: &[&str],
    #[case] exiled: &[&str],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Council's Judgment"])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(plains()),
                ready(plains()),
                ready(free_creature("Mine", 1, 1)),
            ],
        );
    let mut foes: Vec<String> = Vec::new();
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            [
                ready(free_creature(&format!("Bear {seat}a"), 2, 2)),
                ready(free_creature(&format!("Bear {seat}b"), 2, 2)),
                ready(forest()),
            ],
        );
        foes.push(format!("Bear {seat}a"));
        foes.push(format!("Bear {seat}b"));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Council's Judgment")?;
    let asked = play_out(&mut driver, |decision, driver| {
        let DecisionKind::ChooseCards(options) = &decision.kind else {
            panic!("a vote on permanents asks for a card");
        };
        let mut offered: Vec<String> = options.from.iter().map(|id| name(driver, *id)).collect();
        offered.sort();
        let mut want = foes.clone();
        want.sort();
        assert_eq!(
            offered, want,
            "only the nonland permanents the caster does not control"
        );
        pick(decision, driver, votes[decision.player.index()])
    })?;
    assert_eq!(
        asked,
        seats_in_order(seats),
        "the caster first, then turn order"
    );
    assert_eq!(
        voted(&driver),
        seats_in_order(seats),
        "every vote is told, in order"
    );
    let mut want: Vec<String> = exiled.iter().map(|n| (*n).to_owned()).collect();
    want.sort();
    assert_eq!(names_in(&driver, Zone::Exile), want);
    let mut left: Vec<String> = foes
        .iter()
        .filter(|f| !exiled.contains(&f.as_str()))
        .cloned()
        .collect();
    left.sort();
    for foe in &left {
        assert!(
            driver
                .state()
                .zone(Zone::Battlefield)
                .iter()
                .any(|id| &name(&driver, *id) == foe),
            "{foe} stays"
        );
    }
    replays(&driver);
    Ok(())
}

/// Magister of Worth: the vote is on grace or condemnation; grace with more votes returns every creature card from
/// every graveyard, condemnation with more votes or a tie destroys every other creature.
#[rstest]
#[case::two_seats_grace(2, &[0, 0], true)]
#[case::two_seats_tied(2, &[0, 1], false)]
#[case::three_seats_grace_ahead(3, &[1, 0, 0], true)]
#[case::three_seats_condemnation_ahead(3, &[0, 1, 1], false)]
#[case::four_seats_tied(4, &[0, 0, 1, 1], false)]
#[case::four_seats_grace_ahead(4, &[0, 0, 0, 1], true)]
fn magister_of_worth_returns_creatures_on_grace_and_destroys_them_otherwise(
    #[case] seats: usize,
    #[case] votes: &[usize],
    #[case] grace: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).hand_named(0, ["Magister of Worth"]);
    for seat in 0..seats {
        scenario = scenario
            .graveyard(
                seat,
                [free_creature(&format!("Dead {seat}"), 1, 1), island()],
            )
            .battlefield(
                seat,
                if seat == 0 {
                    vec![
                        ready(plains()),
                        ready(plains()),
                        ready(plains()),
                        ready(plains()),
                        ready(plains()),
                        ready(swamp()),
                        ready(free_creature("Bear 0", 2, 2)),
                    ]
                } else {
                    vec![ready(free_creature(&format!("Bear {seat}"), 2, 2))]
                },
            );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Magister of Worth")?;
    let asked = play_out(&mut driver, |decision, _| {
        let DecisionKind::ChooseOption(options) = &decision.kind else {
            panic!("a vote on names asks for an option");
        };
        assert_eq!(options.options.len(), 2, "grace or condemnation");
        Answer::ChooseOption(OptionChoice(votes[decision.player.index()]))
    })?;
    assert_eq!(
        asked,
        seats_in_order(seats),
        "the caster first, then turn order"
    );
    let told: Vec<(PlayerId, Choice)> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::ChoiceMade { player, choice } => Some((*player, choice.clone())),
            _ => None,
        })
        .collect();
    let want: Vec<(PlayerId, Choice)> = (0..seats)
        .map(|seat| (pid(seat), Choice::Option(votes[seat])))
        .collect();
    assert_eq!(told, want, "an open vote is told as it is cast");
    let field = names_in(&driver, Zone::Battlefield);
    if grace {
        for seat in 0..seats {
            assert!(
                field.contains(&format!("Dead {seat}")),
                "Dead {seat} returns"
            );
            assert!(field.contains(&format!("Bear {seat}")), "Bear {seat} lives");
            assert!(
                names_in(&driver, Zone::Graveyard(pid(seat)))
                    .iter()
                    .all(|n| n != &format!("Dead {seat}"))
            );
        }
    } else {
        assert!(
            field.contains(&"Magister of Worth".to_owned()),
            "the Magister stays"
        );
        for seat in 0..seats {
            assert!(
                !field.contains(&format!("Bear {seat}")),
                "Bear {seat} is destroyed"
            );
            assert!(
                names_in(&driver, Zone::Graveyard(pid(seat))).contains(&format!("Dead {seat}"))
            );
        }
    }
    replays(&driver);
    Ok(())
}

/// Custodi Squire: the vote is on the artifact, creature and enchantment cards in the caster's graveyard; each card
/// with the most votes or tied returns to the hand.
#[rstest]
#[case::two_seats_agreeing(2, &["Grave A", "Grave A"], &["Grave A"])]
#[case::two_seats_tied(2, &["Grave A", "Grave B"], &["Grave A", "Grave B"])]
#[case::three_seats_majority(3, &["Grave A", "Grave B", "Grave B"], &["Grave B"])]
#[case::four_seats_tied(4, &["Grave A", "Grave A", "Grave B", "Grave B"], &["Grave A", "Grave B"])]
fn custodi_squire_returns_the_cards_with_the_most_votes(
    #[case] seats: usize,
    #[case] votes: &[&str],
    #[case] returned: &[&str],
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Custodi Squire"])
        .graveyard(
            0,
            [
                free_creature("Grave A", 1, 1),
                free_creature("Grave B", 2, 2),
                island(),
            ],
        )
        .battlefield(0, (0..5).map(|_| ready(plains())).collect::<Vec<_>>());
    if seats > 1 {
        scenario = scenario.graveyard(1, [free_creature("Foe Grave", 3, 3)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Custodi Squire")?;
    let asked = play_out(&mut driver, |decision, driver| {
        let DecisionKind::ChooseCards(options) = &decision.kind else {
            panic!("a vote on cards asks for a card");
        };
        let mut offered: Vec<String> = options.from.iter().map(|id| name(driver, *id)).collect();
        offered.sort();
        assert_eq!(
            offered,
            ["Grave A", "Grave B"],
            "creature cards of the caster's graveyard only"
        );
        pick(decision, driver, votes[decision.player.index()])
    })?;
    assert_eq!(asked, seats_in_order(seats));
    let hand = names_in(&driver, Zone::Hand(pid(0)));
    let mut want: Vec<String> = returned.iter().map(|n| (*n).to_owned()).collect();
    want.sort();
    assert_eq!(hand, want);
    replays(&driver);
    Ok(())
}

/// A secret ballot counts every vote: the caster draws a card for each vote for heads, and no vote stops anyone from
/// voting again in another game.
#[rstest]
fn a_secret_ballot_draws_a_card_for_each_heads_vote(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] heads: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .hand(0, [secret_ballot("Ballot")])
        .battlefield(0, [ready(island())])
        .library(
            0,
            (0..10).map(|n| free_creature(&format!("Card {n}"), 1, 1)),
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Ballot")?;
    let asked = play_out(&mut driver, |decision, _| {
        let DecisionKind::ChooseOption(options) = &decision.kind else {
            panic!("a vote on names asks for an option");
        };
        assert!(
            options
                .options
                .iter()
                .all(|option| matches!(option, mtg_engine::choose::ChoiceOption::Secret(_))),
            "a secret vote offers secret options"
        );
        // The first `heads` seats vote for heads (option 0), the others for tails.
        Answer::ChooseOption(OptionChoice(usize::from(decision.player.index() >= heads)))
    })?;
    assert_eq!(asked, seats_in_order(seats));
    assert_eq!(names_in(&driver, Zone::Hand(pid(0))).len(), heads);
    let secret = driver
        .engine()
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                Event::ChoiceMade {
                    choice: Choice::Vote(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(secret, seats, "each vote is logged as cast face down");
    replays(&driver);
    Ok(())
}
