//! Twins and swaps. A twin is the ability with one part of its JSON swapped for a plain stand-in the renderers write,
//! whose wording gets the card's words back ("a spell with cascade", "and each player", "you own in exile and in your
//! graveyard", "from a colorless source", "its controller gains control of ~", "can't be blocked this turn except by
//! creatures with haste", "becomes a 4/4 artifact creature", "can't be countered", "attacks alone", "while it's
//! exiled", "of the chosen type", "for each color of mana spent to cast it", "enters from your graveyard" and "less
//! than or equal to the amount of life you lost this turn"). A swap keeps the ability as it is (each of its parts is its
//! own stand-in) and edits the wordings found so far ("and you scry X", "as you activate this ability", "Choose a
//! color", a granted ability "until end of turn" and "When you do").

use super::{c6_af_json, c6_af_variants, c6_af_written};
use crate::ability::Ability;

/// One entry: the JSON swaps that make the twin (each must be found) and the word swaps that give the card's wording
/// (each must be found, in order).
struct C6AfTwin {
    /// The parts of the ability's JSON and their plain stand-ins; a part that is its own stand-in only guards.
    rules: &'static [(&'static str, &'static str)],
    /// The twin's words and the card's.
    words: &'static [(&'static str, &'static str)],
}

impl C6AfTwin {
    /// Whether the entry keeps the ability as it is and only edits the wordings found so far.
    fn c6_af_is_swap(&self) -> bool {
        self.rules.iter().all(|(from, to)| from == to)
    }
}

/// The guard of a count of targets made as the ability is activated.
const C6_AF_COUNTED_TARGETS: &str = r#""bounds":{"min":0,"max":{"Count":"#;

