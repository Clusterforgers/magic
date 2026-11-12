//! Whole abilities of the r30-6b card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R306B_WHOLES: [(&str, &str); 26] = [
    // kravens_last_hunt
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Mill":{"count":5}},{"Reflexive":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Greatest":{"of":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}],"stat":"Power"}}}}]}}]}}"#,
        r"I — Mill five cards. When you do, this Saga deals damage equal to the greatest power among creature cards in your graveyard to target creature.",
    ),
    // roads_go_ever_ever_on
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Subtype":"Plains"}],"count":{"min":0,"max":2},"as":"plains"}},{"Exile":{"what":{"Bound":"plains"}}},{"Shuffle":{}},{"GainLife":{"amount":2}}]}}"#,
        r"I — Search your library for up to two basic Plains cards, exile them, then shuffle. You gain 2 life.",
    ),
    // roads_go_ever_ever_on
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Choose":{"from":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}}]}}"#,
        r"II — Put a card exiled with this Saga into its owner's hand.",
    ),
    // roads_go_ever_ever_on
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Choose":{"from":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}}]}}"#,
        r"III — Put a card exiled with this Saga into its owner's hand.",
    ),
    // roads_go_ever_ever_on
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":4}},"effects":[{"DelayedTrigger":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"once":false,"this_turn":true,"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Count":[{"Subtype":"Plains"},{"ControlledBy":"You"}]},"toughness":{"Count":[{"Subtype":"Plains"},{"ControlledBy":"You"}]}}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"IV — Whenever you attack this turn, target creature you control gets +1/+1 until end of turn for each Plains you control.",
    ),
    // the_trickster_gods_heist
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Not":{"Supertype":"Basic"}},{"Not":{"Type":"Creature"}}]}},{"what":{"Object":[{"Not":{"Supertype":"Basic"}},{"Not":{"Type":"Creature"}},{"SharesType":{"Target":0}},"Other"]}}],"effects":[{"May":{"effects":[{"ExchangeControl":{"what":[{"Target":0},{"Target":1}],"duration":"Forever"}}]}}]}}"#,
        r"II — You may exchange control of two target nonbasic, noncreature permanents that share a card type.",
    ),
    // forgotten_ancient
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"RepeatWhile":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}},"most":1000,"ask":"You","effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Other"]},"as":"onto"}},{"MoveCounters":{"from":"This","to":{"Bound":"onto"},"counter":"+1/+1","count":1}}]}}]}}"#,
        r"At the beginning of your upkeep, you may move any number of +1/+1 counters from this creature onto other creatures.",
    ),
    // sanctum_of_all
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"Search":{"also":["Graveyard"],"filter":[{"OwnedBy":"You"},{"Subtype":"Shrine"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{}}}},{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}]}}]}}"#,
        r"At the beginning of your upkeep, you may search your library and/or graveyard for a Shrine card and put it onto the battlefield. If you search your library this way, shuffle.",
    ),
    // vault_112_sadistic_simulation
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Tap":{"what":{"Target":0}}},{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}},{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}}]}}"#,
        r"I — Tap up to one target creature and put a stun counter on it. You get {E}{E}.",
    ),
    // vault_112_sadistic_simulation
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Tap":{"what":{"Target":0}}},{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}},{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}}]}}"#,
        r"II — Tap up to one target creature and put a stun counter on it. You get {E}{E}.",
    ),
    // vault_112_sadistic_simulation
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Pay":{"cost":[{"PayEnergy":"X"}],"paid":[{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":1}},"then":[{"Shuffle":{}},{"ExileTop":{"count":"X","as":"exiled"}},{"Choose":{"from":{"Bound":"exiled"},"count":{"min":0,"max":1},"as":"played"}},{"CastFromZone":{"what":{"Bound":"played"},"free":true,"play":true}}]}}]}}]}}"#,
        r"III — Pay any amount of {E}. If you paid one or more {E} this way, shuffle your library, then exile that many cards from the top. You may play one of those cards without paying its mana cost.",
    ),
    // vorinclex_the_grand_evolution
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Mill":{"count":10,"as":"milled"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"milled"}},{"In":"Graveyard"},{"Type":"Creature"}]},"count":{"min":0,"max":2},"as":"chosen"}},{"Move":{"what":{"Bound":"chosen"},"to":{"Battlefield":{}}}}]}}"#,
        r"I — Mill ten cards. Put up to two creature cards from among the milled cards onto the battlefield.",
    ),
    // vorinclex_the_grand_evolution
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":"{1}","targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}}}],"duration":"UntilEndOfTurn"}},{"Exile":{"what":"This","as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}}"#,
        r#"III — Until end of turn, creatures you control gain "{1}: This creature fights target creature you don't control." Exile this Saga, then return it to the battlefield."#,
    ),
    // urza_assembles_the_titans
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Scry":{"count":4}},{"May":{"effects":[{"Bind":{"what":{"Each":["TopOfLibrary",{"OwnedBy":"You"},{"In":"Library"}]},"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Move":{"what":{"Each":[{"Is":{"Bound":"top"}},{"Type":"Planeswalker"}]},"to":"Hand"}}]}}]}}"#,
        r"I — Scry 4, then you may reveal the top card of your library. If a planeswalker card is revealed this way, put it into your hand.",
    ),
    // the_legend_of_yangchen_avatar_yangchen
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":"Opponent"}],"effects":[{"May":{"effects":[{"Draw":{"who":{"Target":0},"count":3}}]}},{"If":{"condition":"Performed","then":[{"Draw":{"count":3}}]}}]}}"#,
        r"II — You may have target opponent draw three cards. If you do, draw three cards.",
    ),
    // tamiyo_meets_the_story_circle
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"optional":true,"as":"discarded"}},{"CreateToken":{"token":{"Named":"clue"},"count":{"Product":[{"Count":[{"Is":{"Bound":"discarded"}},{"In":"Graveyard"}]},2]}}}]}}"#,
        r"II — Discard any number of cards, then investigate twice for each card discarded this way.",
    ),
    // jin_gitaxias_the_great_synthesis
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"RepeatWhile":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}}]},"op":"Ge","right":1}},"most":100,"ask":"You","effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Not":{"Type":"Land"}}]},"as":"spell"}},{"CastFromZone":{"what":{"Bound":"spell"},"free":true}}]}},{"Exile":{"what":"This","as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}}"#,
        r"III — You may cast any number of spells from your hand without paying their mana costs. Exile this Saga, then return it to the battlefield.",
    ),
    // quistis_trepe
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"}]}}],"effects":[{"CastFromZone":{"what":{"Target":0},"spend_as":"AnyType","as":"cast"}},{"Shield":{"when":{"ZoneChange":{"what":[{"In":"Stack"},{"Is":{"Bound":"cast"}}],"from":"Stack","to":"Graveyard"}},"by":{"MoveTo":"Exile"},"duration":"Forever"}}]}}"#,
        r"Blue Magic — When ~ enters, you may cast target instant or sorcery card from a graveyard, and mana of any type can be spent to cast that spell. If that spell would be put into a graveyard, exile it instead.",
    ),
    // summon_esper_valigarmanda
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Bound":"player"}},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]},"as":"card"}},{"Exile":{"what":{"Bound":"card"}}}]}}]}}"#,
        r"I — Exile an instant or sorcery card from each graveyard.",
    ),
    // summon_esper_valigarmanda
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Counters":{"on":"This","counter":{"Other":"lore"}}}}},{"Choose":{"from":{"Each":[{"In":"Exile"},{"ExiledWith":"This"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]},"as":"card"}},{"CastFromZone":{"what":{"Bound":"card"},"spend_as":"AnyType"}}]}}"#,
        r"II — Add {R} for each lore counter on this Saga. You may cast an instant or sorcery card exiled with this Saga, and mana of any type can be spent to cast that spell.",
    ),
    // summon_esper_valigarmanda
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Counters":{"on":"This","counter":{"Other":"lore"}}}}},{"Choose":{"from":{"Each":[{"In":"Exile"},{"ExiledWith":"This"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]},"as":"card"}},{"CastFromZone":{"what":{"Bound":"card"},"spend_as":"AnyType"}}]}}"#,
        r"III — Add {R} for each lore counter on this Saga. You may cast an instant or sorcery card exiled with this Saga, and mana of any type can be spent to cast that spell.",
    ),
    // summon_esper_valigarmanda
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":4}},"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Counters":{"on":"This","counter":{"Other":"lore"}}}}},{"Choose":{"from":{"Each":[{"In":"Exile"},{"ExiledWith":"This"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]},"as":"card"}},{"CastFromZone":{"what":{"Bound":"card"},"spend_as":"AnyType"}}]}}"#,
        r"IV — Add {R} for each lore counter on this Saga. You may cast an instant or sorcery card exiled with this Saga, and mana of any type can be spent to cast that spell.",
    ),
    // king_narfis_betrayal
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Mill":{"who":"EachPlayer","count":4}},{"May":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Bound":"player"}},{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]},"as":"card"}},{"Exile":{"what":{"Bound":"card"}}}]}}]}}]}}"#,
        r"I — Each player mills four cards. Then you may exile a creature or planeswalker card from each graveyard.",
    ),
    // king_narfis_betrayal
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"spend_as":"AnyColor"}},"duration":"UntilEndOfTurn"}}]}}"#,
        r"II — Until end of turn, you may cast spells from among cards exiled with this Saga, and you may spend mana as though it were mana of any color to cast those spells.",
    ),
    // king_narfis_betrayal
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"spend_as":"AnyColor"}},"duration":"UntilEndOfTurn"}}]}}"#,
        r"III — Until end of turn, you may cast spells from among cards exiled with this Saga, and you may spend mana as though it were mana of any color to cast those spells.",
    ),
    // the_curse_of_fenric
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"PerPlayer":{"players":"EachPlayer","what":[{"Type":"Creature"}]}},"bounds":{"min":0,"max":1}}],"effects":[{"Destroy":{"what":{"Target":0},"as":"destroyed"}},{"Together":{"effects":[{"RepeatEach":{"over":{"Bound":"destroyed"},"as":"each2","effects":[{"CreateToken":{"who":{"ControllerOf":{"Departed":{"Bound":"each2"}}},"token":{"Custom":{"name":"Mutant","colors":["Green"],"types":["Creature"],"subtypes":["Mutant"],"power":3,"toughness":3,"abilities":[{"Keyword":"Deathtouch"}]}}}}]}}]}}]}}"#,
        r"I — For each player, destroy up to one target creature that player controls. For each creature destroyed this way, its controller creates a 3/3 green Mutant creature token with deathtouch.",
    ),
];

