//! Goal: round 7 builder B: the Oracle wordings the writers said another way (pair mana, a quote's period, subtype bonuses,
//! the card or a creature named again, one duration, the controller's choice) read back, and a rule of another meaning
//! still reads Differs.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PAIR_MANA: &str = r#"{"Activated":{"cost":[{"Mana":"{U/R}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"among":["Blue","Red"],"count":2}}}}]}}"#;
const FILTER: &str = "{U/R}, {T}: Add {U}{U}, {U}{R}, or {R}{R}.";
const GIANT: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"duration":"UntilEndOfTurn","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}}],"targets":[{"what":"AnyTarget"}]}}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const GIANT_TEXT: &str = "When this creature enters, other creatures you control gain \"{T}: This creature deals 2 damage to any target\" until end of turn.";
const DINOSAURS: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Subtype":"Dinosaur"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Vigilance"},{"AddKeyword":"Trample"}]}}"#;
const DINOSAURS_TEXT: &str =
    "Other Dinosaurs you control get +1/+1 and have vigilance and trample.";
const THRESHER: &str = r#"{"Activated":{"cost":[{"RemoveCounters":{"count":2,"counter":"+1/+1"}}],"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":4,"toughness":4}}]}}]}}"#;
const HERO: &str = r#"{"Triggered":{"effects":[{"PutCounters":{"counter":"+1/+1","on":"This"}}],"when":{"SpellCast":{"by":"You","what":[{"Targeting":["This"]}]}}}}"#;
const BOLT_AND_GROW: &str = r#"{"Activated":{"cost":"{1}","effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}},{"PutCounters":{"counter":"+1/+1","on":"This"}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const THREATEN: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetController":"You"}]}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"},{"AddKeyword":"Trample"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const THREATEN_TEXT: &str = "Gain control of target creature until end of turn. Untap it. It gains trample and haste until end of turn.";
const SLAG: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":5,"to":{"Target":0}}},{"Destroy":{"what":{"Each":[{"AttachedTo":{"Target":0}},{"Subtype":"Equipment"}]}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SLAG_TEXT: &str = "This spell deals 5 damage to target creature. Destroy all Equipment attached to that creature.";
const ANALYST: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"RemoveKeyword":"Defender"}]}},{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Vigilance"}]}}],"when":{"Nth":{"event":{"Drew":{"who":"You"}},"nth":2}}}}"#;
const ANALYST_TEXT: &str = "Whenever you draw your second card each turn, this creature loses defender and gains vigilance until end of turn.";
const HELIX: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"Move":{"to":"Hand","what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}]}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const HELIX_TEXT: &str = "Until end of turn, target creature gains \"{T}: Return target nonland permanent to its owner's hand.\"";
const GARGOYLE: &str = r#"{"Activated":{"cost":"{3}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"RemoveKeyword":"Defender"}]}},{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Flying"}]}}]}}"#;
const GARGOYLE_TEXT: &str =
    "{3}: Until end of turn, this creature loses defender and gains flying.";
const PROWESS: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":"Trample"}]}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"GrantAbility":{"Triggered":{"effects":[{"Draw":{"count":"Triggering"}}],"when":{"DealsDamage":{"combat":true,"source":["This"],"to":"EachPlayer"}}}}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const PROWESS_TEXT: &str = "Until end of turn, target creature gets +3/+3 and gains trample and \"Whenever this creature deals combat damage to a player, draw that many cards.\"";
const WONDER: &str = r#"{"Activated":{"cost":[{"PayEnergy":1}],"effects":[{"Either":{"first":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Flying"}]}}],"second":[{"Either":{"first":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Vigilance"}]}}],"second":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Lifelink"}]}}]}}]}}]}}"#;
const WONDER_TEXT: &str =
    "Pay {E}: This creature gains your choice of flying, vigilance, or lifelink until end of turn.";
const BRIGHTLING: &str = r#"{"Activated":{"cost":"{1}","effects":[{"Either":{"first":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":-1}}]}}],"second":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":-1,"toughness":1}}]}}]}}]}}"#;
const BRIGHTLING_TEXT: &str = "{1}: This creature gets +1/-1 or -1/+1 until end of turn.";

#[rstest]
#[case::pair_mana(PAIR_MANA, FILTER, true)]
#[case::pair_mana_three(&PAIR_MANA.replace(r#""count":2"#, r#""count":3"#), FILTER, false)]
#[case::pair_mana_other_color(&PAIR_MANA.replace("Red", "Green"), FILTER, false)]
#[case::quote_period(GIANT, GIANT_TEXT, true)]
#[case::quote_period_other_amount(&GIANT.replace(r#""amount":2"#, r#""amount":3"#), GIANT_TEXT, false)]
#[case::subtype_bonus(DINOSAURS, DINOSAURS_TEXT, true)]
#[case::subtype_bonus_with_this(&DINOSAURS.replace(r#""Other","#, ""), DINOSAURS_TEXT, false)]
// Hexproof matters to a noncreature Dinosaur: the subtype alone is another group.
#[case::subtype_bonus_hexproof(
    &DINOSAURS.replace("Trample", "Hexproof"),
    "Other Dinosaurs you control get +1/+1 and have vigilance and hexproof.",
    false
)]
#[case::outlaws(
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Subtype":"Assassin"},{"Subtype":"Mercenary"},{"Subtype":"Pirate"},{"Subtype":"Rogue"},{"Subtype":"Warlock"}]},{"ControlledBy":"You"}]},"condition":"YourTurn","mods":[{"AddKeyword":"FirstStrike"}]}}"#,
    "During your turn, outlaws you control have first strike.",
    true
)]
#[case::card_again(
    THRESHER,
    "Remove two +1/+1 counters from this creature: It gets +4/+4 until end of turn.",
    true
)]
#[case::card_again_trigger(
    HERO,
    "Whenever you cast a spell that targets this creature, put a +1/+1 counter on it.",
    true
)]
// After a target creature is named, "it" is that creature, not the card.
#[case::card_after_a_target(
    BOLT_AND_GROW,
    "{1}: This creature deals 2 damage to target creature. Put a +1/+1 counter on it.",
    false
)]
#[case::that_creature_as_it(THREATEN, THREATEN_TEXT, true)]
#[case::untap_the_card(&THREATEN.replace(r#"{"Untap":{"what":{"Target":0}}}"#, r#"{"Untap":{"what":"This"}}"#), THREATEN_TEXT, false)]
#[case::it_as_that_creature(SLAG, SLAG_TEXT, true)]
#[case::equipment_of_the_card(&SLAG.replace(r#"{"AttachedTo":{"Target":0}}"#, r#"{"AttachedTo":"This"}"#).replace(r#"{"Spell":{"#, r#"{"Activated":{"cost":"{1}","#), "{1}: This creature deals 5 damage to target creature. Destroy all Equipment attached to that creature.", false)]
#[case::one_duration(ANALYST, ANALYST_TEXT, true)]
#[case::two_durations(&ANALYST.replacen("UntilEndOfTurn", "UntilYourNextTurn", 1), ANALYST_TEXT, false)]
#[case::duration_first(HELIX, HELIX_TEXT, true)]
#[case::duration_first_longer(&HELIX.replace("UntilEndOfTurn", "UntilYourNextTurn"), HELIX_TEXT, false)]
#[case::one_sentence(GARGOYLE, GARGOYLE_TEXT, true)]
#[case::one_sentence_two_durations(&GARGOYLE.replacen("UntilEndOfTurn", "UntilYourNextTurn", 1), GARGOYLE_TEXT, false)]
#[case::one_sentence_quoted(PROWESS, PROWESS_TEXT, true)]
#[case::one_sentence_other_subject(&PROWESS.replace(r#"{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"GrantAbility""#, r#"{"affected":"This","duration":"UntilEndOfTurn","mods":[{"GrantAbility""#), PROWESS_TEXT, false)]
#[case::your_choice(WONDER, WONDER_TEXT, true)]
#[case::your_choice_other_order(
    WONDER,
    "Pay {E}: This creature gains your choice of lifelink, flying, or vigilance until end of turn.",
    true
)]
// Gaining all three is not a choice.
#[case::all_three(
    r#"{"Activated":{"cost":[{"PayEnergy":1}],"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Flying"},{"AddKeyword":"Vigilance"},{"AddKeyword":"Lifelink"}]}}]}}"#,
    WONDER_TEXT,
    false
)]
#[case::choice_of_bonuses(BRIGHTLING, BRIGHTLING_TEXT, true)]
#[case::choice_of_other_bonuses(&BRIGHTLING.replace(r#""power":-1,"toughness":1"#, r#""power":-1,"toughness":2"#), BRIGHTLING_TEXT, false)]
fn r7b_wordings(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

const DEED: &str = r#"{"Activated":{"cost":[{"Mana":"{X}"},"SacrificeSelf"],"effects":[{"Destroy":{"what":{"Each":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"}]},{"Compare":{"op":"Le","stat":"ManaValue","to":"X"}}]}}}]}}"#;
const DEED_TEXT: &str = "{X}, Sacrifice this enchantment: Destroy each artifact, creature, and enchantment with mana value X or less.";
const RECYCLER: &str = r#"{"Activated":{"cost":["Tap",{"Sacrifice":{"filter":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]}]}}],"effects":[{"LoseLife":{"amount":1,"who":"Opponents"}},{"GainLife":{"amount":1}}]}}"#;
const RECYCLER_TEXT: &str =
    "{T}, Sacrifice an artifact or creature: Each opponent loses 1 life and you gain 1 life.";
const LOOTER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{"who":{"Target":0}}},{"Discard":{"who":{"Target":0}}}],"targets":[{"what":"Player"}]}}"#;
const LOOTER_TEXT: &str = "{T}: Target player draws a card, then discards a card.";
const WALKER: &str = r#"{"Triggered":{"effects":[{"GainLife":{"amount":3}},{"PutCounters":{"count":3,"counter":{"Other":"energy"},"on":"You"}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const WALKER_TEXT: &str = "When this creature enters, you gain 3 life and get {E}{E}{E}.";
const END: &str = r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Triggered":{"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}}]}}"#;
const END_TEXT: &str =
    "Enchanted creature has \"At the beginning of your upkeep, sacrifice a creature.\"";
