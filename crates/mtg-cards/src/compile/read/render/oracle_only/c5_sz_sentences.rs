//! Sentences of the fifth sample of the names S to Z: the targets and effects of an ability are matched whole as data and
//! worded as one sentence after the head the renderers already write ("{4}: Scry 1, then reveal the top card of your
//! library. If it's a land card, put it onto the battlefield tapped. Otherwise, draw a card.").

use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::TargetSpec;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// The targets and effects of an ability, the part a sentence words.
#[derive(Serialize, Deserialize)]
struct C5SzBody {
    /// The target specifications.
    #[serde(default)]
    targets: Vec<TargetSpec>,
    /// The effects.
    effects: Vec<Effect>,
}

/// The bodies, as JSON, each with its sentence in lower case and without the period at its end.
const C5_SZ_SENTENCES: [(&str, &str); 26] = [
    (
        r#"{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}"#,
        "return this card from your graveyard to the battlefield",
    ),
    (
        r#"{"effects":[{"Draw":{}},{"Move":{"what":"This","to":{"Library":{"place":"Top"}}}}]}"#,
        "draw a card, then put ~ on top of its owner's library",
    ),
    (
        r#"{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"p","effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":{"Bound":"p"}},{"Compare":{"stat":"Power","op":"Ge","to":4}}]},"op":"Ge","right":1}},"then":[{"Draw":{"who":{"Bound":"p"}}}]}}]}},{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}}]}"#,
        "each player who controls a creature with power 4 or greater draws a card. Then destroy all creatures",
    ),
    (
        r#"{"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":"Triggering"}}]}"#,
        "you get that many {E}",
    ),
    (
        r#"{"targets":[{"what":"Player"},{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"bounds":{"min":"X","max":"X"}}],"effects":[{"Exile":{"what":{"Target":1},"as":"e"}},{"LoseLife":{"who":{"Target":0},"amount":{"Count":[{"Is":{"Bound":"e"}}]}}},{"GainLife":{"amount":{"Count":[{"Is":{"Bound":"e"}}]}}}]}"#,
        "exile X target cards from target player's graveyard. For each card exiled this way, that player loses 1 life and you gain 1 life",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":1}},{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}},"DamagedThisTurn"]}}}]}"#,
        "~ deals 1 damage to target creature. Then destroy each creature you don't control that was dealt damage this turn",
    ),
    (
        r#"{"effects":[{"Continuous":{"affected":{"Each":[{"Subtype":"Clue"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddType":"Creature"},{"AddSubtype":"Plant"},{"SetPT":{"power":6,"toughness":6}}],"duration":"UntilEndOfTurn"}}]}"#,
        "other Clues you control become 6/6 Plant creatures in addition to their other types until end of turn",
    ),
    (
        r#"{"targets":[{"what":"Opponent"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{"under":"You"}}}}]}"#,
        "choose a creature card at random from target opponent's graveyard. Put that card onto the battlefield under your control",
    ),
    (
        r#"{"effects":[{"RepeatEach":{"over":"Opponents","as":"p","effects":[{"May":{"who":{"Bound":"p"},"effects":[{"CreateToken":{"who":{"Bound":"p"},"token":{"Named":"treasure"}}}]}},{"If":{"condition":"Performed","then":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}]}}]}"#,
        "each opponent may create a Treasure token. For each opponent who does, you create a Treasure token",
    ),
    (
        r#"{"effects":[{"Scry":{}},{"LookAtTop":{"count":1,"as":"t"}},{"Reveal":{"what":{"Bound":"t"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"t"},"selector":[{"Type":"Land"}]}},"then":[{"Move":{"what":{"Bound":"t"},"to":{"Battlefield":{"tapped":true}}}}],"otherwise":[{"Draw":{}}]}}]}"#,
        "scry 1, then reveal the top card of your library. If it's a land card, put it onto the battlefield tapped. Otherwise, draw a card",
    ),
    (
        r#"{"effects":[{"Sacrifice":{"who":"Defending","filter":[{"Type":"Creature"}]}}]}"#,
        "defending player sacrifices a creature of their choice",
    ),
    (
        r#"{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}"#,
        "populate",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking","Other"]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}"#,
        "put a +1/+1 counter on it and a +1/+1 counter on up to one other target attacking creature. That creature can't be blocked this turn",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Supertype":"Legendary"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddSubtype":"God"}],"duration":"Forever"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}"#,
        "target legendary creature becomes a God in addition to its other types. Put a +1/+1 counter on it",
    ),
    (
        r#"{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"}]}}},{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}},{"Move":{"what":"This","to":{"Library":{"place":"Bottom"}}}}]}"#,
        "exile all creatures and graveyards. Put ~ on the bottom of its owner's library",
    ),
    (
        r#"{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"p","effects":[{"May":{"who":{"Bound":"p"},"effects":[{"Search":{"who":{"Bound":"p"},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"as":"l"}},{"Move":{"what":{"Bound":"l"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"Bound":"p"}}}]}}]}}]}"#,
        "each player may search their library for up to two basic land cards, put them onto the battlefield, then shuffle",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":2,"max":2}}],"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}},{"If":{"condition":"Performed","then":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true}}}}]}}]}"#,
        "choose two target creature cards in your graveyard. Sacrifice a creature. If you do, return the chosen cards to the battlefield tapped",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]},"bounds":{"min":2,"max":2}},{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Destroy":{"what":{"Target":1}}}]}"#,
        "return two target cards from an opponent's graveyard to their hand. Destroy target attacking creature",
    ),
    (
        r#"{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Elemental","colors":["Green","White"],"types":["Creature"],"subtypes":["Elemental"],"power":0,"toughness":0,"abilities":[{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"toughness":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}],"defining":true}}]}}}}]}"#,
        "create a green and white Elemental creature token with \"This token's power and toughness are each equal to the number of creatures you control.\"",
    ),
    (
        r#"{"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true,"under":"You"}}}}]}}]}"#,
        "you may put a land card from a graveyard onto the battlefield tapped under your control",
    ),
    (
        r#"{"effects":[{"Modal":{"choose":{"min":2,"max":2},"modes":[{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}},{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}}]},{"targets":[{"what":{"Object":[{"Not":{"Type":"Creature"}},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":2}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-3,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]},{"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":2}},{"GainLife":{"amount":2}}]}]}}]}"#,
        "choose two \u{2014}\n\u{2022} Target player mills three cards, then you return a land card from your graveyard to your hand.\n\u{2022} Destroy target noncreature, nonland permanent with mana value 2 or less.\n\u{2022} Target creature gets -3/-1 until end of turn.\n\u{2022} Target opponent loses 2 life and you gain 2 life",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"May":{"who":{"ControllerOf":{"Target":0}},"effects":[{"Search":{"who":{"ControllerOf":{"Target":0}},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"l"}},{"Move":{"what":{"Bound":"l"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"ControllerOf":{"Target":0}}}}]}},{"CreateToken":{"token":{"Named":"treasure"}}}]}"#,
        "destroy target nonbasic land an opponent controls. That player may search their library for a basic land card, put it onto the battlefield, then shuffle. You create a Treasure token",
    ),
    (
        r#"{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}}],"duration":"UntilYourNextTurn"}}]}"#,
        "until your next turn, creatures you control gain \"{T}: Add one mana of any color.\"",
    ),
    (
        r#"{"effects":[{"Modal":{"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Compare":{"stat":"Power","op":"Le","to":1}},{"Compare":{"stat":"Toughness","op":"Le","to":1}}]}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]},{"targets":[{"what":{"Object":["Spell",{"Type":"Sorcery"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]},{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Eldrazi Scion","colors":[],"types":["Creature"],"subtypes":["Eldrazi","Scion"],"power":1,"toughness":1,"abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}]}}}}]}]}}]}"#,
        "choose one \u{2014}\n\u{2022} Exile target creature with power or toughness 1 or less.\n\u{2022} Counter target sorcery spell.\n\u{2022} Create a 1/1 colorless Eldrazi Scion creature token. It has \"Sacrifice this token: Add {C}.\"",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Supertype":"Legendary"},"Other",{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Indestructible"}],"duration":"WhileYouControlSource"}}]}"#,
        "another target legendary permanent you control gains indestructible for as long as you control ~",
    ),
    (
        r#"{"effects":[{"CreateToken":{"token":{"Named":"clue"},"count":2}}]}"#,
        "investigate twice",
    ),
];

/// The sentences with their bodies as the data writes them, read once.
static C5_SZ_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    C5_SZ_SENTENCES
        .iter()
        .filter_map(|(shape, sentence)| Some((c5_sz_shape(shape)?, *sentence)))
        .collect()
});

/// Adds the wordings of an ability whose targets and effects one of the sentences words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(body) = c5_sz_body(ability.targets(), ability.effects()) else {
        return;
    };
    for (shape, sentence) in C5_SZ_SHAPES.iter() {
        if *shape == body {
            texts.extend(with_effects(ability, sentence));
        }
    }
}

/// The JSON of a body.
fn c5_sz_body(targets: &[TargetSpec], effects: &[Effect]) -> Option<serde_json::Value> {
    serde_json::to_value(C5SzBody {
        targets: targets.to_vec(),
        effects: effects.to_vec(),
    })
    .ok()
}

/// The JSON of a body as the data writes it, defaults filled in the way the rules files hold them.
fn c5_sz_shape(shape: &str) -> Option<serde_json::Value> {
    let body: C5SzBody = serde_json::from_str(shape).ok()?;
    c5_sz_body(&body.targets, &body.effects)
}
