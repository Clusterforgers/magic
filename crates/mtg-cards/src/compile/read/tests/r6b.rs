//! Goal: round 6 builder B: the Oracle wordings of activated and static abilities the writers wrote another way read back,
//! and a rule of another meaning still reads Differs.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn paired(first: &str, second: &str, clause: &str) -> bool {
    super::super::static_pairs::texts(&ability(first), &ability(second))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CANT_ATTACK_OR_BLOCK: &str =
    r#"{"Static":{"affected":"Attached","mods":["CantAttack","CantBlock"]}}"#;
const NO_ACTIVATIONS: &str = r#"{"Rule":{"rule":{"Cant":{"action":{"ActivateKind":{"kind":"Every","sources":[{"Is":"Attached"}]}},"who":"EachPlayer"}}}}"#;
const ARREST_TEXT: &str =
    "Enchanted creature can't attack or block, and its activated abilities can't be activated.";
const BONUS: &str =
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}"#;
const UNBLOCKABLE: &str = r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked"]}}"#;
const VIGILANT: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Vigilance"}]}}"#;
const NOT_YOU: &str = r#"{"Static":{"affected":"Attached","mods":[{"CantAttackYou":"Both"}]}}"#;
const BY_TOUGHNESS: &str =
    r#"{"Rule":{"rule":{"Combat":{"quirk":"AssignsByToughness","who":"Attached"}}}}"#;

#[rstest]
#[case::arrest(CANT_ATTACK_OR_BLOCK, NO_ACTIVATIONS, ARREST_TEXT, true)]
#[case::petrify(
    CANT_ATTACK_OR_BLOCK,
    NO_ACTIVATIONS,
    "Enchanted permanent can't attack or block, and its activated abilities can't be activated.",
    true
)]
// Activated abilities of creatures only are said only after "Enchanted creature".
#[case::creature_sources_of_a_permanent(
    CANT_ATTACK_OR_BLOCK,
    &NO_ACTIVATIONS.replace(r#"[{"Is":"Attached"}]"#, r#"[{"Type":"Creature"},{"Is":"Attached"}]"#),
    "Enchanted permanent can't attack or block, and its activated abilities can't be activated.",
    false
)]
// Mana abilities kept apart are another rule.
#[case::not_mana(
    CANT_ATTACK_OR_BLOCK,
    &NO_ACTIVATIONS.replace("Every", "Any"),
    ARREST_TEXT,
    false
)]
// Only the activations of the attached permanent.
#[case::of_every_permanent(CANT_ATTACK_OR_BLOCK, &NO_ACTIVATIONS.replace(r#","sources":[{"Is":"Attached"}]"#, ""), ARREST_TEXT, false)]
#[case::unblockable(
    BONUS,
    UNBLOCKABLE,
    "Enchanted creature gets +1/+0 and can't be blocked.",
    true
)]
#[case::unblockable_source(BONUS, &UNBLOCKABLE.replace(r#""Attached""#, r#""This""#), "Enchanted creature gets +1/+0 and can't be blocked.", false)]
#[case::three_parts(
    VIGILANT,
    NOT_YOU,
    "Enchanted creature gets +2/+2, has vigilance, and can't attack you or planeswalkers you control.",
    true
)]
#[case::by_toughness(
    &BONUS.replace(r#""power":1,"toughness":0"#, r#""power":0,"toughness":2"#),
    BY_TOUGHNESS,
    "Enchanted creature gets +0/+2 and assigns combat damage equal to its toughness rather than its power.",
    true
)]
#[case::by_toughness_of_the_source(
    &BONUS.replace(r#""power":1,"toughness":0"#, r#""power":0,"toughness":2"#),
    &BY_TOUGHNESS.replace(r#""who":"Attached""#, r#""who":"This""#),
    "Enchanted creature gets +0/+2 and assigns combat damage equal to its toughness rather than its power.",
    false
)]
// A conditional first half is not said in the joined sentence.
#[case::conditional_front(
    r#"{"Static":{"affected":"Attached","condition":"YourTurn","mods":[{"ModifyPT":{"power":1,"toughness":0}}]}}"#,
    UNBLOCKABLE,
    "Enchanted creature gets +1/+0 and can't be blocked.",
    false
)]
fn r6b_two_abilities_of_the_attached_permanent_are_one_sentence(
    #[case] first: &str,
    #[case] second: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(paired(first, second, clause), reads, "{clause}");
}

