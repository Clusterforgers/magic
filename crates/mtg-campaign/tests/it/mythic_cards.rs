//! Goal: the run-defining Mythic relics do what their words say at a table of two to four: the Maelstrom Crown cascades
//! from an instant and leaves the cards it passes in exile, the Codex and the Hydra Heart give cascade to the spells they
//! name and no other, the Banner doubles creature tokens, the Chalice turns life gained into the opponents' loss, the
//! Tome draws an extra card, the Lantern casts a creature from the graveyard, and the statics change the creatures they
//! name; each Mythic relic wants what its rules care about, so the usefulness filter offers it only to a deck that holds
//! enough of it.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use mtg_campaign::relics::{MIN_CARDS, usable};
use mtg_campaign::{DeckMakeup, Rarity, Want, content};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{
    cost, creature, forest, free_creature, instant, island, mountain, plains,
};
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const HERO: usize = 0;

fn relic(id: &str) -> Arc<CardDefinition> {
    content().unwrap().card(id).unwrap().definition()
}

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

/// The hero (seat 0) with `relics`, four lands of each of red, blue, white and green, a Bear and `hand`, over `library`
/// (top card first); every other seat has a Brute and a library of Mountains.
fn table(
    seats: usize,
    relics: &[&str],
    hand: Vec<Arc<CardDefinition>>,
    library: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut lands = vec![ready(free_creature("Bear", 2, 2))];
    for _ in 0..4 {
        lands.extend([
            ready(mountain()),
            ready(island()),
            ready(plains()),
            ready(forest()),
        ]);
    }
    let mut scenario = Scenario::new(seats)
        .life(HERO, 20)
        .battlefield(HERO, lands)
        .hand(HERO, hand)
        .library(HERO, library.into_iter().chain((0..20).map(|_| mountain())))
        .emblems(HERO, relics.iter().map(|id| relic(id)));
    for seat in 1..seats {
        scenario = scenario
            .life(seat, 40)
            .battlefield(seat, [ready(free_creature("Brute", 3, 3))])
            .library(seat, (0..40).map(|_| mountain()));
    }
    scenario
}

fn names(driver: &Driver, zone: Zone) -> Vec<String> {
    let state = driver.state();
    state
        .zone(zone)
        .iter()
        .filter_map(|id| state.object(*id))
        .map(|o| o.card().name.clone())
        .collect()
}

/// Passes and says yes to every free cast until the stack is empty.
fn resolve_saying_yes(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if matches!(driver.decision()?.kind, DecisionKind::YesNo(_)) {
            driver.answer(Answer::YesNo(YesNoChoice(true)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn sorcery(name: &str, generic: u32, colored: &[Color]) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(generic, colored)),
    )
}

#[rstest]
fn the_maelstrom_crown_cascades_from_an_instant_and_keeps_the_rest_in_exile(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] crown: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if crown { &["maelstrom_crown"] } else { &[] };
    let library = vec![
        forest(),
        instant("Too Big", 3, &[]),
        creature("Cub", 1, &[], 1, 1),
    ];
    let scenario = table(
        seats,
        relics,
        vec![instant("Bolt", 2, &[Color::Red])],
        library,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Bolt")?;
    resolve_saying_yes(&mut driver)?;
    let field = names(&driver, Zone::Battlefield);
    let exile = names(&driver, Zone::Exile);
    if crown {
        assert!(
            field.contains(&"Cub".to_owned()),
            "the card found is cast free"
        );
        let mut exile = exile;
        exile.sort();
        assert_eq!(
            exile,
            ["Forest", "Too Big"],
            "the cards passed stay in exile"
        );
    } else {
        assert!(!field.contains(&"Cub".to_owned()));
        assert_eq!(exile, Vec::<String>::new());
    }
    Ok(())
}

#[rstest]
#[case::codex_on_a_sorcery("codex_of_echoes", sorcery("Rite", 2, &[Color::Red]), true)]
#[case::codex_not_on_an_instant("codex_of_echoes", instant("Quick", 2, &[Color::Red]), false)]
#[case::hydra_on_a_big_creature("hydra_heart", creature("Hydra", 4, &[Color::Green], 5, 5), true)]
#[case::hydra_not_on_a_small_one("hydra_heart", creature("Pup", 3, &[Color::Green], 3, 3), false)]
fn a_cascade_mythic_gives_cascade_to_the_spells_it_names(
    #[case] id: &str,
    #[case] spell: Arc<CardDefinition>,
    #[case] cascades: bool,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let name = spell.name.clone();
    let library = vec![forest(), creature("Cub", 1, &[], 1, 1)];
    let mut driver = Driver::from_scenario(table(seats, &[id], vec![spell], library))?;
    driver.pass_until(Step::Main1)?;
    driver.cast(&name)?;
    resolve_saying_yes(&mut driver)?;
    let field = names(&driver, Zone::Battlefield);
    assert_eq!(field.contains(&"Cub".to_owned()), cascades, "{id}: {name}");
    // A real cascade puts the cards passed on the bottom.
    assert_eq!(names(&driver, Zone::Exile), Vec::<String>::new());
    Ok(())
}

#[rstest]
fn the_banner_doubles_creature_tokens(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] banner: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if banner {
        &["banner_of_multitudes"]
    } else {
        &[]
    };
    let alarm = corpus::card("Dance with Devils").unwrap();
    let mut driver = Driver::from_scenario(table(seats, relics, vec![alarm], Vec::new()))?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Dance with Devils")?;
    resolve_saying_yes(&mut driver)?;
    let soldiers = names(&driver, Zone::Battlefield)
        .iter()
        .filter(|n| n.contains("Devil") && !n.contains("Dance"))
        .count();
    assert_eq!(soldiers, if banner { 4 } else { 2 });
    Ok(())
}

