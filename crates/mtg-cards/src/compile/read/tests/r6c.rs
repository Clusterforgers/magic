//! Goal: each Oracle wording of a rule the writers write in other words (a land counting "other lands", a kicked
//! creature's counters, a copy of "any" permanent, "artifact creatures", "an additional" bonus, a sum naming its
//! constant first, a colored damage source, "can't be blocked except by", two statics of one object as one sentence)
//! reads back for that rule only; another count, condition, zone, type, keyword, bound or duration still differs.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const KICKED: &str = r#"{"Replacement":{"by":{"WithCounters":{"count":2,"counter":"+1/+1"}},"condition":"Kicked","when":{"Enters":{"what":["This"]}}}}"#;
const THREE_LANDS: &str = r#"{"Replacement":{"by":"Tapped","condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Gt","right":2}},"when":{"Enters":{"what":["This"]}}}}"#;
const UNLESS_TWO: &str = r#"{"Replacement":{"by":"Tapped","condition":{"Not":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":2}}},"when":{"Enters":{"what":["This"]}}}}"#;
const IF_TWO: &str = r#"{"Replacement":{"by":"Tapped","condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Ge","right":2}},"when":{"Enters":{"what":["This"]}}}}"#;
const CLONE: &str = r#"{"Replacement":{"by":{"AsCopy":{"of":[{"Type":"Creature"},"Other"]}},"when":{"Enters":{"what":["This"]}}}}"#;
const CAVE_IN: &str =
    r#"{"FreeCast":{"cost":[{"ExileFromHand":{"filter":[{"Color":"Red"},"Other"]}}]}}"#;
const DRONE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Type":"Artifact"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}"#;
const REVELER: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"OwnedBy":"You"},{"In":"Graveyard"}]}}}"#;
const LOGIC: &str = r#"{"Spell":{"effects":[{"Pay":{"cost":"{1}","times":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"unpaid":[{"Counter":{"what":{"Target":0}}}],"who":{"ControllerOf":{"Target":0}}}}],"targets":[{"what":{"Object":["Spell"]}}]}}"#;
const PYROKINESIS: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":"Divided","to":{"Target":0}}}],"targets":[{"bounds":{"max":4,"min":0},"divide":4,"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const LEDGEWALKER: &str = r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]}]}}"#;
const WARLOCK: &str = r#"{"Static":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"Color":"Black"}}]}]}}"#;
const RIDER: &str = r#"{"Static":{"affected":"This","defining":true,"mods":[{"SetPT":{"power":{"Sum":[{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},1]},"toughness":{"Sum":[{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},1]}}}]}}"#;
const JAGGED: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":3,"to":{"Target":0}}}],"targets":[{"bounds":{"max":2,"min":2},"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SPHERE: &str = r#"{"Replacement":{"by":{"PreventUpTo":2},"when":{"Damage":{"source":[{"Color":"Green"}],"to":"You"}}}}"#;
const WAR_PAINT: &str = r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Type":"Creature"}],"subject":"Attached"}},"mods":[{"ModifyPT":{"power":2,"toughness":1}}]}}"#;
const MATTOCK: &str = r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Subtype":"Human"}],"subject":"Attached"}},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const PUMP: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const ADEPT: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"DoubleStrike"}]}}],"when":{"Nth":{"event":{"SpellCast":{"by":"You","what":[{"ControlledBy":"You"}]}},"nth":2}}}}"#;
const BUTCHER: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Token","Attacking",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Deathtouch"}]}}"#;

