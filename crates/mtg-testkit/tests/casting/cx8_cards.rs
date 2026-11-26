//! Goal: the cx8 cards play whole at two to four seats: Dack Fayden, Helping Hand reveals until one creature card per
//! opponent, gives each opponent a different one, goaded for the rest of the game, and copes with a library short of
//! creatures; Celestial Reunion puts the card found onto the battlefield only when two creatures of the chosen type were
//! beheld and the card is of that type; Q, Stacking the Deck gives the nonland card it exiles an ability that copies it
//! from exile on its owner's turn; Massimo, the Magician exiles a one-mana instant or sorcery as a creature enters and that
//! creature casts a copy of it free on each hit; each game replays to its state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::modify::Duration;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{CardsChoice, ChoiceOption, OptionChoice, TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::effects::{Modification, Selector};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const DACK: &str = "Dack Fayden, Helping Hand";
const REUNION: &str = "Celestial Reunion";

fn pid(seat: usize) -> PlayerId {
    PlayerId::from_index(seat)
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

/// Answers every question until the stack is empty and seat 0 has priority again: mana is paid automatically, `pick`
/// answers first, the first legal targets are taken and every yes-or-no is yes.
fn settle(driver: &mut Driver, pick: &dyn Fn(&Driver, &Decision) -> Option<Answer>) {
    for _ in 0..300 {
        let decision = driver.decision().unwrap().clone();
        if std::env::var("CX8_TRACE").is_ok() {
            let text = format!("{:?}", decision.kind);
            eprintln!("{}", &text[..text.len().min(300)]);
        }
        if let Some(answer) = pick(driver, &decision) {
            driver.answer(answer).unwrap();
            continue;
        }
        let answer = match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().resolving().is_none()
                    && decision.player == pid(0) =>
            {
                return;
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
            DecisionKind::ChooseTargets(options) => Answer::ChooseTargets(TargetChoice::Targets(
                options
                    .legal
                    .iter()
                    .copied()
                    .take(options.min.max(1) as usize)
                    .collect(),
            )),
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game did not settle");
}

fn named(driver: &Driver, zone: Zone, name: &str) -> Vec<ObjectId> {
    driver
        .state()
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| crate::common::name_of(driver.state(), *id) == name)
        .collect()
}

fn controller(driver: &Driver, id: ObjectId) -> PlayerId {
    mtg_engine::characteristics::controller_of(driver.state(), id).unwrap()
}

/// Whether a goad without end, begun by seat 0, holds the permanent.
fn goaded_for_good(driver: &Driver, id: ObjectId) -> bool {
    driver.state().effects().iter().any(|effect| {
        matches!(effect.modification, Modification::Goaded { by } if by == pid(0))
            && effect.duration == Duration::Forever
            && matches!(&effect.selector, Selector::Objects(objects) if objects.iter().any(|o| o.id == id))
    })
}

/// Seat 0 holds Dack with six Plains; its library holds `creatures` Wolves among Forests.
fn dack_table(seats: usize, creatures: usize) -> Driver {
    let mut library: Vec<Arc<CardDefinition>> = Vec::new();
    for at in 0..12 {
        library.push(if at % 2 == 1 && at / 2 < creatures {
            free_creature("Wolf", 2, 2)
        } else {
            forest()
        });
    }
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, (0..6).map(|_| ready(plains())).collect::<Vec<_>>())
        .hand(0, [card(DACK).unwrap()])
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Answers the opponent choice with the last opponent offered, else the first.
fn opponent_at(last: bool) -> impl Fn(&Driver, &Decision) -> Option<Answer> {
    move |_, decision| match &decision.kind {
        DecisionKind::ChooseOption(options)
            if options
                .options
                .iter()
                .all(|o| matches!(o, ChoiceOption::Player(_))) =>
        {
            Some(Answer::ChooseOption(OptionChoice(if last {
                options.options.len() - 1
            } else {
                0
            })))
        }
        _ => None,
    }
}

#[rstest]
fn dack_gives_each_opponent_a_different_goaded_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] last: bool,
) {
    let mut driver = dack_table(seats, 6);
    let before = driver.state().zone(Zone::Library(pid(0))).len();
    driver.begin_cast(DACK).unwrap();
    settle(&mut driver, &opponent_at(last));
    let wolves = named(&driver, Zone::Battlefield, "Wolf");
    assert_eq!(wolves.len(), seats - 1, "one creature card per opponent");
    let mut takers: Vec<PlayerId> = wolves.iter().map(|id| controller(&driver, *id)).collect();
    takers.sort();
    let expected: Vec<PlayerId> = (1..seats).map(pid).collect();
    assert_eq!(takers, expected, "each opponent takes exactly one");
    assert!(wolves.iter().all(|id| goaded_for_good(&driver, *id)));
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        before - (seats - 1),
        "the revealed lands are shuffled back"
    );
    replays(&driver);
    driver.idle_until(3, Step::Main1).unwrap();
    let wolves = named(&driver, Zone::Battlefield, "Wolf");
    assert!(
        wolves.iter().all(|id| goaded_for_good(&driver, *id)),
        "the goad outlasts the goader's next turn"
    );
}

