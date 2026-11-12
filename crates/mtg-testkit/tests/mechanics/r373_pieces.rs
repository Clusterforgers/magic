//! Goal: the r373 pieces in play: life gained "equal to the damage dealt, but not more than" what the recipient had before (a player's life, a planeswalker's loyalty, a creature's toughness); a mana ability that adds as much of each type as the pool holds, restricted mana included; the keywords of other objects, landwalk and protection whatever their parameter, and nothing not named; and fortify, which attaches only to a land its controller controls and gives it what the Fortification says; at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_core::mana::ManaType;
use mtg_testkit::cards::{bear, forest, keyworded_creature, swamp, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"))
}

/// A free sorcery: "~ deals 5 damage to any target. You gain life equal to the damage dealt, but not more ...".
fn siphon() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Siphon")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(rules(
                r#"{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":5}},{"GainLife":{"amount":"DealtUpToHealth"}}]}}"#,
            )),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::a_player_with_more_life("player", 20, 5)]
#[case::a_player_with_less_life("player", 3, 3)]
#[case::a_creature_of_toughness_two("Bear", 20, 2)]
#[case::a_planeswalker_of_loyalty_three("Walker", 20, 3)]
fn the_life_gained_is_the_damage_up_to_what_the_recipient_had(
    #[case] target: &str,
    #[case] their_life: i32,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [siphon()])
            .battlefield(1, [ready(bear()), ready(walker("Walker", 1))])
            .life(1, their_life),
    )?;
    let at = if target == "player" {
        driver.target_player(1)
    } else {
        driver.target_named(target)?
    };
    driver.cast_at("Siphon", &[at])?;
    settle(&mut driver)?;
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20 + gained);
    Ok(())
}

/// A free artifact: "{T}: Double the amount of each type of unspent mana you have."
fn cube() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Cube")
            .with_type(CardType::Artifact)
            .with_rules(rules(
                r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":"Unspent"}}]}}"#,
            )),
    )
}

#[rstest]
#[case::nothing_doubles_to_nothing(0, 0)]
#[case::one_land(1, 0)]
#[case::two_types(2, 1)]
fn the_pool_doubles_by_type(
    #[case] swamps: usize,
    #[case] forests: usize,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut board = vec![ready(cube())];
    board.extend(std::iter::repeat_with(|| ready(swamp())).take(swamps));
    board.extend(std::iter::repeat_with(|| ready(forest())).take(forests));
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(0, board))?;
    for _ in 0..swamps {
        driver.tap_for_mana("Swamp")?;
    }
    for _ in 0..forests {
        driver.tap_for_mana("Forest")?;
    }
    let offered = driver.tap_for_mana("Cube");
    assert_eq!(offered.is_ok(), swamps + forests > 0, "an empty pool doubles to nothing");
    let pool = driver.state().player(pid(0)).unwrap().mana();
    assert_eq!(pool.get(ManaType::Black), 2 * u32::try_from(swamps).unwrap());
    assert_eq!(pool.get(ManaType::Green), 2 * u32::try_from(forests).unwrap());
    Ok(())
}

/// A free creature with "This creature has each keyword of `names` that a card in your graveyard has."
fn borrower(names: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Borrower")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(rules(&format!(
                r#"{{"Static":{{"affected":"This","mods":[{{"KeywordsOf":{{"from":{{"Each":[{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}},"names":{names}}}}}]}}}}"#
            ))),
    )
}

fn swampwalk() -> Keyword {
    Keyword::Landwalk(Selector(vec![Pred::Subtype(Subtype("Swamp".into()))]))
}

fn from_red() -> Keyword {
    Keyword::Protection(Selector(vec![Pred::Color(mtg_core::color::Color::Red)]))
}

#[rstest]
#[case::flying_named(vec![Keyword::Flying], r#"["Flying","Trample"]"#, vec![Keyword::Flying])]
#[case::flying_not_named(vec![Keyword::Flying], r#"["Trample"]"#, vec![])]
#[case::landwalk_of_any_land(vec![swampwalk()], r#"["Landwalk"]"#, vec![swampwalk()])]
#[case::protection_of_any_quality(vec![from_red(), Keyword::Vigilance], r#"["Protection"]"#, vec![from_red()])]
fn a_permanent_has_the_named_keywords_of_other_cards(
    #[case] held: Vec<Keyword>,
    #[case] names: &str,
    #[case] expected: Vec<Keyword>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let donor = keyworded_creature("Donor", 1, 1, &held);
    let driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, [ready(borrower(names))])
            .graveyard(0, [donor]),
    )?;
    let id = driver.id_of("Borrower")?;
    let has: Vec<Keyword> = mtg_engine::keywords::keywords_of(driver.state(), id)
        .into_iter()
        .cloned()
        .collect();
    assert_eq!(has, expected);
    Ok(())
}

