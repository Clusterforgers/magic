//! Goal: the writers of choices and payments give the oracle wording of "you may" over a put from the hand or graveyard, a flicker, a returned card that changes, base power and toughness like the source and damage to itself, of a step and then "you may", of "you may pay ... If you do" and the reflexive "When you do", of "unless its controller pays" and "unless they discard", and a wrong count, kind, player, zone, duration or condition never reads back.
use super::*;

/// The wordings the checker accepts for the rules, for the message of a failed golden.
fn wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const SOULHERDER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"May":{"effects":[{"Exile":{"what":{"Target":0}}},{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}]}}"#;
const KAALIA: &str = r#"{"Triggered":{"when":{"AttacksPlayer":{"what":["This"],"at":"Opponents"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"AnyOf":[{"Subtype":"Angel"},{"Subtype":"Demon"},{"Subtype":"Dragon"}]},{"Type":"Creature"}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{"tapped":true}}}},{"JoinAttack":{"what":{"Bound":"c"}}}]}}]}}"#;
const BRASS: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Subtype":"Pirate"},{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"counters":[[{"Other":"finality"},1]]}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"SetPT":{"power":4,"toughness":4}}],"duration":"Forever"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const MAP: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"Opponents"}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}}]}}]}}"#;
const TILLER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"HasCounter":"-1/-1"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"OwnedBy":"You"},{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;
const TANAZIR: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]},"mods":[{"SetPT":{"power":{"Stat":{"of":"This","stat":"Power"}},"toughness":{"Stat":{"of":"This","stat":"Toughness"}}}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const VRONDISS: &str = r#"{"Triggered":{"when":{"DiceRolled":{"who":"You"}},"effects":[{"May":{"effects":[{"DealDamage":{"source":"This","to":"This","amount":1}}]}}]}}"#;
const GRAFTER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Draw":{}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}}]}}]}}"#;
const INVISIBLE: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Pay":{"cost":"{R}{G}{W}{U}","paid":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"toughness":0}},"CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const SPEED: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasKeyword":"Haste"}]}}],"effects":[{"Pay":{"cost":"{1}","paid":[{"Continuous":{"affected":{"Target":0},"mods":[{"CantBeBlockedBy":[{"Not":{"HasKeyword":"Haste"}}]}],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const LORTHOS: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[]},"bounds":{"min":0,"max":8}}],"effects":[{"Pay":{"cost":"{8}","paid":[{"Tap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":["DoesntUntap"],"duration":"NextUntapStep"}}]}}]}}"#;
const FROST: &str = r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"],"by":[{"ControlledBy":"Opponents"}]}},"effects":[{"Pay":{"who":{"ControllerOf":"Triggering"},"cost":"{2}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]}}"#;
const RESEARCH: &str = r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Draw":{"who":{"Target":0},"count":3}},{"Pay":{"who":{"Target":0},"cost":[{"Discard":{"filter":[{"Type":"Land"}]}}],"unpaid":[{"Discard":{"who":{"Target":0},"count":2}}]}}]}}"#;

