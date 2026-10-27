//! Whole abilities of the fifth sample of the names S to Z whose head or cost the renderers do not write: the ability is
//! matched whole as data and worded as the card prints it ("Whenever you attack, each opponent loses life equal to the
//! number of creatures attacking them.").

use crate::ability::Ability;

/// The abilities, as JSON, each with its wording.
const C5_SZ_WHOLES: [(&str, &str); 10] = [
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":13}},"effects":[{"WinGame":{}}]}}"#,
        "At the beginning of your upkeep, if you have exactly thirteen cards in your hand, you win the game.",
    ),
    (
        r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"Opponents"}},"effects":[{"RepeatEach":{"over":"Opponents","as":"p","effects":[{"LoseLife":{"who":{"Bound":"p"},"amount":{"Count":["Attacking",{"AttackingPlayer":{"Bound":"p"}}]}}}]}}]}}"#,
        "Whenever you attack, each opponent loses life equal to the number of creatures attacking them.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"OwnedBy":"Opponents"}],"to":"Graveyard"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        "Whenever a card is put into an opponent's graveyard from anywhere, put a +1/+1 counter on ~.",
    ),
    (
        r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked",{"AddKeyword":"Shroud"}]}}"#,
        "Equipped creature can't be blocked and has shroud.",
    ),
    (
        r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilities",{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}}]}}"#,
        "All lands have \"{T}: Add one mana of any color\" and lose all other abilities.",
    ),
    (
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Tallied":{"who":"Opponents","what":"Drawn"}}}}}}"#,
        "~ enters with a number of +1/+1 counters on it equal to the greatest number of cards an opponent has drawn this turn.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard"}}}}"#,
        "You may cast this card from your graveyard.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}"},{"ReturnToHand":{"filter":[{"Type":"Land"}],"count":2}}],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#,
        "{2}, Return two lands you control to their owner's hand: Copy target instant or sorcery spell. You may choose new targets for the copy.",
    ),
    (
        r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Ge","right":2}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Knight","colors":["White","Blue"],"types":["Creature"],"subtypes":["Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Vigilance"}]}}}}]}}"#,
        "{T}: Create a 2/2 white and blue Knight creature token with vigilance. Activate only if you've cast two or more spells this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Color":"Red"},{"ControlledBy":"You"}]}}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        "Whenever a red source you control deals damage to one or more permanents and/or players, put a +1/+1 counter on ~.",
    ),
];

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(data) = serde_json::to_value(ability) else {
        return;
    };
    for (shape, text) in C5_SZ_WHOLES {
        if c5_sz_shape(shape).is_some_and(|shape| shape == data) {
            texts.push(text.to_owned());
        }
    }
}

/// The JSON of an ability as the data writes it, defaults filled in the way the rules files hold them.
fn c5_sz_shape(shape: &str) -> Option<serde_json::Value> {
    let ability: Ability = serde_json::from_str(shape).ok()?;
    serde_json::to_value(ability).ok()
}
