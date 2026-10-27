//! Twins: an ability the renderers cannot write, or write in other words, is changed into a plain twin they can write
//! (one part of its JSON swapped, or its intervening "if" dropped), and the twin's wording gets the card's words back:
//! "tapped and attacking", "twice X", "up to one target", "of the chosen type", "on top of your library", "enters
//! untapped", "an Eldrazi Scion", "spell or permanent", "two or more creatures", "for each time it was kicked", "where X
//! is the amount of life you gained this turn", the lands a player controls, "if its power is 6 or greater", "you may
//! sacrifice it", "Target player searches" and a distribution among any number of targets.

use super::{c5_af_json, c5_af_parse, c5_af_variants};
use crate::ability::Ability;

/// One twin: the JSON swaps that make it (each must be found), the intervening "if" it drops (as JSON, which the
/// ability must have), and the word swaps that give the card's wording (each must be found, in order).
struct C5AfTwin {
    /// The parts of the ability's JSON and their plain stand-ins.
    rules: &'static [(&'static str, &'static str)],
    /// The intervening condition of a triggered ability the twin leaves out.
    drop_if: Option<&'static str>,
    /// The twin's words and the card's.
    words: &'static [(&'static str, &'static str)],
}

/// The condition that the defending player controls more lands than the controller.
const C5_AF_DEFENDER_LANDS: &str = r#"{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"Defending"}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}"#;

/// The condition that an opponent controls more lands than the controller.
const C5_AF_OPPONENT_LANDS: &str = r#"{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}"#;

