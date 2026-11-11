//! Goal: the engine pieces of ep33 play by the rules at tables of two and three seats. A card an effect returns to the battlefield face down (`Destination::BattlefieldFaceDown`) is a 2/2 creature with no name under the player the move names, only that player looks at it, and it is turned face up for its morph cost and never for its mana cost; a face-down card that has no morph stays face down. Mana for turning permanents face up, or for any cost but a spell's, pays the morph cost and mana for another special action or for spells does not; mana for "your commander" casts the commander and no other spell. "While voting, you get an additional vote" gives one more ballot, never blank, for each such rule.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::decision::{DecisionKind, PriorityAction};
use mtg_engine::events::ZoneCause;
use mtg_engine::object::FaceDown;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event};
use mtg_testkit::cards::{cost, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A 4/1 for {2} that returns face down under `under` when it dies, with morph {1} when `morph`, and "Sacrifice ~: You
/// gain 1 life."
fn phoenix(morph: bool, under: &str) -> Arc<CardDefinition> {
    let rule = |json: &str| -> Ability { serde_json::from_str(json).unwrap() };
    let mut card = CardDefinition::new("Phoenix")
        .with_type(CardType::Creature)
        .with_mana_cost(cost(2, &[]))
        .with_power_toughness(4, 1)
        .with_rules(rule(&format!(
            r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"Move":{{"what":"This","to":{{"BattlefieldFaceDown":{under}}}}}}}]}}}}"#
        )))
        .with_rules(rule(
            r#"{"Activated":{"cost":["SacrificeSelf"],"effects":[{"GainLife":{"amount":1}}]}}"#,
        ));
    if morph {
        card = card.with_rules(rule(r#"{"Keyword":{"Morph":"{1}"}}"#));
    }
    Arc::new(card)
}

/// Passes priority until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// The face-down permanents on the battlefield.
fn face_down(driver: &Driver) -> Vec<ObjectId> {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().face_down().is_some())
        .collect()
}

/// The permanents seat 0 may turn face up now.
fn turnable(driver: &Driver) -> Vec<ObjectId> {
    match &driver.decision().unwrap().kind {
        DecisionKind::Priority(options) => options.turnable.clone(),
        _ => Vec::new(),
    }
}

#[rstest]
fn a_card_returned_face_down_is_a_hidden_2_2_turned_up_only_for_its_morph(
    #[values(2, 3)] seats: usize,
    #[values(true, false)] morph: bool,
    #[values(r#"{"under":"You"}"#, "{}")] under: &str,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        vec![
            ready(phoenix(morph, under)),
            ready(plains()),
            ready(plains()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Phoenix")?;
    settle(&mut driver)?;
    let made = face_down(&driver);
    assert_eq!(made.len(), 1, "the card came back face down");
    let id = made[0];
    let state = driver.state();
    let object = state.object(id).unwrap();
    assert_eq!(object.face_down(), Some(FaceDown::Morph));
    assert_eq!(object.whole_card().name, "Phoenix");
    assert_eq!(object.card().name, "");
    assert_eq!(object.controller(), pid(0));
    let chars = characteristics(state, id).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(2), Some(2)));
    assert!(chars.types.contains(CardType::Creature));
    for seat in 0..seats {
        assert_eq!(state.may_look_at(pid(seat), id), seat == 0, "seat {seat}");
    }
    let cause = driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::ZoneChanged {
                object,
                to: Zone::Battlefield,
                cause,
                ..
            } if *object == id => Some(*cause),
            _ => None,
        });
    assert_eq!(cause, Some(ZoneCause::PutFaceDown { by: pid(0) }));
    // Its mana cost of {2} is paid by the two Plains, yet only morph turns it face up.
    assert_eq!(turnable(&driver), if morph { vec![id] } else { vec![] });
    if morph {
        driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
        crate::common::casting::pay(&mut driver)?;
        let object = driver.state().object(id).unwrap();
        assert_eq!(object.face_down(), None);
        assert_eq!(object.card().name, "Phoenix");
    }
    Ok(())
}

