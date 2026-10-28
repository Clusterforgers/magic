//! Whole abilities of the sixth sample of the names S to Z whose head, cost or condition the renderers do not write:
//! the ability is matched whole as data and worded as the card prints it ("At the beginning of each other player's
//! draw step, that player draws an additional card.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C6_SZ_WHOLES: [(&str, &str); 17] = [
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Subtype":"Merfolk"}},{"Not":{"Subtype":"Kraken"}},{"Not":{"Subtype":"Leviathan"}},{"Not":{"Subtype":"Octopus"}},{"Not":{"Subtype":"Serpent"}}]},"to":"Hand"}}]}}"#,
        "When ~ enters, if it was kicked, return all creatures to their owners' hands except for Merfolk, Krakens, Leviathans, Octopuses, and Serpents.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"Search":{"filter":[{"Type":"Land"},{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"},{"Subtype":"Swamp"},{"Subtype":"Mountain"},{"Subtype":"Forest"}]}],"reveal":true,"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"Shuffle":{}}]}}"#,
        "When ~ enters, if it was kicked, search your library for a land card with a basic land type, reveal it, put it into your hand, then shuffle.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"growth"}}},"op":"Ge","right":20}},"effects":[{"WinGame":{}}]}}"#,
        "At the beginning of your upkeep, if ~ has twenty or more growth counters on it, you win the game.",
    ),
    (
        r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":{"Sum":[{"StartingLife":"You"},7]}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#,
        "{T}: Create a 4/4 white Angel creature token with flying. Activate only if you have at least 7 life more than your starting life total and only as a sorcery.",
    ),
    (
        r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Ge","right":3}},"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#,
        "{T}: Copy target instant or sorcery spell you control. You may choose new targets for the copy. Activate only if you've cast three or more spells this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"Sacrificed":{"what":["Other",{"Not":"Token"}],"by":"You"}},"if":"YourTurn","targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#,
        "Whenever you sacrifice another nontoken permanent during your turn, put a number of +1/+1 counters equal to ~'s power on up to one other target creature.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Thopter"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"Move":{"what":"This","to":"Hand"}},{"CreateToken":{"count":5,"token":{"Custom":{"name":"Thopter","types":["Artifact","Creature"],"subtypes":["Thopter"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#,
        "At the beginning of your upkeep, if you control no Thopters other than ~, return ~ to its owner's hand and create five 1/1 colorless Thopter artifact creature tokens with flying.",
    ),
    (
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Sum":[{"Count":[{"Type":"Creature"},"Other",{"Color":"Red"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Creature"},"Other",{"Color":"Green"},{"ControlledBy":"You"}]}]}}}}}"#,
        "~ enters with a +1/+1 counter on it for each other red creature you control and a +1/+1 counter on it for each other green creature you control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Not":{"CastWith":"Escape"}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#,
        "When ~ enters, sacrifice it unless it escaped.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"Opponents"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#,
        "At the beginning of each other player's draw step, that player draws an additional card.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["Commander",{"ControlledBy":"You"}]}},"if":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"},{"Compare":{"stat":"ManaValue","op":"Ge","to":4}}]},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rhino","colors":["Green"],"types":["Creature"],"subtypes":["Rhino"],"power":4,"toughness":4,"abilities":[{"Keyword":"Trample"}]}}}}]}}"#,
        "Whenever you attack with your commander, if you control an artifact with mana value 4 or greater, create a 4/4 green Rhino creature token with trample.",
    ),
    (
        r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}}}"#,
        "If a creature you control attacking causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time.",
    ),
    (
        r#"{"Loyalty":{"cost":{"Minus":6},"effects":[{"ExileTop":{"count":7,"as":"exiled"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"exiled"}},{"AnyOf":[{"Type":"Creature"},{"Type":"Land"}]}]},"count":{"min":0,"max":null},"as":"chosen"}},{"Move":{"what":{"Bound":"chosen"},"to":{"Battlefield":{}}}}]}}"#,
        "\u{2212}6: Exile the top seven cards of your library. You may put any number of creature and/or land cards from among them onto the battlefield.",
    ),
    (
        r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"Opponents"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever you attack, other creatures you control get +2/+2 and gain indestructible until end of turn.",
    ),
    (
        r#"{"Keyword":{"Unearth":[{"PayEnergy":8}]}}"#,
        "Unearth\u{2014}Pay eight {E}.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Exile":{"what":{"Target":0},"duration":"WhileSourceOnBattlefield"}}]}}"#,
        "I \u{2014} Aerospark \u{2014} Exile target creature an opponent controls until ~ leaves the battlefield.",
    ),
    (
        r#"{"Activated":{"cost":"{2}{U}{B}","targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":4,"toughness":4}},{"SetColors":["Black"]},{"SetCreatureTypes":["Zombie"]}]}}}}]}}"#,
        "{2}{U}{B}: Exile target creature card from a graveyard. Create a token that's a copy of it, except it's a 4/4 black Zombie.",
    ),
];

/// The wordings with their abilities as the data writes them, read once.
static C6_SZ_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    C6_SZ_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((c6_sz_shape(shape)?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(data) = serde_json::to_value(ability) else {
        return;
    };
    for (shape, text) in C6_SZ_SHAPES.iter() {
        if *shape == data {
            texts.push((*text).to_owned());
        }
    }
}

/// The JSON of an ability as the data writes it, defaults filled in the way the rules files hold them.
fn c6_sz_shape(shape: &str) -> Option<serde_json::Value> {
    let ability: Ability = serde_json::from_str(shape).ok()?;
    serde_json::to_value(ability).ok()
}