/// The twins this sample needs.
const C5_AF_TWINS: [C5AfTwin; 19] = [
    C5AfTwin {
        rules: &[(r#",{"JoinAttack":{"what":{"Target":0}}}"#, "")],
        drop_if: None,
        words: &[(
            "to the battlefield tapped.",
            "to the battlefield tapped and attacking.",
        )],
    },
    C5AfTwin {
        rules: &[(r#""count":{"Times":{"by":2,"of":"X"}}"#, r#""count":"X""#)],
        drop_if: None,
        words: &[("~ enters with X ", "~ enters with twice X ")],
    },
    C5AfTwin {
        rules: &[(r#","bounds":{"min":0,"max":1}},{"what""#, r#"},{"what""#)],
        drop_if: None,
        words: &[("attach target ", "attach up to one target ")],
    },
    C5AfTwin {
        rules: &[(
            r#","ChosenSubtype"],"to":"Battlefield""#,
            r#"],"to":"Battlefield""#,
        )],
        drop_if: None,
        words: &[(
            " you control enters,",
            " you control of the chosen type enters,",
        )],
    },
    C5AfTwin {
        rules: &[
            (r#""from":"Graveyard""#, r#""from":"Graveyard""#),
            (
                r#"{"Move":{"what":"This","to":{"Library":{"place":"Top"}}}}"#,
                r#"{"Move":{"what":"This","to":"Hand"}}"#,
            ),
        ],
        drop_if: None,
        words: &[(
            ": Return ~ from your graveyard to your hand.",
            ": Put ~ from your graveyard on top of your library.",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#""what":["This","Untapped"],"to":"Battlefield""#,
            r#""what":["This"],"to":"Battlefield""#,
        )],
        drop_if: None,
        words: &[(
            "When ~ enters, you create",
            "When ~ enters untapped, create",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#"{"Sacrifice":{"filter":[{"Subtype":"Eldrazi"},{"Subtype":"Scion"}]}}"#,
            r#"{"Sacrifice":{"filter":[{"Subtype":"Scion"}]}}"#,
        )],
        drop_if: None,
        words: &[("Sacrifice a Scion:", "Sacrifice an Eldrazi Scion:")],
    },
    C5AfTwin {
        rules: &[(
            r#"{"Either":[{"Object":["Spell"]},{"Object":[]}]}"#,
            r#"{"Object":[]}"#,
        )],
        drop_if: None,
        words: &[(
            "Target permanent becomes a white ",
            "Target spell or permanent becomes white ",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#"{"Sacrifice":{"filter":[{"Type":"Creature"},"ChosenSubtype"]}}"#,
            r#"{"Sacrifice":{"filter":[{"Type":"Creature"}]}}"#,
        )],
        drop_if: None,
        words: &[(
            "Sacrifice a creature:",
            "Sacrifice a creature of the chosen type:",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#"{"GroupAttacks":{"what":[{"ControlledBy":"Opponents"},{"AttackingPlayer":"You"}],"at_least":2}}"#,
            r#"{"PlayerAttacks":{"by":"Opponents","at":"You"}}"#,
        )],
        drop_if: None,
        words: &[(
            "attacks you with one or more creatures, you ",
            "attacks you with two or more creatures, ",
        )],
    },
    C5AfTwin {
        rules: &[(r#""count":"Kicks""#, r#""count":1"#)],
        drop_if: None,
        words: &[(
            " counter on it.",
            " counter on it for each time it was kicked.",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#""op":"Le","to":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}"#,
            r#""op":"Le","to":0"#,
        )],
        drop_if: None,
        words: &[
            ("mana value 0 or less", "mana value X or less"),
            (
                " to the battlefield.",
                " to the battlefield, where X is the amount of life you gained this turn.",
            ),
        ],
    },
    C5AfTwin {
        rules: &[(
            r#"{"Supertype":"Basic"},{"Subtype":"Plains"}"#,
            r#"{"Supertype":"Basic"},{"Type":"Land"}"#,
        )],
        drop_if: Some(C5_AF_DEFENDER_LANDS),
        words: &[
            (
                " attacks, search ",
                " attacks, if defending player controls more lands than you, search ",
            ),
            (
                "basic land card, put that card onto",
                "basic Plains card, put it onto",
            ),
        ],
    },
    C5AfTwin {
        rules: &[],
        drop_if: Some(C5_AF_OPPONENT_LANDS),
        words: &[(
            " enters, you create ",
            " enters, if an opponent controls more lands than you, create ",
        )],
    },
    C5AfTwin {
        rules: &[],
        drop_if: Some(
            r#"{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Ge","right":6}}"#,
        ),
        words: &[(
            " attacks, draw ",
            " attacks, if its power is 6 or greater, draw ",
        )],
    },
    C5AfTwin {
        rules: &[(
            r#"[{"May":{"effects":[{"Sacrifice":{"filter":[{"Is":"Dealer"}]}}]}},{"If":{"condition":"Performed","then":[{"Draw":{"count":{"Stat":{"of":"Dealer","stat":"Power"}}}}]}}]"#,
            r#"[{"Draw":{"count":2}}]"#,
        )],
        drop_if: None,
        words: &[(
            " to a player, you draw two cards.",
            " to a player, you may sacrifice it. If you do, draw cards equal to its power.",
        )],
    },
    C5AfTwin {
        rules: &[
            (r#""targets":[{"what":"Player"}],"#, ""),
            (r#""Search":{"who":{"Target":0},"#, r#""Search":{"#),
            (r#"{"Shuffle":{"who":{"Target":0}}}"#, r#"{"Shuffle":{}}"#),
        ],
        drop_if: None,
        words: &[
            (
                "Search your library for ",
                "Target player searches their library for ",
            ),
            (
                ", put it onto the battlefield tapped, then shuffle.",
                ", puts it onto the battlefield tapped, then shuffles.",
            ),
        ],
    },
    C5AfTwin {
        rules: &[
            (
                r#","bounds":{"min":0,"max":"X"},"divide":"X"}"#,
                r#","bounds":{"min":0,"max":"X"}}"#,
            ),
            (r#","count":"Divided""#, ""),
        ],
        drop_if: None,
        words: &[(
            "put a +1/+1 counter on each of up to X target ",
            "distribute X +1/+1 counters among any number of target ",
        )],
    },
    C5AfTwin {
        rules: &[
            (
                r#","bounds":{"min":0,"max":"X"},"divide":"X"}"#,
                r#","bounds":{"min":0,"max":"X"}}"#,
            ),
            (r#","count":"Divided""#, ""),
        ],
        drop_if: None,
        words: &[(
            "Put a +1/+1 counter on each of up to X target ",
            "Distribute X +1/+1 counters among any number of target ",
        )],
    },
];

/// The plain twin of the ability, when every swap of the entry applies.
fn c5_af_twin(ability: &Ability, twin: &C5AfTwin) -> Option<Ability> {
    if twin.rules.is_empty() && twin.drop_if.is_none() {
        return None;
    }
    let mut json = c5_af_json(ability);
    for (from, to) in twin.rules {
        json.contains(from).then_some(())?;
        json = json.replace(from, to);
    }
    let mut plain = c5_af_parse(&json)?;
    if let Some(condition) = twin.drop_if {
        let Ability::Triggered { intervening_if, .. } = &mut plain else {
            return None;
        };
        let held = serde_json::to_string(intervening_if.as_ref()?).ok()?;
        (held == condition).then_some(())?;
        *intervening_if = None;
    }
    Some(plain)
}

/// The twin's wording with each of the card's words put back; `None` when one of them is not found.
fn c5_af_words(text: &str, words: &[(&str, &str)]) -> Option<String> {
    let mut text = text.to_owned();
    for (from, to) in words {
        text.contains(from).then_some(())?;
        text = text.replacen(from, to, 1);
    }
    Some(text)
}

/// Adds the wordings of each twin that applies to the ability.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    for twin in &C5_AF_TWINS {
        let Some(plain) = c5_af_twin(ability, twin) else {
            continue;
        };
        texts.extend(
            c5_af_variants(&plain)
                .iter()
                .filter_map(|text| c5_af_words(text, twin.words)),
        );
    }
}
