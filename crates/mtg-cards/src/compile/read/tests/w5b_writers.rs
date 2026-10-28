//! Goal: the writers of spells built of several steps and of the draw, choice, rule and mana steps (r3w9) give the printed wording of each shape from the data, and a changed number, kind, player, zone, duration or condition never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn exiled(head: &str, who: &str, count: &str, duration: &str, more: &str) -> String {
    format!(
        r#"{{{head}"effects":[{{"ExileTop":{{"who":"{who}","count":{count},"as":"card"}}}},{{"Rule":{{"rule":{{"MayCast":{{"from":"Exile","only":{{"Bound":"card"}}}}}},"duration":"{duration}"}}}}{more}]}}}}"#
    )
}

const SPELL: &str = r#""Spell":{"#;
const PIRATES: &str = r#""Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Subtype":"Pirate"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}},"#;
const PILLAGER: &str =
    r#""Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"#;
const TEN_MANA: &str = r#",{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Hand"}]}},"then":[{"AddMana":{"mana":{"AnyColor":10}}}]}}"#;
const APEX: &str = "Exile the top seven cards of your library. Until end of turn, you may cast spells from among them. If this spell was cast from your hand, add ten mana of any one color.";
const RAMIREZ: &str = "Whenever one or more Pirates you control deal combat damage to a player, exile the top card of that player's library. You may cast that card for as long as it remains exiled.";
const DREAM: &str = "Whenever ~ deals combat damage to a player, exile that many cards from the top of your library. Until end of turn, you may cast spells from among those exiled cards.";

#[rstest]
#[case::apex(SPELL, "You", "7", "UntilEndOfTurn", TEN_MANA, APEX, true)]
#[case::apex_other_count(SPELL, "You", "6", "UntilEndOfTurn", TEN_MANA, APEX, false)]
#[case::apex_for_good(SPELL, "You", "7", "Forever", TEN_MANA, APEX, false)]
#[case::apex_without_mana(SPELL, "You", "7", "UntilEndOfTurn", "", APEX, false)]
#[case::ramirez(PIRATES, "Triggering", "1", "Forever", "", RAMIREZ, true)]
#[case::ramirez_own_library(PIRATES, "You", "1", "Forever", "", RAMIREZ, false)]
#[case::ramirez_this_turn(PIRATES, "Triggering", "1", "UntilEndOfTurn", "", RAMIREZ, false)]
#[case::dream(PILLAGER, "You", r#""Triggering""#, "UntilEndOfTurn", "", DREAM, true)]
#[case::dream_two(PILLAGER, "You", "2", "UntilEndOfTurn", "", DREAM, false)]
#[case::dream_other_library(
    PILLAGER,
    "Triggering",
    r#""Triggering""#,
    "UntilEndOfTurn",
    "",
    DREAM,
    false
)]
fn exiled_cards_cast(
    #[case] head: &str,
    #[case] who: &str,
    #[case] count: &str,
    #[case] duration: &str,
    #[case] more: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    let json = exiled(head, who, count, duration, more);
    assert_eq!(written(&json, clause), reads, "{json}");
}

fn graveyard_cast(head: &str, card: &str, duration: &str) -> String {
    format!(
        r#"{{{head}"targets":[{{"what":{{"Object":[{card},{{"In":"Graveyard"}},{{"OwnedBy":"You"}}]}}}}],"effects":[{{"Rule":{{"rule":{{"MayCast":{{"from":"Graveyard","only":{{"Target":0}}}}}},"duration":"{duration}"}}}}]}}}}"#
    )
}

const TAP: &str = r#""Activated":{"cost":["Tap"],"#;
const ZOMBIE: &str = r#"{"Subtype":"Zombie"},{"Type":"Creature"}"#;
const ARTIFACT: &str = r#"{"Type":"Artifact"}"#;
const ZUL: &str = "{T}: You may cast target Zombie creature card from your graveyard this turn.";
const SILAS: &str = "Whenever ~ deals combat damage to a player, choose target artifact card in your graveyard. You may cast that card this turn.";

#[rstest]
#[case::zul(TAP, ZOMBIE, "UntilEndOfTurn", ZUL, true)]
#[case::zul_artifact(TAP, ARTIFACT, "UntilEndOfTurn", ZUL, false)]
#[case::zul_for_good(TAP, ZOMBIE, "Forever", ZUL, false)]
#[case::silas(PILLAGER, ARTIFACT, "UntilEndOfTurn", SILAS, true)]
#[case::silas_zombie(PILLAGER, ZOMBIE, "UntilEndOfTurn", SILAS, false)]
fn graveyard_target_cast(
    #[case] head: &str,
    #[case] card: &str,
    #[case] duration: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    let json = graveyard_cast(head, card, duration);
    assert_eq!(written(&json, clause), reads, "{json}");
}

const FELIX: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}}}}}}"#;
const FELIX_ANY: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer"}}}}}}"#;
const VEYRAN: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"AnyOf":[{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},{"SpellCopied":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}}]}}}}}"#;
const VEYRAN_CAST: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}}}}}}"#;
const FELIX_TEXT: &str = "If a creature you control dealing combat damage to a player causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.";
const VEYRAN_TEXT: &str = "If you casting or copying an instant or sorcery spell causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.";