#[rstest]
fn fortify_attaches_to_a_land_and_gives_it_indestructible(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let garrison = card("Darksteel Garrison").unwrap();
    let mut driver = Driver::from_scenario(Scenario::new(seats).battlefield(
        0,
        [
            ready(garrison),
            ready(forest()),
            ready(swamp()),
            ready(swamp()),
            ready(swamp()),
        ],
    ))?;
    driver.begin_activation("Darksteel Garrison")?;
    let land = driver.target_named("Forest")?;
    driver.choose_targets(&[land])?;
    driver.pay_tapping(&["Swamp", "Swamp", "Swamp"], 0)?;
    settle(&mut driver)?;
    let forest = driver.id_of("Forest")?;
    assert_eq!(
        driver.state().host_of(driver.id_of("Darksteel Garrison")?),
        Some(forest)
    );
    assert!(mtg_engine::keywords::has_keyword(
        driver.state(),
        forest,
        &Keyword::Indestructible
    ));
    Ok(())
}

/// A free instant: "~ deals 1 damage to target creature."
fn zap() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Zap")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(rules(
                r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
            )),
    )
}

/// The names of the permanents the deciding player may target, sorted.
fn targetable(driver: &Driver) -> (Vec<String>, u32) {
    match &driver.decision().unwrap().kind {
        mtg_engine::decision::DecisionKind::ChooseTargets(options) => {
            let mut names: Vec<String> = options
                .legal
                .iter()
                .filter_map(|target| match target {
                    mtg_engine::choose::Target::Object(object) => {
                        driver.state().object(object.id).map(|o| o.card().name.clone())
                    }
                    mtg_engine::choose::Target::Player(_) => None,
                })
                .collect();
            names.sort();
            (names, options.min)
        }
        other => panic!("{other:?}"),
    }
}

#[rstest]
#[case::an_opponent_s_flagbearer(Some(0), vec!["Standard Bearer"], 1)]
#[case::the_caster_s_own_flagbearer(Some(1), vec!["Grizzly Bear", "Standard Bearer"], 1)]
#[case::no_flagbearer(None, vec!["Grizzly Bear"], 1)]
fn an_opponent_targets_a_flagbearer_if_able(
    #[case] bearer: Option<usize>,
    #[case] legal: Vec<&str>,
    #[case] min: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let standard = card("Standard Bearer").unwrap();
    let mut scenario = Scenario::new(seats)
        .at(1, 1, mtg_engine::turn::Step::Main1)
        .hand(1, [zap()])
        .battlefield(0, [ready(Arc::new(
            CardDefinition::new("Grizzly Bear")
                .with_type(CardType::Creature)
                .with_power_toughness(2, 2),
        ))]);
    if let Some(seat) = bearer {
        scenario = scenario.battlefield(seat, [ready(standard)]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Zap")?;
    let (names, least) = targetable(&driver);
    assert_eq!(names, legal);
    assert_eq!(least, min);
    Ok(())
}

/// An artifact: "At the beginning of your first main phase, add mana equal to the mana cost of the card named Model."
fn resonator() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Resonator")
            .with_type(CardType::Artifact)
            .with_rules(rules(
                r#"{"Triggered":{"when":{"StepBegins":{"step":"Main1","whose":"You"}},"effects":[{"AddMana":{"mana":{"ManaCostOf":{"Each":[{"Named":"Model"}]}}}}]}}"#,
            )),
    )
}

#[rstest]
#[case::generic_and_colors("{2}{W}{U}", &[(ManaType::Colorless, 2), (ManaType::White, 1), (ManaType::Blue, 1)])]
#[case::one_color_twice("{G}{G}", &[(ManaType::Green, 2)])]
#[case::phyrexian_as_its_color("{1}{B/P}", &[(ManaType::Colorless, 1), (ManaType::Black, 1)])]
fn mana_equal_to_a_mana_cost(
    #[case] cost: &str,
    #[case] added: &[(ManaType, u32)],
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let model = Arc::new(
        CardDefinition::new("Model")
            .with_type(CardType::Artifact)
            .with_mana_cost(mtg_core::mana::ManaCost::parse(cost).unwrap()),
    );
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(2, 0, mtg_engine::turn::Step::Draw)
            .battlefield(0, [ready(resonator()), ready(model)]),
    )?;
    for _ in 0..20 {
        if driver.state().turn().step() == mtg_engine::turn::Step::Main1
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    let pool = driver.state().player(pid(0)).unwrap().mana();
    for (mana, count) in added {
        assert_eq!(pool.get(*mana), *count, "{mana:?}");
    }
    assert_eq!(pool.total(), added.iter().map(|(_, count)| count).sum::<u32>());
    Ok(())
}