const PUNISHER: &str = r#"{"Activated":{"cost":[{"Mana":"{B}"},"SacrificeSelf"],"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasCounter":"-1/-1"}]}}]}}"#;
const PUNISHER_TEXT: &str =
    "{B}, Sacrifice this creature: Destroy target creature that has a -1/-1 counter on it.";
const FLICKER: &str = r#"{"Spell":{"effects":[{"Exile":{"as":"exiled","what":{"Target":0}}},{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"exiled"}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]}}"#;
const FLICKER_TEXT: &str = "Exile target creature you control, then return it to the battlefield under its owner's control.";
const FORMATION: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}]}}],"targets":[{"bounds":{"max":{"Count":[{"Type":"Creature"}]},"min":0},"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const FORMATION_TEXT: &str =
    "Any number of target creatures each get +1/+1 and gain flying until end of turn.";
const BORDERPOST: &str = r#"{"FreeCast":{"cost":[{"Mana":"{1}"},{"ReturnToHand":{"filter":[{"Type":"Land"},{"Supertype":"Basic"}]}}]}}"#;
const BORDERPOST_TEXT: &str = "You may pay {1} and return a basic land you control to its owner's hand rather than pay this spell's mana cost.";

#[rstest]
#[case::each_and(DEED, DEED_TEXT, true)]
#[case::each_and_other_types(&DEED.replace("Enchantment", "Land"), DEED_TEXT, false)]
#[case::an_vowel(RECYCLER, RECYCLER_TEXT, true)]
#[case::an_vowel_other_cost(&RECYCLER.replace(r#"{"Type":"Artifact"}"#, r#"{"Type":"Enchantment"}"#), RECYCLER_TEXT, false)]
#[case::then_discards(LOOTER, LOOTER_TEXT, true)]
// The player who discards is the one who drew.
#[case::you_discard(&LOOTER.replace(r#"{"Discard":{"who":{"Target":0}}}"#, r#"{"Discard":{}}"#), LOOTER_TEXT, false)]
#[case::and_get(WALKER, WALKER_TEXT, true)]
#[case::opponents_get(&WALKER.replace(r#""on":"You""#, r#""on":"Opponents""#), WALKER_TEXT, false)]
#[case::quoted_sacrifice(END, END_TEXT, true)]
#[case::quoted_sacrifice_each_upkeep(&END.replace(r#""whose":"You""#, r#""whose":"EachPlayer""#), END_TEXT, false)]
#[case::that_has(PUNISHER, PUNISHER_TEXT, true)]
#[case::that_has_other_counter(&PUNISHER.replace("-1/-1", "+1/+1"), PUNISHER_TEXT, false)]
#[case::flicker_it(FLICKER, FLICKER_TEXT, true)]
#[case::flicker_the_card(&FLICKER.replace(r#"{"Bound":"exiled"}"#, r#""This""#), FLICKER_TEXT, false)]
#[case::any_number(FORMATION, FORMATION_TEXT, true)]
// At most as many as there are lands is a limit.
#[case::up_to_lands(&FORMATION.replace(r#""max":{"Count":[{"Type":"Creature"}]}"#, r#""max":{"Count":[{"Type":"Land"}]}"#), FORMATION_TEXT, false)]
#[case::borderpost(BORDERPOST, BORDERPOST_TEXT, true)]
#[case::borderpost_any_land(&BORDERPOST.replace(r#",{"Supertype":"Basic"}"#, ""), BORDERPOST_TEXT, false)]
fn r7b_more(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}