#[rstest]
fn the_chalice_turns_life_gained_into_the_opponents_loss(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // The Sun Idol gains 1 life for a white spell; the Chalice makes each opponent lose it.
    let scenario = table(
        seats,
        &["sun_idol", "bloodbond_chalice"],
        vec![creature("Squire", 0, &[Color::White], 1, 1)],
        Vec::new(),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    driver.cast("Squire")?;
    resolve_saying_yes(&mut driver)?;
    let life = |seat: usize| driver.state().player(pid(seat)).unwrap().life();
    assert_eq!(life(HERO), 21);
    for seat in 1..seats {
        assert_eq!(life(seat), 39, "seat {seat}");
    }
    Ok(())
}

#[rstest]
fn the_tome_draws_an_extra_card_each_draw_step(
    #[values(true, false)] tome: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if tome { &["endless_tome"] } else { &[] };
    let mut driver = Driver::from_scenario(table(2, relics, Vec::new(), Vec::new()))?;
    driver.idle_until(3, Step::Main1)?;
    let hand = driver.state().zone(Zone::Hand(pid(HERO))).len();
    assert_eq!(hand, if tome { 2 } else { 1 });
    Ok(())
}

#[rstest]
fn the_lantern_casts_a_creature_from_the_graveyard(
    #[values(true, false)] lantern: bool,
) -> Result<(), ScriptError> {
    let relics: &[&str] = if lantern {
        &["gravecallers_lantern"]
    } else {
        &[]
    };
    let scenario = table(2, relics, Vec::new(), Vec::new())
        .graveyard(HERO, [creature("Ghoul", 1, &[Color::Red], 2, 2)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    assert_eq!(driver.cast("Ghoul").is_ok(), lantern);
    Ok(())
}

#[rstest]
#[case::censer_shrinks_the_opponents("blight_censer", (2, 2), (2, 2))]
#[case::standard_grows_the_heroes("warlords_standard", (3, 3), (3, 3))]
fn a_static_mythic_changes_the_creatures_it_names(
    #[case] id: &str,
    #[case] bear: (i32, i32),
    #[case] brute: (i32, i32),
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(table(seats, &[id], Vec::new(), Vec::new()))?;
    let stats = |name: &str| -> Result<(i32, i32), ScriptError> {
        let c = characteristics(driver.state(), driver.id_of(name)?).unwrap();
        Ok((c.power.unwrap_or(0), c.toughness.unwrap_or(0)))
    };
    assert_eq!(stats("Bear")?, bear, "{id}");
    assert_eq!(stats("Brute")?, brute, "{id}");
    Ok(())
}

#[test]
fn a_mythic_is_offered_only_to_a_deck_that_holds_what_it_wants() {
    let content = content().unwrap();
    let crown = content.card("maelstrom_crown").unwrap();
    assert_eq!(crown.rarity, Rarity::Mythic);
    assert_eq!(crown.wants, [Want::Type(CardType::Instant)]);
    let deck = |instants: u32| {
        let mut makeup = DeckMakeup::default();
        for _ in 0..instants {
            makeup.add_type_line("U", "Instant");
        }
        for _ in 0..12 {
            makeup.add_type_line("U", "Sorcery");
        }
        makeup
    };
    assert!(
        !usable(crown, &deck(MIN_CARDS - 1)),
        "sorceries do not count"
    );
    assert!(usable(crown, &deck(MIN_CARDS)));
}
