//! Goal: the rmt2 writers read back a countered spell put into its owner's library, a prevention shield divided among targets, permanents of a target player held down through their next untap step and protection from a quality or from the color of your choice, a choice of two kinds to destroy, a target player who can't play lands and damage of a target spell dealt to its controller exactly as printed, and a near miss (another place, amount, player, quality or order) is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common::{printed, token_sources};
use mtg_cards::Caps;
use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check};
use rstest::rstest;

const LAPSE: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Shield":{"when":{"Countered":{"what":[{"Is":{"Target":0}},"Spell"]}},"by":{"IntoLibrary":{"place":"PLACE"}},"duration":"UntilEndOfTurn"}},{"Counter":{"what":{"Target":0}}}]}}]}]}"#;
const REMEDY: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":0,"max":null},"divide":AMOUNT}],"effects":[{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventCounted":"Divided"},"duration":"UntilEndOfTurn"}}]}}]}]}"#;
const VAPORS: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"WHO"}],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"KIND"},{"ControlledBy":{"Target":0}}]},"mods":["DoesntUntap"],"duration":"NextUntapStep"}}]}}]}]}"#;
const BARRIER: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":{"Object":[{"ControlledBy":"You"}]}}],"effects":[{"Either":{"first":[FIRST],"second":[SECOND]}}]}}]}]}"#;
const ARTIFACTS: &str = r#"{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":{"Protection":[{"Type":"Artifact"}]}}],"duration":"UntilEndOfTurn"}}"#;
const CREATURES: &str = r#"{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":{"Protection":[{"Type":"Creature"}]}}],"duration":"UntilEndOfTurn"}}"#;
const CATASTROPHE: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"Either":{"first":[{"Destroy":{"what":{"Each":[{"Type":"Land"}]}}}],"second":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":REGEN}}]}}]}}]}]}"#;
const TURF: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"WHO"}],"effects":[{"Rule":{"rule":{"Cant":{"who":{"Target":0},"action":"PlayLand"}},"duration":"UntilEndOfTurn"}},{"Draw":{}}]}}]}]}"#;
const REVERB: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Sorcery"}]}}],"effects":[{"Shield":{"when":{"Damage":{"source":[{"Is":{"Target":0}}]}},"by":{"Redirect":{"to":{"ControllerOf":{"Target":0}},"next":NEXT}},"duration":"UntilEndOfTurn"}}]}}]}]}"#;
const HYMN: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"PutCounters":{"on":"This","counter":{"Other":"verse"}}}]}}]}},{"Activated":{"cost":["SacrificeSelf"],"targets":[{"what":"AnyTarget","bounds":{"min":0,"max":null},"divide":{"Counters":{"on":"This","counter":{"Other":"KIND"}}}}],"effects":[{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventCounted":"Divided"},"duration":"UntilEndOfTurn"}}]}}]}]}"#;
const POLLEN: &str = r#"{"faces":[{"abilities":[{"Keyword":{"Kicker":[{"Sacrifice":{"filter":[{"Type":"Land"}]}}]}},{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":0,"max":null},"divide":{"When":{"condition":"Kicked","then":KICKED,"otherwise":3}}}],"effects":[{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventCounted":"Divided"},"duration":"UntilEndOfTurn"}}]}}]}]}"#;
const TENDRILS: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"DealDamage":{"source":{"Target":0},"to":{"Target":1},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":[{"Is":{"Target":WHICH}}],"from":"Battlefield","to":"Graveyard"}},"this_turn":true,"effects":[{"GainLife":{"amount":2}}]}}]}}]}]}"#;
const COLOR: &str = r#"{"Continuous":{"affected":{"Target":0},"mods":["AddProtectionFromChosenColor"],"duration":"UntilEndOfTurn"}}"#;

fn verdict(name: &str, rules: &str) -> Verdict {
    let cards = printed();
    let names: BTreeSet<String> = cards.iter().map(|card| card.name.to_lowercase()).collect();
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let card = cards.iter().find(|card| card.name == name).unwrap();
    check(rules, card, &tokens, &names, &Caps::everything()).verdict()
}

#[rstest]
#[case::lapse_on_top("Memory Lapse", LAPSE.replace("PLACE", "Top"), true)]
#[case::lapse_not_on_the_bottom("Memory Lapse", LAPSE.replace("PLACE", "Bottom"), false)]
#[case::lapse_not_shuffled("Lapse of Certainty", LAPSE.replace("PLACE", "Shuffled"), false)]
#[case::remedy_five("Remedy", REMEDY.replace("AMOUNT", "5"), true)]
#[case::remedy_not_four("Remedy", REMEDY.replace("AMOUNT", "4"), false)]
#[case::vapors_lands_of_a_player("Mana Vapors", VAPORS.replace("WHO", "Player").replace("KIND", "Land"), true)]
#[case::vapors_not_of_an_opponent("Mana Vapors", VAPORS.replace("WHO", "Opponent").replace("KIND", "Land"), false)]
#[case::vapors_not_creatures("Mana Vapors", VAPORS.replace("WHO", "Player").replace("KIND", "Creature"), false)]
#[case::misstep_that_players_step("Misstep", VAPORS.replace("WHO", "Player").replace("KIND", "Creature"), true)]
#[case::barrier_artifacts_or_color("Razor Barrier", BARRIER.replace("FIRST", ARTIFACTS).replace("SECOND", COLOR), true)]
#[case::barrier_not_creatures("Razor Barrier", BARRIER.replace("FIRST", CREATURES).replace("SECOND", COLOR), false)]
#[case::barrier_not_the_other_order("Razor Barrier", BARRIER.replace("FIRST", COLOR).replace("SECOND", ARTIFACTS), false)]
#[case::catastrophe_without_regeneration("Catastrophe", CATASTROPHE.replace("REGEN", "false"), true)]
#[case::catastrophe_not_with_regeneration("Catastrophe", CATASTROPHE.replace("REGEN", "true"), false)]
#[case::turf_wound_target_player("Turf Wound", TURF.replace("WHO", "Player"), true)]
#[case::turf_wound_not_an_opponent("Turf Wound", TURF.replace("WHO", "Opponent"), false)]
#[case::reverberation_all_damage("Reverberation", REVERB.replace("NEXT", "false"), true)]
#[case::reverberation_not_the_next("Reverberation", REVERB.replace("NEXT", "true"), false)]
#[case::hymn_verse_counters("Serra's Hymn", HYMN.replace("KIND", "verse"), true)]
#[case::hymn_not_charge_counters("Serra's Hymn", HYMN.replace("KIND", "charge"), false)]
#[case::pollen_six_when_kicked("Pollen Remedy", POLLEN.replace("KICKED", "6"), true)]
#[case::pollen_not_five_when_kicked("Pollen Remedy", POLLEN.replace("KICKED", "5"), false)]
#[case::tendrils_the_permanent_dies("Devouring Tendrils", TENDRILS.replace("WHICH", "1"), true)]
#[case::tendrils_not_your_creature("Devouring Tendrils", TENDRILS.replace("WHICH", "0"), false)]
fn the_rmt2_writers_read_back_the_printed_clause(
    #[case] name: &str,
    #[case] rules: String,
    #[case] accepted: bool,
) {
    let found = verdict(name, &rules);
    assert_eq!(found == Verdict::Accepted, accepted, "{name}: {found:?}");
}
