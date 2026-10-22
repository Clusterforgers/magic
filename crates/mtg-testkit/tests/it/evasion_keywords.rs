//! Goal: fear, intimidate, skulk, shadow and landwalk keep an attacker from being blocked as the rules say and are the one reading everyone shares: the options offered to the defender leave out the blockers that can't block, a block that was not offered is refused and changes nothing, fear needs an artifact or black blocker, intimidate an artifact or one that shares a color (a colorless attacker shares none), skulk a blocker whose power is not greater (judged as the layers leave it when blocks are declared), shadow works both ways and flying or reach do not answer it, landwalk looks at the lands the defending player controls (a land type an effect gave counts, another player's lands do not), keywords an effect gives or takes away count, and menace and several evasion keywords combine; at two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::combat::to_blockers;
use common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::effects::{Duration, Modification};
use mtg_engine::error::IllegalAnswer;
use mtg_engine::turn::Step;
use mtg_engine::{EngineError, PresetPermanent};
use mtg_testkit::cards::{cost, free_creature, island, swamp};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn walks(land: &str) -> Keyword {
    Keyword::Landwalk(Selector(vec![Pred::Subtype(Subtype(land.to_owned()))]))
}

/// A 2/3 creature with colors, perhaps an artifact, and keywords.
fn body(
    name: &str,
    power: i32,
    colors: &[Color],
    artifact: bool,
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(power, 3);
    if !colors.is_empty() {
        card = card.with_mana_cost(cost(0, colors));
    }
    if artifact {
        card = card.with_type(CardType::Artifact);
    }
    Arc::new(keywords.iter().fold(card, |card, keyword| {
        card.with_rules(mtg_cards::ability::Ability::Keyword(keyword.clone()))
    }))
}

/// The names of the blockers the pending blockers decision offers for "A", sorted; empty when it is not asked.
fn offered(driver: &Driver) -> Vec<String> {
    let Ok(decision) = driver.decision() else {
        return Vec::new();
    };
    let DecisionKind::DeclareBlockers(options) = &decision.kind else {
        return Vec::new();
    };
    let mut names: Vec<String> = options
        .blockers
        .iter()
        .filter(|blocker| {
            blocker
                .can_block
                .iter()
                .any(|attacker| name_of(driver.state(), *attacker) == "A")
        })
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect();
    names.sort();
    names
}

/// Seat 0 about to declare attackers with "A"; seat 1 holds `blockers` and `lands`.
fn duel(
    seats: usize,
    attacker: Arc<CardDefinition>,
    blockers: Vec<PresetPermanent>,
) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(attacker)])
        .battlefield(1, blockers);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    Driver::from_scenario(scenario)
}

fn ask(driver: &mut Driver) -> Result<(), ScriptError> {
    driver.attack(&[("A", 1)])?;
    to_blockers(driver, 1)
}

/// One blocker: name, power, colors, artifact.
type Blocker = (&'static str, i32, &'static [Color], bool);

const NONE: &[Color] = &[];
const BLACK: &[Color] = &[Color::Black];
const WHITE: &[Color] = &[Color::White];
const RED: &[Color] = &[Color::Red];
const BLUE: &[Color] = &[Color::Blue];
const GREEN_BLACK: &[Color] = &[Color::Green, Color::Black];
const GREEN_BLUE: &[Color] = &[Color::Green, Color::Blue];
const RED_GREEN: &[Color] = &[Color::Red, Color::Green];

fn blockers(list: &[Blocker]) -> Vec<PresetPermanent> {
    list.iter()
        .map(|&(name, power, colors, artifact)| ready(body(name, power, colors, artifact, &[])))
        .collect()
}