fn indomitable(least: u32, tapped: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"MayCast":{{"what":["This"],"from":"Graveyard"}}}},"condition":{{"Compare":{{"left":{{"Count":[{{"AnyOf":[{{"Subtype":"Pirate"}},{{"Subtype":"Vehicle"}}]}}{tapped},{{"ControlledBy":"You"}}]}},"op":"Ge","right":{least}}}}}}}}}"#
    )
}

const INDOMITABLE: &str = "You may cast this card from your graveyard as long as you control three or more tapped Pirates and/or Vehicles.";

#[rstest]
#[case::felix(FELIX, FELIX_TEXT, true)]
#[case::felix_any_damage(FELIX_ANY, FELIX_TEXT, false)]
#[case::veyran(VEYRAN, VEYRAN_TEXT, true)]
#[case::veyran_cast_only(VEYRAN_CAST, VEYRAN_TEXT, false)]
#[case::indomitable(&indomitable(3, r#","Tapped""#), INDOMITABLE, true)]
#[case::indomitable_two(&indomitable(2, r#","Tapped""#), INDOMITABLE, false)]
#[case::indomitable_untapped(&indomitable(3, ""), INDOMITABLE, false)]
fn rules(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{json}");
}

fn szeras(stat: &str, color: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap",{{"Sacrifice":{{"filter":[{{"Type":"Creature"}},"Other"]}}}}],"effects":[{{"AddMana":{{"mana":{{"Fixed":["{color}"]}},"times":{{"Stat":{{"of":"Paid","stat":"{stat}"}}}}}}}}]}}}}"#
    )
}

fn plaza(kind: &str, who: &str) -> String {
    format!(
        r#"{{"Activated":{{"cost":["Tap"],"effects":[{{"AddMana":{{"mana":{{"ProducedBy":{{"among":[{kind},{{"ControlledBy":"{who}"}}],"count":1,"colorless":true}}}}}}}}]}}}}"#
    )
}

const PLAZA: &str = "{T}: Add one mana of any type that a Gate you control could produce.";
const GATE: &str = r#"{"Subtype":"Gate"}"#;

#[rstest]
#[case::szeras(&szeras("ManaValue", "Black"), "{T}, Sacrifice another creature: Add an amount of {B} equal to the sacrificed creature's mana value.", true)]
#[case::szeras_power(&szeras("Power", "Black"), "{T}, Sacrifice another creature: Add an amount of {B} equal to the sacrificed creature's mana value.", false)]
#[case::szeras_red(&szeras("ManaValue", "Red"), "{T}, Sacrifice another creature: Add an amount of {B} equal to the sacrificed creature's mana value.", false)]
#[case::plaza(&plaza(GATE, "You"), PLAZA, true)]
#[case::plaza_opponents(&plaza(GATE, "Opponents"), PLAZA, false)]
#[case::plaza_desert(&plaza(r#"{"Subtype":"Desert"}"#, "You"), PLAZA, false)]
fn mana(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{json}");
}

fn kopala(kind: &str, who: &str, change: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":[{kind},{{"ControlledBy":"{who}"}},{{"Targeting":[{{"Subtype":"Merfolk"}},{{"ControlledBy":"You"}}]}}],"change":"{change}","mana":"{{2}}"}}}}"#
    )
}

const SPELLS: &str = r#""Spell""#;
const ABILITIES: &str = r#"{"Ability":"Activated"}"#;
const KOPALA_SPELLS: &str =
    "Spells your opponents cast that target a Merfolk you control cost {2} more to cast.";
const KOPALA_ABILITIES: &str = "Abilities your opponents activate that target a Merfolk you control cost {2} more to activate.";

#[rstest]
#[case::spells(SPELLS, "Opponents", "Increase", KOPALA_SPELLS, true)]
#[case::abilities(ABILITIES, "Opponents", "Increase", KOPALA_ABILITIES, true)]
#[case::abilities_as_spells(ABILITIES, "Opponents", "Increase", KOPALA_SPELLS, false)]
#[case::your_spells(SPELLS, "You", "Increase", KOPALA_SPELLS, false)]
#[case::cheaper(SPELLS, "Opponents", "Reduce", KOPALA_SPELLS, false)]
fn targeting_costs(
    #[case] kind: &str,
    #[case] who: &str,
    #[case] change: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(&kopala(kind, who, change), clause),
        reads,
        "{clause}"
    );
}

fn champion(source: &str, least: u32) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[{source}],"to":"EachPlayer"}}}},"if":{{"Compare":{{"left":"Triggering","op":"Ge","right":{least}}}}},"effects":[{{"Draw":{{}}}}]}}}}"#
    )
}

const YOURS: &str = r#"{"ControlledBy":"You"}"#;
const CHAMPION: &str =
    "Whenever a source you control deals 5 or more damage to a player, draw a card.";

#[rstest]
#[case::champion(YOURS, 5, CHAMPION, true)]
#[case::champion_four(YOURS, 4, CHAMPION, false)]
#[case::champion_creature(r#"{"Type":"Creature"},{"ControlledBy":"You"}"#, 5, CHAMPION, false)]
#[case::champion_opponents(r#"{"ControlledBy":"Opponents"}"#, 5, CHAMPION, false)]
fn damage_at_least(
    #[case] source: &str,
    #[case] least: u32,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(&champion(source, least), clause), reads, "{clause}");
}
