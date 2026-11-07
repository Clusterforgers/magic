//! Goal: the round 7 Differs wordings ("can't attack unless you control", "costs less to cast if", mana colors in any order, a filter land's pairs, "That creature", "unless they pay", two changes swapped, "and/or", "you may have", "each" for "all", the controller of a land or a spell, "can block only", "except by", "then" for the last step) read a printed clause back to the same rule, and a rule of another meaning still reads Differs.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const RHONAS: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"Compare":{"op":"Ge","stat":"Power","to":4}}]},"op":"Eq","right":0}},"mods":["CantAttack","CantBlock"]}}"#;
const WARGS: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"Subtype":"Wolf"},{"ControlledBy":"You"},"Other"]},"op":"Le","right":1}},"mods":["CantAttack"]}}"#;
const FALCON: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"AnyOf":[{"Subtype":"Knight"},{"Subtype":"Soldier"}]},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"mods":["CantAttack"]}}"#;
const BRAWLERS: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"},"Untapped"]},"op":"Ge","right":1}},"mods":["CantBlock"]}}"#;
const HELLRAISER: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","condition":{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":9}},"mana":"{3}"}}"#;
const THE_END: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","condition":{"Compare":{"left":{"Life":"You"},"op":"Le","right":5}},"mana":"{2}"}}"#;
const ICE_OUT: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","condition":"AdditionalCostPaid","mana":"{1}"}}"#;
const HIERARCH: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"OneOf":["White","Blue","Green"]}}}]}}"#;
const HEATH: &str = r#"{"Activated":{"cost":[{"Mana":"{W/B}"},"Tap"],"effects":[{"AddMana":{"mana":{"AnyCombination":{"among":["White","Black"],"count":2}}}}]}}"#;
const OVERTAKER: &str = r#"{"Activated":{"cost":[{"Mana":"{3}{U}"},"Tap",{"Discard":{}}],"effects":[{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetController":"You"}]}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Haste"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const BARRIER: &str = r#"{"Triggered":{"effects":[{"Pay":{"cost":"{1}","unpaid":[{"Sacrifice":{"filter":[],"who":"Triggering"}}],"who":"Triggering"}}],"when":{"SpellCast":{"what":[{"Type":"Creature"}]}}}}"#;
const HUMILITY: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"SetPT":{"power":1,"toughness":1}},"LoseAbilities"]}}"#;
const SURGE: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":0}},{"AddKeyword":"Trample"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const PAINTMAGE: &str = r#"{"Triggered":{"effects":[{"AddMana":{"mana":{"Fixed":["Blue","Red"]},"only":[{"Spell":{"types":["Instant","Sorcery"]}}]}}],"when":{"StepBegins":{"step":"Main1","whose":"You"}}}}"#;
const NULLS: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Discard":{"who":"Triggering"}}]}}],"if":{"Compare":{"left":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"when":{"DealsDamage":{"combat":true,"source":["This"],"to":"EachPlayer"}}}}"#;
const LEAFKIN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"op":"Ge","stat":"Power","to":4}}]}}}]}}"#;
const COPPERHORN: &str = r#"{"Triggered":{"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}}],"when":{"Attacks":{"what":["This"]}}}}"#;
const CRYOCLASM: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"DealDamage":{"amount":3,"to":{"ControllerOf":{"Target":0}}}}],"targets":[{"what":{"Object":[{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"}]}]}}]}}"#;
const IONIZE: &str = r#"{"Spell":{"effects":[{"Counter":{"what":{"Target":0}}},{"DealDamage":{"amount":2,"to":{"ControllerOf":{"Target":0}}}}],"targets":[{"what":{"Object":["Spell"]}}]}}"#;
const STEALTH: &str = r#"{"Spell":{"effects":[{"PutCounters":{"count":2,"counter":"+1/+1","on":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":["CantBeBlocked"]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]}}"#;
const CHAOSPHERE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}]}}"#;
const BRACERS: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]}]}}"#;
const CLOAK: &str = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Phasing"},{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"Subtype":"Wall"}}]}]}}"#;
const RITES: &str = r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"LoseLife":{"amount":2}},{"Mill":{"count":2}}]}}"#;