#[rstest]
#[case::a_black_creature(&[("B", 2, BLACK, false)], &["B"])]
#[case::an_artifact_creature(&[("B", 2, NONE, true)], &["B"])]
#[case::a_black_artifact_creature(&[("B", 2, BLACK, true)], &["B"])]
#[case::a_green_black_creature(&[("B", 2, GREEN_BLACK, false)], &["B"])]
#[case::a_white_creature(&[("B", 2, WHITE, false)], &[])]
#[case::a_colorless_creature_that_is_no_artifact(&[("B", 2, NONE, false)], &[])]
#[case::an_artifact_and_a_white_creature(&[("W", 2, WHITE, false), ("B", 2, NONE, true)], &["B"])]
#[case::two_black_creatures(&[("B1", 5, BLACK, false), ("B2", 0, BLACK, false)], &["B1", "B2"])]
fn fear_can_be_blocked_only_by_artifact_creatures_and_black_creatures(
    #[case] list: &[Blocker],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        seats,
        body("A", 2, RED, false, &[Keyword::Fear]),
        blockers(list),
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[rstest]
#[case::shares_the_color(RED, &[("B", 2, RED, false)], &["B"])]
#[case::shares_one_of_two_colors(RED_GREEN, &[("B", 2, GREEN_BLUE, false)], &["B"])]
#[case::an_artifact_creature(RED, &[("B", 2, NONE, true)], &["B"])]
#[case::an_artifact_of_another_color(RED, &[("B", 2, BLUE, true)], &["B"])]
#[case::another_color(RED, &[("B", 2, BLUE, false)], &[])]
#[case::colorless_and_no_artifact(RED, &[("B", 2, NONE, false)], &[])]
#[case::a_colorless_attacker_shares_no_color(NONE, &[("B", 2, RED, false)], &[])]
#[case::a_colorless_attacker_and_an_artifact(NONE, &[("B", 2, NONE, true)], &["B"])]
#[case::among_all(RED, &[("X", 2, BLUE, false), ("Y", 2, RED, false), ("Z", 2, NONE, true)], &["Y", "Z"])]
fn intimidate_can_be_blocked_only_by_artifact_creatures_and_creatures_that_share_a_color(
    #[case] colors: &[Color],
    #[case] list: &[Blocker],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        seats,
        body("A", 2, colors, false, &[Keyword::Intimidate]),
        blockers(list),
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[rstest]
#[case::lesser_power(2, 1, true)]
#[case::equal_power(2, 2, true)]
#[case::greater_power(2, 3, false)]
#[case::no_power(2, 0, true)]
#[case::a_big_attacker(5, 5, true)]
#[case::a_big_blocker(0, 1, false)]
fn skulk_can_not_be_blocked_by_creatures_with_greater_power(
    #[case] attacker: i32,
    #[case] blocker: i32,
    #[case] may_block: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        seats,
        body("A", attacker, &[], false, &[Keyword::Skulk]),
        blockers(&[("B", blocker, &[], false)]),
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), if may_block { vec!["B"] } else { vec![] });
    Ok(())
}

#[rstest]
#[case::grown_past(1, 2, false)]
#[case::shrunk_back(3, -2, true)]
#[case::grown_but_not_past(1, 1, true)]
fn skulk_reads_the_power_of_a_blocker_as_the_layers_leave_it(
    #[case] power: i32,
    #[case] change: i32,
    #[case] may_block: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        body("A", 2, NONE, false, &[Keyword::Skulk]),
        blockers(&[("B", power, &[], false)]),
    )?;
    driver.attack(&[("A", 1)])?;
    let blocker = driver.id_of("B")?;
    driver.add_effect(
        driver.on(blocker),
        Modification::ModifyPowerToughness {
            power: change,
            toughness: 0,
        },
        Duration::UntilEndOfTurn,
    )?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver), if may_block { vec!["B"] } else { vec![] });
    Ok(())
}

#[rstest]
#[case::a_plain_blocker(&[], &[])]
#[case::a_flier_is_no_answer(&[Keyword::Flying], &[])]
#[case::reach_is_no_answer(&[Keyword::Reach], &[])]
#[case::shadow_answers(&[Keyword::Shadow], &["B"])]
#[case::shadow_and_more(&[Keyword::Shadow, Keyword::Flying], &["B"])]
fn shadow_can_be_blocked_only_by_creatures_with_shadow(
    #[case] keywords: &[Keyword],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let blocker = body("B", 2, NONE, false, keywords);
    let mut driver = duel(
        seats,
        body("A", 2, NONE, false, &[Keyword::Shadow]),
        vec![ready(blocker)],
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[rstest]
#[case::a_plain_attacker_and_a_plain_blocker(&[], &[], true)]
#[case::a_plain_attacker_and_a_shadow_blocker(&[], &[Keyword::Shadow], false)]
#[case::a_flying_attacker_and_a_shadow_blocker(&[Keyword::Flying], &[Keyword::Shadow, Keyword::Reach], false)]
#[case::a_shadow_attacker_and_a_shadow_blocker(&[Keyword::Shadow], &[Keyword::Shadow], true)]
fn a_creature_with_shadow_can_block_only_creatures_with_shadow(
    #[case] attacker: &[Keyword],
    #[case] blocker: &[Keyword],
    #[case] may_block: bool,
) -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        body("A", 2, NONE, false, attacker),
        vec![ready(body("B", 2, NONE, false, blocker))],
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), if may_block { vec!["B"] } else { vec![] });
    Ok(())
}