/// The parsed entries; one that does not parse is dropped (a test holds that none is).
static R306B_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R306B_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r306b_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R306B_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r30-6b table parses and reads back as its wording, and the same ability with any one
    //! number changed reads back as something else.
    #![allow(clippy::unwrap_used)]
    use super::{R306B_SHAPES, R306B_WHOLES};
    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r306b_every_whole_parses_and_reads_back() {
        assert_eq!(R306B_SHAPES.len(), R306B_WHOLES.len());
        for (shape, text) in R306B_SHAPES.iter() {
            assert!(
                written_variants(shape).iter().any(|said| said == text),
                "{text}"
            );
        }
    }

    /// Each entry with one number raised by one, wherever the result still parses.
    fn near_misses(json: &str) -> Vec<Ability> {
        let mut misses = Vec::new();
        for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
            let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
            let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
            if let Ok(ability) = serde_json::from_str::<Ability>(&changed) {
                misses.push(ability);
            }
        }
        misses
    }

    #[test]
    fn r306b_a_changed_number_does_not_read_back() {
        for (json, text) in R306B_WHOLES {
            let shape: Ability = serde_json::from_str(json).unwrap();
            for miss in near_misses(json).into_iter().filter(|miss| *miss != shape) {
                assert!(
                    !written_variants(&miss).iter().any(|said| said == text),
                    "{text} read back for {miss:?}"
                );
            }
        }
    }
}
