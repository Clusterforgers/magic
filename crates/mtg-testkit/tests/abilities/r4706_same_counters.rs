//! Goal: "Whenever you put one or more counters on another creature, if it's not a Kree, you may put the same number and
//! kind of counters on Captain Marvel." plays at two to four seats: counters its controller puts on another creature are
//! copied in number and kind, every kind of one putting together; counters an opponent puts, counters on a Kree and
//! counters on Captain Marvel herself copy nothing, nor does declining the "may".
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const MARVEL: &str = "Captain Marvel, Apex Avenger";

/// A creature with a subtype of its own.
fn r4706_creature(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(2, 2),
    )
}

/// An artifact whose "{0}: Put its counters on target creature." puts every kind it holds at once.
fn r4706_donor() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Donor")
            .with_type(CardType::Artifact)
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Activated":{"cost":"{0}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCountersOf":{"on":{"Target":0},"of":"This"}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

/// Answers until the stack is empty; a yes-or-no question gets `yes`.
fn r4706_settle(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..100 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn r4706_counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(kind)
}

/// The Donor of seat `donor` (two +1/+1 and one shield counter) puts its counters on `target`; Captain Marvel is seat
/// 0's, "Ours" (a Human) and "Kree" (a Kree) are seat 0's, "Theirs" seat 1's.
#[rstest]
#[case::on_your_creature(0, "Ours", true, (2, 1))]
#[case::declined(0, "Ours", false, (0, 0))]
#[case::on_an_opponents_creature(0, "Theirs", true, (2, 1))]
#[case::on_a_kree(0, "Kree", true, (0, 0))]
#[case::on_herself(0, MARVEL, true, (0, 0))]
#[case::an_opponent_puts_them(1, "Theirs", true, (0, 0))]
#[case::an_opponent_puts_them_on_yours(1, "Ours", true, (0, 0))]
fn r4706_marvel_copies_the_counters_you_put(
    #[case] donor: usize,
    #[case] target: &str,
    #[case] yes: bool,
    #[case] (plus, shield): (u32, u32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let gift = with_counters(
        with_counters(ready(r4706_donor()), CounterKind::PlusOnePlusOne, 2),
        CounterKind::Other("shield".into()),
        1,
    );
    let mut mine = vec![
        ready(card(MARVEL).unwrap()),
        ready(r4706_creature("Ours", "Human")),
        ready(r4706_creature("Kree", "Kree")),
    ];
    let mut theirs = vec![ready(r4706_creature("Theirs", "Human"))];
    if donor == 0 {
        mine.push(gift);
    } else {
        theirs.push(gift);
    }
    let mut scenario = Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, theirs);
    if donor == 1 {
        scenario = scenario.at(2, 1, Step::Main1);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Donor")?;
    let chosen = driver.target_named(target)?;
    driver.choose_targets(&[chosen])?;
    driver.pay_tapping(&[], 0)?;
    r4706_settle(&mut driver, yes)?;
    let on_marvel = (
        r4706_counters(&driver, MARVEL, &CounterKind::PlusOnePlusOne),
        r4706_counters(&driver, MARVEL, &CounterKind::Other("shield".into())),
    );
    // Counters put on Captain Marvel herself are hers already; "another creature" copies none of them.
    let copied = if target == MARVEL {
        (on_marvel.0 - 2, on_marvel.1 - 1)
    } else {
        on_marvel
    };
    assert_eq!(copied, (plus, shield));
    Ok(())
}