#[rstest]
#[case::unless_another(
    RHONAS,
    "~ can't attack or block unless you control another creature with power 4 or greater.",
    true
)]
#[case::unless_other_power(&RHONAS.replace(r#""to":4"#, r#""to":3"#), "~ can't attack or block unless you control another creature with power 4 or greater.", false)]
#[case::unless_more(
    WARGS,
    "This creature can't attack unless you control two or more other Wolves.",
    true
)]
#[case::unless_other_count(&WARGS.replace(r#""right":1"#, r#""right":2"#), "This creature can't attack unless you control two or more other Wolves.", false)]
#[case::unless_either(
    FALCON,
    "This creature can't attack unless you control a Knight or a Soldier.",
    true
)]
#[case::unless_block(
    FALCON,
    "This creature can't block unless you control a Knight or a Soldier.",
    false
)]
#[case::cant_if(
    BRAWLERS,
    "This creature can't block if you control an untapped land.",
    true
)]
#[case::cant_if_tapped(&BRAWLERS.replace(r#""Untapped""#, r#""Tapped""#), "This creature can't block if you control an untapped land.", false)]
#[case::costs_if_cards(
    HELLRAISER,
    "This spell costs {3} less to cast if you have nine or more cards in your graveyard.",
    true
)]
#[case::costs_if_other_count(&HELLRAISER.replace(r#""right":9"#, r#""right":8"#), "This spell costs {3} less to cast if you have nine or more cards in your graveyard.", false)]
#[case::costs_if_life(
    THE_END,
    "This spell costs {2} less to cast if your life total is 5 or less.",
    true
)]
#[case::costs_if_more_life(&THE_END.replace(r#""op":"Le""#, r#""op":"Ge""#), "This spell costs {2} less to cast if your life total is 5 or less.", false)]
#[case::bargained(ICE_OUT, "This spell costs {1} less to cast if it's bargained.", true)]
#[case::colors_any_order(HIERARCH, "{T}: Add {G}, {W}, or {U}.", true)]
#[case::other_colors(&HIERARCH.replace("Blue", "Black"), "{T}: Add {G}, {W}, or {U}.", false)]
#[case::filter_pairs(HEATH, "{W/B}, {T}: Add {W}{W}, {W}{B}, or {B}{B}.", true)]
#[case::filter_other(&HEATH.replace("Black", "Red"), "{W/B}, {T}: Add {W}{W}, {W}{B}, or {B}{B}.", false)]
#[case::that_creature(
    OVERTAKER,
    "{3}{U}, {T}, Discard a card: Untap target creature and gain control of it until end of turn. That creature gains haste until end of turn.",
    true
)]
#[case::that_creature_trample(&OVERTAKER.replace("Haste", "Trample"), "{3}{U}, {T}, Discard a card: Untap target creature and gain control of it until end of turn. That creature gains haste until end of turn.", false)]
#[case::they_pay(
    BARRIER,
    "Whenever a player casts a creature spell, that player sacrifices a permanent of their choice unless they pay {1}.",
    true
)]
#[case::they_pay_more(&BARRIER.replace(r#""cost":"{1}""#, r#""cost":"{2}""#), "Whenever a player casts a creature spell, that player sacrifices a permanent of their choice unless they pay {1}.", false)]
#[case::loses_first(
    HUMILITY,
    "All creatures lose all abilities and have base power and toughness 1/1.",
    true
)]
#[case::loses_other_size(&HUMILITY.replace(r#""power":1"#, r#""power":2"#), "All creatures lose all abilities and have base power and toughness 1/1.", false)]
#[case::gains_first(
    SURGE,
    "Target creature gains trample and gets +X/+0 until end of turn, where X is that creature's mana value.",
    true
)]
#[case::gains_other(&SURGE.replace("Trample", "Flying"), "Target creature gains trample and gets +X/+0 until end of turn, where X is that creature's mana value.", false)]
#[case::instant_and_sorcery(
    PAINTMAGE,
    "At the beginning of your first main phase, add {U}{R}. Spend this mana only to cast instant and sorcery spells.",
    true
)]
#[case::instant_only(&PAINTMAGE.replace(r#""Instant","Sorcery""#, r#""Instant""#), "At the beginning of your first main phase, add {U}{R}. Spend this mana only to cast instant and sorcery spells.", false)]
#[case::may_have(
    NULLS,
    "Whenever this creature deals combat damage to a player, if you control a Swamp, you may have that player discard a card.",
    true
)]
#[case::must_discard(&NULLS.replace(r#"{"May":{"effects":[{"Discard":{"who":"Triggering"}}]}}"#, r#"{"Discard":{"who":"Triggering"}}"#), "Whenever this creature deals combat damage to a player, if you control a Swamp, you may have that player discard a card.", false)]
#[case::controller_last(
    LEAFKIN,
    "{T}: Add {G} for each creature with power 4 or greater you control.",
    true
)]
#[case::controller_opponents(&LEAFKIN.replace(r#""ControlledBy":"You""#, r#""ControlledBy":"Opponents""#), "{T}: Add {G} for each creature with power 4 or greater you control.", false)]
#[case::untap_each(
    COPPERHORN,
    "Whenever this creature attacks, untap each other creature you control.",
    true
)]
#[case::untap_each_with_self(&COPPERHORN.replace(r#","Other""#, ""), "Whenever this creature attacks, untap each other creature you control.", false)]
#[case::land_controller(
    CRYOCLASM,
    "Destroy target Plains or Island. This spell deals 3 damage to that land's controller.",
    true
)]
#[case::land_controller_more(&CRYOCLASM.replace(r#""amount":3"#, r#""amount":2"#), "Destroy target Plains or Island. This spell deals 3 damage to that land's controller.", false)]
#[case::spell_controller(
    IONIZE,
    "Counter target spell. This spell deals 2 damage to that spell's controller.",
    true
)]
#[case::that_creature_unblocked(
    STEALTH,
    "Put two +1/+1 counters on target creature you control. That creature can't be blocked this turn.",
    true
)]
#[case::that_creature_one_counter(&STEALTH.replace(r#""count":2,"#, ""), "Put two +1/+1 counters on target creature you control. That creature can't be blocked this turn.", false)]
#[case::block_only(
    CHAOSPHERE,
    "Creatures with flying can block only creatures with flying.",
    true
)]
#[case::block_only_reach(&CHAOSPHERE.replace(r#"{"HasKeyword":"Flying"}]}]"#, r#"{"HasKeyword":"Reach"}]}]"#), "Creatures with flying can block only creatures with flying.", false)]
#[case::except_by_keyword(
    BRACERS,
    "Enchanted creature gets +1/+1 and can't be blocked except by creatures with flying.",
    true
)]
#[case::except_by_subtype(
    CLOAK,
    "Enchanted creature has phasing and can't be blocked except by Walls.",
    true
)]
#[case::except_by_other(&CLOAK.replace("Wall", "Elf"), "Enchanted creature has phasing and can't be blocked except by Walls.", false)]
#[case::then_last(RITES, "You draw two cards, lose 2 life, then mill two cards.", true)]
#[case::then_last_more(&RITES.replace(r#"{"Mill":{"count":2}}"#, r#"{"Mill":{"count":3}}"#), "You draw two cards, lose 2 life, then mill two cards.", false)]
fn r7a_wordings(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{clause}");
}
