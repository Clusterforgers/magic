//! Goal: r7d writers read back on-trust statics and effects (an impulse draw under any binding, "an opponent controls",
//! combat rules of the card and for the turn, a creature exiled instead of dying, a spell exiled instead of countered, a
//! change while the source stays tapped, a target that blocks or can't block the source, a land type for the turn), and
//! a rule of another meaning reads `Differs`.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const CARAVAN: &str = r#"{"Activated":{"cost":"{1}{U}{U}","effects":[{"ExileTop":{"as":"exiled"}},{"Rule":{"duration":"UntilEndOfTurn","rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled"},"play":true}}}}]}}"#;
const PERCUSSIONIST: &str = r#"{"Triggered":{"effects":[{"ExileTop":{"as":"exiled"}},{"Rule":{"duration":"UntilEndOfYourNextTurn","rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled"},"play":true}}}}],"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}}}"#;
const BEASTS: &str = r#"{"Static":{"affected":"This","condition":{"Compare":{"left":{"Count":[{"Color":"White"},{"ControlledBy":"Opponents"},{"Not":"Token"}]},"op":"Ge","right":1}},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const RUST_TICK: &str = r#"{"Rule":{"rule":{"Combat":{"quirk":"MayStayTapped","who":"This"}}}}"#;
const SHINEN: &str =
    r#"{"Rule":{"rule":{"Combat":{"quirk":{"MustBeBlocked":"Every"},"who":"This"}}}}"#;
const SATYR: &str = r#"{"Activated":{"cost":"{3}{G}","effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":{"MustBeBlocked":"AtLeastOne"},"who":{"Target":0}}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const GROWTH: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":5,"toughness":5}}]}},{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":{"MustBeBlocked":"AtLeastOne"},"who":{"Target":0}}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const SCENT: &str = r#"{"Spell":{"effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":{"MustBeBlocked":"Every"},"who":{"Target":0}}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const NESTGUARD: &str = r#"{"Activated":{"cost":"{2}{G}","effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"Combat":{"quirk":"AttackDespiteDefender","who":"This"}}}}]}}"#;
const BARGAIN: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":5,"to":{"Target":0}}},{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Is":{"Target":0}},{"Type":"Creature"}]}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const AUTHORITY: &str = r#"{"Spell":{"effects":[{"Shield":{"by":{"MoveTo":"Exile"},"duration":"UntilEndOfTurn","when":{"Countered":{"what":[{"Is":{"Target":0}},"Spell"]}}}},{"Counter":{"what":{"Target":0}}}],"targets":[{"what":{"Object":["Spell"]}}]}}"#;
const QUARTERS: &str = r#"{"Activated":{"cost":[{"Mana":"{6}"},"Tap"],"effects":[{"Tap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"duration":"WhileSourceTapped","mods":["DoesntUntap"]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const COURIER: &str = r#"{"Activated":{"cost":[{"Mana":"{2}{G}"},"Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"WhileSourceTapped","mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Subtype":"Elf"}]}}]}}"#;
const BLOODSCALE: &str = r#"{"Activated":{"cost":"{2}{G}","effects":[{"Rule":{"duration":"UntilEndOfTurn","rule":{"MustBlockAttacker":{"attacker":"This","who":{"Target":0}}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const CRAWLER: &str = r#"{"Activated":{"cost":"{1}{R}","effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"CanBlockOnly":[{"Not":{"Is":"This"}}]}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const RECLUSE: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"SetLandTypes":["Forest"]}]}}],"targets":[{"what":{"Object":[{"Type":"Land"}]}}]}}"#;

