//! Goal: a value chosen while an ability resolves is the color, land type or creature type that a later "becomes" step sets, only on the objects the step names.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::characteristics::characteristics;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_testkit::cards::{bear, forest, free_creature, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::translated;

/// Passes until the stack is empty again, answering a choice of options with the first one that `wanted` accepts.
pub(crate) fn settle_choosing(driver: &mut Driver, wanted: &dyn Fn(&ChoiceOption) -> bool) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::ChooseOption(o) => Answer::ChooseOption(OptionChoice(
                o.options
                    .iter()
                    .position(wanted)
                    .expect("the option is offered"),
            )),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game does not settle");
}

const DYE: &str = "Name:Dye\nManaCost:W\nTypes:Sorcery\nA:SP$ ChooseColor | Defined$ You | SubAbility$ DBA\nSVar:DBA:DB$ Animate | ValidTgts$ Creature | Colors$ ChosenColor | OverwriteColors$ True\n";

#[rstest]
#[case::red(Color::Red)]
#[case::blue(Color::Blue)]
fn a_target_becomes_the_color_chosen_and_nothing_else_does(#[case] color: Color) {
    let card = translated(DYE, "Dye", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(
            0,
            [
                ready(plains()),
                ready(bear()),
                ready(free_creature("Ogre", 3, 3)),
            ],
        )
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let target = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Dye", &[target]).unwrap();
    settle_choosing(
        &mut driver,
        &|o| matches!(o, ChoiceOption::Color(c) if *c == color),
    );

    let colors = |driver: &Driver, name: &str| {
        let id = driver.id_of(name).unwrap();
        characteristics(driver.state(), id)
            .unwrap()
            .colors
            .iter()
            .collect::<Vec<_>>()
    };
    assert_eq!(colors(&driver, "Grizzly Bears"), [color]);
    assert_eq!(colors(&driver, "Ogre"), Vec::<Color>::new());
}

const FLASK: &str = "Name:Flask\nManaCost:W\nTypes:Sorcery\nA:SP$ ChooseType | Defined$ You | Type$ Basic Land | SubAbility$ DBA\nSVar:DBA:DB$ AnimateAll | ValidCards$ Land.YouCtrl | Types$ ChosenType | RemoveLandTypes$ True\n";

#[test]
fn every_land_you_control_becomes_the_basic_type_chosen_and_theirs_do_not() {
    let card = translated(FLASK, "Flask", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(plains()), ready(forest())])
        .battlefield(1, [ready(forest())])
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Flask").unwrap();
    settle_choosing(&mut driver, &|o| format!("{o:?}").contains("Island"));

    let state = driver.state();
    let kinds: Vec<(usize, Vec<String>)> = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .map(|id| {
            let chars = characteristics(state, *id).unwrap();
            let mut names: Vec<String> = chars.subtypes.iter().map(|s| s.0.clone()).collect();
            names.sort();
            (state.object(*id).unwrap().controller().index(), names)
        })
        .collect();
    let mine: Vec<_> = kinds.iter().filter(|(seat, _)| *seat == 0).collect();
    assert!(
        mine.iter().all(|(_, names)| names == &["Island"]),
        "{kinds:?}"
    );
    let theirs: Vec<_> = kinds.iter().filter(|(seat, _)| *seat == 1).collect();
    assert!(
        theirs.iter().all(|(_, names)| names != &["Island"]),
        "{kinds:?}"
    );
}
