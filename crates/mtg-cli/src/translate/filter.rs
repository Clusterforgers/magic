//! The grammar of Forge filters (`Creature.YouCtrl+!token,Planeswalker.Other`): split into alternatives, a base and
//! properties without losing a word, then each word lowered by the readers of `valid` or by a row of the atom table.
//! An unknown word is a gap, never a guess, and a string reports every unknown word, not only the first.
//!
//! ```text
//! filter := alt ("," alt)*          any of
//! alt    := base ("." prop ("+" prop)*)?   all of
//! prop   := "!"? atom
//! ```

use serde_json::{Value, json};

use super::ctx::{Ctx, SUPERTYPES, Tr, Unmapped, unmapped};
use super::keyword::keyword_name;

/// A filter: any of its alternatives.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Filter<'a> {
    pub alts: Vec<Alt<'a>>,
}

/// One alternative: a base word and the properties that all hold.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Alt<'a> {
    pub base: &'a str,
    pub props: Vec<Prop>,
}

/// A property word, negated with `!`.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Prop {
    pub not: bool,
    pub atom: String,
}

/// Splits a filter; an empty alternative, base or property is a gap, so the whole string is consumed or refused.
pub(super) fn parse(spec: &str) -> Result<Filter<'_>, String> {
    let mut alts = Vec::new();
    for alt in spec.split(',') {
        let (base, rest) = alt.trim().split_once('.').unwrap_or((alt.trim(), ""));
        if base.is_empty() {
            return Err("valid:empty-alternative".into());
        }
        let mut props = Vec::new();
        for part in super::valid::split_properties(rest) {
            let (not, atom) = match part.strip_prefix('!') {
                Some(atom) => (true, atom.to_owned()),
                None => (false, part.clone()),
            };
            if atom.is_empty() {
                return Err("valid:empty-property".into());
            }
            props.push(Prop { not, atom });
        }
        alts.push(Alt { base, props });
    }
    Ok(Filter { alts })
}

#[cfg(test)]
impl Filter<'_> {
    /// The text the filter was split from.
    pub(super) fn render(&self) -> String {
        let alts: Vec<String> = self
            .alts
            .iter()
            .map(|alt| {
                let props: Vec<String> = alt
                    .props
                    .iter()
                    .map(|prop| format!("{}{}", if prop.not { "!" } else { "" }, prop.atom))
                    .collect();
                if props.is_empty() {
                    alt.base.to_owned()
                } else {
                    format!("{}.{}", alt.base, props.join("+"))
                }
            })
            .collect();
        alts.join(",")
    }
}

/// Where a filter is read: the scopes an atom may be legal in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Objects on the table or in a zone.
    Object,
    /// A damage source (`ValidSource$`): the object that deals the damage.
    Source,
    /// A target being chosen: it may not read another target of the same line, which is not chosen yet.
    Targeting,
}

/// One row of the atom table.
pub struct Atom {
    /// The whole word, matched exactly.
    pub word: &'static str,
    /// The scopes where the word is legal; any other scope refuses it.
    pub scopes: &'static [Scope],
    /// Whether the predicates read another target of the line.
    pub reads_target: bool,
    /// A card whose script uses the word, the first test of the row.
    #[cfg_attr(not(test), allow(dead_code))]
    pub probe: &'static str,
    /// The predicates the word stands for.
    pub lower: fn(&Ctx) -> Tr<Vec<Value>>,
}

const OBJECT: &[Scope] = &[Scope::Object, Scope::Source, Scope::Targeting];
const SOURCE: &[Scope] = &[Scope::Source];

// The rows share the signature of `Atom::lower`.
#[allow(clippy::unnecessary_wraps)]
fn one(value: Value) -> Tr<Vec<Value>> {
    Ok(vec![value])
}

fn colored(name: &'static str) -> Value {
    json!({ "Color": name })
}

/// The rows. Words that need an engine fact are not here: they stay gaps.
pub(super) const ATOMS: &[Atom] = &[
    Atom {
        word: "YouDontOwn",
        scopes: OBJECT,
        reads_target: false,
        probe: "Edea, Possessed Sorceress",
        lower: |_| one(json!({"Not": {"OwnedBy": "You"}})),
    },
    Atom {
        word: "IsRingbearer",
        scopes: OBJECT,
        reads_target: false,
        probe: "Frodo, Adventurous Hobbit",
        lower: |_| one(json!("RingBearer")),
    },
    Atom {
        word: "suspended",
        scopes: OBJECT,
        reads_target: false,
        probe: "Pardic Dragon",
        lower: |_| one(json!("Suspended")),
    },
    Atom {
        word: "escaped",
        scopes: OBJECT,
        reads_target: false,
        probe: "Phlage, Titan of Fire's Fury",
        lower: |_| one(json!({"CastWith": "Escape"})),
    },
    Atom {
        word: "DamagedBy",
        scopes: OBJECT,
        reads_target: false,
        probe: "Baron Sengir",
        lower: |_| one(json!({"DamagedBy": "This"})),
    },
    Atom {
        word: "nonChosenCard",
        scopes: OBJECT,
        reads_target: false,
        probe: "Mount Doom",
        lower: |ctx| match &ctx.chosen_card {
            Some(name) => one(json!({"Not": {"Is": {"Bound": name}}})),
            None => unmapped("valid:nonChosenCard-without-a-choice"),
        },
    },
    Atom {
        word: "RedSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Sphere of Law",
        lower: |_| one(colored("Red")),
    },
    Atom {
        word: "BlueSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Circle of Protection: Blue",
        lower: |_| one(colored("Blue")),
    },
    Atom {
        word: "BlackSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Pilgrim of Virtue",
        lower: |_| one(colored("Black")),
    },
    Atom {
        word: "GreenSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Circle of Protection: Green",
        lower: |_| one(colored("Green")),
    },
    Atom {
        word: "WhiteSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Circle of Protection: White",
        lower: |_| one(colored("White")),
    },
    Atom {
        word: "ColorlessSource",
        scopes: SOURCE,
        reads_target: false,
        probe: "Lithomancer's Focus",
        lower: |_| one(json!("Colorless")),
    },
    Atom {
        word: "equalPT",
        scopes: OBJECT,
        reads_target: false,
        probe: "Master Symmetrist",
        lower: |_| one(stat_test("powerEQtoughness").unwrap_or_default()),
    },
];

