//! Goal: "can't attack during its controller's next turn" writes back from `Continuous { CantAttack }` lasting `DuringItsControllersNextTurn`, for the creature of the event and for one target; another restriction, another duration and a group write nothing of it.
use super::*;

/// A block trigger and a spell with the restriction on `affected`, for `mods` and `duration`.
fn shapes(affected: &str, mods: &str, duration: &str) -> [String; 2] {
    let effect = format!(
        r#"{{"Continuous":{{"affected":{affected},"mods":{mods},"duration":"{duration}"}}}}"#
    );
    [
        format!(
            r#"{{"Triggered":{{"when":{{"BecomesBlocked":{{"what":[{{"Type":"Creature"}}],"by":["This"]}}}},"effects":[{effect}]}}}}"#
        ),
        format!(
            r#"{{"Spell":{{"targets":[{{"what":{{"Object":[{{"Type":"Creature"}}]}}}}],"effects":[{effect}]}}}}"#
        ),
    ]
}

#[rstest]
#[case::wall_of_dust(
    0,
    r#""Triggering""#,
    "Whenever ~ blocks a creature, that creature can't attack during its controller's next turn."
)]
#[case::target(
    1,
    r#"{"Target":0}"#,
    "Target creature can't attack during its controller's next turn."
)]
fn l294_turn_window_writes_back(#[case] shape: usize, #[case] affected: &str, #[case] text: &str) {
    let json = &shapes(
        affected,
        r#"["CantAttack"]"#,
        "DuringItsControllersNextTurn",
    )[shape];
    let written = render_variants(&ability(json));
    assert!(written.iter().any(|w| w == text), "{written:?}");
}

#[rstest]
#[case::cant_block(r#""Triggering""#, r#"["CantBlock"]"#, "DuringItsControllersNextTurn")]
#[case::both(
    r#""Triggering""#,
    r#"["CantAttack","CantBlock"]"#,
    "DuringItsControllersNextTurn"
)]
#[case::until_your_next_turn(r#""Triggering""#, r#"["CantAttack"]"#, "UntilYourNextTurn")]
#[case::a_group(
    r#"{"Each":[{"Type":"Creature"}]}"#,
    r#"["CantAttack"]"#,
    "DuringItsControllersNextTurn"
)]
fn l294_turn_window_near_misses_write_nothing(
    #[case] affected: &str,
    #[case] mods: &str,
    #[case] duration: &str,
) {
    for json in shapes(affected, mods, duration) {
        let written = render_variants(&ability(&json));
        assert!(
            written
                .iter()
                .all(|w| !w.contains("during its controller's next turn")),
            "{written:?}"
        );
    }
}
