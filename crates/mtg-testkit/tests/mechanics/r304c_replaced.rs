//! Goal: the r30-4c pack's replacements play in a game at two to four seats: a draw skipped by choice or replaced by a
//! revealed card (creature to the graveyard, otherwise drawn), small damage prevented and larger damage dealt, damage
//! to an opponent turned into milling, and damage to a player turned into exiling from their library; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario};
use rstest::rstest;

use crate::common::{names, pid, spell};

/// Passes and answers each yes-or-no question with `may` until seat 0's precombat main phase with an empty stack.
fn to_main(driver: &mut Driver, may: bool) {
    for _ in 0..60 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().turn().step() == Step::Main1
                    && driver.state().stack().is_empty() =>
            {
                return;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(may))).unwrap();
            }
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the game did not reach the main phase");
}

fn replays(scenario: Scenario, driver: &Driver) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
fn the_draw_of_the_draw_step_is_skipped_by_choice(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] skip: bool,
) {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .battlefield_named(0, ["Obstinate Familiar"])
        .library_named(0, ["Grizzly Bears", "Forest"]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    to_main(&mut driver, skip);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    if skip {
        assert!(hand.is_empty(), "{hand:?}");
        assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 2);
    } else {
        assert_eq!(hand, ["Grizzly Bears"]);
    }
    replays(scenario, &driver);
}

#[rstest]
fn a_revealed_creature_goes_to_the_graveyard_and_another_card_is_drawn(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] creature_on_top: bool,
) {
    let library = if creature_on_top {
        ["Grizzly Bears", "Forest"]
    } else {
        ["Forest", "Grizzly Bears"]
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Upkeep)
        .battlefield_named(0, ["Enduring Renewal"])
        .library_named(0, library);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    to_main(&mut driver, false);
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    if creature_on_top {
        assert!(hand.is_empty(), "{hand:?}");
        assert_eq!(graveyard, ["Grizzly Bears"]);
    } else {
        assert_eq!(hand, ["Forest"]);
        assert!(graveyard.is_empty(), "{graveyard:?}");
    }
    replays(scenario, &driver);
}

#[rstest]
#[case::three(3, false)]
#[case::one(1, false)]
#[case::four(4, true)]
fn only_small_damage_to_callous_giant_is_prevented(
    #[values(2, 3, 4)] seats: usize,
    #[case] amount: u32,
    #[case] dealt: bool,
) {
    let burn = spell(
        "Burn",
        CardType::Instant,
        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
        &format!(r#"[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]"#),
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [burn])
        .battlefield_named(1, ["Callous Giant"]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let giant = driver.target_named("Callous Giant").unwrap();
    driver.cast_at("Burn", &[giant]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    // The 3/3 Giant dies to 4 damage and takes none of 3 or less.
    match driver.id_of("Callous Giant") {
        Ok(id) if !dealt => {
            let damage = driver
                .state()
                .object(id)
                .and_then(|giant| giant.permanent())
                .map_or(u32::MAX, mtg_engine::object::PermanentState::damage);
            assert_eq!(damage, 0);
        }
        _ => assert_eq!(
            names(driver.state(), Zone::Graveyard(pid(1))),
            ["Callous Giant"]
        ),
    }
    replays(scenario, &driver);
}

#[rstest]
fn damage_to_a_player_exiles_from_their_library_instead(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] amount: u32,
) {
    let burn = spell(
        "Burn",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        &format!(r#"[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]"#),
    );
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [burn])
        .battlefield_named(0, ["Crumbling Sanctuary"])
        .library_named(1, ["Forest", "Forest", "Forest"]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let target = driver.target_player(1);
    let life = driver.state().player(pid(1)).unwrap().life();
    driver.cast_at("Burn", &[target]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), life);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(1))).len(),
        3 - amount as usize
    );
    replays(scenario, &driver);
}

/// A permanent of seat 0 whose replacement exiles a creature dealt damage this turn by a source its controller
/// controlled.
fn etching() -> std::sync::Arc<mtg_cards::CardDefinition> {
    let rule: mtg_cards::ability::Ability = serde_json::from_str(
        r#"{"Replacement":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"DamagedThisTurnBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"by":{"MoveTo":"Exile"}}}"#,
    )
    .unwrap();
    std::sync::Arc::new(
        mtg_cards::CardDefinition::new("Etching")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

#[rstest]
#[case::burned_by_you(0, true, true)]
#[case::burned_by_its_controller(1, true, false)]
#[case::destroyed(0, false, false)]
fn a_creature_damaged_by_your_source_is_exiled_as_it_dies(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] burn: bool,
    #[case] exiled: bool,
) {
    let kill = if burn {
        spell(
            "Kill",
            CardType::Instant,
            r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
            r#"[{"DealDamage":{"to":{"Target":0},"amount":2}}]"#,
        )
    } else {
        spell(
            "Kill",
            CardType::Instant,
            r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
            r#"[{"Destroy":{"what":{"Target":0}}}]"#,
        )
    };
    let scenario = Scenario::new(seats)
        .at(3, caster, Step::Main1)
        .hand(caster, [kill])
        .battlefield(0, [mtg_testkit::scenario::ready(etching())])
        .battlefield_named(1, ["Grizzly Bears"]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let bears = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Kill", &[bears]).unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let in_exile = names(driver.state(), Zone::Exile).contains(&"Grizzly Bears".to_owned());
    let in_graveyard =
        names(driver.state(), Zone::Graveyard(pid(1))).contains(&"Grizzly Bears".to_owned());
    assert_eq!((in_exile, in_graveyard), (exiled, !exiled));
    replays(scenario, &driver);
}

#[rstest]
#[case::enough(2, false)]
#[case::all(3, false)]
#[case::too_few(4, true)]
fn damage_to_the_lich_exiles_from_the_graveyard_or_loses(
    #[values(2, 3, 4)] seats: usize,
    #[case] amount: u32,
    #[case] loses: bool,
) {
    let burn = spell(
        "Burn",
        CardType::Instant,
        r#"[{"what":"Player"}]"#,
        &format!(r#"[{{"DealDamage":{{"to":{{"Target":0}},"amount":{amount}}}}}]"#),
    );
    let scenario = Scenario::new(seats)
        .at(3, 1, Step::Main1)
        .hand(1, [burn])
        .battlefield_named(0, ["Nefarious Lich"])
        .graveyard_named(0, ["Forest", "Forest", "Forest"]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let target = driver.target_player(0);
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.cast_at("Burn", &[target]).unwrap();
    for _ in 0..20 {
        if driver.state().stack().is_empty() || !driver.state().player(pid(0)).unwrap().is_in_game()
        {
            break;
        }
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) => driver.pass().unwrap(),
            _ => driver.answer_passively().unwrap(),
        };
    }
    let player = driver.state().player(pid(0)).unwrap();
    assert_eq!(!player.is_in_game(), loses);
    assert_eq!(player.life(), life);
    if !loses {
        assert_eq!(
            driver.state().zone(Zone::Graveyard(pid(0))).len(),
            3 - amount as usize
        );
    }
    replays(scenario, &driver);
}
