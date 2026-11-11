//! Goal: the wf27 writers give the oracle wording of maximum hand sizes, the top card of each library revealed, crewing as
//! though stronger, a life floor, landwalk ignored, an enchanted land's types, permanents that don't untap, every creature
//! type, a static while defending player controls enough, a card name chosen as a permanent enters, damage to or by a
//! creature prevented, colorless and extra blocks until end of turn, the basic land type of your choice, casting
//! conditions and a cast in place of the mana cost; another player, number, kind, counter, condition or object never
//! reads back as the clause.
use super::*;

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_written_text(wording, clause))
}

fn rule(rule: &str) -> String {
    format!(r#"{{"Rule":{{"rule":{rule}}}}}"#)
}

#[rstest]
#[case::size(&rule(r#"{"MaxHandSize":{"who":"You","size":8}}"#), "Your maximum hand size is eight.", true)]
#[case::other_size(&rule(r#"{"MaxHandSize":{"who":"You","size":7}}"#), "Your maximum hand size is eight.", false)]
#[case::opponents_size(&rule(r#"{"MaxHandSize":{"who":"Opponents","size":8}}"#), "Your maximum hand size is eight.", false)]
#[case::increased(&rule(r#"{"HandSizeChange":{"who":"You","by":1}}"#), "Your maximum hand size is increased by one.", true)]
#[case::reduced_for_increased(&rule(r#"{"HandSizeChange":{"who":"You","by":-1}}"#), "Your maximum hand size is increased by one.", false)]
#[case::opponents_reduced(&rule(r#"{"HandSizeChange":{"who":"Opponents","by":-2}}"#), "Each opponent's maximum hand size is reduced by two.", true)]
#[case::yours_reduced(&rule(r#"{"HandSizeChange":{"who":"You","by":-2}}"#), "Each opponent's maximum hand size is reduced by two.", false)]
#[case::tops_revealed(&rule(r#"{"RevealTop":"EachPlayer"}"#), "Players play with the top card of their libraries revealed.", true)]
#[case::your_top_revealed(&rule(r#"{"RevealTop":"You"}"#), "Players play with the top card of their libraries revealed.", false)]
#[case::crews(&rule(r#"{"Combat":{"who":"This","quirk":{"TapPower":{"Plus":2}}}}"#), "~ saddles Mounts and crews Vehicles as though its power were 2 greater.", true)]
#[case::crews_more(&rule(r#"{"Combat":{"who":"This","quirk":{"TapPower":{"Plus":3}}}}"#), "~ saddles Mounts and crews Vehicles as though its power were 2 greater.", false)]
#[case::crews_only(&rule(r#"{"Combat":{"who":"This","quirk":{"TapPower":{"Plus":2}}}}"#), "~ crews Vehicles as though its power were 2 greater.", false)]
#[case::life_floor(&rule(r#"{"LifeFloor":{"who":"You","floor":1}}"#), "Damage that would reduce your life total to less than 1 reduces it to 1 instead.", true)]
#[case::life_floor_of_opponents(&rule(r#"{"LifeFloor":{"who":"Opponents","floor":1}}"#), "Damage that would reduce your life total to less than 1 reduces it to 1 instead.", false)]
#[case::life_floor_while(
    r#"{"Rule":{"rule":{"LifeFloor":{"who":"You","floor":7}},"condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":7}}}}"#,
    "As long as you have 7 or more life, damage that would reduce your life total to less than 7 reduces it to 7 instead.",
    true
)]
#[case::life_floor_always(&rule(r#"{"LifeFloor":{"who":"You","floor":7}}"#), "As long as you have 7 or more life, damage that would reduce your life total to less than 7 reduces it to 7 instead.", false)]
#[case::landwalk(&rule(r#"{"Combat":{"who":{"Each":[{"HasKeyword":{"Landwalk":[{"Subtype":"Island"}]}}]},"quirk":{"IgnoreLandwalk":[{"Subtype":"Island"}]}}}"#), "Creatures with islandwalk can be blocked as though they didn't have islandwalk.", true)]
#[case::other_landwalk(&rule(r#"{"Combat":{"who":{"Each":[{"HasKeyword":{"Landwalk":[{"Subtype":"Island"}]}}]},"quirk":{"IgnoreLandwalk":[{"Subtype":"Swamp"}]}}}"#), "Creatures with islandwalk can be blocked as though they didn't have islandwalk.", false)]
#[case::lure(&rule(r#"{"Combat":{"who":"Attached","quirk":{"MustBeBlocked":"Every"}}}"#), "All creatures able to block enchanted creature do so.", true)]
#[case::lure_if_able(&rule(r#"{"Combat":{"who":"Attached","quirk":{"MustBeBlocked":"AtLeastOne"}}}"#), "All creatures able to block enchanted creature do so.", false)]
fn wf27_rules(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn untap(affected: &str) -> String {
    format!(r#"{{"Static":{{"affected":{affected},"mods":["DoesntUntap"]}}}}"#)
}

const PUPA: &str = r#"{"Static":{"affected":"Attached","mods":["DoesntUntap"],"condition":{"Compare":{"left":{"Count":["This",{"HasCounter":{"Other":"pupa"}}]},"op":"Ge","right":1}}}}"#;

#[rstest]
#[case::islands(&untap(r#"{"Each":[{"Subtype":"Island"}]}"#), "Islands don't untap during their controllers' untap steps.", true)]
#[case::swamps(&untap(r#"{"Each":[{"Subtype":"Swamp"}]}"#), "Islands don't untap during their controllers' untap steps.", false)]
#[case::strong(&untap(r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":3}}]}"#), "Creatures with power 3 or greater don't untap during their controllers' untap steps.", true)]
#[case::weak(&untap(r#"{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":3}}]}"#), "Creatures with power 3 or greater don't untap during their controllers' untap steps.", false)]
#[case::depleted(&untap(r#"{"Each":["This",{"HasCounter":{"Other":"depletion"}}]}"#), "~ doesn't untap during your untap step if it has a depletion counter on it.", true)]
#[case::always(&untap(r#""This""#), "~ doesn't untap during your untap step if it has a depletion counter on it.", false)]
#[case::other_counter(&untap(r#"{"Each":["This",{"HasCounter":{"Other":"ice"}}]}"#), "~ doesn't untap during your untap step if it has a depletion counter on it.", false)]
#[case::hourglass(&untap(r#"{"Each":[{"HasCounter":{"Other":"hourglass"}}]}"#), "Each permanent with an hourglass counter on it doesn't untap during its controller's untap step.", true)]
#[case::asleep(&untap(r#"{"Each":[{"Type":"Creature"},{"Is":"Attached"},{"HasCounter":{"Other":"sleep"}}]}"#), "Enchanted creature doesn't untap during its controller's untap step if it has a sleep counter on it.", true)]
#[case::aura_counter(PUPA, "Enchanted creature doesn't untap during your untap step if this Aura has a pupa counter on it.", true)]
#[case::creature_counter(&PUPA.replace(r#"["This","#, r#"[{"Is":"Attached"},"#), "Enchanted creature doesn't untap during your untap step if this Aura has a pupa counter on it.", false)]
#[case::while_you_control(
    r#"{"Static":{"affected":"Attached","mods":["DoesntUntap"],"condition":{"Compare":{"left":{"Count":[{"Supertype":"Snow"},"Other",{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}"#,
    "As long as you control another snow permanent, enchanted creature doesn't untap during its controller's untap step.",
    true
)]
#[case::while_an_opponent_controls(
    r#"{"Static":{"affected":"Attached","mods":["DoesntUntap"],"condition":{"Compare":{"left":{"Count":[{"Supertype":"Snow"},"Other",{"ControlledBy":"Opponents"}]},"op":"Ge","right":1}}}}"#,
    "As long as you control another snow permanent, enchanted creature doesn't untap during its controller's untap step.",
    false
)]
fn wf27_doesnt_untap(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn attached(mods: &str) -> String {
    format!(r#"{{"Static":{{"affected":"Attached","mods":{mods}}}}}"#)
}

const ARMORER: &str = r#"{"Static":{"affected":"This","mods":["CantBeBlocked"],"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"Defending"}]},"op":"Ge","right":3}}}}"#;

#[rstest]
#[case::island(&attached(r#"[{"SetLandTypes":["Island"]}]"#), "Enchanted land is an Island.", true)]
#[case::swamp_for_island(&attached(r#"[{"SetLandTypes":["Swamp"]}]"#), "Enchanted land is an Island.", false)]
#[case::island_too(&attached(r#"[{"AddSubtype":"Island"}]"#), "Enchanted land is an Island.", false)]
#[case::three(&attached(r#"[{"SetLandTypes":["Mountain","Forest","Plains"]}]"#), "Enchanted land is a Mountain, Forest, and Plains.", true)]
#[case::chosen(&attached(r#"[{"SetLandTypes":[]},"AddChosenSubtype"]"#), "Enchanted land is the chosen type.", true)]
#[case::chosen_too(&attached(r#"["AddChosenSubtype"]"#), "Enchanted land is the chosen type.", false)]
#[case::every_type(&attached(r#"[{"ModifyPT":{"power":3,"toughness":0}},"AddAllCreatureTypes"]"#), "Equipped creature gets +3/+0 and is every creature type.", true)]
#[case::without_every_type(&attached(r#"[{"ModifyPT":{"power":3,"toughness":0}}]"#), "Equipped creature gets +3/+0 and is every creature type.", false)]
#[case::every_type_and_reach(&attached(r#"[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Reach"},"AddAllCreatureTypes"]"#), "Enchanted creature gets +2/+2, has reach, and is every creature type.", true)]
#[case::defending(ARMORER, "~ can't be blocked as long as defending player controls three or more artifacts.", true)]
#[case::defending_for_you(&ARMORER.replace("Defending", "You"), "~ can't be blocked as long as defending player controls three or more artifacts.", false)]
#[case::defending_two(&ARMORER.replace(r#""right":3"#, r#""right":2"#), "~ can't be blocked as long as defending player controls three or more artifacts.", false)]
fn wf27_statics(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn chosen(by: &str) -> String {
    format!(r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{by}}}}}"#)
}