const AIR_NOMAD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const ESCAPE: &str = r#"{"Activated":{"cost":"{2}{U}","effects":[{"Move":{"to":"Hand","what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Color":"White"},{"Color":"Black"}]},{"ControlledBy":"You"}]}}]}}"#;
const CHIEF: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Type":"Artifact"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const SENU: &str = r#"{"Activated":{"cost":["Tap","ExileSelf"],"effects":[{"GainLife":{"amount":2}},{"Scry":{"count":2}}]}}"#;
const GREENBELT: &str = r#"{"Activated":{"cost":"{3}{G}","effects":[{"PutCounters":{"count":3,"counter":"+1/+1","on":"This"}}],"limit":"OncePerGame"}}"#;
const BEDLAM: &str =
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["CantBlock"]}}"#;
const EEL: &str = r#"{"Activated":{"cost":"{R}{R}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":2,"toughness":0}}]}},{"DealDamage":{"amount":1,"to":"You"}}]}}"#;
const CANOPY: &str = r#"{"Activated":{"cost":"{1}{G}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Flying"}]}},{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"RemoveKeyword":"Trample"}]}}]}}"#;
const DRUID: &str = r#"{"Activated":{"cost":[{"Mana":"{G}"},"Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"Forever","mods":[{"SetPT":{"power":2,"toughness":2}},{"SetColors":["Green"]},{"AddType":"Creature"}]}}],"targets":[{"what":{"Object":[{"Type":"Land"}]}}]}}"#;
const AVATAR: &str = r#"{"Loyalty":{"cost":{"Minus":6},"effects":[{"CreateToken":{"token":{"Custom":{"abilities":[{"Static":{"affected":"This","defining":true,"mods":[{"SetPT":{"power":{"Life":"You"},"toughness":{"Life":"You"}}}]}}],"colors":["White"],"name":"Avatar","power":0,"subtypes":["Avatar"],"toughness":0,"types":["Creature"]}}}}]}}"#;
const AEGIS: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":2}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"op":"Eq","stat":"Power","to":1}},{"Compare":{"op":"Eq","stat":"Toughness","to":1}}]}}]}}"#;
const MAGEMARK: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"HasAttached":[{"Subtype":"Aura"}]},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const STALWART: &str = r#"{"Activated":{"cost":["Tap",{"TapPermanents":{"filter":[{"AnyOf":[{"Type":"Creature"},{"Type":"Artifact"}]}]}}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#;
const ORZHOVA: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Lifelink"},{"AddKeyword":"Flying"}]}}"#;
const KASMINA: &str = r#"{"Static":{"affected":"Attached","mods":[{"SetPT":{"power":1,"toughness":1}},"LoseAbilities"]}}"#;
const PATH: &str = r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Type":"Land"}],"subject":"Attached"}},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green","Green"]}}}]}}}]}}"#;
const GRIFFIN: &str = r#"{"Activated":{"cost":"{W}","effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetColors":["White"]}]}}],"targets":[{"what":{"Object":[]}}]}}"#;
const SAGE: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},{"RemoveCounters":{"counter":"+1/+1","from":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Draw":{}}]}}"#;
const SEARCH: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"Search":{"as":"found","filter":[{"Subtype":"Goblin"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}]}},{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"found"}}},{"Shuffle":{}}]}}"#;

