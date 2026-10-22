//! Goal: a copy of a spell (CR 707.10) is put on the stack above the spell by Twincast, Reverberate, Fork, Reiterate, Dualcaster Mage and Dual Casting, at tables of two, three and four seats: it is not cast (no `SpellCast`, no cast trigger, nothing paid), it has the characteristics and the announced choices of the original (targets, modes, X, optional costs, the half of a split card or an Adventure), its controller may choose new targets for it with the ordinary target decision, a copy that resolves or is countered ceases to exist and is never in a graveyard, a copy can be copied, an exception of the effect (Fork's red) is a characteristic of the copy only, the buyback of the original is not the copy's, and the log of such games replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::keep_targets_until_done;
use common::copying::{
    casts, game, hand, lands, life, object_of, on_the_stack, pass_round, replays, run, say,
    spell_copies, twincast,
};
use common::{names, pid};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::characteristics;
use mtg_engine::choose::{CostsChoice, ModeChoice, NumberChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::object::{ObjectKind, ObjectRef};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, study};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, ScriptError, attached_to, ready};
use rstest::rstest;

/// Casts `spell` at `targets` (paying automatically) and then Twincast, Reverberate or Fork at the spell.
fn cast_and_copy(
    driver: &mut Driver,
    spell: &str,
    targets: &[Target],
    copier: &str,
) -> Result<(), ScriptError> {
    driver.cast_at(spell, targets)?;
    let spell = on_the_stack(driver, spell);
    driver.cast_at(copier, &[spell])?;
    Ok(())
}

/// Starts casting `spell`, answers `x` when it asks for X, chooses `targets` and pays by tapping `taps`.
fn cast_x_at(
    driver: &mut Driver,
    spell: &str,
    x: Option<u32>,
    targets: &[Target],
    taps: &[&str],
) -> Result<(), ScriptError> {
    driver.begin_cast(spell)?;
    if let Some(x) = x {
        driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    }
    driver.choose_targets(targets)?;
    driver.pay_tapping(taps, 0)?;
    Ok(())
}

#[rstest]
fn a_copy_with_the_targets_of_the_original_hits_the_same_player_and_ceases_to_exist(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || game(seats, &["Lightning Bolt", "Twincast"], 2, 1);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    cast_and_copy(&mut driver, "Lightning Bolt", &[foe], "Twincast")?;
    let casts_before = casts(&driver);
    run(&mut driver, seats)?;
    let DecisionKind::YesNo(question) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert_eq!(
        question.text,
        "Choose new targets for the copy of Lightning Bolt?"
    );
    say(&mut driver, false)?;
    let copies = spell_copies(&driver);
    assert_eq!(copies.len(), 1);
    let copy = driver.state().object(copies[0]).unwrap();
    assert_eq!(copy.kind(), ObjectKind::Token);
    assert_eq!(copy.card().name, "Lightning Bolt");
    assert_eq!(copy.controller(), pid(0));
    assert_eq!(copy.choices().targets, vec![vec![foe]]);
    assert_eq!(
        copy.copied_from().unwrap().id,
        object_of(on_the_stack(&driver, "Lightning Bolt"))
    );
    assert_eq!(
        driver.state().stack().last(),
        Some(&copies[0]),
        "the copy is on top of the stack"
    );
    assert_eq!(casts(&driver), casts_before, "a copy is not cast");
    run(&mut driver, seats)?;
    assert_eq!(
        life(&driver, 1),
        14,
        "the original and the copy each dealt 3"
    );
    assert_eq!(driver.state().stack(), []);
    assert!(
        driver.state().has_ceased(copies[0]),
        "the copy ceased to exist"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Twincast", "Lightning Bolt"],
        "only the cards are in the graveyard, in the order they resolved"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn new_targets_are_chosen_with_the_target_decision_and_leave_the_original_alone(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || game(seats, &["Lightning Bolt", "Reverberate"], 0, 3);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    cast_and_copy(&mut driver, "Lightning Bolt", &[foe], "Reverberate")?;
    run(&mut driver, seats)?;
    say(&mut driver, true)?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    assert_eq!(
        (options.min, options.max, options.can_cancel),
        (1, 1, false)
    );
    assert!(options.legal.contains(&driver.target_player(2)));
    assert_eq!(driver.decision()?.player, pid(0));
    let third = driver.target_player(2);
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![third])))?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.choices().targets, vec![vec![third]]);
    let original = object_of(on_the_stack(&driver, "Lightning Bolt"));
    assert_eq!(
        driver.state().object(original).unwrap().choices().targets,
        vec![vec![foe]]
    );
    run(&mut driver, seats)?;
    assert_eq!((life(&driver, 1), life(&driver, 2)), (17, 17));
    replays(&driver, &setup);
    Ok(())
}