/// What seat 1 controls for a landwalk table: lands by name.
fn lands(list: &[&str]) -> Vec<PresetPermanent> {
    list.iter()
        .map(|name| {
            ready(match *name {
                "Island" => island(),
                "Swamp" => swamp(),
                "Forest" => mtg_testkit::cards::forest(),
                "Plains" => mtg_testkit::cards::plains(),
                other => panic!("no land called {other}"),
            })
        })
        .collect()
}

#[rstest]
#[case::islandwalk_and_an_island(&[walks("Island")], &["Island"], &[])]
#[case::islandwalk_and_a_swamp(&[walks("Island")], &["Swamp"], &["B"])]
#[case::islandwalk_among_other_lands(&[walks("Island")], &["Plains", "Island", "Forest"], &[])]
#[case::islandwalk_and_no_land(&[walks("Island")], &[], &["B"])]
#[case::forestwalk_and_a_forest(&[walks("Forest")], &["Forest"], &[])]
#[case::swampwalk_and_a_swamp(&[walks("Swamp")], &["Swamp"], &[])]
#[case::two_walks_the_first_applies(&[walks("Island"), walks("Swamp")], &["Island"], &[])]
#[case::two_walks_the_second_applies(&[walks("Island"), walks("Swamp")], &["Swamp"], &[])]
#[case::two_walks_neither_applies(&[walks("Island"), walks("Swamp")], &["Forest"], &["B"])]
fn a_creature_with_landwalk_can_not_be_blocked_while_the_defender_controls_the_land(
    #[case] keywords: &[Keyword],
    #[case] defender_lands: &[&str],
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut perms = vec![ready(body("B", 2, NONE, false, &[]))];
    perms.extend(lands(defender_lands));
    let mut driver = duel(seats, body("A", 2, NONE, false, keywords), perms)?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[test]
fn the_lands_of_another_player_do_not_count() -> Result<(), ScriptError> {
    // Seat 2 has an Island; seat 1, the defender, has none.
    let mut scenario = Scenario::new(3)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(body("A", 2, NONE, false, &[walks("Island")]))])
        .battlefield(1, [ready(body("B", 2, NONE, false, &[]))])
        .battlefield(2, [ready(island())]);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..10).map(|_| free_creature("Filler", 1, 1)));
    }
    let mut driver = Driver::from_scenario(scenario)?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), ["B"]);
    Ok(())
}

#[test]
fn a_land_type_an_effect_gave_counts() -> Result<(), ScriptError> {
    // A Forest that has become an Island too (Stormtide Leviathan makes every land one).
    let mut driver = duel(
        2,
        body("A", 2, NONE, false, &[walks("Island")]),
        [
            vec![ready(body("B", 2, NONE, false, &[]))],
            lands(&["Forest"]),
        ]
        .concat(),
    )?;
    driver.attack(&[("A", 1)])?;
    let forest = driver.id_of("Forest")?;
    driver.add_effect(
        driver.on(forest),
        Modification::AddSubtype(Subtype("Island".into())),
        Duration::UntilEndOfTurn,
    )?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver), Vec::<String>::new());
    Ok(())
}

#[test]
fn a_creature_that_is_an_island_is_no_land_for_landwalk() -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        body("A", 2, NONE, false, &[walks("Island")]),
        vec![ready(body("B", 2, NONE, false, &[]))],
    )?;
    driver.attack(&[("A", 1)])?;
    let blocker = driver.id_of("B")?;
    driver.add_effect(
        driver.on(blocker),
        Modification::AddSubtype(Subtype("Island".into())),
        Duration::UntilEndOfTurn,
    )?;
    to_blockers(&mut driver, 1)?;
    assert_eq!(offered(&driver), ["B"]);
    Ok(())
}

