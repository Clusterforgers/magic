//! Goal: a tap held down while you control the source (one target or several), the tap of a target player's creatures held through that player's next untap step, "those creatures" held through the next untap step after up to X targets, and "~ enters tapped and doesn't untap during your untap step" read into the tap and the rule over the same permanents (or the replacement and the static), each written back as the card's text; other players, steps and durations stay with the table.
use super::*;
use crate::compile::read::{Own, static_pair, static_pair_texts};
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

fn c10_uc_effects(read: Ability) -> Vec<Effect> {
    match read {
        Ability::Triggered { effects, .. } | Ability::Spell { effects, .. } => effects,
        other => panic!("no effects: {other:?}"),
    }
}

#[rstest]
#[case::one_target(
    "When this creature enters, tap target creature an opponent controls. That creature doesn't untap during its controller's untap step for as long as you control this creature.",
    ClauseKind::Triggered,
    Duration::WhileYouControlSource,
    "{\"Target\":0}"
)]
#[case::up_to_four_targets(
    "When this creature becomes monstrous, tap up to four target creatures. Those creatures don't untap during their controllers' untap steps for as long as you control this creature.",
    ClauseKind::Triggered,
    Duration::WhileYouControlSource,
    "{\"Target\":0}"
)]
#[case::a_target_players_creatures(
    "Tap all creatures target player controls. Those creatures don't untap during that player's next untap step.",
    ClauseKind::Spell,
    Duration::NextUntapStep,
    "{\"Each\":[{\"Type\":\"Creature\"},{\"ControlledBy\":{\"Target\":0}}]}"
)]
#[case::up_to_x_targets(
    "Whenever this creature mutates, tap up to X target creatures, where X is the number of times this creature has mutated. Those creatures don't untap during their controller's next untap step.",
    ClauseKind::Triggered,
    Duration::NextUntapStep,
    "{\"Target\":0}"
)]
fn c10_uc_the_tapped_permanents_are_held_down(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] lasts: Duration,
    #[case] held: &str,
) {
    let effects = c10_uc_effects(golden(text, kind, false));
    let [
        Effect::Tap { what, bind: _ },
        Effect::Continuous {
            affected,
            mods,
            duration,
        },
    ] = effects.as_slice()
    else {
        panic!("{text}: {effects:?}");
    };
    assert_eq!(what, affected, "{text}");
    assert_eq!(mods, &[Modification::DoesntUntap], "{text}");
    assert_eq!(*duration, lasts, "{text}");
    let held: Ref = serde_json::from_str(held).unwrap_or_else(|error| panic!("{held}: {error}"));
    assert_eq!(*what, held, "{text}");
}

#[rstest]
#[case::the_next_untap_step_while_you_control(
    "Tap target creature. It doesn't untap during its controller's next untap step for as long as you control this creature."
)]
#[case::your_untap_step(
    "Tap target creature. That creature doesn't untap during your untap step for as long as you control this creature."
)]
#[case::another_players_step(
    "Tap all creatures target player controls. Those creatures don't untap during your next untap step."
)]
#[case::all_creatures(
    "Tap all creatures. Those creatures don't untap during that player's next untap step."
)]
#[case::untapped_instead(
    "Untap all creatures target player controls. Those creatures don't untap during that player's next untap step."
)]
fn c10_uc_other_holds_stay_with_the_table(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    assert!(
        read.is_none_or(|ability| !format!("{ability:?}").contains("DoesntUntap")),
        "{text}"
    );
}

#[rstest]
#[case::enters_tapped_and_stays("~ enters tapped and doesn't untap during your untap step.", true)]
#[case::its_controllers_step(
    "~ enters tapped and doesn't untap during its controller's untap step.",
    false
)]
#[case::untapped("~ enters untapped and doesn't untap during your untap step.", false)]
fn c10_uc_a_card_that_enters_tapped_and_stays_tapped(#[case] text: &str, #[case] read: bool) {
    let found = static_pair(text, false, &Own::default());
    assert_eq!(found.is_some(), read, "{text}: {found:?}");
    let Some(found) = found else { return };
    let [first, second] = found.as_slice() else {
        panic!("{text}: {found:?}");
    };
    assert!(matches!(first, Ability::Replacement { .. }), "{text}");
    assert!(
        matches!(second, Ability::Static { affected: Ref::This, mods, .. } if mods == &[Modification::DoesntUntap]),
        "{text}"
    );
    assert!(
        static_pair_texts(first, second)
            .iter()
            .any(|written| same_text(written, text)),
        "{text}"
    );
}
