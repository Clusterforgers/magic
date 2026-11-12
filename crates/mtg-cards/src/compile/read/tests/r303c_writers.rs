//! Goal: pack r30-3c writes a static, rule or replacement under a condition as "As long as C, S." from its twin, two
//! abilities under one condition as one sentence, a keyword with "can't be blocked", Walker tokens and "~ and at least two
//! Zombies attack"; another condition, subject, number, kind or keyword reads Differs.
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

fn on_card(pred: &str) -> String {
    format!(r#"{{"Matches":{{"subject":"This","selector":[{pred}]}}}}"#)
}

fn enters(by: &str, held: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["Other"]}}}},"by":"{by}","condition":{held}}}}}"#
    )
}

fn must_be_blocked(who: &str, held: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"Combat":{{"who":"{who}","quirk":{{"MustBeBlocked":"AtLeastOne"}}}}}},"condition":{held}}}}}"#
    )
}

fn unseen(mods: &str, pred: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":{mods},"condition":{{"Not":{}}}}}}}"#,
        on_card(pred)
    )
}

fn kinds(at_least: u32) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":2,"toughness":4}}}}],"condition":{{"Compare":{{"left":{{"Distinct":{{"of":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}],"stat":"CounterKinds"}}}},"op":"Ge","right":{at_least}}}}}}}}}"#
    )
}

fn shielded(counted: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Damage":{{"to":"This"}}}},"by":"Prevent","condition":{{"Compare":{{"left":{{"Count":{counted}}},"op":"Ge","right":1}}}}}}}}"#
    )
}

fn horse(source: &str) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Damage":{{"to":"This","source":{source}}}}},"by":"Prevent","condition":{{"Compare":{{"left":{{"Count":[{{"Type":"Creature"}},{{"ControlledBy":"You"}},"Other"]}},"op":"Ge","right":1}}}}}}}}"#
    )
}

fn walkers(count: u32, name: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"{name}","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}}}},"count":{count}}}}}]}}}}"#
    )
}