#[rstest]
#[case::given(Some(Keyword::Fear), None, true)]
#[case::taken_away(None, Some(Keyword::Fear), false)]
fn a_keyword_an_effect_gives_or_takes_away_counts(
    #[case] give: Option<Keyword>,
    #[case] take: Option<Keyword>,
    #[case] has_fear_when_blocked: bool,
) -> Result<(), ScriptError> {
    let printed: &[Keyword] = if take.is_some() {
        &[Keyword::Fear]
    } else {
        &[]
    };
    let mut driver = duel(
        2,
        body("A", 2, RED, false, printed),
        blockers(&[("B", 2, WHITE, false)]),
    )?;
    driver.attack(&[("A", 1)])?;
    let attacker = driver.id_of("A")?;
    if let Some(keyword) = give {
        driver.add_effect(
            driver.on(attacker),
            Modification::AddKeyword(keyword),
            Duration::UntilEndOfTurn,
        )?;
    }
    if let Some(keyword) = take {
        driver.add_effect(
            driver.on(attacker),
            Modification::RemoveKeyword(keyword),
            Duration::UntilEndOfTurn,
        )?;
    }
    to_blockers(&mut driver, 1)?;
    assert_eq!(
        offered(&driver),
        if has_fear_when_blocked {
            vec![]
        } else {
            vec!["B".to_owned()]
        }
    );
    Ok(())
}

#[rstest]
#[case::menace_and_fear_with_one_black(&[Keyword::Menace, Keyword::Fear], &[("B1", 2, BLACK, false), ("B2", 2, WHITE, false)], &[])]
#[case::menace_and_fear_with_two_black(&[Keyword::Menace, Keyword::Fear], &[("B1", 2, BLACK, false), ("B2", 2, BLACK, false)], &["B1", "B2"])]
#[case::fear_and_skulk(&[Keyword::Fear, Keyword::Skulk], &[("B1", 3, BLACK, false), ("B2", 1, BLACK, false)], &["B2"])]
#[case::fear_and_intimidate_need_both(&[Keyword::Fear, Keyword::Intimidate], &[("B1", 2, BLACK, false), ("B2", 2, RED, false), ("B3", 2, NONE, true)], &["B3"])]
fn evasion_keywords_combine(
    #[case] keywords: &[Keyword],
    #[case] list: &[Blocker],
    #[case] expected: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = duel(2, body("A", 2, RED, false, keywords), blockers(list))?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[test]
fn a_block_that_was_not_offered_is_refused_and_changes_nothing() -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        body("A", 2, RED, false, &[Keyword::Fear]),
        blockers(&[("White", 2, WHITE, false), ("Black", 2, BLACK, false)]),
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), ["Black"]);
    let (white, attacker) = (driver.id_of("White")?, driver.id_of("A")?);
    let before = driver.engine().state_hash();
    let refused = driver.answer(Answer::DeclareBlockers(vec![(white, attacker)]));
    assert_eq!(
        refused.err(),
        Some(ScriptError::Engine(EngineError::Illegal(
            IllegalAnswer::NotOffered("blocker")
        )))
    );
    assert_eq!(driver.engine().state_hash(), before);
    driver.block(&[("Black", "A")])?;
    Ok(())
}

#[test]
fn an_unblocked_evader_deals_its_damage_to_the_player() -> Result<(), ScriptError> {
    let mut driver = duel(
        2,
        body("A", 3, RED, false, &[Keyword::Intimidate]),
        blockers(&[("B", 2, BLUE, false)]),
    )?;
    ask(&mut driver)?;
    assert_eq!(offered(&driver), Vec::<String>::new());
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Main2 {
            break;
        }
        match &driver.decision()?.kind {
            DecisionKind::DeclareBlockers(_) => driver.block(&[])?,
            _ => driver.answer_passively()?,
        };
    }
    assert_eq!(driver.state().player(common::pid(1)).unwrap().life(), 17);
    Ok(())
}
