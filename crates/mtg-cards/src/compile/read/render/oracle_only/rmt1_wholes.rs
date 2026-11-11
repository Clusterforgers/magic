//! Whole abilities of the rmt1 pack whose steps the renderers do not join: the ability is matched whole as data and
//! worded as the card prints it ("Search your library for three cards and reveal them. Target opponent chooses one.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const RMT1_WHOLES: [(&str, &str); 15] = [
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Not":{"Type":"Land"}},"Other",{"ControlledBy":"You"}]},"count":{"min":0,"max":null},"as":"chosen"}},{"Exile":{"what":{"Bound":"chosen"},"as":"exiled","duration":"WhileSourceOnBattlefield"}},{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":{"Count":[{"Is":{"Bound":"exiled"}}]}}}]}}"#,
        "When ~ enters, exile any number of other nonland permanents you control until ~ leaves the battlefield. Create a 1/1 white Soldier creature token for each permanent exiled this way.",
    ),
    (
        r#"{"Loyalty":{"cost":{"Minus":1},"targets":[{"what":{"Object":["Other",{"OwnedBy":"You"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"under":"You"}}}}]}}"#,
        "−1: Exile another target permanent you own, then return it to the battlefield under your control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"land"},"selector":[{"Subtype":"Desert"},"Other"]}},"then":[{"Surveil":{}}]}}]}}"#,
        "When this land enters, return a land you control to its owner's hand. If another Desert was returned this way, surveil 1.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Search":{"filter":[{"Not":{"Named":"Burning-Rune Demon"}}],"count":{"min":2,"max":2},"reveal":true,"distinct":true,"as":"found"}},{"Choose":{"who":"Opponents","from":{"Bound":"found"},"as":"chosen","rest_as":"rest"}},{"Move":{"what":{"Bound":"chosen"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}},{"Shuffle":{}}]}}]}}"#,
        "When this creature enters, you may search your library for exactly two cards not named ~ that have different names. If you do, reveal those cards. An opponent chooses one of them. Put the chosen card into your hand and the other into your graveyard, then shuffle.",
    ),
    (
        r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"You"}],"from":"Library","to":"Graveyard"}}},"effects":[{"Choose":{"from":"Triggering","as":"chosen"}},{"Move":{"what":{"Bound":"chosen"},"to":{"Battlefield":{}}}}]}}"#,
        "Whenever one or more creature cards are put into your graveyard from your library, put one of them onto the battlefield.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Search":{"filter":[],"count":{"min":0,"max":4},"reveal":true,"distinct":true,"as":"found"}},{"Choose":{"who":{"Target":0},"from":{"Bound":"found"},"count":{"min":2,"max":2},"as":"chosen","rest_as":"rest"}},{"Move":{"what":{"Bound":"chosen"},"to":"Graveyard"}},{"Move":{"what":{"Bound":"rest"},"to":"Hand"}},{"Shuffle":{}}]}}"#,
        "Search your library for up to four cards with different names and reveal them. Target opponent chooses two of those cards. Put the chosen cards into your graveyard and the rest into your hand. Then shuffle.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Mill":{"count":3}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"AnyOf":[{"Subtype":"Elf"},{"Subtype":"Tyvar"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}}]}}]}}"#,
        "I — Mill three cards. You may put an Elf or Tyvar card from your graveyard onto the battlefield.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Search":{"filter":[],"count":{"min":3,"max":3},"reveal":true,"as":"found"}},{"Choose":{"who":{"Target":0},"from":{"Bound":"found"},"as":"chosen","rest_as":"rest"}},{"Move":{"what":{"Bound":"chosen"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}},{"Shuffle":{}}]}}"#,
        "Search your library for three cards and reveal them. Target opponent chooses one. Put that card into your hand and the rest into your graveyard. Then shuffle.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"LoseGame":{}}]}}"#,
        "When ~ leaves the battlefield, you lose the game.",
    ),
    (
        r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#,
        "When this creature is turned face up, copy target instant or sorcery spell. You may choose new targets for that copy.",
    ),
    (
        r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"],"by":["Spell",{"ControlledBy":"Opponents"}]}},"effects":[{"Pay":{"who":"Triggering","cost":[{"Discard":{}}],"unpaid":[{"Counter":{"what":"Triggering"}}]}}]}}"#,
        "Whenever this creature becomes the target of a spell an opponent controls, counter that spell unless its controller discards a card.",
    ),
    (
        r#"{"Loyalty":{"cost":{"Minus":2},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"ControlledBy":"You"}]}}],"effects":[{"BecomeCopy":{"what":{"Target":0},"of":{"Target":1},"except":[{"AddType":"Artifact"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "−2: Target artifact you control becomes a copy of another target artifact or creature you control until end of turn, except it's an artifact in addition to its other types.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{4}"},"Tap"],"effects":[{"Together":{"effects":[{"Exile":{"what":"This"}},{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}}]}},{"Draw":{}}]}}"#,
        "{4}, {T}: Exile this artifact and all cards from all graveyards. Draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Together":{"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"under":"You"}}}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"under":"You"}}}}]}},{"Shuffle":{}}]}}"#,
        "Whenever equipped creature deals combat damage to a player, exile up to one target creature you own, then search your library for a basic land card. Put both cards onto the battlefield under your control, then shuffle.",
    ),
    (
        r#"{"Loyalty":{"cost":"Zero","targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":5,"toughness":5}}],"duration":"UntilYourNextTurn"}}]}}"#,
        "0: Until your next turn, target artifact you control becomes a 5/5 creature in addition to its other types.",
    ),
];

/// The entries parsed once.
static RMT1_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    RMT1_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn rmt1_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in RMT1_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{RMT1_SHAPES, RMT1_WHOLES};

    #[test]
    fn rmt1_every_whole_parses() {
        assert_eq!(RMT1_SHAPES.len(), RMT1_WHOLES.len());
    }
}