#[rstest]
#[case::impulse_any_binding(
    CARAVAN,
    "{1}{U}{U}: Exile the top card of your library. Until end of turn, you may play that card."
)]
#[case::impulse_this_turn(
    CARAVAN,
    "{1}{U}{U}: Exile the top card of your library. You may play it this turn."
)]
#[case::impulse_next_turn_last(
    PERCUSSIONIST,
    "When this creature dies, exile the top card of your library. You may play it until the end of your next turn."
)]
#[case::an_opponent_controls(
    BEASTS,
    "This creature gets +1/+1 as long as an opponent controls a nontoken white permanent."
)]
#[case::may_stay_tapped(
    RUST_TICK,
    "You may choose not to untap this creature during your untap step."
)]
#[case::all_able_block_itself(SHINEN, "All creatures able to block this creature do so.")]
#[case::must_be_blocked(SATYR, "{3}{G}: Target creature must be blocked this turn if able.")]
#[case::changed_and_blocked(
    GROWTH,
    "Target creature gets +5/+5 until end of turn and must be blocked this turn if able."
)]
#[case::changed_then_blocked(
    GROWTH,
    "Target creature gets +5/+5 until end of turn. It must be blocked this turn if able."
)]
#[case::all_able_block(SCENT, "All creatures able to block target creature this turn do so.")]
#[case::despite_defender(
    NESTGUARD,
    "{2}{G}: This creature can attack this turn as though it didn't have defender."
)]
#[case::exiled_instead_of_dying(
    BARGAIN,
    "~ deals 5 damage to target creature. If that creature would die this turn, exile it instead."
)]
#[case::planeswalker_exiled_instead_of_dying(
    &BARGAIN
        .replace(r#""Object":[{"Type":"Creature"}]"#, r#""Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#)
        .replace(r#"{"Is":{"Target":0}},{"Type":"Creature"}"#, r#"{"Is":{"Target":0}},{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}"#),
    "~ deals 5 damage to target creature or planeswalker. If that creature or planeswalker would die this turn, exile it instead."
)]
#[case::exiled_instead_of_countered(
    AUTHORITY,
    "Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard."
)]
#[case::held_while_tapped(
    QUARTERS,
    "{6}, {T}: Tap target creature. It doesn't untap during its controller's untap step for as long as this artifact remains tapped."
)]
#[case::held_that_creature(
    QUARTERS,
    "{6}, {T}: Tap target creature. That creature doesn't untap during its controller's untap step for as long as this artifact remains tapped."
)]
#[case::pumped_while_tapped(
    COURIER,
    "{2}{G}, {T}: Target Elf creature gets +2/+2 and has trample for as long as this creature remains tapped."
)]
#[case::blocks_the_source(
    BLOODSCALE,
    "{2}{G}: Target creature blocks this creature this turn if able."
)]
#[case::cant_block_the_source(
    CRAWLER,
    "{1}{R}: Target creature can't block this creature this turn."
)]
#[case::land_type(RECLUSE, "{T}: Target land becomes a Forest until end of turn.")]
fn r7d_the_rule_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
// The permission names another binding than the cards exiled.
#[case::impulse_other_binding(
    &CARAVAN.replace(r#""only":{"Bound":"exiled"}"#, r#""only":{"Bound":"other"}"#),
    "{1}{U}{U}: Exile the top card of your library. Until end of turn, you may play that card."
)]
#[case::impulse_other_duration(
    PERCUSSIONIST,
    "When this creature dies, exile the top card of your library. You may play it this turn."
)]
#[case::you_control(
    &BEASTS.replace(r#""ControlledBy":"Opponents""#, r#""ControlledBy":"You""#),
    "This creature gets +1/+1 as long as an opponent controls a nontoken white permanent."
)]
#[case::other_quirk(
    &RUST_TICK.replace("MayStayTapped", "CantAttackAlone"),
    "You may choose not to untap this creature during your untap step."
)]
#[case::one_blocker_for_all(
    &SHINEN.replace(r#"{"MustBeBlocked":"Every"}"#, r#"{"MustBeBlocked":"AtLeastOne"}"#),
    "All creatures able to block this creature do so."
)]
#[case::every_for_one(
    &SATYR.replace("AtLeastOne", "Every"),
    "{3}{G}: Target creature must be blocked this turn if able."
)]
#[case::forever(
    &SATYR.replace("UntilEndOfTurn", "Forever"),
    "{3}{G}: Target creature must be blocked this turn if able."
)]
// The rule is on the source, the change on the target.
#[case::other_creature_blocked(
    &GROWTH.replace(r#""who":{"Target":0}"#, r#""who":"This""#),
    "Target creature gets +5/+5 until end of turn and must be blocked this turn if able."
)]
// A creature that is not the target is exiled.
#[case::any_creature_exiled(
    &BARGAIN.replace(r#"[{"Is":{"Target":0}},{"Type":"Creature"}]"#, r#"[{"Type":"Creature"}]"#),
    "~ deals 5 damage to target creature. If that creature would die this turn, exile it instead."
)]
// A planeswalker target that would die is not exiled by a creature shield.
#[case::planeswalker_target(
    &BARGAIN.replace(r#""Object":[{"Type":"Creature"}]"#, r#""Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]"#),
    "~ deals 5 damage to target creature or planeswalker. If that creature or planeswalker would die this turn, exile it instead."
)]
#[case::countered_to_hand(
    &AUTHORITY.replace(r#"{"MoveTo":"Exile"}"#, r#"{"MoveTo":"Hand"}"#),
    "Counter target spell. If that spell is countered this way, exile it instead of putting it into its owner's graveyard."
)]
#[case::held_through_next_untap(
    &QUARTERS.replace("WhileSourceTapped", "UntilEndOfTurn"),
    "{6}, {T}: Tap target creature. It doesn't untap during its controller's untap step for as long as this artifact remains tapped."
)]
#[case::pumped_for_the_turn(
    &COURIER.replace("WhileSourceTapped", "UntilEndOfTurn"),
    "{2}{G}, {T}: Target Elf creature gets +2/+2 and has trample for as long as this creature remains tapped."
)]
#[case::blocks_another(
    &BLOODSCALE.replace(r#""attacker":"This""#, r#""attacker":"Triggering""#),
    "{2}{G}: Target creature blocks this creature this turn if able."
)]
#[case::cant_block_any(
    &CRAWLER.replace(r#"[{"Not":{"Is":"This"}}]"#, r#"[{"Is":"This"}]"#),
    "{1}{R}: Target creature can't block this creature this turn."
)]
#[case::other_land_type(RECLUSE, "{T}: Target land becomes an Island until end of turn.")]
#[case::land_type_for_good(
    &RECLUSE.replace("UntilEndOfTurn", "Forever"),
    "{T}: Target land becomes a Forest until end of turn."
)]
fn r7d_a_rule_of_another_meaning_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