fn zombies_attack(counted: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"if":{{"AtTrigger":{{"Compare":{{"left":{{"Count":{counted}}},"op":"Ge","right":2}}}}}},"effects":[{{"Continuous":{{"affected":"This","mods":[{{"AddKeyword":"Indestructible"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    )
}

fn kili(held: &str) -> String {
    format!(
        r#"{{"FreeActivation":{{"kind":"Equip","condition":{{"All":[{held},{{"Compare":{{"left":{{"Tally":{{"event":{{"KindActivated":"Equip"}},"who":"You","filter":[]}}}},"op":"Lt","right":1}}}}]}}}}}}"#
    )
}

const ARCHELOS_TAPPED: &str = "As long as ~ is tapped, other permanents enter tapped.";
const ENKIRA: &str = "As long as ~ is equipped, it must be blocked if able.";
const RATON: &str = "As long as ~ hasn't dealt damage yet, it has hexproof and can't be blocked.";
const VETERAN: &str = "As long as there are three or more different kinds of counters among creatures you control, ~ gets +2/+4.";
const SANWELL: &str = "As long as an artifact creature you control is attacking, prevent all damage that would be dealt to ~.";
const HORSE: &str = "As long as you control another creature, prevent all damage that would be dealt to ~ by spells that target it.";
const WALKERS: &str = "When ~ enters, create two Walker tokens.";
const ZOMBIES: &str =
    "Whenever ~ and at least two Zombies attack, ~ gains indestructible until end of turn.";
const KILI: &str = "As long as you have an enduring story, you may pay {0} rather than pay the equip cost of the first equip ability you activate each turn.";
const EQUIPPED: &str = r#"[{"HasAttached":[{"Subtype":"Equipment"}]}]"#;

#[rstest]
#[case::archelos_tapped(enters("Tapped", &on_card(r#""Tapped""#)), ARCHELOS_TAPPED, true)]
#[case::archelos_untapped(
    enters("Untapped", &on_card(r#""Untapped""#)),
    "As long as ~ is untapped, other permanents enter untapped.",
    true
)]
#[case::archelos_swapped(enters("Untapped", &on_card(r#""Tapped""#)), ARCHELOS_TAPPED, false)]
#[case::archelos_other_state(enters("Tapped", &on_card(r#""Untapped""#)), ARCHELOS_TAPPED, false)]
#[case::enkira(must_be_blocked("This", &format!(r#"{{"Matches":{{"subject":"This","selector":{EQUIPPED}}}}}"#)), ENKIRA, true)]
#[case::enkira_enchanted(
    must_be_blocked(
        "This",
        r#"{"Matches":{"subject":"This","selector":[{"HasAttached":[{"Subtype":"Aura"}]}]}}"#
    ),
    ENKIRA,
    false
)]
#[case::raton(
    unseen(
        r#"[{"AddKeyword":"Hexproof"},"CantBeBlocked"]"#,
        r#""DealtDamageYet""#
    ),
    RATON,
    true
)]
#[case::raton_combat_damage(
    unseen(
        r#"[{"AddKeyword":"Hexproof"},"CantBeBlocked"]"#,
        r#""DealtCombatDamageYet""#
    ),
    RATON,
    false
)]
#[case::raton_shroud(
    unseen(r#"[{"AddKeyword":"Shroud"},"CantBeBlocked"]"#, r#""DealtDamageYet""#),
    RATON,
    false
)]
#[case::raton_blockable(
    unseen(r#"[{"AddKeyword":"Hexproof"}]"#, r#""DealtDamageYet""#),
    RATON,
    false
)]
#[case::veteran(kinds(3), VETERAN, true)]
#[case::veteran_two(kinds(2), VETERAN, false)]
#[case::sanwell(
    shielded(r#"[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"},"Attacking"]"#),
    SANWELL,
    true
)]
#[case::sanwell_any_creature(
    shielded(r#"[{"Type":"Creature"},{"ControlledBy":"You"},"Attacking"]"#),
    SANWELL,
    false
)]
#[case::sanwell_not_attacking(
    shielded(r#"[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]"#),
    SANWELL,
    false
)]
#[case::horse(horse(r#"["Spell",{"Targeting":["This"]}]"#), HORSE, true)]
#[case::horse_any_spell(horse(r#"["Spell"]"#), HORSE, false)]
#[case::walkers(walkers(2, "Walker"), WALKERS, true)]
#[case::walkers_three(walkers(3, "Walker"), WALKERS, false)]
#[case::walkers_another_name(walkers(2, "Shambler"), WALKERS, false)]
#[case::zombies(
    zombies_attack(r#"[{"Subtype":"Zombie"},"Attacking","Other"]"#),
    ZOMBIES,
    true
)]
#[case::zombies_any_creatures(
    zombies_attack(r#"[{"Type":"Creature"},"Attacking","Other"]"#),
    ZOMBIES,
    false
)]
#[case::zombies_other_kind(
    zombies_attack(r#"[{"Subtype":"Human"},"Attacking","Other"]"#),
    ZOMBIES,
    false
)]
#[case::kili(kili(r#"{"EnduringStory":"You"}"#), KILI, true)]
#[case::kili_monarch(kili(r#"{"Monarch":"You"}"#), KILI, false)]
fn conditions_and_shapes_read_back(
    #[case] json: String,
    #[case] clause: &str,
    #[case] expected: bool,
) {
    assert_eq!(written(&json, clause), expected, "{json}\n{clause}");
}

const MASAMUNE: &str =
    "As long as equipped creature is attacking, it has first strike and must be blocked if able.";
const ATTACKING: &str = r#"{"Matches":{"subject":"Attached","selector":["Attacking"]}}"#;
const BLOCKING: &str = r#"{"Matches":{"subject":"Attached","selector":["Blocking"]}}"#;

fn keyword_while(keyword: &str, held: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"Attached","mods":[{{"AddKeyword":"{keyword}"}}],"condition":{held}}}}}"#
    )
}

#[rstest]
#[case::masamune(
    keyword_while("FirstStrike", ATTACKING),
    must_be_blocked("Attached", ATTACKING),
    true
)]
#[case::other_keyword(
    keyword_while("Trample", ATTACKING),
    must_be_blocked("Attached", ATTACKING),
    false
)]
#[case::other_conditions(
    keyword_while("FirstStrike", ATTACKING),
    must_be_blocked("Attached", BLOCKING),
    false
)]
#[case::other_subject(
    keyword_while("FirstStrike", ATTACKING),
    must_be_blocked("This", ATTACKING),
    false
)]
fn two_abilities_under_one_condition(
    #[case] first: String,
    #[case] second: String,
    #[case] expected: bool,
) {
    assert_eq!(
        paired(&first, &second, MASAMUNE),
        expected,
        "{first}\n{second}"
    );
}
