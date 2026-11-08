//! Goal: a stackless zone-change trigger that only forgets remembered objects prints nothing and is left out, while one
//! that does anything else stays a named gap.
use rstest::rstest;

use super::{abilities, gaps};

fn card(trigger: &str, svars: &str) -> String {
    format!("Name:T\nTypes:Creature Elf\nT:{trigger}\n{svars}\n")
}

#[rstest]
#[case::cleanup(
    "Mode$ ChangesZone | Origin$ Battlefield | ValidCard$ Card.Self | Destination$ Any | Execute$ DBCleanup | Static$ True",
    "SVar:DBCleanup:DB$ Cleanup | ClearRemembered$ True | ClearImprinted$ True"
)]
#[case::forget(
    "Mode$ ChangesZone | Origin$ Exile | Destination$ Any | Static$ True | ValidCard$ Card.IsImprinted+ExiledWithSource | Execute$ DBForget",
    "SVar:DBForget:DB$ Pump | ForgetImprinted$ TriggeredCard"
)]
#[case::chain(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ A | Static$ True",
    "SVar:A:DB$ Pump | ForgetObjects$ TriggeredCard | SubAbility$ B\nSVar:B:DB$ Cleanup | ClearRemembered$ True"
)]
fn a_forgetting_trigger_is_left_out(#[case] trigger: &str, #[case] svars: &str) {
    let script = card(trigger, svars);
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script).len(), 0);
}

#[rstest]
#[case::counts(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ A | Static$ True",
    "SVar:A:DB$ StoreSVar | SVar$ Lands | Type$ Number | Expression$ 0"
)]
#[case::remembers(
    "Mode$ ChangesZone | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ A | Static$ True",
    "SVar:A:DB$ Pump | RememberObjects$ Valid Card.AttachedBy"
)]
#[case::real_effect_after_forgetting(
    "Mode$ ChangesZone | Origin$ Battlefield | Destination$ Any | ValidCard$ Card.Self | Execute$ A | Static$ True",
    "SVar:A:DB$ Cleanup | ClearRemembered$ True | SubAbility$ B\nSVar:B:DB$ Draw | NumCards$ 1"
)]
fn a_stackless_trigger_that_does_more_is_not_left_out(#[case] trigger: &str, #[case] svars: &str) {
    let script = card(trigger, svars);
    assert_eq!(abilities(&script).len(), 0);
    assert_eq!(gaps(&script).len(), 1, "{:?}", gaps(&script));
}