#[rstest]
#[case::you_control_first(AIR_NOMAD, "Creatures you control with flying get +1/+1.", true)]
#[case::you_control_first_other_keyword(&AIR_NOMAD.replace("Flying", "Reach"), "Creatures you control with flying get +1/+1.", false)]
#[case::you_control_first_opponents(&AIR_NOMAD.replace(r#""ControlledBy":"You""#, r#""ControlledBy":"Opponents""#), "Creatures you control with flying get +1/+1.", false)]
#[case::colors_first(
    ESCAPE,
    "{2}{U}: Return target white or black creature you control to its owner's hand.",
    true
)]
#[case::colors_first_other_color(&ESCAPE.replace("White", "Red"), "{2}{U}: Return target white or black creature you control to its owner's hand.", false)]
#[case::artifact_creatures(CHIEF, "Other artifact creatures you control get +1/+1.", true)]
#[case::artifact_creatures_not_other(&CHIEF.replace(r#""Other","#, ""), "Other artifact creatures you control get +1/+1.", false)]
#[case::implied_you(SENU, "{T}, Exile ~: You gain 2 life and scry 2.", true)]
#[case::implied_you_surveil(&SENU.replace("Scry", "Surveil"), "{T}, Exile ~: You gain 2 life and scry 2.", false)]
#[case::exhaust(
    GREENBELT,
    "Exhaust \u{2014} {3}{G}: Put three +1/+1 counters on ~.",
    true
)]
// Once each turn is not exhaust.
#[case::exhaust_each_turn(&GREENBELT.replace("OncePerGame", "OncePerTurn"), "Exhaust \u{2014} {3}{G}: Put three +1/+1 counters on ~.", false)]
#[case::exhaust_unlimited(&GREENBELT.replace(r#","limit":"OncePerGame""#, ""), "Exhaust \u{2014} {3}{G}: Put three +1/+1 counters on ~.", false)]
#[case::creatures(BEDLAM, "Creatures can't block.", true)]
#[case::creatures_cant_attack(&BEDLAM.replace("CantBlock", "CantAttack"), "Creatures can't block.", false)]
#[case::joined_damage(
    EEL,
    "{R}{R}: ~ gets +2/+0 until end of turn and deals 1 damage to you.",
    true
)]
#[case::joined_damage_to_opponents(&EEL.replace(r#""to":"You""#, r#""to":"Opponents""#), "{R}{R}: ~ gets +2/+0 until end of turn and deals 1 damage to you.", false)]
#[case::joined_both_until(
    CANOPY,
    "{1}{G}: ~ gains flying and loses trample until end of turn.",
    true
)]
// A loss that lasts is not said "until end of turn".
#[case::joined_loss_forever(&CANOPY.replacen(r#""duration":"UntilEndOfTurn","mods":[{"RemoveKeyword""#, r#""duration":"Forever","mods":[{"RemoveKeyword""#, 1), "{1}{G}: ~ gains flying and loses trample until end of turn.", false)]
#[case::still_a_land(
    DRUID,
    "{G}, {T}: Target land becomes a 2/2 green creature that's still a land.",
    true
)]
#[case::still_a_land_red(&DRUID.replace("Green\"]", "Red\"]"), "{G}, {T}: Target land becomes a 2/2 green creature that's still a land.", false)]
#[case::token_has(
    AVATAR,
    "\u{2212}6: Create a white Avatar creature token. It has \"This token's power and toughness are each equal to your life total.\"",
    true
)]
#[case::token_has_black(&AVATAR.replace("White", "Black"), "\u{2212}6: Create a white Avatar creature token. It has \"This token's power and toughness are each equal to your life total.\"", false)]
#[case::size_first(
    AEGIS,
    "{1}, {T}: Target 1/1 creature gets +1/+2 until end of turn.",
    true
)]
#[case::size_first_two_two(&AEGIS.replace(r#""stat":"Toughness","to":1"#, r#""stat":"Toughness","to":2"#), "{1}, {T}: Target 1/1 creature gets +1/+2 until end of turn.", false)]
#[case::enchanted_group(MAGEMARK, "Creatures you control that are enchanted get +1/+1.", true)]
#[case::enchanted_group_bigger(&MAGEMARK.replace(r#""power":1,"toughness":1"#, r#""power":2,"toughness":2"#), "Creatures you control that are enchanted get +1/+1.", false)]
#[case::artifact_or_creature(
    STALWART,
    "{T}, Tap an untapped artifact or creature you control: Add one mana of any color.",
    true
)]
#[case::artifact_or_land(&STALWART.replace("Creature", "Land"), "{T}, Tap an untapped artifact or creature you control: Add one mana of any color.", false)]
#[case::keyword_pair(
    ORZHOVA,
    "Enchanted creature gets +1/+1 and has flying and lifelink.",
    true
)]
#[case::keyword_pair_trample(&ORZHOVA.replace("Lifelink", "Trample"), "Enchanted creature gets +1/+1 and has flying and lifelink.", false)]
#[case::loses_first(
    KASMINA,
    "Enchanted creature loses all abilities and has base power and toughness 1/1.",
    true
)]
#[case::loses_first_size(&KASMINA.replace(r#""power":1,"toughness":1"#, r#""power":0,"toughness":1"#), "Enchanted creature loses all abilities and has base power and toughness 1/1.", false)]
#[case::noun_is_condition(PATH, "Enchanted land has \"{T}: Add {G}{G}.\"", true)]
// A condition the noun does not say stays said.
#[case::noun_is_not_condition(&PATH.replace(r#"{"Type":"Land"}"#, r#"{"Type":"Creature"}"#), "Enchanted land has \"{T}: Add {G}{G}.\"", false)]
#[case::bare_color(
    GRIFFIN,
    "{W}: Target permanent becomes white until end of turn.",
    true
)]
#[case::bare_color_blue(&GRIFFIN.replace("White", "Blue"), "{W}: Target permanent becomes white until end of turn.", false)]
#[case::counter_of_your_creature(
    SAGE,
    "{2}, Remove a +1/+1 counter from a creature you control: Draw a card.",
    true
)]
#[case::counter_of_your_artifact(&SAGE.replace("Creature", "Artifact"), "{2}, Remove a +1/+1 counter from a creature you control: Draw a card.", false)]
#[case::permanent_card(
    SEARCH,
    "{3}, {T}: Search your library for a Goblin permanent card, put it onto the battlefield, then shuffle.",
    true
)]
// A Goblin card of any type is not a Goblin permanent card.
#[case::any_card(&SEARCH.replace(r#",{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]}"#, ""), "{3}, {T}: Search your library for a Goblin permanent card, put it onto the battlefield, then shuffle.", false)]
#[case::five_types(&SEARCH.replace(r#"{"Type":"Battle"},"#, ""), "{3}, {T}: Search your library for a Goblin permanent card, put it onto the battlefield, then shuffle.", false)]
fn r6b_oracle_wordings_read_back(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

const PLAGUE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"ChosenSubtype",{"ControlledBy":"Opponents"}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}]}}"#;
const SPITTING: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"HasCounter":"-1/-1"},{"ControlledBy":"Opponents"}]},"mods":["CantBlock"]}}"#;
const CLOCK: &str = r#"{"Activated":{"cost":"{2}{U}","effects":[{"PutCounters":{"counter":{"Other":"hour"},"on":"This"}}]}}"#;
const ELF_KNIGHT: &str = r#"{"Activated":{"cost":[{"Mana":"{5}{W}"},"Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"abilities":[{"Keyword":"Vigilance"}],"colors":["White","Green"],"name":"Elf Knight","power":2,"subtypes":["Elf","Knight"],"toughness":2,"types":["Creature"]}}}}]}}"#;
const SIEGEMASTER: &str = r#"{"Static":{"affected":{"Each":[{"AnyOf":[{"Subtype":"Goblin"},{"Subtype":"Orc"}]},{"ControlledBy":"You"},"Other"]},"mods":[{"AddKeyword":"Trample"}]}}"#;
const CHANTER: &str = r#"{"Activated":{"cost":"{5}{W}","effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Lifelink"},{"AddKeyword":"Vigilance"}]}}]}}"#;
const STOP_COLD: &str = r#"{"Triggered":{"effects":[{"Tap":{"what":"Attached"}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const HARRIER: &str = r#"{"Activated":{"cost":[{"Mana":"{W}"},"Tap",{"Discard":{}}],"effects":[{"CreateToken":{"token":{"Custom":{"abilities":[{"Activated":{"cost":[{"Mana":"{W}"},"Tap"],"effects":[{"Tap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}],"colors":["White"],"name":"Goldmeadow Harrier","power":1,"subtypes":["Kithkin","Soldier"],"toughness":1,"types":["Creature"]}}}}]}}"#;
const HARRIER_TEXT: &str = "{W}, {T}, Discard a card: Create a 1/1 white Kithkin Soldier creature token named Goldmeadow Harrier. It has \"{W}, {T}: Tap target creature.\"";