/// Prints `id<TAB>state<TAB>closest wording` for each `id<TAB>clause<TAB>ability json` line of the file in `ZZ_PROBE`.
#[test]
#[ignore = "probe"]
fn zz_probe() {
    let Ok(path) = std::env::var("ZZ_PROBE") else {
        return;
    };
    let all = std::env::var("ZZ_ALL").is_ok();
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(id), Some(clause), Some(json)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(read) = serde_json::from_str::<Ability>(json) else {
            println!("{id}\tbad\t");
            continue;
        };
        let wordings = written_variants(&read);
        let state = if wordings.iter().any(|w| same_text(w, clause)) {
            "written"
        } else if wordings.is_empty() {
            "none"
        } else {
            "differs"
        };
        if all {
            for w in &wordings {
                println!("{id}\t{state}\t{w}");
            }
        } else {
            println!(
                "{id}\t{state}\t{}",
                wordings.first().cloned().unwrap_or_default()
            );
        }
    }
}

const TORMENT: &str = r#"{"Static":{"affected":"Attached","condition":{"Matches":{"selector":[{"Type":"Creature"}],"subject":"Attached"}},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}},"CantBlock"]}}"#;
const TORMENT_TEXT: &str =
    "As long as enchanted permanent is a creature, it gets -1/-1 and can't block.";

#[rstest]
#[case::creature_condition(TORMENT, true)]
// Without the condition the static reaches a noncreature permanent (r6 audit F1).
#[case::no_condition(&TORMENT.replace(r#""condition":{"Matches":{"selector":[{"Type":"Creature"}],"subject":"Attached"}},"#, ""), false)]
#[case::other_condition(&TORMENT.replace(r#"{"Type":"Creature"}"#, r#"{"Type":"Artifact"}"#), false)]
fn r7d_the_creature_condition_of_the_attached_permanent_is_said_first(
    #[case] json: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, TORMENT_TEXT), reads, "{json}");
}
