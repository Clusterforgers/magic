//! Goal: the writers of round 15 pack e15g give the oracle wording of a Licid's ability and of a cast trigger whose spell gains keywords ("that spell gains rebound"), and another price, change, target, keyword or trigger never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// A Licid's ability with this price, these modifications and this target.
fn licid(price: &str, mods: &str, target: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":[{{"Mana":"{{W}}"}},"Tap"],"effects":[{{"Continuous":{{"affected":"This","duration":{{"UntilPaid":"{price}"}},"mods":{mods}}}}},{{"Attach":{{"to":{{"Target":0}},"what":"This"}}}}],"targets":[{{"what":{{"Object":{target}}}}}]}}}}"#
    )
}

const MODS: &str = r#"[{"AddSubtype":"Aura"},{"SetTypes":["Enchantment"]},{"AddKeyword":{"Enchant":[{"Type":"Creature"}]}},"LoseThisAbility"]"#;
const KEEPS_IT: &str = r#"[{"AddSubtype":"Aura"},{"SetTypes":["Enchantment"]},{"AddKeyword":{"Enchant":[{"Type":"Creature"}]}}]"#;
const ENCHANT_LAND: &str = r#"[{"AddSubtype":"Aura"},{"SetTypes":["Enchantment"]},{"AddKeyword":{"Enchant":[{"Type":"Land"}]}},"LoseThisAbility"]"#;
const CREATURE: &str = r#"[{"Type":"Creature"}]"#;
const YOURS: &str = r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const CALMING: &str = "{W}, {T}: ~ loses this ability and becomes an Aura enchantment with enchant creature. Attach it to target creature. You may pay {W} to end this effect.";

/// A cast trigger whose spell gains these keywords.
fn grant(what: &str, mods: &str, when: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Continuous":{{"affected":"{what}","duration":"Forever","mods":{mods}}}}}],"when":{when}}}}}"#
    )
}

const CAST: &str =
    r#"{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}"#;
const DIES: &str =
    r#"{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"}]}}"#;
const REBOUND: &str = r#"[{"AddKeyword":"Rebound"}]"#;
const LIFELINK: &str = r#"[{"AddKeyword":"Lifelink"}]"#;
const SWIFTSPEAR: &str = "Whenever you cast an instant or sorcery spell, that spell gains rebound.";

#[rstest]
#[case::calming(licid("{W}", MODS, CREATURE), CALMING)]
#[case::transmogrifying(
    licid("{1}", MODS, CREATURE),
    "{W}, {T}: ~ loses this ability and becomes an Aura enchantment with enchant creature. Attach it to target creature. You may pay {1} to end this effect."
)]
#[case::that_spell(grant("Triggering", REBOUND, CAST), SWIFTSPEAR)]
#[case::it(
    grant("Triggering", REBOUND, CAST),
    "Whenever you cast an instant or sorcery spell, it gains rebound."
)]
fn the_writer_reads_the_clause(#[case] json: String, #[case] clause: &str) {
    assert!(written(&json, clause), "{json}");
}

#[rstest]
#[case::another_price(licid("{U}", MODS, CREATURE), CALMING)]
#[case::keeps_the_ability(licid("{W}", KEEPS_IT, CREATURE), CALMING)]
#[case::enchant_land(licid("{W}", ENCHANT_LAND, CREATURE), CALMING)]
#[case::another_target(licid("{W}", MODS, YOURS), CALMING)]
#[case::another_keyword(grant("Triggering", LIFELINK, CAST), SWIFTSPEAR)]
#[case::the_source(grant("This", REBOUND, CAST), SWIFTSPEAR)]
#[case::not_a_cast(
    grant("Triggering", REBOUND, DIES),
    "Whenever a creature dies, that spell gains rebound."
)]
fn a_wrong_rule_never_reads_back(#[case] json: String, #[case] clause: &str) {
    assert!(!written(&json, clause), "{json}");
}
