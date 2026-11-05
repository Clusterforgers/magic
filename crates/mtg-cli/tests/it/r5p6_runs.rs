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

const HUSH: &str = "Name:Hush\nManaCost:W\nTypes:Enchantment\nT:Mode$ SpellCast | ValidCard$ Creature | Execute$ TC | TriggerZones$ Battlefield\nSVar:TC:DB$ Counter | Defined$ TriggeredSpellAbility\n";
const BOON: &str = "Name:Boon\nManaCost:W\nTypes:Sorcery\nA:SP$ GainLife | LifeAmount$ 3\n";

#[rstest]
#[case::creature_spell_countered("Grizzly Bears", true, 0, 20)]
#[case::creature_spell_without_the_trigger("Grizzly Bears", false, 1, 20)]
#[case::other_spell_resolves("Boon", true, 0, 23)]
fn a_cast_trigger_counters_only_the_spells_it_names(
    #[case] cast: &str,
    #[case] with_hush: bool,
    #[case] on_battlefield: usize,
    #[case] life: i32,
) {
    use mtg_core::ids::PlayerId;
    let hush = translated(HUSH, "Hush", "Enchantment", "{W}");
    let boon = translated(BOON, "Boon", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [boon, bear()])
        .battlefield(
            0,
            [ready(plains()), ready(forest()), ready(forest())]
                .into_iter()
                .chain(with_hush.then(|| ready(hush))),
        )
        .library(0, [plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast(cast).unwrap();
    crate::r3p3_runs::settle(&mut driver);
    let state = driver.state();
    let bears = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == cast))
        .count();
    assert_eq!(bears, on_battlefield);
    assert_eq!(state.player(PlayerId::from_index(0)).unwrap().life(), life);
}

const MILL_THEN_GAIN: &str = "Name:Lore\nManaCost:W\nTypes:Sorcery\nA:SP$ Mill | NumCards$ 2 | Defined$ You | Optional$ True | SubAbility$ DBG\nSVar:DBG:DB$ GainLife | LifeAmount$ 3\n";

#[rstest]
#[case::accepted(true, 2)]
#[case::declined(false, 0)]
fn the_line_after_a_may_happens_whether_or_not_the_player_agrees(
    #[case] agree: bool,
    #[case] milled: usize,
) {
    use mtg_core::ids::PlayerId;
    use mtg_engine::zones::Zone;
    let card = translated(MILL_THEN_GAIN, "Lore", "Sorcery", "{W}");
    let scenario = Scenario::new(2)
        .hand(0, [card])
        .battlefield(0, [ready(plains())])
        .library(0, [plains(), plains(), plains()])
        .library(1, [plains()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Lore").unwrap();
    for _ in 0..50 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::YesNo(_) => Answer::YesNo(mtg_engine::choose::YesNoChoice(agree)),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    let state = driver.state();
    let yard = state
        .zone(Zone::Graveyard(PlayerId::from_index(0)))
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|o| o.card().name == "Plains")
        })
        .count();
    assert_eq!(yard, milled);
    assert_eq!(state.player(PlayerId::from_index(0)).unwrap().life(), 23);
}