/// An artifact with "{T}: Add {C}{C}{C}" whose mana may be spent only on `only` (a JSON list of uses).
fn restricted_source(name: &str, only: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Colorless","Colorless","Colorless"]}},"only":{only}}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

/// The morph Phoenix comes back face down with only `source` to pay its morph {1}: mana for turning permanents face up
/// pays it, as does mana for any cost but a spell's; mana for unlocking a door or for spells does not.
#[rstest]
#[case::turning_face_up(r#"[{"Special":"TurnFaceUp"}]"#, true)]
#[case::other_costs(r#"[{"Ability":{}},"OtherCosts"]"#, true)]
#[case::unlocking(r#"[{"Special":"Unlock"}]"#, false)]
#[case::foretelling(r#"[{"Special":"Foretell"}]"#, false)]
#[case::creature_spells(r#"[{"Spell":{"types":["Creature"]}}]"#, false)]
fn mana_for_turning_permanents_face_up_turns_one_up(
    #[case] only: &str,
    #[case] turns: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        vec![
            ready(phoenix(true, r#"{"under":"You"}"#)),
            ready(restricted_source("Zealot", only)),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_activation("Phoenix")?;
    settle(&mut driver)?;
    let id = face_down(&driver)[0];
    assert_eq!(turnable(&driver), if turns { vec![id] } else { vec![] });
    if turns {
        driver.answer(Answer::Priority(PriorityAction::TurnFaceUp(id)))?;
        crate::common::casting::pay(&mut driver)?;
        assert_eq!(driver.state().object(id).unwrap().face_down(), None);
    }
    Ok(())
}

/// A commander and a creature of the same cost, with only mana for "your commander": the commander is cast with it, the
/// other creature is not.
#[rstest]
fn mana_for_your_commander_casts_only_the_commander(
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let commander = Arc::new(
        CardDefinition::new("Cmd")
            .with_type(CardType::Creature)
            .with_supertype(mtg_core::card::Supertype::Legendary)
            .with_mana_cost(cost(3, &[]))
            .with_power_toughness(2, 2),
    );
    let scenario = Scenario::new(seats)
        .format(Arc::new(mtg_engine::format::Commander))
        .commanders(0, [commander])
        .hand(0, [mtg_testkit::cards::creature("Bear", 3, &[], 2, 2)])
        .battlefield(
            0,
            vec![ready(restricted_source(
                "Lotus",
                r#"[{"Spell":{"facts":["YourCommander"]}}]"#,
            ))],
        )
        .at(1, 0, Step::Main1);
    let driver = Driver::from_scenario(scenario)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("seat 0 has priority");
    };
    let names: Vec<String> = options
        .castable
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect();
    assert_eq!(names, ["Cmd"]);
    Ok(())
}

/// A free sorcery: everyone votes for gift or life; if gift gets more votes, its caster draws a card, otherwise gains
/// one life.
fn ballot() -> Arc<CardDefinition> {
    let spell: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[{"Vote":{"options":{"Named":["gift","life"]}}},{"If":{"condition":{"Compare":{"left":{"Players":{"Bound":"gift"}},"op":"Gt","right":{"Players":{"Bound":"life"}}}},"then":[{"Draw":{}}],"otherwise":[{"GainLife":{"amount":1}}]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Ballot")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(spell),
    )
}

/// An artifact with "While voting, you get an additional vote."
fn representative(n: usize) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(r#"{"Rule":{"rule":{"ExtraVote":"You"}}}"#).unwrap();
    Arc::new(
        CardDefinition::new(&format!("Representative {n}"))
            .with_type(CardType::Artifact)
            .with_rules(rule),
    )
}

/// Seat 0 controls `extra` representatives and votes gift with every ballot, the others life: seat 0 is asked once for
/// each of its votes, with no blank ballot, before the others in turn order, and gift wins when its votes outnumber the
/// others'.
#[rstest]
#[case::two_seats_one_extra(2, 1, true)]
#[case::three_seats_one_extra_ties(3, 1, false)]
#[case::three_seats_two_extras(3, 2, true)]
#[case::four_seats_two_extras_tie(4, 2, false)]
fn an_additional_vote_is_cast_and_counted(
    #[case] seats: usize,
    #[case] extra: usize,
    #[case] drew: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .seed(33)
        .hand(0, [ballot()])
        .battlefield(
            0,
            (0..extra)
                .map(|n| ready(representative(n)))
                .collect::<Vec<_>>(),
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..6).map(|_| mtg_testkit::cards::forest()));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let start = driver.state().player(pid(0)).unwrap().life();
    driver.cast("Ballot")?;
    let mut asked = Vec::new();
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::ChooseOption(options) => {
                assert_eq!(options.options.len(), 2, "no blank ballot");
                let seat = decision.player.index();
                asked.push(seat);
                let vote = usize::from(seat != 0);
                driver.answer(Answer::ChooseOption(mtg_engine::choose::OptionChoice(vote)))?;
            }
            _ => {
                driver.pass()?;
            }
        }
    }
    let want: Vec<usize> = std::iter::repeat_n(0, 1 + extra).chain(1..seats).collect();
    assert_eq!(asked, want);
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        usize::from(drew)
    );
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, start + i32::from(!drew));
    Ok(())
}
