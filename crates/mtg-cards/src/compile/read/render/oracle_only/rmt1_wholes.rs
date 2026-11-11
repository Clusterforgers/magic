//! Whole abilities of the rmt1 pack whose steps the renderers do not join: the ability is matched whole as data and
//! worded as the card prints it ("Search your library for three cards and reveal them. Target opponent chooses one.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const RMT1_WHOLES: [(&str, &str); 34] = [
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
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Pay":{"who":{"Bound":"player"},"cost":"{X}","paid":[{"RememberValue":{"value":"X","slot":"paid"}}]}}]}},{"CreateToken":{"who":"EachPlayer","token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":{"Remembered":{"on":"This","slot":"paid"}}}}]}}"#,
        "Starting with you, each player may pay any amount of mana. Each player creates X 1/1 white Soldier creature tokens, where X is the total amount of mana paid this way.",
    ),
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Pay":{"who":{"Bound":"player"},"cost":"{X}","paid":[{"RememberValue":{"value":"X","slot":"paid"}}]}}]}},{"RepeatEach":{"over":"EachPlayer","as":"searcher","effects":[{"Search":{"who":{"Bound":"searcher"},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":{"Remembered":{"on":"This","slot":"paid"}}},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{"who":{"Bound":"searcher"}}}]}}]}}"#,
        "Starting with you, each player may pay any amount of mana. Each player searches their library for up to X basic land cards, where X is the total amount of mana paid this way, puts them onto the battlefield tapped, then shuffles.",
    ),
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Pay":{"who":{"Bound":"player"},"cost":"{X}","paid":[{"RememberValue":{"value":"X","slot":"paid"}}]}}]}},{"Draw":{"who":"EachPlayer","count":{"Remembered":{"on":"This","slot":"paid"}}}}]}}"#,
        "Starting with you, each player may pay any amount of mana. Each player draws X cards, where X is the total amount of mana paid this way.",
    ),
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Pay":{"who":{"Bound":"player"},"cost":"{X}","paid":[{"RememberValue":{"value":"X","slot":"paid"}}]}}]}},{"Mill":{"who":"EachPlayer","count":{"Remembered":{"on":"This","slot":"paid"}}}}]}}"#,
        "Starting with you, each player may pay any amount of mana. Each player mills X cards, where X is the total amount of mana paid this way.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"OwnedBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"from":"Graveyard","effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}}}}]}}"#,
        "Whenever a nontoken creature is put into your graveyard from the battlefield, if this card is in your graveyard, create a 2/2 black Zombie creature token.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"OwnedBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"from":"Graveyard","effects":[{"Exile":{"what":"This"}}]}}"#,
        "When a creature is put into an opponent's graveyard from the battlefield, if this card is in your graveyard, exile this card.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}{G}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"In":"Battlefield"}]}},"then":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":{"Stat":{"of":"This","stat":"Power"}}}},{"DealDamage":{"source":{"Target":0},"to":"This","amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}]}}"#,
        "{1}{G}, {T}: If this creature is on the battlefield, it deals damage equal to its power to target attacking creature. That creature deals damage equal to its power to this creature.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"Pay":{"who":{"Target":0},"cost":[{"ExileFromGraveyard":{"filter":[]}}],"unpaid":[{"May":{"effects":[{"Draw":{}}]}}]}}]}}"#,
        "At the beginning of your upkeep, target opponent may exile a card from their graveyard. If that player doesn't, you may draw a card.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Player","bounds":{"min":2,"max":2}}],"effects":[{"RepeatEach":{"over":{"Target":0},"as":"player","effects":[{"Search":{"who":{"Bound":"player"},"filter":[],"as":"card"}},{"Shuffle":{"who":{"Bound":"player"}}},{"Move":{"what":{"Bound":"card"},"to":{"Library":{"place":"Top"}}}}]}}]}}"#,
        "Choose two target players. Each of them searches their library for a card, then shuffles and puts that card on top.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        "I — Gungnir — Destroy target creature an opponent controls.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        "Gungnir — Destroy target creature an opponent controls.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Continuous":{"affected":"This","mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"LoseGame":{"who":"Triggering"}}]}}}],"duration":"Forever"}}]}}"#,
        "II — Zantetsuken — This creature gains \"Whenever this creature deals combat damage to a player, that player loses the game.\"",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Continuous":{"affected":"This","mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"LoseGame":{"who":"Triggering"}}]}}}],"duration":"Forever"}}]}}"#,
        "Zantetsuken — This creature gains \"Whenever this creature deals combat damage to a player, that player loses the game.\"",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Draw":{"count":2}},{"LoseLife":{"who":"EachPlayer","amount":2}}]}}"#,
        "III — Hall of Sorrow — Draw two cards. Each player loses 2 life.",
    ),
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Draw":{"count":2}},{"LoseLife":{"who":"EachPlayer","amount":2}}]}}"#,
        "Hall of Sorrow — Draw two cards. Each player loses 2 life.",
    ),
    (
        r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"mods":[{"AddSubtype":"Plains"},{"AddSubtype":"Island"},{"AddSubtype":"Swamp"},{"AddSubtype":"Mountain"},{"AddSubtype":"Forest"}],"duration":"UntilEndOfTurn"}},{"Draw":{}}]}}"#,
        "Lands you control gain all basic land types until end of turn.\nDraw a card.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]},"bounds":{"min":"X","max":"X"}}],"effects":[{"BecomeBlocked":{"what":{"Target":0}}},{"DealDamage":{"to":{"Target":0},"amount":1}}]}}"#,
        "X target attacking creatures become blocked. ~ deals 1 damage to each of those creatures.",
    ),
    (
        r#"{"Activated":{"cost":"{1}{G/U}","targets":[{"what":{"Object":[{"Subtype":"Frog"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":"This","mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"AnyOf":[{"HasKeyword":"Flying"},{"HasKeyword":"Reach"}]}}]}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Target":0},"mods":[{"CantBeBlockedBy":[{"Type":"Creature"},{"Not":{"AnyOf":[{"HasKeyword":"Flying"},{"HasKeyword":"Reach"}]}}]}],"duration":"UntilEndOfTurn"}}]}}"#,
        "{1}{G/U}: ~ and target Frog you control can't be blocked this turn except by creatures with flying or reach.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Tap":{"what":{"Each":[{"Type":"Land"},{"ControlledBy":{"Target":0}}]}}},{"LoseUnspentMana":{"who":{"Target":0}}}]}}"#,
        "Tap all lands target player controls and that player loses all unspent mana.",
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
