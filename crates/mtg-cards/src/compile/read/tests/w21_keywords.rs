//! Goal: a static ability that gives a keyword to spells, to a group or to the card itself under a condition is written back as the oracle wording, and a wrong keyword, number, player, place, condition or first-of-the-turn never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn spells(preds: &str, keyword: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{preds}]}},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
    )
}

fn first(preds: &str, filter: &str, keyword: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{preds}]}},"condition":{{"Compare":{{"left":{{"Tally":{{"event":"SpellCast","filter":[{filter},{{"Not":{{"Is":"Affected"}}}}],"who":"EachPlayer"}}}},"op":"Eq","right":0}}}},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
    )
}

fn itself(condition: &str, keyword: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","condition":{condition},"mods":[{{"AddKeyword":{keyword}}}]}}}}"#
    )
}

const CAST: &str = r#"{"ControlledBy":"You"},{"WasCast":{}},{"In":"Stack"}"#;
const SPELLS: &str = r#"{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}"#;
const HISTORIC: &str =
    r#"{"AnyOf":[{"Supertype":"Legendary"},{"Type":"Artifact"},{"Subtype":"Saga"}]}"#;

fn counters(op: &str, to: u32) -> String {
    format!(
        r#"{{"Matches":{{"selector":[{{"CounterCount":{{"kind":"+1/+1","op":"{op}","to":{to}}}}}],"subject":"This"}}}}"#
    )
}

fn tally(filter: &str, op: &str, right: u32) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Tally":{{"event":"SpellCast","filter":[{filter}],"who":"EachPlayer"}}}},"op":"{op}","right":{right}}}}}"#
    )
}

