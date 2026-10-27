//! Wordings of the fifth sample of the names G to L written from a plain twin: the ability with one part of its JSON
//! swapped for a part the renderers write, whose wordings take the card's words in place of the twin's.

use crate::ability::Ability;

/// One twin writer: the JSON parts swapped in order (each must be in the ability) and the words of the twin's
/// wordings swapped in order (each must be in the wording).
pub(super) struct C5GlTwin {
    /// The parts of the ability's compact JSON and the plain parts that stand for them.
    pub json: &'static [(&'static str, &'static str)],
    /// The twin's words and the card's.
    pub words: &'static [(&'static str, &'static str)],
}

/// The writers whose twin differs from the ability.
const C5_GL_TWINS: [C5GlTwin; 23] = [
    C5GlTwin {
        json: &[
            (
                r#""if":{"Compare":{"left":{"Counters":{"on":"This","counter":"loyalty"}},"op":"Ge","right":1}},"#,
                "",
            ),
            (
                r#"{"MoveCounters":{"from":"This","to":"Triggering","counter":"loyalty"}}"#,
                r#"{"PutCounters":{"on":"Triggering","counter":"loyalty"}}"#,
            ),
        ],
        words: &[(
            ", you may put a loyalty counter on it.",
            ", if ~ has loyalty counters on it, you may move a loyalty counter from ~ onto that planeswalker.",
        )],
    },
    C5GlTwin {
        json: &[(r#","except":[{"AddSubtype":"Tyranid"}]"#, "")],
        words: &[(
            "a copy of it.",
            "a copy of that creature, except it's a Tyranid in addition to its other types.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#",{"Mill":{"who":{"OwnerOf":{"Target":0}},"count":"X"}}"#,
            "",
        )],
        words: &[(
            " under your control.",
            " under your control. Then that player mills X cards.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#,
            r#"{"Move":{"what":"This","to":"Hand"}}"#,
        )],
        words: &[(
            "return ~ to its owner's hand.",
            "return ~ to the battlefield at the beginning of the next end step.",
        )],
    },
    C5GlTwin {
        json: &[
            (
                r#"{"Choose":{"from":{"Each":"#,
                r#"{"Exile":{"what":{"Each":"#,
            ),
            (
                r#"]},"count":{"min":0,"max":null},"as":"chosen"}},{"Exile":{"what":{"Bound":"chosen"},"#,
                "]},",
            ),
        ],
        words: &[("enters, exile all ", "enters, you may exile any number of ")],
    },
    C5GlTwin {
        json: &[(
            r#",{"If":{"condition":{"Compare":{"left":{"Counters":{"on":{"Target":0},"counter":"+1/+1"}},"op":"Ge","right":3}},"then":[{"Transform":{"what":"This"}}]}}"#,
            "",
        )],
        words: &[(
            "target creature you control.",
            "target creature you control. Then if that creature has three or more +1/+1 counters on it, transform ~.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"Explore":{"what":{"Each":[{"Subtype":"Merfolk"},{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#,
            r#"{"Explore":{"what":"This"}}"#,
        )],
        words: &[(
            ", ~ explores.",
            ", each Merfolk creature you control explores.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#",{"If":{"condition":{"Not":"Performed"},"then":[{"Draw":{}}]}}"#,
            "",
        )],
        words: &[(
            "from your hand onto the battlefield.",
            "from your hand onto the battlefield. If you don't, draw a card.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"Either":[{"Object":[{"Type":"Creature"},{"Subtype":"Dragon"},{"In":"Graveyard"},{"OwnedBy":"You"}]},{"Object":[{"Type":"Planeswalker"},{"Subtype":"Ugin"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}"#,
            r#"{"Object":[{"Type":"Creature"},{"Subtype":"Dragon"},{"In":"Graveyard"},{"OwnedBy":"You"}]}"#,
        )],
        words: &[(
            "target Dragon creature card from",
            "target Dragon creature card or Ugin planeswalker card from",
        )],
    },
    C5GlTwin {
        json: &[
            (
                r#"{"May":{"effects":[{"RevealUntil":{"filter":[{"Type":"Creature"},{"SharesSubtype":"Triggering"}]"#,
                r#"{"RevealUntil":{"filter":[{"Type":"Creature"}]"#,
            ),
            (
                r#""order":"Random"}}}}]}}]}}"#,
                r#""order":"Random"}}}}]}}"#,
            ),
        ],
        words: &[
            ("dies, reveal cards", "dies, you may reveal cards"),
            (
                "until you reveal a creature card.",
                "until you reveal a creature card that shares a creature type with it.",
            ),
        ],
    },
    C5GlTwin {
        json: &[(
            r#",{"If":{"condition":"Kicked","then":[{"Untap":{"what":{"Bound":"found"}}},{"Continuous":{"affected":{"Bound":"found"},"mods":[{"AddType":"Creature"},{"SetPT":{"power":3,"toughness":3}},{"SetColors":["Green"]},{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}"#,
            "",
        )],
        words: &[(
            "tapped, then shuffle.",
            "tapped, then shuffle.\nIf this spell was kicked, untap all Forests put onto the battlefield this way. They become 3/3 green creatures with haste that are still lands.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#",{"Type":"Creature"},"ChosenSubtype"]"#,
            r#",{"Type":"Creature"}]"#,
        )],
        words: &[(
            "reveal a creature card from among",
            "reveal a creature card of the chosen type from among",
        )],
    },
    C5GlTwin {
        json: &[(
            r#",{"Continuous":{"affected":{"Bound":"card"},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"card"}}]}}]}}"#,
            "",
        )],
        words: &[(
            "from your hand onto the battlefield.",
            "from your hand onto the battlefield. That creature gains haste until end of turn. Sacrifice it at the beginning of the next end step.",
        )],
    },
    C5GlTwin {
        json: &[
            (
                r#"{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":1,"colorless":true}}"#,
                r#"{"Fixed":["Green"]}"#,
            ),
            (
                r#"{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":3,"colorless":true}}"#,
                r#"{"Fixed":["Green","Green","Green"]}"#,
            ),
        ],
        words: &[
            (
                "Add {G}.",
                "Add one mana of any type that a land you control could produce.",
            ),
            (
                "add {G}{G}{G} instead.",
                "add three mana of that type instead.",
            ),
        ],
    },
    C5GlTwin {
        json: &[(
            r#"{"Largest":{"who":"EachPlayer","of":[{"Type":"Creature"}]}}"#,
            r#"{"Count":[{"Type":"Creature"}]}"#,
        )],
        words: &[(
            "enters with X suspect counters on it, where X is the number of creatures on the battlefield.",
            "enters with a number of suspect counters on it equal to the greatest number of creatures a player controls.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"Continuous":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Stat":{"of":"Attached","stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}"#,
            r#"{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Stat":{"of":"This","stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}"#,
        )],
        words: &[(
            ": ~ gets +X/+0 until end of turn, where X is its power.",
            ": Double equipped creature's power until end of turn.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":["Goaded"],"duration":"UntilYourNextTurn"}}"#,
            r#"{"Draw":{}}"#,
        )],
        words: &[(
            "enters, draw a card.",
            "enters, until your next turn, creatures your opponents control attack each combat if able and attack a player other than you if able.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#""source":[{"AnyOf":["This",{"Is":"Attached"}]}]"#,
            r#""source":["This"]"#,
        )],
        words: &[
            ("Whenever ~ deals", "Whenever ~ or equipped creature deals"),
            ("goad all creatures", "goad each creature"),
        ],
    },
    C5GlTwin {
        json: &[(
            r#",{"Rule":{"rule":{"PlayersHave":{"who":"You","keyword":"Hexproof"}},"duration":"UntilEndOfTurn"}}"#,
            "",
        )],
        words: &[(
            "Permanents you control gain hexproof",
            "You and permanents you control gain hexproof",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"AddKeyword":{"Afflict":2}}"#,
            r#"{"AddKeyword":"Flying"}"#,
        )],
        words: &[(" have flying.", " have afflict 2.")],
    },
    C5GlTwin {
        json: &[(
            r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}"#,
            r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
        )],
        words: &[(
            "~ costs {1} less to cast for each creature you control.",
            "~ costs {1} less to cast for each 1 life you gained this turn.",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"PlayerAttacks":{"by":"You","at":"EachPlayer"}}"#,
            r#"{"PlayerAttacks":{"by":"You","at":"You"}}"#,
        )],
        words: &[(
            "Whenever you attack you with one or more creatures,",
            "Whenever you attack a player,",
        )],
    },
    C5GlTwin {
        json: &[(
            r#"{"Shield":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}},"#,
            "",
        )],
        words: &[(
            "if you control your commander, other creatures",
            "if you control your commander, prevent all combat damage that would be dealt to creatures you control this turn. Other creatures",
        )],
    },
];

/// Adds the wordings of each twin writer that applies to the ability.
pub(super) fn c5_gl_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    for twin in &C5_GL_TWINS {
        texts.extend(c5_gl_twin_wordings(&json, twin));
    }
}

/// The wordings of one twin writer for the ability's compact JSON; none when a part is missing. A twin that differs
/// from the ability is written with every writer, oracle-only ones included (each step of that recursion swaps a part
/// away); the ability itself only with the renderers.
pub(super) fn c5_gl_twin_wordings(json: &str, twin: &C5GlTwin) -> Vec<String> {
    let mut plain = json.to_owned();
    for (from, to) in twin.json {
        if !plain.contains(from) {
            return Vec::new();
        }
        plain = plain.replacen(from, to, 1);
    }
    let Ok(ability) = serde_json::from_str::<Ability>(&plain) else {
        return Vec::new();
    };
    let wordings = if twin.json.is_empty() {
        super::super::variants(&ability)
    } else {
        super::super::written_variants(&ability)
    };
    wordings
        .into_iter()
        .filter(|text| twin.words.iter().all(|(from, _)| text.contains(from)))
        .map(|text| {
            twin.words
                .iter()
                .fold(text, |text, (from, to)| text.replacen(from, to, 1))
        })
        .collect()
}