/// The twins and swaps this sample needs; a swap runs after the twins whose wordings it edits.
const C6_AF_TWINS: [C6AfTwin; 19] = [
    C6AfTwin {
        rules: &[(
            r#""Tallied":{"who":"You","what":"LifeLost"}}}}]},"bounds":{"min":0,"max":1}}"#,
            r#""Tallied":{"who":"You","what":"LifeGainedTotal"}}}}]}}"#,
        )],
        words: &[
            ("Return target ", "Return up to one target "),
            (
                " with mana value X or less ",
                " with mana value less than or equal to the amount of life you lost this turn ",
            ),
            (
                " to the battlefield, where X is the amount of life you gained this turn.",
                " to the battlefield.",
            ),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#"{"HasKeyword":"Cascade"}],"by":"You""#,
            r#"{"Type":"Creature"}],"by":"You""#,
        )],
        words: &[("cast a creature spell,", "cast a spell with cascade,")],
    },
    C6AfTwin {
        rules: &[
            (r#",{"DealDamage":{"to":"EachPlayer","amount":4}}"#, ""),
            (r#",{"DealDamage":{"to":"EachPlayer","amount":1}}"#, ""),
        ],
        words: &[
            (" without flying.", " without flying and each player."),
            (
                " Then if this spell was kicked, ~ deals ",
                " If this spell was kicked, it deals ",
            ),
            (
                " without flying instead.",
                " without flying and each player instead.",
            ),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#"{"AnyOf":[{"In":"Exile"},{"In":"Graveyard"}]},{"OwnedBy":"You"}"#,
            r#"{"In":"Graveyard"},{"OwnedBy":"You"}"#,
        )],
        words: &[
            ("equal to the number of ", "equal to the total number of "),
            (
                " cards in your graveyard",
                " cards you own in exile and in your graveyard",
            ),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#"{"ControlledBy":"You"},"Colorless"]"#,
            r#"{"ControlledBy":"You"}]"#,
        )],
        words: &[(
            "ability you control",
            "ability you control from a colorless source",
        )],
    },
    C6AfTwin {
        rules: &[(
            r#"{"Continuous":{"affected":"This","mods":[{"SetController":{"ControllerOf":{"Target":0}}}],"duration":"Forever"}}"#,
            r#"{"Draw":{}}"#,
        )],
        words: &[(
            "If you do, draw a card.",
            "If you do, its controller gains control of ~.",
        )],
    },
    C6AfTwin {
        rules: &[(
            r#",{"CantBeBlockedBy":[{"Not":{"HasKeyword":"Haste"}}]}"#,
            "",
        )],
        words: &[
            (
                " gain haste until end of turn,",
                " each gain haste until end of turn and can't be blocked this turn except by creatures with haste,",
            ),
            (
                " you control.",
                " you control as you activate this ability.",
            ),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#"{"AddType":"Artifact"},{"AddType":"Creature"}"#,
            r#"{"AddType":"Creature"}"#,
        )],
        words: &[
            (", until end of turn, each ", ", each "),
            (
                " becomes artifact creatures with base power and toughness 4/4.",
                " becomes a 4/4 artifact creature until end of turn.",
            ),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#",{"Rule":{"rule":{"CantBeCountered":{"Each":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}]}},"duration":"UntilEndOfTurn"}}"#,
            "",
        )],
        words: &[(
            "Add {R} or {G}.",
            "Add {R} or {G}. Creature spells you cast this turn can't be countered.",
        )],
    },
    C6AfTwin {
        rules: &[
            (
                r#""AttacksAlone":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#,
                r#""Attacks":{"what":["This"]}"#,
            ),
            (r#""affected":"Triggering""#, r#""affected":"This""#),
        ],
        words: &[
            (
                "Whenever ~ attacks, until end of turn, he gets ",
                "Whenever a creature you control attacks alone, it gets ",
            ),
            (", where X", " until end of turn, where X"),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#""counter":{"Other":"time"}}},"from":"Exile","#,
            r#""counter":{"Other":"time"}}},"#,
        )],
        words: &[(
            "removed from ~, you create ",
            "removed from ~ while it's exiled, create ",
        )],
    },
    C6AfTwin {
        rules: &[(
            r#""what":["ChosenSubtype"],"by":"You""#,
            r#""what":[{"Type":"Creature"}],"by":"You""#,
        )],
        words: &[("cast a creature spell,", "cast a spell of the chosen type,")],
    },
    C6AfTwin {
        rules: &[(r#""count":"ColorsSpent""#, r#""count":"X""#)],
        words: &[(
            " enters with X +1/+1 counters on it",
            " enters with a +1/+1 counter on it for each color of mana spent to cast it",
        )],
    },
    C6AfTwin {
        rules: &[
            (r#""from":"Graveyard","#, ""),
            (
                r#""Other",{"OwnedBy":"You"}"#,
                r#""Other",{"ControlledBy":"You"}"#,
            ),
        ],
        words: &[
            (
                "Whenever ~ or another creature you control enters,",
                "Whenever ~ or another creature enters from your graveyard,",
            ),
            ("that permanent's power", "its power"),
        ],
    },
    C6AfTwin {
        rules: &[(
            r#"{"Scry":{"count":{"Count":"#,
            r#"{"Scry":{"count":{"Count":"#,
        )],
        words: &[(". Then scry X, where X", " and you scry X, where X")],
    },
    C6AfTwin {
        rules: &[(
            r#"{"AnyColor":{"Counters":{"on":"This","counter":"charge"}}}"#,
            r#"{"AnyColor":{"Counters":{"on":"This","counter":"charge"}}}"#,
        )],
        words: &[(
            "Add X mana of any one color, where X is the number of charge counters on ~.",
            "Choose a color. Add one mana of that color for each charge counter on ~.",
        )],
    },
    C6AfTwin {
        rules: &[(
            r#""mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"#,
            r#""mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"#,
        )],
        words: &[
            (
                " gains \"{T}: Until end of turn, destroy ",
                " gain \"{T}: Destroy ",
            ),
            (
                "Each Angel you control",
                "Until end of turn, Angels you control",
            ),
            ("power.\".", "power.\""),
        ],
    },
    C6AfTwin {
        rules: &[(C6_AF_COUNTED_TARGETS, C6_AF_COUNTED_TARGETS)],
        words: &[(
            " you control. Then activate only as a sorcery.",
            " you control as you activate this ability. Activate only as a sorcery.",
        )],
    },
    C6AfTwin {
        rules: &[(
            r#"]}},{"If":{"condition":"Performed","then":["#,
            r#"]}},{"If":{"condition":"Performed","then":["#,
        )],
        words: &[("Then then if you do, ", "When you do, ")],
    },
];

/// The plain twin of the ability, when every swap of the entry applies.
fn c6_af_twin(ability: &Ability, twin: &C6AfTwin) -> Option<Ability> {
    let mut json = c6_af_json(ability);
    for (from, to) in twin.rules {
        json.contains(from).then_some(())?;
        json = json.replace(from, to);
    }
    serde_json::from_str(&json).ok()
}

/// The wording with each of the card's words put back; `None` when one of them is not found.
fn c6_af_words(text: &str, words: &[(&str, &str)]) -> Option<String> {
    let mut text = text.to_owned();
    for (from, to) in words {
        text.contains(from).then_some(())?;
        text = text.replacen(from, to, 1);
    }
    Some(text)
}

/// Adds the wordings of each twin and swap that applies to the ability.
pub(super) fn c6_af_add(ability: &Ability, texts: &mut Vec<String>) {
    for twin in &C6_AF_TWINS {
        let Some(plain) = c6_af_twin(ability, twin) else {
            continue;
        };
        let mut found = c6_af_variants(&plain);
        if twin.c6_af_is_swap() {
            found.extend(texts.iter().cloned());
        } else {
            found.extend(c6_af_written(&plain));
        }
        let edited: Vec<String> = found
            .iter()
            .filter_map(|text| c6_af_words(text, twin.words))
            .collect();
        texts.extend(edited);
    }
}