#[rstest]
#[case::convoke(&spells(&format!(r#"{{"Type":"Artifact"}},{CAST}"#), r#""Convoke""#), "Artifact spells you cast have convoke.")]
#[case::improvise(&spells(&format!(r#"{{"Not":{{"Type":"Creature"}}}},{CAST}"#), r#""Improvise""#), "Noncreature spells you cast have improvise.")]
#[case::affinity(&spells(r#"{"WasCast":{"by":"You"}},{"In":"Stack"}"#, r#"{"Affinity":[{"Type":"Artifact"}]}"#), "Spells you cast have affinity for artifacts.")]
#[case::ripple(&spells(CAST, r#"{"Ripple":4}"#), "Spells you cast have ripple 4.")]
#[case::prowl(&spells(&format!(r#"{{"Subtype":"Dinosaur"}},{CAST}"#), r#"{"Prowl":"{2}{R}"}"#), "Dinosaur spells you cast have prowl {2}{R}.")]
#[case::mana_value(&spells(r#"{"ControlledBy":"You"},{"Compare":{"op":"Ge","stat":"ManaValue","to":6}},{"WasCast":{}},{"In":"Stack"}"#, r#""Cascade""#), "Spells you cast with mana value 6 or greater have cascade.")]
#[case::from_hand(&spells(&format!(r#"{SPELLS},{{"WasCast":{{"by":"You"}}}},{{"WasCast":{{"from":"Hand"}}}},{{"OwnedBy":"You"}},{{"In":"Stack"}}"#), r#""Cascade""#), "Instant and sorcery spells you cast from your hand have cascade.")]
#[case::casualty_each(&spells(&format!(r#"{SPELLS},{{"WasCast":{{"by":"You"}}}},{{"In":"Stack"}}"#), r#"{"Casualty":1}"#), "Each instant and sorcery spell you cast has casualty 1.")]
#[case::storm(&spells(&format!(r#"{SPELLS},{{"WasCast":{{"by":"You"}}}},{{"In":"Stack"}}"#), r#""Storm""#), "Instant and sorcery spells you cast have storm.")]
#[case::controlled(&spells(&format!(r#"{SPELLS},{{"ControlledBy":"You"}},{{"In":"Stack"}}"#), r#""Lifelink""#), "Instant and sorcery spells you control have lifelink.")]
#[case::first_casualty(&first(&format!(r#"{SPELLS},{{"ControlledBy":"You"}},{{"In":"Stack"}}"#), &format!(r#"{SPELLS},{{"ControlledBy":"You"}}"#), r#"{"Casualty":2}"#), "The first instant or sorcery spell you cast each turn has casualty 2.")]
#[case::first_historic(&first(&format!(r#"{HISTORIC},{{"ControlledBy":"You"}},{{"In":"Stack"}}"#), &format!(r#"{HISTORIC},{{"ControlledBy":"You"}}"#), r#""Convoke""#), "The first historic spell you cast each turn has convoke.")]
#[case::first_any(&first(r#"{"ControlledBy":"You"},{"In":"Stack"}"#, r#"{"ControlledBy":"You"}"#, r#""Cascade""#), "The first spell you cast each turn has cascade.")]
#[case::first_from_exile(&first(r#"{"ControlledBy":"You"},{"WasCast":{"from":"Exile"}},{"In":"Stack"}"#, r#"{"ControlledBy":"You"},{"WasCast":{"from":"Exile"}}"#, r#""Cascade""#), "The first spell you cast from exile each turn has cascade.")]
fn spells_are_given_a_keyword(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::counters(&itself(&counters("Ge", 3), r#""Flying""#), "As long as this creature has three or more +1/+1 counters on it, it has flying.")]
#[case::counters_after(&itself(&counters("Gt", 9), r#""Trample""#), "This creature has trample as long as it has ten or more +1/+1 counters on it.")]
#[case::shield(&itself(r#"{"Matches":{"selector":[{"HasCounter":{"Other":"shield"}}],"subject":"This"}}"#, r#""Trample""#), "This creature has trample as long as it has a shield counter on it.")]
#[case::poison(&itself(r#"{"Compare":{"left":{"HighestCounters":{"counter":"poison","who":"Opponents"}},"op":"Ge","right":3}}"#, r#""DoubleStrike""#), "As long as an opponent has three or more poison counters, this creature has double strike.")]
#[case::half_life(&itself(r#"{"Compare":{"left":{"Times":{"by":2,"of":{"Life":"You"}}},"op":"Le","right":{"StartingLife":"You"}}}"#, r#""Indestructible""#), "As long as your life total is less than or equal to half your starting life total, ~ has indestructible.")]
#[case::renowned(&itself(r#"{"Matches":{"selector":["Renowned"],"subject":"This"}}"#, r#""Menace""#), "As long as this creature is renowned, it has menace.")]
#[case::defender(&itself(r#"{"Matches":{"selector":[{"HasKeyword":"Defender"}],"subject":"This"}}"#, r#""Indestructible""#), "This creature has indestructible as long as it has defender.")]
#[case::cast_this_turn(&itself(&tally(SPELLS_YOURS, "Ge", 1), r#""Flying""#), "This creature has flying as long as you've cast an instant or sorcery spell this turn.")]
#[case::no_spell(&itself(&tally(r#"{"ControlledBy":"You"}"#, "Lt", 1), r#""Hexproof""#), "This creature has hexproof as long as you haven't cast a spell this turn.")]
#[case::noncreature(&itself(&tally(r#"{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}"#, "Ge", 2), r#""DoubleStrike""#), "As long as you've cast two or more noncreature spells this turn, ~ has double strike.")]
#[case::entered(&itself(r#"{"Compare":{"left":{"Tally":{"event":"Entered","filter":[{"Not":{"Type":"Land"}},{"ControlledBy":"You"}],"who":"EachPlayer"}},"op":"Ge","right":2}}"#, r#""DoubleStrike""#), "This creature has double strike as long as two or more nonland permanents entered the battlefield under your control this turn.")]
#[case::dungeon(&itself(r#"{"Compare":{"left":{"Tallied":{"what":"DungeonsCompleted","who":"You"}},"op":"Ge","right":1}}"#, r#""DoubleStrike""#), "As long as you've completed a dungeon, this creature has double strike.")]
#[case::controls_two(&itself(r#"{"Compare":{"left":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#, r#""Flying""#), "As long as you control an artifact or enchantment, this creature has flying.")]
#[case::controls_another(&itself(r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Type":"Artifact"},"Other",{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#, r#""Shroud""#), "This creature has shroud as long as you control another artifact creature.")]
#[case::group_entered(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"Compare":{"left":{"Count":["This","EnteredThisTurn"]},"op":"Ge","right":1}},"mods":[{"AddKeyword":"Haste"}]}}"#, "As long as this creature entered this turn, creatures you control have haste.")]
#[case::other_turns(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Tapped",{"ControlledBy":"You"}]},"condition":{"Not":"YourTurn"},"mods":[{"AddKeyword":"Hexproof"}]}}"#, "During turns other than yours, tapped creatures you control have hexproof.")]
fn a_condition_of_the_card_is_worded(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

const SPELLS_YOURS: &str =
    r#"{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}"#;

#[rstest]
#[case::afflict(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Type":"Artifact"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Afflict":3}}]}}"#, "Artifact creatures you control have afflict 3.")]
#[case::equipped_afflict(r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":{"Afflict":1}}]}}"#, "Equipped creature gets +2/+0 and has afflict 1.")]
#[case::training(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Macro":{"amount":0,"name":"training"}}}]}}"#, "Creature tokens you control have training.")]
#[case::outlast(r#"{"Static":{"affected":{"Each":[{"Subtype":"Sliver"},{"ControlledBy":"You"},"Other"]},"mods":[{"AddKeyword":{"Outlast":"{2}"}}]}}"#, "Other Slivers you control have outlast {2}.")]
#[case::ward_life(r#"{"Static":{"affected":{"Each":[{"Subtype":"Warlock"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Ward":[{"PayLife":2}]}}]}}"#, "Warlocks you control have \"Ward—Pay 2 life.\"")]
#[case::cumulative(
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"CumulativeUpkeep":"{1}"}}]}}"#,
    "Enchanted creature has \"Cumulative upkeep {1}.\""
)]
#[case::landwalk(r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":{"Landwalk":[{"Type":"Land"},{"Type":"Artifact"}]}}]}}"#, "Equipped creature gets +2/+0 and has artifact landwalk.")]
#[case::blockers(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"AnyOf":[{"BlockedBy":"This"},{"Blocks":"This"}]}]},"mods":[{"AddKeyword":"Lifelink"}]}}"#, "Creatures blocking or blocked by this creature have lifelink.")]
#[case::human(r#"{"Static":{"affected":{"Each":[{"Subtype":"Human"},{"Is":"Attached"}]},"mods":[{"AddKeyword":"Trample"}]}}"#, "As long as enchanted creature is a Human, it has trample.")]
#[case::human_or_angel(r#"{"Static":{"affected":{"Each":[{"Is":"Attached"},{"AnyOf":[{"Subtype":"Human"},{"Subtype":"Angel"}]}]},"mods":[{"AddKeyword":"Vigilance"}]}}"#, "As long as equipped creature is a Human or an Angel, it has vigilance.")]
#[case::untapped(r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Type":"Creature"},"Untapped"],"subject":"Attached"}},"mods":[{"AddKeyword":"Shroud"}]}}"#, "Enchanted creature has shroud as long as it's untapped.")]
#[case::two_equipment(r#"{"Static":{"affected":"Attached","condition":{"Compare":{"left":{"Count":[{"Subtype":"Equipment"},{"AttachedTo":{"Each":[{"Type":"Creature"},{"Is":"Attached"}]}}]},"op":"Ge","right":2}},"mods":[{"AddKeyword":"DoubleStrike"}]}}"#, "Equipped creature has double strike as long as two or more Equipment are attached to it.")]
#[case::enchanted_host(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"Compare":{"left":{"Count":["This",{"HasAttached":[{"Subtype":"Aura"}]}]},"op":"Ge","right":1}},"mods":[{"AddKeyword":"Vigilance"}]}}"#, "Creatures you control have vigilance as long as this creature is enchanted.")]
fn a_group_is_given_a_keyword_with_a_number_or_a_cost(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::other_keyword(&spells(&format!(r#"{{"Type":"Artifact"}},{CAST}"#), r#""Convoke""#), "Artifact spells you cast have improvise.")]
#[case::other_kind(&spells(&format!(r#"{{"Type":"Artifact"}},{CAST}"#), r#""Convoke""#), "Creature spells you cast have convoke.")]
#[case::opponents(&spells(&format!(r#"{{"Type":"Artifact"}},{CAST}"#), r#""Convoke""#), "Artifact spells your opponents cast have convoke.")]
#[case::spells_you_control(&spells(&format!(r#"{{"Type":"Artifact"}},{CAST}"#), r#""Convoke""#), "Artifact spells you control have convoke.")]
#[case::casualty_number(&spells(&format!(r#"{SPELLS},{{"WasCast":{{"by":"You"}}}},{{"In":"Stack"}}"#), r#"{"Casualty":1}"#), "Each instant and sorcery spell you cast has casualty 2.")]
#[case::from_hand_missing(&spells(&format!(r#"{SPELLS},{{"WasCast":{{"by":"You"}}}},{{"WasCast":{{"from":"Hand"}}}},{{"OwnedBy":"You"}},{{"In":"Stack"}}"#), r#""Cascade""#), "Instant and sorcery spells you cast have cascade.")]
#[case::first_dropped(&first(r#"{"ControlledBy":"You"},{"In":"Stack"}"#, r#"{"ControlledBy":"You"}"#, r#""Cascade""#), "Spells you cast have cascade.")]
#[case::first_another_kind(&first(&format!(r#"{SPELLS},{{"ControlledBy":"You"}},{{"In":"Stack"}}"#), r#"{"ControlledBy":"You"}"#, r#"{"Casualty":2}"#), "The first instant or sorcery spell you cast each turn has casualty 2.")]
#[case::counters_number(&itself(&counters("Ge", 3), r#""Flying""#), "As long as this creature has two or more +1/+1 counters on it, it has flying.")]
#[case::counters_ten(&itself(&counters("Ge", 9), r#""Trample""#), "This creature has trample as long as it has ten or more +1/+1 counters on it.")]
#[case::other_counter(&itself(r#"{"Matches":{"selector":[{"HasCounter":{"Other":"ice"}}],"subject":"This"}}"#, r#""Defender""#), "As long as this creature has an ice counter on it, it has defender.")]
#[case::poison_number(&itself(r#"{"Compare":{"left":{"HighestCounters":{"counter":"poison","who":"Opponents"}},"op":"Ge","right":3}}"#, r#""DoubleStrike""#), "As long as an opponent has two or more poison counters, this creature has double strike.")]
#[case::other_keyword_condition(&itself(r#"{"Matches":{"selector":["Renowned"],"subject":"This"}}"#, r#""Menace""#), "As long as this creature is renowned, it has trample.")]
#[case::dropped_condition(
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}]}}"#,
    "As long as this creature has three or more +1/+1 counters on it, it has flying."
)]
#[case::spell_count(&itself(&tally(r#"{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}"#, "Ge", 2), r#""DoubleStrike""#), "As long as you've cast three or more noncreature spells this turn, ~ has double strike.")]
#[case::spell_kind(&itself(&tally(r#"{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}"#, "Ge", 2), r#""DoubleStrike""#), "As long as you've cast two or more creature spells this turn, ~ has double strike.")]
#[case::another_turn(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Tapped",{"ControlledBy":"You"}]},"condition":{"Not":"YourTurn"},"mods":[{"AddKeyword":"Hexproof"}]}}"#, "During your turn, tapped creatures you control have hexproof.")]
#[case::ward_amount(r#"{"Static":{"affected":{"Each":[{"Subtype":"Warlock"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Ward":[{"PayLife":2}]}}]}}"#, "Warlocks you control have \"Ward—Pay 3 life.\"")]
#[case::afflict_number(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Type":"Artifact"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Afflict":3}}]}}"#, "Artifact creatures you control have afflict 2.")]
#[case::blockers_other_keyword(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"AnyOf":[{"BlockedBy":"This"},{"Blocks":"This"}]}]},"mods":[{"AddKeyword":"Lifelink"}]}}"#, "Creatures blocking or blocked by this creature have deathtouch.")]
#[case::human_angel(r#"{"Static":{"affected":{"Each":[{"Subtype":"Human"},{"Is":"Attached"}]},"mods":[{"AddKeyword":"Trample"}]}}"#, "As long as enchanted creature is a Human or an Angel, it has trample.")]
#[case::human_keyword(r#"{"Static":{"affected":{"Each":[{"Subtype":"Human"},{"Is":"Attached"}]},"mods":[{"AddKeyword":"Trample"}]}}"#, "As long as enchanted creature is a Human, it has flying.")]
#[case::tapped(r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Type":"Creature"},"Untapped"],"subject":"Attached"}},"mods":[{"AddKeyword":"Shroud"}]}}"#, "Enchanted creature has shroud as long as it's tapped.")]
#[case::three_equipment(r#"{"Static":{"affected":"Attached","condition":{"Compare":{"left":{"Count":[{"Subtype":"Equipment"},{"AttachedTo":{"Each":[{"Type":"Creature"},{"Is":"Attached"}]}}]},"op":"Ge","right":2}},"mods":[{"AddKeyword":"DoubleStrike"}]}}"#, "Equipped creature has double strike as long as three or more Equipment are attached to it.")]
#[case::auras(r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"condition":{"Compare":{"left":{"Count":["This",{"HasAttached":[{"Subtype":"Aura"}]}]},"op":"Ge","right":1}},"mods":[{"AddKeyword":"Vigilance"}]}}"#, "Creatures you control have vigilance as long as this creature is equipped.")]
fn near_misses_do_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

#[test]
fn a_keyword_given_twice_is_worded_only_as_a_quoted_list() {
    let twice = r#"{"Static":{"affected":{"Each":["Colorless",{"ControlledBy":"You"},{"OwnedBy":"You"},{"WasCast":{"from":"Hand"}},{"Compare":{"op":"Ge","stat":"ManaValue","to":7}},{"In":"Stack"}]},"mods":[{"AddKeyword":"Cascade"},{"AddKeyword":"Cascade"}]}}"#;
    assert_eq!(
        written_variants(&ability(twice)),
        [
            "Colorless spells you cast from your hand with mana value 7 or greater have \"Cascade, cascade.\""
        ]
    );
}