#[rstest]
#[case::kicked(
    KICKED,
    "If this creature was kicked, it enters with two +1/+1 counters on it."
)]
#[case::fewer_other_lands(
    THREE_LANDS,
    "This land enters tapped unless you control two or fewer other lands."
)]
#[case::unless_other_lands(
    UNLESS_TWO,
    "This land enters tapped unless you control two or more other lands."
)]
#[case::if_other_lands(
    IF_TWO,
    "If you control two or more other lands, this land enters tapped."
)]
#[case::copy_of_any(
    CLONE,
    "You may have this creature enter as a copy of any creature on the battlefield."
)]
#[case::exile_a_card(
    CAVE_IN,
    "You may exile a red card from your hand rather than pay this spell's mana cost."
)]
#[case::artifact_creatures(DRONE, "Artifact creatures you control get +1/+0.")]
#[case::instant_and_sorcery(
    REVELER,
    "This spell costs {1} less to cast for each instant and sorcery card in your graveyard."
)]
#[case::one_for_each(
    LOGIC,
    "Counter target spell unless its controller pays {1} for each card in your graveyard."
)]
#[case::any_number_divided(
    PYROKINESIS,
    "~ deals 4 damage divided as you choose among any number of target creatures."
)]
#[case::except_with_flying(
    LEDGEWALKER,
    "This creature can't be blocked except by creatures with flying."
)]
#[case::except_black(WARLOCK, "This creature can't be blocked except by black creatures.")]
#[case::constant_first(
    RIDER,
    "~'s power and toughness are each equal to 1 plus the number of lands you control."
)]
#[case::each_of_two(JAGGED, "~ deals 3 damage to each of two target creatures.")]
#[case::colored_source(
    SPHERE,
    "If a green source would deal damage to you, prevent 2 of that damage."
)]
#[case::enchanted_permanent(
    WAR_PAINT,
    "As long as enchanted permanent is a creature, it gets +2/+1."
)]
#[case::additional_held(
    MATTOCK,
    "As long as equipped creature is a Human, it gets an additional +1/+1."
)]
#[case::duration_last(
    ADEPT,
    "Flurry — Whenever you cast your second spell each turn, this creature gains double strike until end of turn."
)]
#[case::attacking_tokens(BUTCHER, "Attacking tokens you control have deathtouch.")]
fn an_oracle_wording_of_the_rule_reads_back(#[case] json: &str, #[case] clause: &str) {
    let clause = clause.strip_prefix("Flurry — ").unwrap_or(clause);
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::not_kicked(
    r#"{"Replacement":{"by":{"WithCounters":{"count":2,"counter":"+1/+1"}},"when":{"Enters":{"what":["This"]}}}}"#,
    "If this creature was kicked, it enters with two +1/+1 counters on it."
)]
#[case::three_counters(
    KICKED,
    "If this creature was kicked, it enters with three +1/+1 counters on it."
)]
#[case::three_other_lands(
    THREE_LANDS,
    "This land enters tapped unless you control three or fewer other lands."
)]
#[case::if_not_unless(
    IF_TWO,
    "This land enters tapped unless you control two or more other lands."
)]
#[case::unless_not_if(
    UNLESS_TWO,
    "If you control two or more other lands, this land enters tapped."
)]
#[case::graveyard_copy(
    r#"{"Replacement":{"by":{"AsCopy":{"of":[{"Type":"Creature"},"Other",{"In":"Graveyard"}]}},"when":{"Enters":{"what":["This"]}}}}"#,
    "You may have this creature enter as a copy of any creature card in a graveyard."
)]
#[case::copy_of_an_artifact(
    CLONE,
    "You may have this creature enter as a copy of any artifact on the battlefield."
)]
#[case::exile_a_blue_card(
    CAVE_IN,
    "You may exile a blue card from your hand rather than pay this spell's mana cost."
)]
#[case::enchantment_creatures(DRONE, "Enchantment creatures you control get +1/+0.")]
#[case::instant_and_creature(
    REVELER,
    "This spell costs {1} less to cast for each instant and creature card in your graveyard."
)]
#[case::creature_cards(
    LOGIC,
    "Counter target spell unless its controller pays {1} for each creature card in your graveyard."
)]
#[case::three_targets(
    r#"{"Spell":{"effects":[{"DealDamage":{"amount":"Divided","to":{"Target":0}}}],"targets":[{"bounds":{"max":3,"min":0},"divide":4,"what":{"Object":[{"Type":"Creature"}]}}]}}"#,
    "~ deals 4 damage divided as you choose among any number of target creatures."
)]
#[case::except_with_reach(
    LEDGEWALKER,
    "This creature can't be blocked except by creatures with reach."
)]
#[case::except_red(WARLOCK, "This creature can't be blocked except by red creatures.")]
#[case::two_plus(
    RIDER,
    "~'s power and toughness are each equal to 2 plus the number of lands you control."
)]
#[case::up_to_two(
    r#"{"Spell":{"effects":[{"DealDamage":{"amount":3,"to":{"Target":0}}}],"targets":[{"bounds":{"max":2,"min":0},"what":{"Object":[{"Type":"Creature"}]}}]}}"#,
    "~ deals 3 damage to each of two target creatures."
)]
#[case::creature_source(
    r#"{"Replacement":{"by":{"PreventUpTo":2},"when":{"Damage":{"source":[{"Color":"Green"},{"Type":"Creature"}],"to":"You"}}}}"#,
    "If a green source would deal damage to you, prevent 2 of that damage."
)]
#[case::prevent_three(
    SPHERE,
    "If a green source would deal damage to you, prevent 3 of that damage."
)]
#[case::the_card_itself(
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":1}}]}}"#,
    "As long as enchanted permanent is a creature, it gets +2/+1."
)]
// The rule without the creature condition reaches a noncreature permanent too (r6 audit F1).
#[case::no_creature_condition(
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":1}}]}}"#,
    "As long as enchanted permanent is a creature, it gets +2/+1."
)]
#[case::no_creature_condition_grant(
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Lifelink"}]}}"#,
    "As long as enchanted permanent is a creature, it has lifelink."
)]
#[case::additional_alone(PUMP, "Target creature gets an additional +2/+2 until end of turn.")]
#[case::until_your_next_turn(
    r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfYourNextTurn","mods":[{"AddKeyword":"DoubleStrike"}]}}],"when":{"Nth":{"event":{"SpellCast":{"by":"You","what":[{"ControlledBy":"You"}]}},"nth":2}}}}"#,
    "Whenever you cast your second spell each turn, this creature gains double strike until end of turn."
)]
#[case::all_tokens(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Deathtouch"}]}}"#,
    "Attacking tokens you control have deathtouch."
)]
fn a_rule_of_another_meaning_still_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

const CANT_BLOCK: &str = r#"{"Static":{"affected":"This","mods":["CantBlock"]}}"#;
const UNBLOCKABLE: &str = r#"{"Static":{"affected":"This","mods":["CantBeBlocked"]}}"#;
const HELD_UNBLOCKABLE: &str =
    r#"{"Static":{"affected":"This","condition":"YourTurn","mods":["CantBeBlocked"]}}"#;

#[rstest]
#[case::same_object(CANT_BLOCK, UNBLOCKABLE, true)]
#[case::another_condition(CANT_BLOCK, HELD_UNBLOCKABLE, false)]
#[case::another_object(
    CANT_BLOCK,
    r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked"]}}"#,
    false
)]
fn two_statics_of_one_object_are_one_sentence(
    #[case] first: &str,
    #[case] second: &str,
    #[case] reads: bool,
) {
    let texts = static_pair_texts(&ability(first), &ability(second));
    let found = texts
        .iter()
        .any(|text| same_text(text, "This creature can't block and can't be blocked."));
    assert_eq!(found, reads, "{texts:?}");
}