#[test]
fn a_target_that_is_not_a_legal_choice_cannot_be_named_and_the_choice_cannot_be_backed_out_of()
-> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(2, &["Lightning Bolt", "Twincast"], 2, 1))?;
    let foe = driver.target_player(1);
    cast_and_copy(&mut driver, "Lightning Bolt", &[foe], "Twincast")?;
    run(&mut driver, 2)?;
    say(&mut driver, true)?;
    let stranger = Target::Object(ObjectRef {
        id: ObjectId(9999),
        counter: 0,
    });
    assert!(
        driver
            .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![stranger])))
            .is_err()
    );
    assert!(
        driver
            .answer(Answer::ChooseTargets(TargetChoice::Cancel))
            .is_err()
    );
    Ok(())
}

#[test]
fn fork_makes_the_copy_red_and_the_original_keeps_its_colours() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(game(2, &["Opt", "Fork"], 1, 2))?;
    driver.cast("Opt")?;
    let opt = on_the_stack(&driver, "Opt");
    driver.cast_at("Fork", &[opt])?;
    run(&mut driver, 2)?;
    let copy = spell_copies(&driver)[0];
    let colors = characteristics(driver.state(), copy).unwrap().colors;
    assert_eq!(colors.iter().collect::<Vec<_>>(), [Color::Red]);
    let colors = characteristics(driver.state(), object_of(opt))
        .unwrap()
        .colors;
    assert_eq!(colors.iter().collect::<Vec<_>>(), [Color::Blue]);
    Ok(())
}

