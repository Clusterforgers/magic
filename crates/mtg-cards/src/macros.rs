//! Keyword macros: a keyword or keyword action that expands into existing nodes, so a new one is a table entry and no engine code.
//!
//! Design record: `docs/cards-dsl.md`, "Keyword macros". A template is JSON for the abilities (keywords) or effects
//! (actions) it stands for; `$N` is the keyword's number, `$ON` the object an action works on.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// One macro: its name as the card prints it and the JSON it expands to.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The name in lower case.
    pub name: &'static str,
    /// A JSON list of abilities (keyword) or effects (action) with `$N` and `$ON`.
    pub template: &'static str,
}

/// The keyword abilities that expand into abilities of their own.
pub const KEYWORDS: &[Entry] = &[
    Entry {
        name: "bushido",
        template: r#"[{"Triggered":{"when":{"AnyOf":[{"Blocks":{"what":["This"]}},{"BecomesBlocked":{"what":["This"]}}]},"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":$N,"toughness":$N}}],"duration":"UntilEndOfTurn"}}]}}]"#,
    },
    Entry {
        name: "modular",
        template: r#"[{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":$N}}}},
            {"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Type":"Creature"}]}}],
             "effects":[{"May":{"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}]}}]"#,
    },
    Entry {
        name: "mobilize",
        template: r#"[{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[
            {"CreateToken":{"token":{"Custom":{"name":"Warrior","colors":["Red"],"types":["Creature"],"subtypes":["Warrior"],"power":1,"toughness":1}},"count":$N,"tapped":true,"attacking":true,"as":"warriors"}},
            {"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"warriors"}}],"count":$N}}]}}]}}]"#,
    },
    Entry {
        name: "flanking",
        template: r#"[{"Triggered":{"when":{"BecomesBlocked":{"what":["This"],"by":[{"Not":{"HasKeyword":{"Macro":{"name":"flanking","amount":0}}}}]}},"effects":[{"Continuous":{"affected":"Dealer","mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}]"#,
    },
    Entry {
        name: "firebending",
        template: r#"[{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":$N,"lasts":"Combat"}}]}}]"#,
    },
    Entry {
        name: "soulshift",
        template: r#"[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Subtype":"Spirit"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":$N}}]}}],
             "effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}]"#,
    },
    Entry {
        name: "bloodthirst",
        template: r#"[{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":$N}},
             "condition":{"Compare":{"left":{"Tallied":{"who":"Opponents","what":"DamageTaken"}},"op":"Ge","right":1}}}}]"#,
    },
    Entry {
        name: "training",
        template: r#"[{"Triggered":{"when":{"Attacks":{"what":["This"]}},
             "if":{"Compare":{"left":{"Count":["Attacking",{"Compare":{"stat":"Power","op":"Gt","to":{"Stat":{"of":"This","stat":"Power"}}}}]},"op":"Ge","right":1}},
             "effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":1}}]}}]"#,
    },
    Entry {
        name: "ingest",
        template: r#"[{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"ExileTop":{"who":"Triggering","as":"ingested"}}]}}]"#,
    },
    Entry {
        name: "unleash",
        template: r#"[{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"MayWithCounters":{"counter":"+1/+1","count":1}}}},
            {"Static":{"affected":"This","mods":["CantBlock"],"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}}}}]"#,
    },
    Entry {
        name: "living metal",
        template: r#"[{"Static":{"affected":"This","mods":[{"AddType":"Creature"}],"condition":"YourTurn"}}]"#,
    },
    Entry {
        name: "teamwork",
        template: r#"[{"AdditionalCost":{"cost":[{"TapPower":{"filter":[{"Type":"Creature"},{"ControlledBy":"You"}],"power":$N}}]}}]"#,
    },
    Entry {
        name: "increment",
        template: r#"[{"Triggered":{"when":{"SpellCast":{"by":"You"}},
             "if":{"Any":[{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}},
                          {"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Toughness"}}}}]},
             "effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":1}}]}}]"#,
    },
    Entry {
        name: "provoke",
        template: r#"[{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Defending"}]}}],
             "effects":[{"May":{"effects":[{"Untap":{"what":{"Target":0}}},
               {"Rule":{"rule":{"MustBlockAttacker":{"who":{"Target":0},"attacker":"This"}},"duration":"UntilEndOfCombat"}}]}}]}}]"#,
    },
    Entry {
        name: "rampage",
        template: r#"[{"Triggered":{"when":{"BecomesBlocked":{"what":["This"]}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Blocks":"This"}]},"op":"Ge","right":2}},"then":[{"Continuous":{"affected":"This",
             "mods":[{"ModifyPT":{"power":{"Times":{"by":$N,"of":{"Sum":[{"Count":[{"Blocks":"This"}]},-1]}}},"toughness":{"Times":{"by":$N,"of":{"Sum":[{"Count":[{"Blocks":"This"}]},-1]}}}}}],"duration":"UntilEndOfTurn"}}]}}]}}]"#,
    },
    Entry {
        name: "enlist",
        template: r#"[{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[
             {"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"Untapped",{"Not":"Attacking"},{"Not":{"HasKeyword":{"Macro":{"name":"enlist","amount":0}}}},{"Not":"SummoningSick"}]},"as":"enlisted"}},
             {"Tap":{"what":{"Bound":"enlisted"}}},
             {"Reflexive":{"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Bound":"enlisted"},"stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}]}}]"#,
    },
];

/// The keyword actions that expand into effects: populate, endure, earthbend, blight, recruit, empower and assimilate.
pub const ACTIONS: &[Entry] = &[
    Entry {
        name: "populate",
        template: r#"[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},
            {"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]"#,
    },
    Entry {
        name: "endure",
        template: r#"[{"If":{"condition":{"Compare":{"left":{"Count":[{"Is":$ON},{"In":"Battlefield"}]},"op":"Ge","right":1}},
            "then":[{"PutCounters":{"on":$ON,"counter":"+1/+1","count":$N}}],
            "otherwise":[{"CreateToken":{"token":{"Sized":{"size":$N,"token":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"]}}}}}]}}]"#,
    },
    Entry {
        name: "earthbend",
        template: r#"[{"Continuous":{"affected":$ON,"mods":[{"AddType":"Creature"},{"SetPT":{"power":0,"toughness":0}},{"AddKeyword":"Haste"}],"duration":"Forever"}},
            {"PutCounters":{"on":$ON,"counter":"+1/+1","count":$N}},
            {"DelayedTrigger":{"when":{"AnyOf":[{"ZoneChange":{"what":[{"Is":$ON}],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Is":$ON}],"from":"Battlefield","to":"Exile"}}]},
             "effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"tapped":true}}}}]}}]"#,
    },
    Entry {
        name: "blight",
        template: r#"[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"blighted"}},
            {"PutCounters":{"on":{"Bound":"blighted"},"counter":"-1/-1","count":$N}}]"#,
    },
    Entry {
        name: "recruit",
        template: r#"[{"Draw":{}},{"Discard":{"as":"discarded"}},
            {"If":{"condition":{"Compare":{"left":{"Count":[{"Is":{"Bound":"discarded"}},{"In":"Graveyard"},{"Not":{"Type":"Land"}}]},"op":"Ge","right":1}},
             "then":[{"CreateToken":{"token":{"Custom":{"name":"Human Soldier","colors":["White"],"types":["Creature"],"subtypes":["Human","Soldier"],"power":1,"toughness":1}}}}]}}]"#,
    },
    Entry {
        name: "empower",
        template: r#"[{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"op":"Lt","right":1}},
             "then":[{"CreateToken":{"token":{"Custom":{"name":"Jace","colors":["Blue"],"types":["Planeswalker"],"subtypes":["Jace"],"abilities":[
               {"Loyalty":{"cost":{"Minus":1},"effects":[{"Surveil":{"count":1}}]}},
               {"Loyalty":{"cost":{"Minus":3},"effects":[{"Draw":{"count":1}}]}}]}}}}]}},
            {"Choose":{"from":{"Each":[{"Subtype":"Jace"},"Token",{"ControlledBy":"You"}]},"as":"jace"}},
            {"PutCounters":{"on":{"Bound":"jace"},"counter":"loyalty","count":$N}}]"#,
    },
    Entry {
        name: "assimilate",
        template: r#"[{"Move":{"what":$ON,"to":{"Battlefield":{"under":"You","counters":[["+1/+1",1]]}}}},
            {"Continuous":{"affected":$ON,"mods":[{"AddType":"Artifact"},{"SetCreatureTypes":["Borg"]}],"duration":"Forever"}}]"#,
    },
];

/// The entry of that name (case-insensitive).
fn find(table: &'static [Entry], name: &str) -> Option<&'static Entry> {
    table
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(name))
}

/// Whether the keyword prints a number after its name ("Bushido 2"); one that does not ("Flanking") has the amount 0.
pub fn takes_number(name: &str) -> bool {
    find(KEYWORDS, name).is_some_and(|entry| entry.template.contains("$N"))
}

/// Whether a keyword macro of that name exists.
pub fn is_keyword(name: &str) -> bool {
    find(KEYWORDS, name).is_some()
}

/// Whether a keyword action of that name exists.
pub fn is_action(name: &str) -> bool {
    find(ACTIONS, name).is_some()
}

/// The template with the number and the object put in.
fn fill(template: &str, amount: &Amount, on: &Ref) -> Option<String> {
    let on = serde_json::to_string(on).ok()?;
    let amount = serde_json::to_string(amount).ok()?;
    Some(template.replace("$ON", &on).replace("$N", &amount))
}

/// The abilities a keyword macro stands for; `None` for a name that is not in the table.
pub fn keyword_abilities(name: &str, amount: u32) -> Option<Vec<Ability>> {
    let entry = find(KEYWORDS, name)?;
    let amount = Amount::Fixed(i32::try_from(amount).ok()?);
    serde_json::from_str(&fill(entry.template, &amount, &Ref::This)?).ok()
}

/// The effects a keyword action stands for on the object `on`; `None` for a name that is not in the table.
pub fn action_effects(name: &str, amount: u32, on: &Ref) -> Option<Vec<Effect>> {
    let amount = Amount::Fixed(i32::try_from(amount).ok()?);
    action_effects_of(name, &amount, on)
}

/// The effects a keyword action stands for with any amount ("empower Jace X"); `None` for a name that is not in the table.
pub fn action_effects_of(name: &str, amount: &Amount, on: &Ref) -> Option<Vec<Effect>> {
    let entry = find(ACTIONS, name)?;
    serde_json::from_str(&fill(entry.template, amount, on)?).ok()
}

#[cfg(test)]
mod tests {
    //! Goal: every table entry expands to valid nodes for every number and object, and the expansion carries the number.
    use rstest::rstest;

    use super::*;

    #[test]
    fn every_template_expands_for_each_number() {
        for amount in 1..=5 {
            for entry in KEYWORDS {
                assert!(
                    keyword_abilities(entry.name, amount).is_some(),
                    "{}",
                    entry.name
                );
            }
            for entry in ACTIONS {
                for on in [Ref::This, Ref::Target(0)] {
                    assert!(
                        action_effects(entry.name, amount, &on).is_some(),
                        "{}",
                        entry.name
                    );
                }
            }
        }
    }

    #[rstest]
    #[case::bushido("bushido", 1)]
    #[case::bushido_three("Bushido", 3)]
    #[case::modular("modular", 2)]
    #[case::mobilize("mobilize", 4)]
    #[case::firebending("firebending", 2)]
    #[case::soulshift("soulshift", 3)]
    #[case::bloodthirst("bloodthirst", 2)]
    fn a_keyword_carries_its_number(#[case] name: &str, #[case] amount: u32) {
        let json = serde_json::to_string(&keyword_abilities(name, amount).unwrap()).unwrap();
        assert!(json.contains(&amount.to_string()), "{json}");
    }

    #[rstest]
    #[case::endure("endure", 3)]
    #[case::earthbend("earthbend", 2)]
    fn an_action_works_on_its_object(#[case] name: &str, #[case] amount: u32) {
        let json =
            serde_json::to_string(&action_effects(name, amount, &Ref::Target(0)).unwrap()).unwrap();
        assert!(
            json.contains("Target") && !json.contains("\"This\""),
            "{json}"
        );
        assert!(json.contains(&amount.to_string()));
    }

    #[rstest]
    #[case::unknown("double team")]
    #[case::empty("")]
    fn an_unknown_name_expands_to_nothing(#[case] name: &str) {
        assert!(
            keyword_abilities(name, 1).is_none() && action_effects(name, 1, &Ref::This).is_none()
        );
    }

    #[test]
    fn names_are_unique_across_the_tables() {
        let mut names: Vec<_> = KEYWORDS.iter().chain(ACTIONS).map(|e| e.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