#[rstest]
fn dack_with_fewer_creatures_than_opponents_gives_what_it_found(
    #[values(3, 4)] seats: usize,
    #[values(0, 1)] creatures: usize,
) {
    let mut driver = dack_table(seats, creatures);
    driver.begin_cast(DACK).unwrap();
    settle(&mut driver, &opponent_at(true));
    let wolves = named(&driver, Zone::Battlefield, "Wolf");
    assert_eq!(wolves.len(), creatures, "the whole library is revealed");
    for id in &wolves {
        assert_eq!(
            controller(&driver, *id),
            pid(seats - 1),
            "the chosen opponent takes it"
        );
    }
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        12 - creatures
    );
    replays(&driver);
}

/// Seat 0 holds Celestial Reunion with Forests and controls `elves` Elves; its library holds an Elf and a Wolf of mana
/// value two.
fn reunion_table(seats: usize, elves: usize) -> Driver {
    let mut own: Vec<_> = (0..4).map(|_| ready(forest())).collect();
    own.extend((0..elves).map(|_| ready(card("Llanowar Elves").unwrap())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own)
        .hand(0, [card(REUNION).unwrap()])
        .library(
            0,
            vec![
                forest(),
                card("Elvish Visionary").unwrap(),
                card("Grizzly Bears").unwrap(),
                forest(),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Casts the Reunion for X = 2: pays the added cost with the Elf type when `pay`, and finds the card `find`.
fn reunion_answers(pay: bool, find: &'static str) -> impl Fn(&Driver, &Decision) -> Option<Answer> {
    move |driver, decision| match &decision.kind {
        DecisionKind::ChooseNumber(_) => Some(Answer::ChooseNumber(
            mtg_engine::choose::NumberChoice::Number(2),
        )),
        DecisionKind::YesNo(_) => Some(Answer::YesNo(YesNoChoice(pay))),
        DecisionKind::ChooseCosts(options) => Some(Answer::ChooseCosts(
            mtg_engine::choose::CostsChoice::Costs(if pay {
                (0..options.optional.len()).collect()
            } else {
                Vec::new()
            }),
        )),
        DecisionKind::ChooseOption(options) => options
            .options
            .iter()
            .position(|o| *o == ChoiceOption::Label("Elf".to_owned()))
            .map(|at| Answer::ChooseOption(OptionChoice(at))),
        DecisionKind::ChooseCards(options)
            if options.from.iter().any(|id| {
                driver
                    .state()
                    .object(*id)
                    .is_some_and(|o| o.zone() == Zone::Library(pid(0)))
            }) =>
        {
            let found = options
                .from
                .iter()
                .copied()
                .find(|id| crate::common::name_of(driver.state(), *id) == find)
                .unwrap();
            Some(Answer::ChooseCards(CardsChoice(vec![found])))
        }
        _ => None,
    }
}

#[rstest]
#[case::beheld_elf_enters(2, true, "Elvish Visionary", Zone::Battlefield)]
#[case::beheld_other_type_to_hand(2, true, "Grizzly Bears", Zone::Hand(PlayerId::from_index(0)))]
#[case::unpaid_elf_to_hand(2, false, "Elvish Visionary", Zone::Hand(PlayerId::from_index(0)))]
#[case::one_elf_cannot_behold(1, true, "Elvish Visionary", Zone::Hand(PlayerId::from_index(0)))]
fn celestial_reunion_puts_the_chosen_type_onto_the_battlefield_when_beheld(
    #[values(2, 3, 4)] seats: usize,
    #[case] elves: usize,
    #[case] pay: bool,
    #[case] find: &'static str,
    #[case] lands_in: Zone,
) {
    let mut driver = reunion_table(seats, elves);
    driver.begin_cast(REUNION).unwrap();
    settle(&mut driver, &reunion_answers(pay, find));
    assert_eq!(
        named(&driver, lands_in, find).len(),
        1,
        "{find} is in {lands_in:?}"
    );
    assert_eq!(named(&driver, Zone::Graveyard(pid(0)), REUNION).len(), 1);
    let drawn = usize::from(lands_in == Zone::Battlefield);
    assert_eq!(
        driver.state().zone(Zone::Library(pid(0))).len(),
        3 - drawn,
        "the Visionary that entered drew"
    );
    replays(&driver);
}

const MIMEOPLASM: &str = "The Mimeoplasm";

/// Seat 0 holds The Mimeoplasm with its mana; the graveyards hold a 4/4 Giant (seat 0) and Elvish Visionary (last seat).
fn mimeoplasm_table(seats: usize) -> Driver {
    let lands = vec![
        ready(mtg_testkit::cards::swamp()),
        ready(forest()),
        ready(mtg_testkit::cards::island()),
        ready(forest()),
        ready(forest()),
    ];
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(0, [card(MIMEOPLASM).unwrap()])
        .graveyard(0, [free_creature("Giant", 4, 4)])
        .graveyard(seats - 1, [card("Elvish Visionary").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// Exiles both graveyard cards when `exile` and copies the one named `copy`.
fn mimeoplasm_answers(
    exile: bool,
    copy: &'static str,
) -> impl Fn(&Driver, &Decision) -> Option<Answer> {
    move |driver, decision| match &decision.kind {
        DecisionKind::ChooseCards(options) if options.max == 2 => {
            Some(Answer::ChooseCards(CardsChoice(if exile {
                options.from.clone()
            } else {
                Vec::new()
            })))
        }
        DecisionKind::ChooseCards(options) if options.from.len() == 2 => {
            let found = options
                .from
                .iter()
                .copied()
                .find(|id| crate::common::name_of(driver.state(), *id) == copy)
                .unwrap();
            Some(Answer::ChooseCards(CardsChoice(vec![found])))
        }
        _ => None,
    }
}

fn plus_counters(driver: &Driver, id: ObjectId) -> u32 {
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne)
}

#[rstest]
#[case::the_giant_with_the_elfs_power("Giant", 1, 0)]
#[case::the_elf_with_the_giants_power("Elvish Visionary", 4, 1)]
fn the_mimeoplasm_enters_as_a_copy_of_one_exiled_card_with_the_others_power(
    #[values(2, 3, 4)] seats: usize,
    #[case] copy: &'static str,
    #[case] counters: u32,
    #[case] drawn: usize,
) {
    let mut driver = mimeoplasm_table(seats);
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast(MIMEOPLASM).unwrap();
    settle(&mut driver, &mimeoplasm_answers(true, copy));
    let copies = named(&driver, Zone::Battlefield, copy);
    assert_eq!(copies.len(), 1, "it entered as {copy}");
    assert_eq!(plus_counters(&driver, copies[0]), counters);
    assert_eq!(
        driver.state().zone(Zone::Exile).len(),
        2,
        "both cards are exiled"
    );
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len(),
        hand - 1 + drawn,
        "a copied enters trigger works"
    );
    replays(&driver);
}

#[rstest]
fn the_mimeoplasm_declined_enters_as_itself_and_dies(#[values(2, 3, 4)] seats: usize) {
    let mut driver = mimeoplasm_table(seats);
    driver.begin_cast(MIMEOPLASM).unwrap();
    settle(&mut driver, &mimeoplasm_answers(false, "Giant"));
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert_eq!(
        named(&driver, Zone::Graveyard(pid(0)), MIMEOPLASM).len(),
        1,
        "a 0/0 dies"
    );
    replays(&driver);
}

const Q: &str = "Q, Stacking the Deck";

/// Seat 0 holds Q, a Forest and Opt with mana for Q and a Bear; its library is Forests and one Grizzly Bears.
fn q_table(seats: usize) -> Driver {
    let mut lands: Vec<_> = (0..4)
        .map(|_| ready(mtg_testkit::cards::mountain()))
        .collect();
    lands.extend((0..2).map(|_| ready(forest())));
    let mut library = vec![forest(); 8];
    library.insert(3, card("Grizzly Bears").unwrap());
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(0, [card(Q).unwrap(), card("Opt").unwrap()])
        .library(0, library);
    for seat in 1..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn q_gives_the_exiled_card_a_copy_ability_on_its_owners_turn(#[values(2, 3, 4)] seats: usize) {
    let mut driver = q_table(seats);
    driver.begin_cast(Q).unwrap();
    settle(&mut driver, &|_, _| None);
    let exiled = named(&driver, Zone::Exile, "Grizzly Bears");
    assert_eq!(exiled.len(), 1, "the one nonland card is exiled");
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 8);
    assert_eq!(
        driver.offered_abilities("Grizzly Bears").unwrap().len(),
        1,
        "the exiled card has the ability on its owner's turn"
    );
    driver.begin_activation("Grizzly Bears").unwrap();
    settle(&mut driver, &|_, _| None);
    assert_eq!(
        named(&driver, Zone::Battlefield, "Grizzly Bears").len(),
        1,
        "the copy was cast"
    );
    assert_eq!(
        named(&driver, Zone::Exile, "Grizzly Bears").len(),
        1,
        "the card stays exiled"
    );
    assert!(
        named(&driver, Zone::Exile, "Opt").len() == 1,
        "the hand card paid the cost"
    );
    replays(&driver);
    driver.idle_until(2, Step::Main1).unwrap();
    while driver.decision().unwrap().player != pid(0) {
        driver.answer_passively().unwrap();
    }
    assert!(
        driver
            .offered_abilities("Grizzly Bears")
            .unwrap()
            .is_empty(),
        "not on another player's turn"
    );
}

const MASSIMO: &str = "Massimo, the Magician";

/// Seat 0 controls Massimo and holds a Wolf; Counterspell and Lightning Bolt lie in its graveyard.
fn massimo_table(seats: usize) -> Driver {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, [ready(card(MASSIMO).unwrap())])
        .hand(0, [free_creature("Wolf", 2, 2)])
        .graveyard(
            0,
            [
                card("Counterspell").unwrap(),
                card("Lightning Bolt").unwrap(),
            ],
        );
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

/// The Bolt copy and every other target choice name seat 1.
fn at_seat_one(_: &Driver, decision: &Decision) -> Option<Answer> {
    match &decision.kind {
        DecisionKind::ChooseTargets(options)
            if options
                .legal
                .contains(&mtg_engine::choose::Target::Player(pid(1))) =>
        {
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![
                mtg_engine::choose::Target::Player(pid(1)),
            ])))
        }
        _ => None,
    }
}

#[rstest]
fn massimo_exiles_a_one_mana_spell_and_the_entering_creature_casts_its_copy_on_each_hit(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = massimo_table(seats);
    driver.begin_cast("Wolf").unwrap();
    settle(&mut driver, &at_seat_one);
    assert_eq!(
        named(&driver, Zone::Exile, "Lightning Bolt").len(),
        1,
        "Bolt is exiled"
    );
    assert_eq!(
        named(&driver, Zone::Graveyard(pid(0)), "Counterspell").len(),
        1,
        "a two-mana spell is no target"
    );
    let next = 1 + u32::try_from(seats).unwrap();
    for (turn, life) in [(next, 15), (next + u32::try_from(seats).unwrap(), 10)] {
        driver.idle_until(turn, Step::DeclareAttackers).unwrap();
        driver.attack(&[("Wolf", 1)]).unwrap();
        while driver.state().turn().step() != Step::End {
            let decision = driver.decision().unwrap().clone();
            let answer = at_seat_one(&driver, &decision).unwrap_or_else(|| match &decision.kind {
                DecisionKind::YesNo(_) => Answer::YesNo(YesNoChoice(true)),
                _ => Driver::passive_answer(&decision),
            });
            driver.answer(answer).unwrap();
        }
        assert_eq!(
            driver.state().player(pid(1)).unwrap().life(),
            life,
            "two damage and a Bolt copy on turn {turn}"
        );
        assert_eq!(named(&driver, Zone::Exile, "Lightning Bolt").len(), 1);
    }
    replays(&driver);
}

const EYE: &str = "Eye of Ojer Taq";

/// Seat 0 controls the Eye, six Plains, a Wastes and two of `material`, and holds Grizzly Bears.
fn eye_table(seats: usize, material: &dyn Fn() -> Arc<CardDefinition>) -> Driver {
    let mut own: Vec<_> = vec![ready(card("Eye of Ojer Taq // Apex Observatory").unwrap())];
    own.extend((0..6).map(|_| ready(plains())));
    own.push(ready(mtg_testkit::cards::wastes()));
    own.extend((0..2).map(|_| ready(material())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, own)
        .hand(0, [card("Grizzly Bears").unwrap()]);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 10]);
    }
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
#[case::two_creatures("Wolf", true)]
#[case::two_artifacts("Sol Ring", false)]
fn eye_of_ojer_taq_crafts_with_two_alike_and_the_observatory_frees_that_type(
    #[values(2, 3, 4)] seats: usize,
    #[case] material: &'static str,
    #[case] bears_free: bool,
) {
    let make = move || {
        if material == "Wolf" {
            free_creature("Wolf", 2, 2)
        } else {
            card(material).unwrap()
        }
    };
    let mut driver = eye_table(seats, &make);
    driver.begin_activation(EYE).unwrap();
    let DecisionKind::ChooseCards(options) = driver.decision().unwrap().kind.clone() else {
        panic!("the materials are asked");
    };
    let pick = |name: &str| -> Vec<ObjectId> {
        options
            .from
            .iter()
            .copied()
            .filter(|id| crate::common::name_of(driver.state(), *id) == name)
            .collect()
    };
    let alike = pick(material);
    let wastes = pick("Wastes");
    assert_eq!(alike.len(), 2);
    assert!(
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![alike[0], wastes[0]])))
            .is_err(),
        "two that share no card type are refused"
    );
    driver
        .answer(Answer::ChooseCards(CardsChoice(alike)))
        .unwrap();
    settle(&mut driver, &|_, _| None);
    let apex = named(&driver, Zone::Battlefield, "Apex Observatory");
    assert_eq!(apex.len(), 1, "the Eye returns transformed");
    assert!(
        driver
            .state()
            .object(apex[0])
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    replays(&driver);
    let next = 1 + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Main1).unwrap();
    driver.begin_activation("Apex Observatory").unwrap();
    settle(&mut driver, &|_, _| None);
    if bears_free {
        driver.begin_cast("Grizzly Bears").unwrap();
        settle(&mut driver, &|_, _| None);
        assert_eq!(
            named(&driver, Zone::Battlefield, "Grizzly Bears").len(),
            1,
            "cast free"
        );
        replays(&driver);
    } else {
        assert!(
            driver.begin_cast("Grizzly Bears").is_err(),
            "a spell of another type pays its cost, and the Plains cannot"
        );
    }
}