/// The predicates of `word` from the table, `None` when no row has the word, a gap when the row is not legal in `scope`.
pub(super) fn atom(ctx: &Ctx, scope: Scope, word: &str) -> Option<Tr<Vec<Value>>> {
    if let Some(row) = ATOMS.iter().find(|row| row.word == word) {
        if !row.scopes.contains(&scope) || (scope == Scope::Targeting && row.reads_target) {
            return Some(unmapped(format!("valid:{word}-in-{scope:?}")));
        }
        return Some((row.lower)(ctx));
    }
    if let Some(test) = stat_test(word) {
        return Some(one(test));
    }
    // `blockingTriggeredAttacker`, `blockedByEquipped`: blocks (is blocked by) the object a `Defined$` word names.
    for (prefix, key) in [("blocking", "Blocks"), ("blockedBy", "BlockedBy")] {
        let Some(name) = word.strip_prefix(prefix) else {
            continue;
        };
        if !name.starts_with(|c: char| c.is_ascii_uppercase()) || name.starts_with("Valid") {
            continue;
        }
        return Some(match super::valid::defined(ctx, name) {
            Ok(who) if !super::valid_atoms::is_player(&who) => one(json!({ key: who })),
            Ok(_) => unmapped(format!("valid:{word}-names-a-player")),
            Err(gap) => Err(gap),
        });
    }
    // `hasKeywordFlash`: the object has the keyword, as `withFlash` says.
    let name = word.strip_prefix("hasKeyword")?;
    let spaced = super::valid::spaced(name);
    if let Some(keyword) = keyword_name(&spaced) {
        return Some(one(json!({"HasKeyword": keyword})));
    }
    let named = mtg_cards::keyword::Keyword::NAMES
        .iter()
        .find(|known| known.eq_ignore_ascii_case(&spaced))?;
    Some(one(json!({"HasKeywordNamed": named})))
}

/// `powerLTtoughness`, `powerGTbasePower`: a stat of the object against another stat of it. The right side is a stat word
/// (parts and numbers are the old readers' and come first); the whole word is matched.
fn stat_test(word: &str) -> Option<Value> {
    const STATS: [(&str, &str); 5] = [
        ("power", "Power"),
        ("toughness", "Toughness"),
        ("basePower", "BasePower"),
        ("baseToughness", "BaseToughness"),
        ("cmc", "ManaValue"),
    ];
    const OPS: [(&str, &str); 7] = [
        ("GE", "Ge"),
        ("LE", "Le"),
        ("EQ", "Eq"),
        ("GT", "Gt"),
        ("LT", "Lt"),
        ("NE", "Ne"),
        ("NOT", "Ne"),
    ];
    let stat = |name: &str| {
        STATS
            .iter()
            .find(|(word, _)| *word == name)
            .map(|(_, kind)| *kind)
    };
    let candidate = |kind: &str| json!({"Stat": {"of": "Candidate", "stat": kind}});
    for (left_word, left) in STATS {
        let Some(rest) = word.strip_prefix(left_word) else {
            continue;
        };
        for (op_word, op) in OPS {
            if let Some(right) = rest.strip_prefix(op_word).and_then(stat) {
                return Some(
                    json!({"Test": {"left": candidate(left), "op": op, "right": candidate(right)}}),
                );
            }
        }
    }
    None
}

/// The predicates of a base word the old reader does not know: a supertype (`Legendary.YouCtrl`).
pub(super) fn base(word: &str) -> Option<Vec<Value>> {
    SUPERTYPES
        .contains(&word)
        .then(|| vec![json!({"Supertype": word})])
}

/// The gaps of one filter: every unknown word, once, in the order met.
#[derive(Default)]
pub(super) struct Unknowns(Vec<String>);

impl Unknowns {
    /// Notes the names of a gap.
    pub(super) fn note(&mut self, gap: &Unmapped) {
        for name in gap.0.split("; ") {
            if !self.0.iter().any(|seen| seen == name) {
                self.0.push(name.to_owned());
            }
        }
    }

    /// The value of a reading, or nothing with its gap noted.
    pub(super) fn take<T: Default>(&mut self, found: Tr<T>) -> T {
        found.unwrap_or_else(|gap| {
            self.note(&gap);
            T::default()
        })
    }

    /// Fails with every name noted, if any.
    pub(super) fn finish(self) -> Tr<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Unmapped(self.0.join("; ")))
        }
    }
}