fn prevent(when: &str) -> String {
    format!(r#"{{"Replacement":{{"when":{{"Damage":{when}}},"by":"Prevent"}}}}"#)
}

#[rstest]
#[case::card_name(&chosen(r#"{"Choose":"CardName"}"#), "As ~ enters, choose a card name.", true)]
#[case::nonland_card_name(&chosen(r#"{"Choose":"NonlandCardName"}"#), "As ~ enters, choose a nonland card name.", true)]
#[case::any_name_for_nonland(&chosen(r#"{"Choose":"CardName"}"#), "As ~ enters, choose a nonland card name.", false)]
#[case::nonland_for_noncreature(&chosen(r#"{"Choose":"NonlandCardName"}"#), "As ~ enters, choose a noncreature, nonland card name.", false)]
#[case::by_creatures(&prevent(r#"{"to":"This","source":[{"Type":"Creature"},{"In":"Battlefield"}]}"#), "Prevent all damage that would be dealt to ~ by creatures.", true)]
#[case::by_anything(&prevent(r#"{"to":"This"}"#), "Prevent all damage that would be dealt to ~ by creatures.", false)]
#[case::by_enchanted(&prevent(r#"{"source":[{"Type":"Creature"},{"Is":"Attached"}]}"#), "Prevent all damage that would be dealt by enchanted creature.", true)]
#[case::combat_by_enchanted(&prevent(r#"{"source":[{"Type":"Creature"},{"Is":"Attached"}],"combat":true}"#), "Prevent all combat damage that would be dealt by enchanted creature.", true)]
#[case::all_for_combat(&prevent(r#"{"source":[{"Type":"Creature"},{"Is":"Attached"}]}"#), "Prevent all combat damage that would be dealt by enchanted creature.", false)]
fn wf27_replacements(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn activated(effects: &str) -> String {
    format!(r#"{{"Activated":{{"cost":"{{2}}","effects":{effects}}}}}"#)
}

const THRUSH: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"ChooseValue":{"kind":"BasicLandType","as":"chosen"}},{"Continuous":{"affected":{"Target":0},"mods":[{"SetLandTypesChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#;
const OF_YOUR_CHOICE: &str =
    "{T}: Target land becomes the basic land type of your choice until end of turn.";

#[rstest]
#[case::colorless(&activated(r#"[{"Continuous":{"affected":"This","mods":[{"SetColors":[]}],"duration":"UntilEndOfTurn"}}]"#), "{2}: ~ becomes colorless until end of turn.", true)]
#[case::colorless_for_good(&activated(r#"[{"Continuous":{"affected":"This","mods":[{"SetColors":[]}],"duration":"Forever"}}]"#), "{2}: ~ becomes colorless until end of turn.", false)]
#[case::black(&activated(r#"[{"Continuous":{"affected":"This","mods":[{"SetColors":["Black"]}],"duration":"UntilEndOfTurn"}}]"#), "{2}: ~ becomes colorless until end of turn.", false)]
#[case::blocks_one_more(&activated(r#"[{"Continuous":{"affected":"This","mods":[{"BlocksAdditional":1}],"duration":"UntilEndOfTurn"}}]"#), "{2}: ~ can block an additional creature this turn.", true)]
#[case::blocks_two_more(&activated(r#"[{"Continuous":{"affected":"This","mods":[{"BlocksAdditional":2}],"duration":"UntilEndOfTurn"}}]"#), "{2}: ~ can block an additional creature this turn.", false)]
#[case::basic_land_type(THRUSH, OF_YOUR_CHOICE, true)]
#[case::any_land_type(&THRUSH.replace("BasicLandType", "LandType"), OF_YOUR_CHOICE, false)]
#[case::creature_type(&THRUSH.replace("BasicLandType", "CreatureType"), OF_YOUR_CHOICE, false)]
#[case::for_good(&THRUSH.replace("UntilEndOfTurn", "Forever"), OF_YOUR_CHOICE, false)]
#[case::another_choice(&THRUSH.replace(r#"[{"SetLandTypesChosen":"chosen"}]"#, r#"[{"SetLandTypesChosen":"other"}]"#), OF_YOUR_CHOICE, false)]
fn wf27_until_end_of_turn(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn free(condition: &str, cost: &str) -> String {
    format!(r#"{{"FreeCast":{{"condition":{condition}{cost}}}}}"#)
}

fn controls(kind: &str, who: &str) -> String {
    format!(
        r#"{{"Compare":{{"left":{{"Count":[{{"Subtype":"{kind}"}},{{"ControlledBy":"{who}"}}]}},"op":"Ge","right":1}}}}"#
    )
}

const LEGATE: &str = "If an opponent controls a Swamp and you control a Plains, you may cast ~ without paying its mana cost.";
const ANOTHER: &str = r#"{"Compare":{"left":{"Tally":{"event":"SpellCast","who":"EachPlayer","filter":[{"ControlledBy":"You"}]}},"op":"Ge","right":1}}"#;
const ATTACKING: &str =
    r#"{"Compare":{"left":{"Count":[{"Type":"Creature"},"Attacking"]},"op":"Eq","right":1}}"#;
const RED_SPELL: &str = r#"{"Compare":{"left":{"Tally":{"event":"SpellCast","who":"EachPlayer","filter":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"Color":"Red"},{"ControlledBy":"Opponents"}]}},"op":"Ge","right":1}}"#;

#[rstest]
#[case::legate(&free(&format!(r#"{{"All":[{},{}]}}"#, controls("Swamp", "Opponents"), controls("Plains", "You")), ""), LEGATE, true)]
#[case::legate_swapped(&free(&format!(r#"{{"All":[{},{}]}}"#, controls("Plains", "Opponents"), controls("Swamp", "You")), ""), LEGATE, false)]
#[case::legate_half(&free(&controls("Plains", "You"), ""), LEGATE, false)]
#[case::exactly_one_attacker(&free(ATTACKING, r#","cost":"{W}""#), "If exactly one creature is attacking, you may pay {W} rather than pay ~'s mana cost.", true)]
#[case::one_or_more_attackers(&free(&ATTACKING.replace("Eq", "Ge"), r#","cost":"{W}""#), "If exactly one creature is attacking, you may pay {W} rather than pay ~'s mana cost.", false)]
#[case::red_spell(&free(RED_SPELL, r#","cost":"{W}""#), "If an opponent cast a red instant or sorcery spell this turn, you may pay {W} rather than pay ~'s mana cost.", true)]
#[case::your_red_spell(&free(&RED_SPELL.replace("Opponents", "You"), r#","cost":"{W}""#), "If an opponent cast a red instant or sorcery spell this turn, you may pay {W} rather than pay ~'s mana cost.", false)]
#[case::two_red_spells(&free(&RED_SPELL.replace(r#""right":1"#, r#""right":2"#), r#","cost":"{W}""#), "If an opponent cast a red instant or sorcery spell this turn, you may pay {W} rather than pay ~'s mana cost.", false)]
#[case::searched(&free(r#"{"Searched":{"who":"Opponents","zone":"Library"}}"#, ""), "If an opponent searched their library this turn, you may pay {0} rather than pay ~'s mana cost.", true)]
#[case::not_your_turn(&free(r#"{"Not":"YourTurn"}"#, r#","cost":[{"ExileFromHand":{"filter":[{"Color":"Green"},"Other"]}}]"#), "If it's not your turn, you may exile a green card from your hand rather than pay ~'s mana cost.", true)]
#[case::your_turn(&free(r#""YourTurn""#, r#","cost":[{"ExileFromHand":{"filter":[{"Color":"Green"},"Other"]}}]"#), "If it's not your turn, you may exile a green card from your hand rather than pay ~'s mana cost.", false)]
#[case::cast_only_after_another(&format!(r#"{{"CastOnly":{{"condition":{ANOTHER}}}}}"#), "Cast ~ only if you've cast another spell this turn.", true)]
#[case::cast_only_after_none(&format!(r#"{{"CastOnly":{{"condition":{}}}}}"#, ANOTHER.replace(r#""Ge","right":1"#, r#""Eq","right":0"#)), "Cast ~ only if you've cast another spell this turn.", false)]
#[case::costs_less(&format!(r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{{2}}","condition":{ANOTHER}}}}}"#), "~ costs {2} less to cast if you've cast another spell this turn.", true)]
#[case::costs_more(&format!(r#"{{"CostModifier":{{"applies_to":["This"],"change":"Increase","mana":"{{2}}","condition":{ANOTHER}}}}}"#), "~ costs {2} less to cast if you've cast another spell this turn.", false)]
#[case::before_blockers(r#"{"CastOnly":{"condition":{"Any":[{"Step":"BeginCombat"},{"Step":"DeclareAttackers"}]}}}"#, "Cast ~ only during combat before blockers are declared.", true)]
#[case::after_blockers(r#"{"CastOnly":{"condition":{"Any":[{"Step":"BeginCombat"},{"Step":"DeclareAttackers"}]}}}"#, "Cast ~ only during combat after blockers are declared.", false)]
fn wf27_casts(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}
