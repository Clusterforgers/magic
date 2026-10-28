//! Goal: a tap and "doesn't untap during its controller's next untap step" read as the tap and a rule over the same permanents that lasts through that step, for one target and for several, each written back as the card's text; the untap step that is not the next one, and a rule that lasts otherwise, stay with the table.
use super::*;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::Ref;

fn effects_of(read: Ability) -> Vec<Effect> {
    match read {
        Ability::Triggered { effects, .. } | Ability::Spell { effects, .. } => effects,
        other => panic!("no effects: {other:?}"),
    }
}

#[rstest]
#[case::that_creature(
    "Whenever you cast an instant or sorcery spell, tap target creature an opponent controls. That creature doesn't untap during its controller's next untap step.",
    ClauseKind::Triggered
)]
#[case::it(
    "Whenever a token you control enters, tap target nonland permanent an opponent controls. It doesn't untap during its controller's next untap step.",
    ClauseKind::Triggered
)]
#[case::several_targets(
    "Tap up to two target creatures. They don't untap during their controller's next untap step.",
    ClauseKind::Spell
)]
#[case::a_spell_with_one_target(
    "Tap target creature. It doesn't untap during its controller's next untap step.",
    ClauseKind::Spell
)]
fn the_tapped_permanents_stay_tapped_through_the_next_untap_step(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    let effects = effects_of(golden(text, kind, false));
    let [
        Effect::Tap { what },
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
    assert_eq!(*duration, Duration::NextUntapStep, "{text}");
    assert_eq!(*what, Ref::Target(0), "{text}");
}

#[rstest]
#[case::not_the_next_one(
    "Tap target creature. It doesn't untap during its controller's untap step."
)]
#[case::another_player("Tap target creature. It doesn't untap during your next untap step.")]
#[case::untapped_instead(
    "Untap target creature. It doesn't untap during its controller's next untap step."
)]
#[case::all_creatures(
    "Tap all creatures. They don't untap during their controller's next untap step."
)]
fn other_wordings_are_not_held_through_the_next_untap_step(#[case] text: &str) {
    let read = clause(text, ClauseKind::Spell, false);
    assert!(
        read.is_none_or(|ability| !format!("{ability:?}").contains("NextUntapStep")),
        "{text}"
    );
}
