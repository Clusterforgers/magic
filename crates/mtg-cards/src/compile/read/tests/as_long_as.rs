//! Goal: a static ability under "as long as" on the state of the card, its counters (a Vehicle that is an artifact creature while it holds enough), the cards in a graveyard or a hand, a devotion or what is controlled reads into one `Static` with a `condition`, writes back as the same text, and leaves every near-miss to the table.
use super::*;

fn stat(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

const GRAVEYARD: &str = r#"[{"In":"Graveyard"},{"OwnedBy":"You"}]"#;

#[rstest]
#[case::devotion_to_one_color(
    "As long as your devotion to white is less than five, ~ isn't a creature.",
    r#"{"Static":{"affected":"This","mods":[{"RemoveType":"Creature"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Devotion":["White"]},"op":"Lt","right":5}}"#
)]
#[case::devotion_to_two_colors(
    "As long as your devotion to green and white is less than seven, ~ isn't a creature.",
    r#"{"Static":{"affected":"This","mods":[{"RemoveType":"Creature"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Devotion":["Green","White"]},"op":"Lt","right":7}}"#
)]
#[case::fewer_cards_in_the_graveyard(
    "As long as there are fewer than eight cards in your graveyard, ~ isn't a creature.",
    r#"{"Static":{"affected":"This","mods":[{"RemoveType":"Creature"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":GRAVEYARD},"op":"Lt","right":8}}"#
)]
#[case::threshold(
    "This creature gets +2/+2 as long as there are seven or more cards in your graveyard.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":GRAVEYARD},"op":"Ge","right":7}}"#
)]
#[case::threshold_first(
    "As long as there are seven or more cards in your graveyard, this creature gets +1/+1 and has first strike.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"FirstStrike"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":GRAVEYARD},"op":"Ge","right":7}}"#
)]
#[case::delirium(
    "This creature gets +2/+2 as long as there are four or more card types among cards in your graveyard.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"CardTypes":GRAVEYARD},"op":"Ge","right":4}}"#
)]
#[case::delirium_keyword(
    "This creature has deathtouch as long as there are four or more card types among cards in your graveyard.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Deathtouch"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"CardTypes":GRAVEYARD},"op":"Ge","right":4}}"#
)]
#[case::no_cards_in_hand(
    "This creature has flying as long as you have no cards in hand.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":0}}"#
)]
#[case::seven_cards_in_hand(
    "This creature gets +2/+1 and has first strike as long as you have seven or more cards in hand.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":1}},{"AddKeyword":"FirstStrike"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Ge","right":7}}"#
)]
#[case::one_or_fewer_cards_in_hand(
    "As long as you have one or fewer cards in hand, this creature gets +2/+2 and has menace.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Menace"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Le","right":1}}"#
)]
#[case::untapped(
    "This creature has hexproof as long as it's untapped.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Hexproof"}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"This","selector":["Untapped"]}}"#
)]
#[case::untapped_first(
    "As long as this creature is untapped, it gets +0/+2.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":0,"toughness":2}}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"This","selector":["Untapped"]}}"#
)]
#[case::attacking(
    "This creature gets +2/+0 as long as it's attacking.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":0}}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"This","selector":["Attacking"]}}"#
)]
#[case::monstrous(
    "As long as this creature is monstrous, it has hexproof and indestructible.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Hexproof"},{"AddKeyword":"Indestructible"}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"This","selector":["Monstrous"]}}"#
)]
#[case::the_host_is_legendary(
    "As long as equipped creature is legendary, it has hexproof.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Hexproof"}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"Attached","selector":[{"Supertype":"Legendary"}]}}"#
)]
#[case::the_host_by_a_pronoun(
    "Equipped creature has trample as long as it's legendary.",
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Trample"}],"condition":CONDITION}}"#,
    r#"{"Matches":{"subject":"Attached","selector":[{"Supertype":"Legendary"}]}}"#
)]
#[case::plus_counters_first(
    "As long as this creature has four or more +1/+1 counters on it, it has flying and vigilance.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Flying"},{"AddKeyword":"Vigilance"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":4}}"#
)]
#[case::plus_counters_after(
    "This creature has trample as long as it has ten or more +1/+1 counters on it.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":10}}"#
)]
#[case::quest_counters(
    "As long as this enchantment has five or more quest counters on it, creatures you control get +2/+0.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":0}}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"quest"}}},"op":"Ge","right":5}}"#
)]
#[case::one_counter(
    "This creature has haste as long as it has a divinity counter on it.",
    r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"divinity"}}},"op":"Ge","right":1}}"#
)]
#[case::the_citys_blessing_hides_a_group(
    "As long as you have the city's blessing, creatures you control can't be blocked.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":["CantBeBlocked"],"condition":CONDITION}}"#,
    r#"{"CitysBlessing":"You"}"#
)]
#[case::no_other_creatures(
    "This creature can't be blocked as long as you control no other creatures.",
    r#"{"Static":{"affected":"This","mods":["CantBeBlocked"],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}}"#
)]
#[case::cannot_block_under_a_condition(
    "This creature can't block as long as you control a Plains.",
    r#"{"Static":{"affected":"This","mods":["CantBlock"],"condition":CONDITION}}"#,
    r#"{"Compare":{"left":{"Count":[{"Subtype":"Plains"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#
)]
fn a_static_ability_under_a_state_or_a_count_reads_as_the_hand_written_files_say(
    #[case] text: &str,
    #[case] json: &str,
    #[case] condition: &str,
) {
    reads_as(
        text,
        ClauseKind::Static,
        false,
        &json.replace("CONDITION", &condition.replace("GRAVEYARD", GRAVEYARD)),
    );
}