#[rstest]
#[case::soulherder(
    SOULHERDER,
    "At the beginning of your end step, you may exile another target creature you control, then return that card to the battlefield under its owner's control."
)]
#[case::kaalia(
    KAALIA,
    "Whenever ~ attacks an opponent, you may put an Angel, Demon, or Dragon creature card from your hand onto the battlefield tapped and attacking that opponent."
)]
#[case::brass(
    BRASS,
    "At the beginning of combat on your turn, you may return target Pirate creature card from your graveyard to the battlefield with a finality counter on it. It has base power and toughness 4/4. It gains haste until end of turn."
)]
#[case::map(
    MAP,
    "Whenever a land an opponent controls enters, you may put a land card from your hand onto the battlefield."
)]
#[case::tiller(
    TILLER,
    "Whenever a creature with a -1/-1 counter on it dies, you may put a land card from your hand or graveyard onto the battlefield tapped."
)]
#[case::tanazir(
    TANAZIR,
    "Whenever ~ attacks, you may have the base power and toughness of other creatures you control become equal to ~'s power and toughness until end of turn."
)]
#[case::vrondiss(
    VRONDISS,
    "Whenever you roll one or more dice, you may have ~ deal 1 damage to itself."
)]
#[case::grafter(
    GRAFTER,
    "At the beginning of your end step, draw a card, then you may put a land card from your hand onto the battlefield."
)]
#[case::invisible(
    INVISIBLE,
    "Whenever you attack, you may pay {R}{G}{W}{U}. When you do, target creature gets +1/+0 until end of turn for each creature you control and can't be blocked this turn."
)]
#[case::speed(
    SPEED,
    "Whenever you cast a noncreature spell, you may pay {1}. When you do, target creature with haste can't be blocked this turn except by creatures with haste."
)]
#[case::lorthos(
    LORTHOS,
    "Whenever ~ attacks, you may pay {8}. If you do, tap up to eight target permanents. Those permanents don't untap during their controllers' next untap steps."
)]
#[case::frost(
    FROST,
    "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}."
)]
#[case::research(
    RESEARCH,
    "Target player draws three cards. Then that player discards two cards unless they discard a land card."
)]
fn writes_the_printed_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::flicker_kept_by_you(
    SOULHERDER,
    "Battlefield\":{}",
    "Battlefield\":{\"under\":\"You\"}",
    "under its owner's control"
)]
#[case::kaalia_from_graveyard(
    KAALIA,
    "{\"In\":\"Hand\"}",
    "{\"In\":\"Graveyard\"}",
    "from your hand"
)]
#[case::kaalia_not_attacking(
    KAALIA,
    ",{\"JoinAttack\":{\"what\":{\"Bound\":\"c\"}}}",
    "",
    "attacking"
)]
#[case::brass_other_size(
    BRASS,
    "\"power\":4,\"toughness\":4",
    "\"power\":5,\"toughness\":5",
    "4/4"
)]
#[case::brass_haste_for_good(
    BRASS,
    "\"Haste\"}],\"duration\":\"UntilEndOfTurn\"",
    "\"Haste\"}],\"duration\":\"Forever\"",
    "until end of turn"
)]
#[case::map_artifact(
    MAP,
    "{\"Type\":\"Land\"},{\"In\":\"Hand\"}",
    "{\"Type\":\"Artifact\"},{\"In\":\"Hand\"}",
    "land card"
)]
#[case::tiller_untapped(
    TILLER,
    "\"tapped\":true",
    "\"tapped\":false",
    "onto the battlefield tapped"
)]
#[case::tiller_their_cards(
    TILLER,
    "{\"OwnedBy\":\"You\"}",
    "{\"OwnedBy\":\"Opponents\"}",
    "from your hand"
)]
#[case::tanazir_for_good(TANAZIR, "\"UntilEndOfTurn\"", "\"Forever\"", "until end of turn")]
#[case::tanazir_toughness_twice(
    TANAZIR,
    "\"stat\":\"Power\"}},\"toughness",
    "\"stat\":\"Toughness\"}},\"toughness",
    "become equal"
)]
#[case::vrondiss_two(VRONDISS, "\"amount\":1", "\"amount\":2", "1 damage")]
#[case::grafter_two_cards(GRAFTER, "{\"Draw\":{}}", "{\"Draw\":{\"count\":2}}", "draw a card")]
#[case::invisible_other_cost(INVISIBLE, "{R}{G}{W}{U}", "{R}{G}{W}", "{R}{G}{W}{U}")]
#[case::speed_by_flyers(
    SPEED,
    "{\"Not\":{\"HasKeyword\":\"Haste\"}}",
    "{\"Not\":{\"HasKeyword\":\"Flying\"}}",
    "except by creatures with haste"
)]
#[case::speed_for_good(SPEED, "\"UntilEndOfTurn\"", "\"Forever\"", "this turn")]
#[case::lorthos_one(LORTHOS, "\"max\":8", "\"max\":1", "eight")]
#[case::frost_three(FROST, "{2}", "{3}", "{2}")]
#[case::frost_you_pay(
    FROST,
    "\"who\":{\"ControllerOf\":\"Triggering\"},",
    "",
    "its controller"
)]
#[case::research_creature(
    RESEARCH,
    "{\"Type\":\"Land\"}",
    "{\"Type\":\"Creature\"}",
    "land card"
)]
#[case::research_three(RESEARCH, "\"count\":2", "\"count\":3", "two cards")]
fn a_changed_ability_does_not_read_back(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] printed: &str,
) {
    assert!(json.contains(from), "{from}");
    let changed = json.replacen(from, to, 1);
    let clause = match json {
        SOULHERDER => {
            "At the beginning of your end step, you may exile another target creature you control, then return that card to the battlefield under its owner's control."
        }
        KAALIA => {
            "Whenever ~ attacks an opponent, you may put an Angel, Demon, or Dragon creature card from your hand onto the battlefield tapped and attacking that opponent."
        }
        BRASS => {
            "At the beginning of combat on your turn, you may return target Pirate creature card from your graveyard to the battlefield with a finality counter on it. It has base power and toughness 4/4. It gains haste until end of turn."
        }
        MAP => {
            "Whenever a land an opponent controls enters, you may put a land card from your hand onto the battlefield."
        }
        TILLER => {
            "Whenever a creature with a -1/-1 counter on it dies, you may put a land card from your hand or graveyard onto the battlefield tapped."
        }
        TANAZIR => {
            "Whenever ~ attacks, you may have the base power and toughness of other creatures you control become equal to ~'s power and toughness until end of turn."
        }
        VRONDISS => "Whenever you roll one or more dice, you may have ~ deal 1 damage to itself.",
        GRAFTER => {
            "At the beginning of your end step, draw a card, then you may put a land card from your hand onto the battlefield."
        }
        INVISIBLE => {
            "Whenever you attack, you may pay {R}{G}{W}{U}. When you do, target creature gets +1/+0 until end of turn for each creature you control and can't be blocked this turn."
        }
        SPEED => {
            "Whenever you cast a noncreature spell, you may pay {1}. When you do, target creature with haste can't be blocked this turn except by creatures with haste."
        }
        LORTHOS => {
            "Whenever ~ attacks, you may pay {8}. If you do, tap up to eight target permanents. Those permanents don't untap during their controllers' next untap steps."
        }
        FROST => {
            "Whenever this creature becomes the target of a spell or ability an opponent controls, counter that spell or ability unless its controller pays {2}."
        }
        _ => {
            "Target player draws three cards. Then that player discards two cards unless they discard a land card."
        }
    };
    assert!(clause.contains(printed), "{printed}");
    let changed = ability(&changed);
    assert!(
        !written_variants(&changed)
            .iter()
            .any(|wording| same_text(wording, clause)),
        "{to}"
    );
}

/// "If you do" stays for a payment whose steps use no target, and "When you do" is only for one whose steps do.
#[test]
fn when_you_do_only_for_targets() {
    let draw = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{1}","paid":[{"Draw":{}}]}}]}}"#;
    assert!(written(
        draw,
        "Whenever ~ attacks, you may pay {1}. If you do, draw a card."
    ));
    assert!(!written(
        draw,
        "Whenever ~ attacks, you may pay {1}. When you do, draw a card."
    ));
    assert!(written(
        INVISIBLE,
        "Whenever you attack, you may pay {R}{G}{W}{U}. If you do, target creature gets +1/+0 until end of turn for each creature you control and can't be blocked this turn."
    ));
}