#[rstest]
fn a_copy_without_targets_asks_nothing_and_does_what_the_spell_does_for_its_controller(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 3, 0)
            .hand(0, [study("Study"), card("Twincast").unwrap()])
            .library(0, (0..8).map(|_| island()))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.cast("Study")?;
    let spell = on_the_stack(&driver, "Study");
    driver.cast_at("Twincast", &[spell])?;
    pass_round(&mut driver, seats)?;
    assert_eq!(
        spell_copies(&driver).len(),
        1,
        "the copy waits on the stack, and nobody was asked"
    );
    assert_eq!(hand(&driver, 0), 0);
    run(&mut driver, seats)?;
    assert_eq!(
        hand(&driver, 0),
        4,
        "Study and its copy each drew two cards"
    );
    assert_eq!(driver.state().stack(), []);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_spell_with_x_has_the_x_of_the_original(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2)] x: u32,
) -> Result<(), ScriptError> {
    let scenario =
        || game(seats, &[], 2, 3).hand(0, [card("Blaze").unwrap(), card("Twincast").unwrap()]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    let taps = ["Mountain", "Mountain", "Mountain"];
    cast_x_at(&mut driver, "Blaze", Some(x), &[foe], &taps[..=x as usize])?;
    twincast(&mut driver, "Blaze")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.choices().x, Some(x));
    assert_eq!(copy.announced_x(), Some(x));
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 20 - 2 * i32::try_from(x).unwrap());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_kicked_spell_is_kicked(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 2, 5).hand(
            0,
            [card("Burst Lightning").unwrap(), card("Twincast").unwrap()],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.begin_cast("Burst Lightning")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain"; 5], 0)?;
    twincast(&mut driver, "Burst Lightning")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.paid_costs(), [0]);
    run(&mut driver, seats)?;
    assert_eq!(
        life(&driver, 1),
        12,
        "a kicked Burst Lightning and its copy each deal 4"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_modal_spell_has_the_modes_of_the_original_and_new_targets_for_them(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 0)
            .hand(0, [card("Izzet Charm").unwrap(), card("Twincast").unwrap()])
            .battlefield(
                0,
                lands(3, 1)
                    .into_iter()
                    .chain([ready(card("Grizzly Bears").unwrap())]),
            )
            .battlefield(1, [ready(card("Grizzly Bears").unwrap())])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Izzet Charm")?;
    driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("{:?}", driver.decision()?.kind.name());
    };
    let owned_by = |seat: usize| {
        *options
            .legal
            .iter()
            .find(|t| {
                t.object()
                    .is_some_and(|o| driver.state().object(o.id).unwrap().controller() == pid(seat))
            })
            .unwrap()
    };
    let (theirs, mine) = (owned_by(1), owned_by(0));
    driver.choose_targets(&[theirs])?;
    driver.pay_tapping(&["Island", "Mountain"], 0)?;
    twincast(&mut driver, "Izzet Charm")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, true)?;
    driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![mine])))?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.choices().modes, [1]);
    assert_eq!(copy.choices().targets, vec![vec![mine]]);
    run(&mut driver, seats)?;
    assert_eq!(
        names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|name| *name == "Grizzly Bears")
            .count(),
        0,
        "the spell killed their Bears and the copy mine"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_an_adventure_is_the_adventure_and_the_card_goes_on_an_adventure_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 2, 2).hand(
            0,
            [
                card("Bonecrusher Giant // Stomp").unwrap(),
                card("Twincast").unwrap(),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.begin_cast("Bonecrusher Giant")?;
    common::casting::choose_way(&mut driver, "Stomp")?;
    driver.choose_targets(&[foe])?;
    driver.pay_tapping(&["Mountain", "Mountain"], 0)?;
    twincast(&mut driver, "Stomp")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.card().name, "Stomp");
    assert_eq!(
        copy.whole_card().name,
        "Stomp",
        "a copy of one face is that face alone"
    );
    assert!(
        copy.base_types()
            .contains(mtg_core::card::CardType::Instant)
    );
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 16);
    assert_eq!(
        names(driver.state(), Zone::Exile),
        ["Bonecrusher Giant"],
        "the card is exiled on an adventure once and its copy is not a card"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_split_half_is_that_half(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 3, 1)
            .hand(0, [card("Fire // Ice").unwrap(), card("Twincast").unwrap()])
            .library(0, (0..6).map(|_| island()))
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.begin_cast("Fire // Ice")?;
    common::casting::choose_way(&mut driver, "Ice")?;
    let land = driver.id_of("Mountain")?;
    let target = Target::Object(driver.state().object(land).unwrap().reference());
    driver.choose_targets(&[target])?;
    driver.pay_tapping(&["Island", "Mountain"], 0)?;
    twincast(&mut driver, "Ice")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copy = driver.state().object(spell_copies(&driver)[0]).unwrap();
    assert_eq!(copy.card().name, "Ice");
    assert_eq!(copy.face(), None);
    run(&mut driver, seats)?;
    assert_eq!(hand(&driver, 0), 2, "Ice and its copy each drew a card");
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Twincast", "Fire // Ice"],
        "the card is in the graveyard once, as both halves"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_triggers_nothing_that_watches_a_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &["Lightning Bolt", "Twincast"], 0, 0).battlefield(
            0,
            lands(2, 1)
                .into_iter()
                .chain([ready(card("Monastery Swiftspear").unwrap())]),
        )
    };
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    let power = |driver: &Driver| {
        let id = driver.id_of("Monastery Swiftspear").unwrap();
        characteristics(driver.state(), id).unwrap().power.unwrap()
    };
    driver.cast_at("Lightning Bolt", &[foe])?;
    let spell = on_the_stack(&driver, "Lightning Bolt");
    driver.cast_at("Twincast", &[spell])?;
    run(&mut driver, seats)?;
    say(&mut driver, false)?;
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 14);
    assert_eq!(
        power(&driver),
        3,
        "prowess saw the two spells that were cast and not the copy"
    );
    Ok(())
}

