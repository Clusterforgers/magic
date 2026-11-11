//! Goal: the writers of ep33 write the card's words and nothing near them. "Whenever you collect evidence" is the action of the card's controller, and a card put onto the battlefield face down is said with its controller and, from the graveyard, beside a face-up return; another player, action, effect, controller or way of entering does not read as it.
use super::*;

const EXAMINER: &str = "Whenever you collect evidence, investigate.";

fn collected(who: &str, action: &str, effect: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"Did":{{"action":"{action}","who":"{who}"}}}},"effects":[{effect}]}}}}"#
    ))
}

const CLUE: &str = r#"{"CreateToken":{"token":{"Named":"clue"}}}"#;

#[rstest]
#[case::examiner(collected("You", "CollectedEvidence", CLUE), EXAMINER, true)]
#[case::an_opponent(collected("Opponents", "CollectedEvidence", CLUE), EXAMINER, false)]
#[case::any_player(collected("EachPlayer", "CollectedEvidence", CLUE), EXAMINER, false)]
#[case::another_action(collected("You", "Surveilled", CLUE), EXAMINER, false)]
#[case::another_effect(
    collected("You", "CollectedEvidence", r#"{"Draw":{}}"#),
    EXAMINER,
    false
)]
#[case::opponent_words(
    collected("Opponents", "CollectedEvidence", CLUE),
    "Whenever an opponent collects evidence, investigate.",
    true
)]
fn ep33_collected_evidence_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const PHOENIX: &str = "When ~ dies, return it to the battlefield face down under your control.";

const RAPTOR: &str = "Whenever a permanent you control is turned face up, you may return ~ from your graveyard to the battlefield face up or face down.";

fn dies(to: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"Move":{{"what":"This","to":{to}}}}}]}}}}"#
    ))
}

fn raptor(up: &str, down: &str, may: bool) -> Ability {
    let either = format!(
        r#"{{"Either":{{"first":[{{"Move":{{"what":"This","to":{up}}}}}],"second":[{{"Move":{{"what":"This","to":{down}}}}}]}}}}"#
    );
    let effect = if may {
        format!(r#"{{"May":{{"effects":[{either}]}}}}"#)
    } else {
        either
    };
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"TurnedFaceUp":{{"what":[{{"ControlledBy":"You"}}]}}}},"from":"Graveyard","effects":[{effect}]}}}}"#
    ))
}

const UP: &str = r#"{"Battlefield":{}}"#;
const DOWN: &str = r#"{"BattlefieldFaceDown":{}}"#;

#[rstest]
#[case::phoenix(dies(r#"{"BattlefieldFaceDown":{"under":"You"}}"#), PHOENIX, true)]
#[case::owners_control(
    dies(DOWN),
    "When ~ dies, return it to the battlefield face down under its owner's control.",
    true
)]
#[case::under_the_owner(dies(DOWN), PHOENIX, false)]
#[case::face_up(dies(r#"{"Battlefield":{"under":"You"}}"#), PHOENIX, false)]
#[case::under_an_opponent(
    dies(r#"{"BattlefieldFaceDown":{"under":"Opponents"}}"#),
    PHOENIX,
    false
)]
#[case::raptor(raptor(UP, DOWN, true), RAPTOR, true)]
#[case::raptor_must(raptor(UP, DOWN, false), RAPTOR, false)]
#[case::raptor_both_down(raptor(DOWN, DOWN, true), RAPTOR, false)]
#[case::raptor_tapped(
    raptor(r#"{"Battlefield":{"tapped":true}}"#, DOWN, true),
    RAPTOR,
    false
)]
#[case::raptor_under_you(
    raptor(UP, r#"{"BattlefieldFaceDown":{"under":"You"}}"#, true),
    RAPTOR,
    false
)]
fn ep33_face_down_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

const PIPES: &str = "When ~ enters, manifest dread twice.";

/// The four steps of manifest dread by `who`, their names numbered `n`.
fn dread(n: u32, who: &str) -> String {
    format!(
        r#"{{"LookAtTop":{{"who":{who},"count":2,"as":"looked{n}"}}}},{{"Choose":{{"who":{who},"from":{{"Bound":"looked{n}"}},"as":"kept{n}","rest_as":"rest{n}"}}}},{{"Move":{{"what":{{"Bound":"rest{n}"}},"to":"Graveyard"}}}},{{"Manifest":{{"who":{who}}}}}"#
    )
}

