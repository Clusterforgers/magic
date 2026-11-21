//! Goal: an activated ability whose cost is "sacrifice an artifact or discard a nonland card" (CR 602.2b) at two to four
//! seats: with both alternatives open the player chooses which to pay and only that one is paid, with one open it is
//! paid without a question, with none the ability is not offered; Bullseye, Death Dealer deals its 2 damage either way.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 controls Bullseye and four Mountains, with a Sol Ring when `artifact` and a Grizzly Bears in hand when `card`;
/// seat 1 controls a Grizzly Bears.
fn r115_table(seats: usize, artifact: bool, card_in_hand: bool) -> Scenario {
    let mut mine = vec![ready(card("Bullseye, Death Dealer").unwrap())];
    mine.extend((0..4).map(|_| ready(mountain())));
    if artifact {
        mine.push(ready(card("Sol Ring").unwrap()));
    }
    let hand = if card_in_hand {
        vec![bear()]
    } else {
        Vec::new()
    };
    Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, hand)
        .battlefield(1, [ready(bear())])
}

/// The index of Bullseye's activated ability.
const R115_ABILITY: usize = 1;

/// Activates Bullseye at seat 1's Grizzly Bears, choosing the cost labelled `pick` when asked; the labels asked.
fn r115_activate(driver: &mut Driver, pick: &str) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    let target = driver.target_named("Grizzly Bears")?;
    driver.begin_ability("Bullseye, Death Dealer", R115_ABILITY)?;
    for _ in 0..12 {
        if !driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            break;
        }
        match &driver.decision()?.kind.clone() {
            DecisionKind::ChooseOption(options) => {
                asked = options
                    .options
                    .iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label.clone(),
                        other => panic!("a cost is a label: {other:?}"),
                    })
                    .collect();
                let at = asked.iter().position(|label| label == pick).unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let cards = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(cards)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[target])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            other => panic!("unexpected {}", other.name()),
        }
    }
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(asked)
}

/// Whether a card of this name is in the zone.
fn r115_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

#[rstest]
#[case::sacrifice_chosen(true, true, "Sacrifice an artifact", "Sol Ring", true)]
#[case::discard_chosen(true, true, "Discard a nonland card", "Grizzly Bears", true)]
#[case::only_the_sacrifice(true, false, "", "Sol Ring", false)]
#[case::only_the_discard(false, true, "", "Grizzly Bears", false)]
fn r115_the_cost_chosen_is_the_one_paid(
    #[values(2, 3, 4)] seats: usize,
    #[case] artifact: bool,
    #[case] card_in_hand: bool,
    #[case] pick: &str,
    #[case] paid: &str,
    #[case] question: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(r115_table(seats, artifact, card_in_hand))?;
    let asked = r115_activate(&mut driver, pick)?;
    if question {
        assert_eq!(asked, ["Sacrifice an artifact", "Discard a nonland card"]);
    } else {
        assert!(asked.is_empty(), "one way to pay is no question");
    }
    let me = mtg_core::ids::PlayerId(0);
    assert!(r115_in(&driver, Zone::Graveyard(me), paid), "{paid} paid");
    if artifact && card_in_hand {
        let kept = if paid == "Sol Ring" {
            r115_in(&driver, Zone::Hand(me), "Grizzly Bears")
        } else {
            r115_in(&driver, Zone::Battlefield, "Sol Ring")
        };
        assert!(kept, "the other alternative is not paid");
    }
    let theirs = mtg_core::ids::PlayerId(1);
    assert!(
        r115_in(&driver, Zone::Graveyard(theirs), "Grizzly Bears"),
        "2 damage kills the 2/2"
    );
    Ok(())
}

#[rstest]
fn r115_with_neither_it_is_not_offered(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(r115_table(seats, false, false))?;
    assert!(
        !driver
            .offered_abilities("Bullseye, Death Dealer")?
            .contains(&R115_ABILITY)
    );
    Ok(())
}