#[rstest]
#[case::devotion_at_most("As long as your devotion to white is five or less, ~ isn't a creature.")]
#[case::a_devotion_of_no_color(
    "As long as your devotion to lime is less than five, ~ isn't a creature."
)]
#[case::no_type_taken_without_a_condition("~ isn't a creature.")]
#[case::another_zone(
    "As long as there are seven or more cards in your hand, this creature gets +2/+2."
)]
#[case::an_opponents_graveyard(
    "As long as there are seven or more cards in an opponent's graveyard, this creature gets +2/+2."
)]
#[case::card_types_among_other_cards(
    "This creature gets +2/+2 as long as there are four or more card types among creatures you control."
)]
#[case::a_state_of_another_thing("This creature has haste as long as target creature is untapped.")]
#[case::a_state_it_does_not_know("This creature has haste as long as it's enchanted.")]
#[case::counters_of_no_kind(
    "This creature has haste as long as it has three or more counters on it."
)]
#[case::counters_it_may_not_name(
    "This creature has haste as long as it has three or more +1/+1 counters."
)]
#[case::controlling_none_of_the_opponents_things(
    "This creature can't be blocked as long as you control no creatures your opponents control."
)]
fn a_condition_or_effect_that_is_not_modeled_stays_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}

#[rstest]
#[case::three_fire_counters(
    "As long as this Vehicle has three or more fire counters on it, it's an artifact creature."
)]
#[case::one_counter(
    "As long as this Vehicle has a fire counter on it, it's an artifact creature."
)]
#[case::plus_counters(
    "As long as this Vehicle has five or more +1/+1 counters on it, it's an artifact creature."
)]
fn a_vehicle_that_is_a_creature_while_it_holds_counters_reads_and_writes_back(#[case] text: &str) {
    golden(text, ClauseKind::Static, false);
}

#[rstest]
#[case::another_permanent(
    "As long as you control three or more artifacts, it's an artifact creature."
)]
#[case::a_creature(
    "As long as this Vehicle has three or more fire counters on it, it's a creature."
)]
#[case::an_artifact_only(
    "As long as this Vehicle has three or more fire counters on it, it's an artifact."
)]
fn near_misses_of_a_vehicle_animated_by_counters_stay_with_the_table(#[case] text: &str) {
    assert_eq!(stat(text), None, "{text}");
}
