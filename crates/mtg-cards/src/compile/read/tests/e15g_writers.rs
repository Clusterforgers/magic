//! Goal: probe.
use super::*;

#[test]
fn probe() {
    for json in [
        r#"{"Triggered":{"effects":[{"Continuous":{"affected":"Triggering","duration":"Forever","mods":[{"AddKeyword":"Rebound"}]}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}}}"#,
        r#"{"Triggered":{"effects":[{"Continuous":{"affected":"Triggering","duration":"Forever","mods":[{"AddKeyword":"Deathtouch"},{"AddKeyword":"Lifelink"}]}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}}}"#,
        r#"{"Triggered":{"effects":[{"Continuous":{"affected":"Triggering","duration":"UntilEndOfTurn","mods":[{"AddKeyword":"Deathtouch"}]}}],"when":{"SpellCast":{"by":"You","what":[{"Type":"Creature"}]}}}}"#,
        r#"{"Triggered":{"effects":[{"Modal":{"modes":[{"effects":[{"Continuous":{"affected":"Triggering","duration":"Forever","mods":[{"AddKeyword":"Deathtouch"},{"AddKeyword":"Lifelink"}]}}]},{"effects":[{"Draw":{}}]}]}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}}}"#,
    ] {
        eprintln!("{:#?}", written_variants(&ability(json)));
    }
}
