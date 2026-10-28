//! Goal: the writers of the shapes the local-model proposals hit give the oracle wording of the ability, and a changed clause does not read back.
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

const BURST: &str = r#"{"Spell":{"effects":[{"PutCounters":{"counter":"+1/+1","on":{"Target":0}}},{"Untap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const STRIDE: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Target":0},"duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":3}},{"AddKeyword":"Reach"}]}},{"Untap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const TAP_OTHER: &str = r#"{"Spell":{"effects":[{"PutCounters":{"counter":"+1/+1","on":{"Target":0}}},{"Tap":{"what":{"Target":1}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const RIME: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Untap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Supertype":"Snow"},"Other"]}}]}}"#;
const SCULPTOR: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Untap":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Supertype":"Snow"},{"Type":"Land"}]}}]}}"#;
const CHAR: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":4,"to":{"Target":0}}},{"DealDamage":{"amount":2,"to":"You"}}],"targets":[{"what":"AnyTarget"}]}}"#;
const FIRESLINGER: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"DealDamage":{"amount":1,"to":{"Target":0}}},{"DealDamage":{"amount":1,"to":"You"}}],"targets":[{"what":"AnyTarget"}]}}"#;
const SPARKS: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":1,"to":{"Target":0}}},{"DealDamage":{"amount":1,"to":{"Target":1}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}]}}"#;
const HOLD: &str = r#"{"Spell":{"effects":[{"Shield":{"by":{"PreventUpTo":7},"duration":"UntilEndOfTurn","when":{"Damage":{"to":{"Target":0}}}}}],"targets":[{"what":"AnyTarget"}]}}"#;
const KEI: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Shield":{"by":{"PreventUpTo":2},"duration":"UntilEndOfTurn","when":{"Damage":{"to":{"Target":0}}}}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const MELT: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"DealDamage":{"amount":2,"to":{"ControllerOf":{"Target":0}}}}],"targets":[{"what":{"Object":[{"Type":"Land"}]}}]}}"#;
const REVEL: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Draw":{}}]}}],"when":{"ZoneChange":{"what":[{"Type":"Artifact"},{"OwnedBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}}}}"#;
const REVEL_ANY: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Draw":{}}]}}],"when":{"ZoneChange":{"what":[{"Type":"Artifact"}],"from":"Battlefield","to":"Graveyard"}}}}"#;

#[rstest]
#[case::burst(BURST, "Put a +1/+1 counter on target creature and untap it.")]
#[case::stride(
    STRIDE,
    "Target creature gets +1/+3 and gains reach until end of turn. Untap it."
)]
#[case::rime(RIME, "{T}: Untap another target snow permanent.")]
#[case::sculptor(SCULPTOR, "{T}: Untap target snow land.")]
#[case::char(CHAR, "~ deals 4 damage to any target and 2 damage to you.")]
#[case::fireslinger(
    FIRESLINGER,
    "{T}: This creature deals 1 damage to any target and 1 damage to you."
)]
#[case::sparks(
    SPARKS,
    "~ deals 1 damage to target creature and 1 damage to target player or planeswalker."
)]
#[case::hold(
    HOLD,
    "Prevent the next 7 damage that would be dealt to any target this turn."
)]
#[case::kei(
    KEI,
    "{T}: Prevent the next 2 damage that would be dealt to target creature this turn."
)]
#[case::melt(
    MELT,
    "Destroy target land. ~ deals 2 damage to that land's controller."
)]
#[case::revel(
    REVEL,
    "Whenever an artifact is put into an opponent's graveyard from the battlefield, you may draw a card."
)]
fn writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::burst_tap(BURST, "Put a +1/+1 counter on target creature and tap it.")]
#[case::burst_count(BURST, "Put two +1/+1 counters on target creature and untap it.")]
#[case::tap_other(TAP_OTHER, "Put a +1/+1 counter on target creature and tap it.")]
#[case::stride_size(
    STRIDE,
    "Target creature gets +3/+1 and gains reach until end of turn. Untap it."
)]
#[case::rime_basic(RIME, "{T}: Untap another target basic permanent.")]
#[case::rime_other(RIME, "{T}: Untap target snow permanent.")]
#[case::sculptor_kind(SCULPTOR, "{T}: Untap target snow creature.")]
#[case::char_amount(CHAR, "~ deals 4 damage to any target and 3 damage to you.")]
#[case::char_player(CHAR, "~ deals 4 damage to any target and 2 damage to each opponent.")]
#[case::sparks_target(
    SPARKS,
    "~ deals 1 damage to target creature and 1 damage to target player."
)]
#[case::hold_amount(
    HOLD,
    "Prevent the next 6 damage that would be dealt to any target this turn."
)]
#[case::hold_all(
    HOLD,
    "Prevent all damage that would be dealt to any target this turn."
)]
#[case::kei_target(
    KEI,
    "{T}: Prevent the next 2 damage that would be dealt to any target this turn."
)]
#[case::melt_kind(
    MELT,
    "Destroy target land. ~ deals 2 damage to that creature's controller."
)]
#[case::melt_amount(
    MELT,
    "Destroy target land. ~ deals 3 damage to that land's controller."
)]
#[case::revel_yours(
    REVEL,
    "Whenever an artifact is put into your graveyard from the battlefield, you may draw a card."
)]
#[case::revel_any(
    REVEL_ANY,
    "Whenever an artifact is put into an opponent's graveyard from the battlefield, you may draw a card."
)]
fn a_changed_clause_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