#[rstest]
fn a_countered_copy_ceases_to_exist_and_the_original_resolves(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &["Lightning Bolt", "Twincast"], 2, 1)
            .battlefield(1, lands(2, 0))
            .hand(1, [card("Counterspell").unwrap()])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    cast_and_copy(&mut driver, "Lightning Bolt", &[foe], "Twincast")?;
    run(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copy = spell_copies(&driver)[0];
    let reference = driver.state().object(copy).unwrap().reference();
    // The player holding Counterspell gets priority after the active player.
    driver.pass()?;
    for _ in 1..seats {
        if driver.decision()?.player == pid(1) {
            break;
        }
        driver.pass()?;
    }
    assert_eq!(driver.decision()?.player, pid(1));
    driver.cast_at("Counterspell", &[Target::Object(reference)])?;
    run(&mut driver, seats)?;
    assert!(driver.state().has_ceased(copy), "the copy is gone");
    assert_eq!(life(&driver, 1), 17, "the original resolved alone");
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Twincast", "Lightning Bolt"],
        "Twincast and the Bolt, and nothing for the copy"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_copy_is_a_copy_of_the_spell_with_the_same_choices(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 4, 1).hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Twincast").unwrap(),
                card("Twincast").unwrap(),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe])?;
    twincast(&mut driver, "Lightning Bolt")?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let first = spell_copies(&driver)[0];
    let reference = driver.state().object(first).unwrap().reference();
    driver.begin_cast("Twincast")?;
    driver.choose_targets(&[Target::Object(reference)])?;
    driver.pay_tapping(&["Island", "Island"], 0)?;
    pass_round(&mut driver, seats)?;
    say(&mut driver, false)?;
    let copies = spell_copies(&driver);
    assert_eq!(copies.len(), 2);
    let second = driver.state().object(copies[1]).unwrap();
    assert_eq!(second.copied_from(), Some(reference));
    assert_eq!(second.card().name, "Lightning Bolt");
    assert_eq!(
        second.choices(),
        driver.state().object(first).unwrap().choices()
    );
    run(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 11, "three Bolts");
    assert!(copies.iter().all(|copy| driver.state().has_ceased(*copy)));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_copy_of_a_spell_with_buyback_does_not_return_to_a_hand(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 2, 7).hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Reiterate").unwrap(),
                card("Twincast").unwrap(),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    cast_x_at(&mut driver, "Lightning Bolt", None, &[foe], &["Mountain"])?;
    let bolt = on_the_stack(&driver, "Lightning Bolt");
    driver.begin_cast("Reiterate")?;
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))?;
    driver.choose_targets(&[bolt])?;
    driver.pay_tapping(&["Mountain"; 6], 0)?;
    twincast(&mut driver, "Reiterate")?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(
        life(&driver, 1),
        11,
        "the Bolt and the two copies of it, one by Reiterate and one by the copy of Reiterate"
    );
    assert_eq!(
        names(driver.state(), Zone::Hand(pid(0))),
        ["Reiterate"],
        "the card came back once"
    );
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        ["Twincast", "Lightning Bolt"]
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn dualcaster_mage_copies_the_spell_when_it_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 4).hand(
            0,
            [
                card("Lightning Bolt").unwrap(),
                card("Dualcaster Mage").unwrap(),
            ],
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe])?;
    driver.cast("Dualcaster Mage")?;
    let casts_before = casts(&driver);
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(casts(&driver), casts_before, "the copy is not cast");
    assert_eq!(life(&driver, 1), 14);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Dualcaster Mage".to_owned()));
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn dual_casting_gives_a_creature_the_ability_to_copy_a_spell_for_a_mana_and_a_tap(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        game(seats, &[], 0, 0)
            .hand(0, [card("Lightning Bolt").unwrap()])
            .battlefield(
                0,
                lands(0, 2).into_iter().chain([
                    ready(card("Grizzly Bears").unwrap()),
                    attached_to(ready(card("Dual Casting").unwrap()), 0, 2),
                ]),
            )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let foe = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[foe])?;
    let bolt = on_the_stack(&driver, "Lightning Bolt");
    driver.begin_activation("Grizzly Bears")?;
    driver.choose_targets(&[bolt])?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    keep_targets_until_done(&mut driver, seats)?;
    assert_eq!(life(&driver, 1), 14);
    let activations = driver
        .engine()
        .events()
        .iter()
        .filter(|e| matches!(e, mtg_engine::Event::AbilityActivated { .. }))
        .count();
    assert_eq!(activations, 1);
    replays(&driver, &setup);
    Ok(())
}
