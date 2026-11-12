//! Goal: a permanent that enters only if its controller pays as it would enter ("sacrifice a Plains instead", "sacrifice two untapped lands instead", Mox Diamond's "you may discard a land card instead") takes the payment before it enters, asks a pick only when there is a choice, and otherwise goes to its owner's graveyard: a land played that way still uses the land play, and a declined or impossible optional payment sends it to the graveyard too; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// A card of `kind` named `name` with the replacement "pay `cost` or it goes to the graveyard".
fn paying(name: &str, kind: CardType, cost: &str, optional: bool) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"PayOrGraveyard":{{"cost":{cost},"optional":{optional}}}}}}}}}"#
    );
    let rule: Ability = serde_json::from_str(&json).unwrap();
    let mut def = CardDefinition::new(name).with_type(kind).with_rules(rule);
    def.mana_cost = (kind != CardType::Land).then(mtg_core::mana::ManaCost::default);
    Arc::new(def)
}

const A_PLAINS: &str = r#"[{"Sacrifice":{"filter":[{"Subtype":"Plains"}]}}]"#;
const TWO_UNTAPPED_LANDS: &str =
    r#"[{"Sacrifice":{"count":2,"filter":[{"Type":"Land"},"Untapped"]}}]"#;
const A_LAND_CARD: &str = r#"[{"Discard":{"filter":[{"Type":"Land"}]}}]"#;

/// Seat 0 holds `card` and `hand` and controls `lands`; every seat has a library.
fn table(
    seats: usize,
    card: Arc<CardDefinition>,
    hand: Vec<Arc<CardDefinition>>,
    lands: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut held = vec![card];
    held.extend(hand);
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, held)
            .battlefield(0, lands.into_iter().map(ready)),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    )
}

/// Answers what the entering asks: `yes` to the payment, and the first objects offered for the pick. The picks asked.
fn answer_entering(driver: &mut Driver, yes: bool) -> Result<usize, ScriptError> {
    let mut picks = 0;
    for _ in 0..4 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let take: Vec<ObjectId> = options.from[..options.min as usize].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(take)))?;
                picks += 1;
            }
            _ => break,
        }
    }
    Ok(picks)
}

#[rstest]
#[case::a_plains_paid(A_PLAINS, vec![plains(), island()], true, 0, &["Plains"][..])]
#[case::no_plains(A_PLAINS, vec![island()], false, 0, &[][..])]
#[case::two_of_three_untapped_lands(TWO_UNTAPPED_LANDS, vec![plains(), island(), forest()], true, 1, &["Island", "Plains"][..])]
#[case::one_untapped_land(TWO_UNTAPPED_LANDS, vec![island()], false, 0, &[][..])]
fn a_land_played_enters_only_when_paid_for(
    #[case] cost: &str,
    #[case] lands: Vec<Arc<CardDefinition>>,
    #[case] enters: bool,
    #[case] asked: usize,
    #[case] sacrificed: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let outpost = paying("Stone Outpost", CardType::Land, cost, false);
    let mut driver = Driver::from_scenario(table(seats, outpost, Vec::new(), lands))?;
    driver.play_land("Stone Outpost")?;
    assert_eq!(answer_entering(&mut driver, true)?, asked);
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert_eq!(battlefield.contains(&"Stone Outpost".to_owned()), enters);
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.sort();
    let mut expected: Vec<String> = sacrificed.iter().map(|s| (*s).to_owned()).collect();
    if !enters {
        expected.push("Stone Outpost".to_owned());
    }
    expected.sort();
    assert_eq!(graveyard, expected);
    assert_eq!(
        driver.state().turn().lands_played(),
        1,
        "the land play is used"
    );
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
#[case::paid(vec![island(), forest()], true, true, 1)]
#[case::declined(vec![island()], false, false, 0)]
#[case::no_land_card(Vec::new(), true, false, 0)]
fn an_optional_discard_lets_the_artifact_enter(
    #[case] hand: Vec<Arc<CardDefinition>>,
    #[case] yes: bool,
    #[case] enters: bool,
    #[case] asked: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mox = paying("Glass Mox", CardType::Artifact, A_LAND_CARD, true);
    let cards = hand.len();
    let mut driver = Driver::from_scenario(table(seats, mox, hand, Vec::new()))?;
    driver.cast("Glass Mox")?;
    for _ in 0..10 {
        if !driver.state().stack().is_empty()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            driver.pass()?;
        } else {
            break;
        }
    }
    // A player with no land card is asked too, and can only say no.
    if let DecisionKind::YesNo(options) = &driver.decision()?.kind {
        assert_eq!(options.only_no, cards == 0);
    }
    assert_eq!(answer_entering(&mut driver, yes && cards > 0)?, asked);
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert_eq!(battlefield.contains(&"Glass Mox".to_owned()), enters);
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.len(), 1, "{graveyard:?}");
    assert_eq!(graveyard[0] == "Glass Mox", !enters);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand.len(), if enters { cards - 1 } else { cards });
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    Ok(())
}
