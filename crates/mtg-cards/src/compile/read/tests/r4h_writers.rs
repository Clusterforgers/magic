//! Goal: the writers of round 4 builder h word an ability as the card prints it, and only the data that means it.
use super::*;

fn line(json: &str, clause: &str) -> bool {
    crate::compile::champion_line(&ability(json)).is_some_and(|line| same_text(&line, clause))
}

fn champion(choices: &str, then: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[
{{"Choose":{{"from":{{"Each":{choices}}},"count":{{"min":0,"max":1}},"as":"champion"}}}},
{{"If":{{"condition":{{"Compare":{{"left":{{"Count":[{{"Is":"This"}},{{"In":"Battlefield"}}]}},"op":"Eq","right":0}}}},"then":[{{"Exile":{{"what":{{"Bound":"champion"}}}}}}]}}}},
{{"Exile":{{"what":{{"Bound":"champion"}},"duration":"WhileSourceOnBattlefield"}}}},
{{"If":{{"condition":{{"Not":"Performed"}},"then":[{then}]}}}}]}}}}"#
    )
}

const SACRIFICE: &str = r#"{"Sacrifice":{"filter":["This"]}}"#;

#[rstest]
#[case::goblin(
    r#"[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Goblin",
    true
)]
#[case::elf(
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#,
    "Champion an Elf",
    true
)]
#[case::creature(
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a creature",
    true
)]
#[case::two(
    r#"[{"AnyOf":[{"Subtype":"Goblin"},{"Subtype":"Shaman"}]},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Goblin or Shaman",
    true
)]
#[case::wrong_article(
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Elf",
    false
)]
#[case::not_other(
    r#"[{"Subtype":"Goblin"},{"ControlledBy":"You"}]"#,
    "Champion a Goblin",
    false
)]
#[case::not_yours(r#"[{"Subtype":"Goblin"},"Other"]"#, "Champion a Goblin", false)]
fn champion_line(#[case] choices: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(line(&champion(choices, SACRIFICE), clause), reads);
}

/// The trigger without the exile that stays when the champion left first returns the card at once: not Champion.
#[test]
fn r5d_a_champion_that_returns_the_card_of_a_champion_gone_first_is_no_champion() {
    let goblin = r#"[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]"#;
    let early = r#"{"If":{"condition":{"Compare":{"left":{"Count":[{"Is":"This"},{"In":"Battlefield"}]},"op":"Eq","right":0}},"then":[{"Exile":{"what":{"Bound":"champion"}}}]}},"#;
    let old = champion(goblin, SACRIFICE).replace(&format!("\n{early}"), "");
    assert!(line(&champion(goblin, SACRIFICE), "Champion a Goblin"));
    assert!(!line(&old, "Champion a Goblin"));
}

fn equip(cost: &str, more: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":{cost},{more}"effects":[{{"Attach":{{"to":{{"Target":0}},"what":"This"}}}}],"targets":[{{"what":{{"Object":[{{"Type":"Creature"}},{{"ControlledBy":"You"}}]}}}}],"timing":"Sorcery"}}}}"#
    )
}

fn equip_text(abilities: &[String]) -> Option<(String, usize)> {
    let abilities: Vec<Ability> = abilities.iter().map(|json| ability(json)).collect();
    crate::compile::equip_line(&abilities)
}

const ONCE: &str = r#""limit":"OncePerTurn","#;
const LESS: &str = r#"[{"Mana":"{3}"},{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"Subtype":"Equipment"},{"ControlledBy":"You"},"Other"]}}}]"#;
const FEW: &str = r#"[{"Mana":"{2}"},{"Changed":{"change":"Reduce","mana":"{1}","per":{"When":{"condition":{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]},"op":"Le","right":1}},"otherwise":0,"then":2}}}}]"#;

#[rstest]
#[case::mana_or_mana(&[equip(r#""{2}""#, ""), equip(r#""{B}""#, "")], "Equip {2} or {B}", 2)]
#[case::mana_or_discard(&[equip(r#""{3}""#, ""), equip(r#"[{"Discard":{}}]"#, "")], "Equip\u{2014}Pay {3} or discard a card.", 2)]
#[case::life_once(&[equip(r#"[{"PayLife":3}]"#, ONCE)], "Equip\u{2014}Pay 3 life. Activate only once each turn.", 1)]
#[case::free_once(&[equip(r#""{0}""#, ONCE)], "Equip {0}. Activate only once each turn.", 1)]
#[case::sacrifice_once(
    &[equip(r#"[{"Sacrifice":{"filter":["Other",{"Not":{"Type":"Land"}}]}}]"#, ONCE)],
    "Equip\u{2014}Sacrifice another nonland permanent. Activate only once each turn.",
    1
)]
#[case::per_equipment(&[equip(LESS, "")], "Equip {3}. This ability costs {1} less to activate for each other Equipment you control.", 1)]
#[case::few_cards(&[equip(FEW, "")], "Equip {2}. This ability costs {2} less to activate if you have one or fewer cards in hand.", 1)]
fn equip_options_line(#[case] abilities: &[String], #[case] line: &str, #[case] count: usize) {
    assert_eq!(equip_text(abilities), Some((line.to_owned(), count)));
}

#[rstest]
#[case::plain(&[equip(r#""{2}""#, "")])]
#[case::other_target(&[equip(r#""{2}""#, ONCE).replace("\"Type\":\"Creature\"", "\"Type\":\"Artifact\"")])]
#[case::two_ways_with_a_limit(&[equip(r#""{2}""#, ONCE), equip(r#""{B}""#, ONCE)])]
#[case::reduction_by_one_not_the_count(&[equip(LESS, "").replace("\"Other\"", "\"Tapped\"")])]
fn equip_options_not_a_line(#[case] abilities: &[String]) {
    assert_eq!(equip_text(abilities), None);
}

#[test]
fn a_trigger_that_does_not_sacrifice_is_no_champion() {
    let json = champion(
        r#"[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]"#,
        "",
    );
    assert!(!line(&json, "Champion a Goblin"));
}