fn enters(effects: &str) -> Ability {
    ability(&format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[{effects}]}}}}"#
    ))
}

const YOU: &str = r#""You""#;

#[rstest]
#[case::twice(enters(&format!("{},{}", dread(1, YOU), dread(2, YOU))), PIPES, true)]
#[case::once(enters(&dread(1, YOU)), PIPES, false)]
#[case::three_times(
    enters(&format!("{},{},{}", dread(1, YOU), dread(2, YOU), dread(3, YOU))),
    PIPES,
    false
)]
#[case::by_an_opponent(
    enters(&format!("{},{}", dread(1, YOU), dread(2, r#""Opponents""#))),
    PIPES,
    false
)]
#[case::then_a_draw(
    enters(&format!("{},{},{{\"Draw\":{{}}}}", dread(1, YOU), dread(2, YOU))),
    PIPES,
    false
)]
fn ep33_manifest_dread_twice_near_misses_differ(
    #[case] ability: Ability,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability);
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

/// "{T}: Add {U}." with the uses of the mana.
fn mana(only: &str) -> Ability {
    ability(&format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"Fixed":["Blue"]}},"only":{only}}}}}]}}}}"#
    ))
}

const LOTUS: &str = "{T}: Add {U}. Spend this mana only to cast your commander.";
const TURBINE: &str = "{T}: Add {U}. This mana can't be spent to cast spells.";
const ZEALOT: &str = "{T}: Add {U}. Spend this mana only to turn permanents face up.";
const HARBINGER: &str = "{T}: Add {U}. Spend this mana only to foretell a card from your hand or cast an instant or sorcery spell.";
const PEEPER: &str = "{T}: Add {U}. Spend this mana only to cast an enchantment spell, unlock a door, or turn a permanent face up.";

#[rstest]
#[case::lotus(r#"[{"Spell":{"facts":["YourCommander"]}}]"#, LOTUS, true)]
#[case::lotus_creature(
    r#"[{"Spell":{"types":["Creature"],"facts":["YourCommander"]}}]"#,
    LOTUS,
    false
)]
#[case::lotus_not_owned(r#"[{"Spell":{"facts":["NotOwned"]}}]"#, LOTUS, false)]
#[case::turbine(r#"[{"Ability":{}},"OtherCosts"]"#, TURBINE, true)]
#[case::turbine_abilities_only(r#"[{"Ability":{}}]"#, TURBINE, false)]
#[case::turbine_other_costs_only(r#"["OtherCosts"]"#, TURBINE, false)]
#[case::zealot(r#"[{"Special":"TurnFaceUp"}]"#, ZEALOT, true)]
#[case::zealot_unlock(r#"[{"Special":"Unlock"}]"#, ZEALOT, false)]
#[case::harbinger(
    r#"[{"Special":"Foretell"},{"Spell":{"types":["Instant"]}},{"Spell":{"types":["Sorcery"]}}]"#,
    HARBINGER,
    true
)]
#[case::harbinger_instants_only(
    r#"[{"Special":"Foretell"},{"Spell":{"types":["Instant"]}}]"#,
    HARBINGER,
    false
)]
#[case::harbinger_any_spell(r#"[{"Special":"Foretell"},{"Spell":{}}]"#, HARBINGER, false)]
#[case::harbinger_red_instants(
    r#"[{"Special":"Foretell"},{"Spell":{"types":["Instant"],"colors":["Red"]}},{"Spell":{"types":["Sorcery"]}}]"#,
    HARBINGER,
    false
)]
#[case::peeper(
    r#"[{"Spell":{"types":["Enchantment"]}},{"Special":"Unlock"},{"Special":"TurnFaceUp"}]"#,
    PEEPER,
    true
)]
#[case::peeper_without_unlock(
    r#"[{"Spell":{"types":["Enchantment"]}},{"Special":"TurnFaceUp"}]"#,
    PEEPER,
    false
)]
#[case::peeper_foretell(
    r#"[{"Spell":{"types":["Enchantment"]}},{"Special":"Unlock"},{"Special":"Foretell"}]"#,
    PEEPER,
    false
)]
#[case::face_down_spells(r#"[{"Spell":{"facts":["FaceDown"]}}]"#, ZEALOT, false)]
fn ep33_restricted_mana_near_misses_differ(
    #[case] only: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&mana(only));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