const ANGEL: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Move":{"to":{"Library":{"place":"Top"}},"what":"This"}}]}}],"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}}"#;
const AFFINITY: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Land"}]},"duration":"UntilEndOfTurn","mods":[{"AddType":"Creature"},{"SetPT":{"power":2,"toughness":2}}]}}]}}"#;
const MENTOR: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Move":{"to":"Hand","what":{"Target":0}}},{"Move":{"to":"Hand","what":"This"}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]}}"#;
const DRAKE: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","condition":{"Not":"YourTurn"}}}"#;
const DRAKE_YOURS: &str = r#"{"CostModifier":{"applies_to":["Spell",{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}","condition":"YourTurn"}}"#;
const RECOIL: &str = r#"{"Spell":{"effects":[{"Move":{"to":"Hand","what":{"Target":0}}},{"Discard":{"who":{"OwnerOf":{"Target":0}}}}],"targets":[{"what":{"Object":[]}}]}}"#;
const DRYAD: &str = r#"{"Triggered":{"effects":[{"PutCounters":{"counter":"+1/+1","on":"This"}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Color":"White"},{"Color":"Blue"},{"Color":"Black"},{"Color":"Red"}]}]}}}}"#;
const TWO_COLORS: &str = r#"{"Triggered":{"effects":[{"Draw":{}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Color":"Green"},{"Color":"White"}]}]}}}}"#;

#[rstest]
#[case::angel(
    ANGEL,
    "When this creature dies, you may put it on top of its owner's library."
)]
#[case::affinity(
    AFFINITY,
    "All lands become 2/2 creatures until end of turn. They're still lands."
)]
#[case::mentor(
    MENTOR,
    "{T}: Return this creature and target creature you control to their owner's hand."
)]
#[case::drake(
    DRAKE,
    "During turns other than yours, spells you cast cost {1} less to cast."
)]
#[case::recoil(
    RECOIL,
    "Return target permanent to its owner's hand. Then that player discards a card."
)]
#[case::dryad(
    DRYAD,
    "Whenever you cast a spell that's white, blue, black, or red, put a +1/+1 counter on this creature."
)]
#[case::two_colors(
    TWO_COLORS,
    "Whenever you cast a spell that's green or white, draw a card."
)]
fn writes_the_wording_of_the_second_group(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}: {:?}", wordings(json));
}

#[rstest]
#[case::angel_bottom(
    ANGEL,
    "When this creature dies, you may put it on the bottom of its owner's library."
)]
#[case::affinity_size(
    AFFINITY,
    "All lands become 3/3 creatures until end of turn. They're still lands."
)]
#[case::affinity_kind(
    AFFINITY,
    "All artifacts become 2/2 creatures until end of turn. They're still artifacts."
)]
#[case::mentor_kind(
    MENTOR,
    "{T}: Return this creature and target creature an opponent controls to their owner's hand."
)]
#[case::drake_yours(
    DRAKE_YOURS,
    "During turns other than yours, spells you cast cost {1} less to cast."
)]
#[case::drake_amount(
    DRAKE,
    "During turns other than yours, spells you cast cost {2} less to cast."
)]
#[case::recoil_count(
    RECOIL,
    "Return target permanent to its owner's hand. Then that player discards two cards."
)]
#[case::dryad_color(
    DRYAD,
    "Whenever you cast a spell that's white, blue, black, or green, put a +1/+1 counter on this creature."
)]
#[case::dryad_and(
    TWO_COLORS,
    "Whenever you cast a spell that's green and white, draw a card."
)]
fn a_changed_clause_of_the_second_group_does_not_read_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!written(json, clause), "{clause}");
}