#[rstest]
#[case::chosen_type_first(
    PLAGUE,
    "Creatures of the chosen type your opponents control get -1/-1.",
    true
)]
#[case::chosen_type_yours(&PLAGUE.replace("Opponents", "You"), "Creatures of the chosen type your opponents control get -1/-1.", false)]
#[case::opponents_first(
    SPITTING,
    "Creatures your opponents control with -1/-1 counters on them can't block.",
    true
)]
#[case::opponents_first_plus(&SPITTING.replace(r#""HasCounter":"-1/-1""#, r#""HasCounter":"+1/+1""#), "Creatures your opponents control with -1/-1 counters on them can't block.", false)]
#[case::an_hour(CLOCK, "{2}{U}: Put an hour counter on ~.", true)]
#[case::an_hour_two(&CLOCK.replace(r#""counter""#, r#""count":2,"counter""#), "{2}{U}: Put an hour counter on ~.", false)]
#[case::colors_swapped(
    ELF_KNIGHT,
    "{5}{W}, {T}: Create a 2/2 green and white Elf Knight creature token with vigilance.",
    true
)]
#[case::colors_one(&ELF_KNIGHT.replace(r#"["White","Green"]"#, r#"["Green"]"#), "{5}{W}, {T}: Create a 2/2 green and white Elf Knight creature token with vigilance.", false)]
#[case::subtypes_swapped(SIEGEMASTER, "Other Orcs and Goblins you control have trample.", true)]
#[case::subtypes_not_other(&SIEGEMASTER.replace(r#","Other""#, ""), "Other Orcs and Goblins you control have trample.", false)]
#[case::keywords_swapped(
    CHANTER,
    "{5}{W}: ~ gains vigilance and lifelink until end of turn.",
    true
)]
#[case::keywords_one(&CHANTER.replace(r#"{"AddKeyword":"Lifelink"},"#, ""), "{5}{W}: ~ gains vigilance and lifelink until end of turn.", false)]
#[case::tap_enchanted_permanent(STOP_COLD, "When ~ enters, tap enchanted permanent.", true)]
#[case::untap_enchanted_permanent(&STOP_COLD.replace(r#"{"Tap""#, r#"{"Untap""#), "When ~ enters, tap enchanted permanent.", false)]
#[case::named_token_has(HARRIER, HARRIER_TEXT, true)]
#[case::named_token_untaps(&HARRIER.replace(r#"[{"Tap":{"what":{"Target":0}}}]"#, r#"[{"Untap":{"what":{"Target":0}}}]"#), HARRIER_TEXT, false)]
fn r6b_lists_and_names_read_back(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::either_order(
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Shroud"}]}}"#,
    "Enchanted creature can't be blocked and has shroud.",
    true
)]
#[case::either_order_hexproof(
    r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Hexproof"}]}}"#,
    "Enchanted creature can't be blocked and has shroud.",
    false
)]
fn r6b_two_statics_of_the_attached_permanent_in_either_order(
    #[case] first: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(paired(first, UNBLOCKABLE, clause), reads, "{clause}");
}

#[test]
#[ignore = "probe"]
fn zz_probe() {
    let Ok(path) = std::env::var("ZZ_PROBE") else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for line in text.lines() {
        let Some((slug, json)) = line.split_once('\t') else {
            continue;
        };
        let Ok(found) = serde_json::from_str::<Ability>(json) else {
            println!("{slug}\tBAD");
            continue;
        };
        for wording in written_variants(&found) {
            println!("{slug}\t{wording}");
        }
    }
}
