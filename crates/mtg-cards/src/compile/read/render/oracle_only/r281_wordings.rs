//! Whole abilities the r28-1 cards word: mana a player of your choice adds, mana with a "When you do" that follows,
//! spend riders of a special action and a spell together, the targets of a spell changed, and cards "from among them"
//! after a reveal.

use super::super::variants;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::{Effect, NewTargets, Produce};
use crate::refs::{Pred, Ref};
use crate::spend::{SpecialAction, SpendFact, SpendFilter, SpendOn};
use mtg_core::card::CardType;
use mtg_core::card::Supertype;

/// Adds the wordings of an ability of one of the shapes above.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let among: Vec<String> = texts
        .iter()
        .filter(|text| {
            text.contains("reveal the top ")
                && text.contains(" cards revealed this way into your hand")
        })
        .map(|text| {
            text.replacen(
                " cards revealed this way into your hand",
                " cards from among them into your hand",
                1,
            )
        })
        .collect();
    texts.extend(among);
    let owner_shuffles: Vec<String> = texts
        .iter()
        .filter(|text| text.ends_with(": Shuffle enchanted creature into its owner's library."))
        .map(|text| {
            text.replacen(
                ": Shuffle enchanted creature into its owner's library.",
                ": Enchanted creature's owner shuffles it into their library.",
                1,
            )
        })
        .collect();
    texts.extend(owner_shuffles);
    costs_said(ability, texts);
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    match effects.as_slice() {
        [
            Effect::Choose {
                who: Ref::You,
                from: Ref::EachPlayer,
                count,
                bind,
                rest_as: None,
            },
            Effect::AddMana { who, .. },
        ] if count.is_one() && *who == Ref::Bound(bind.clone()) => {
            chosen_player(ability, texts);
        }
        [
            Effect::AddMana { who: Ref::You, .. },
            Effect::Reflexive {
                targets, effects, ..
            },
        ] if targets.is_empty() => when_you_do(ability, effects, texts),
        [Effect::AddMana { only, .. }] => {
            if let Some(rider) = rider(only) {
                with_mana(ability, texts, |head, mana| {
                    format!("{head}: Add {mana}. Spend this mana only to {rider}.")
                });
            }
        }
        [
            Effect::ChangeTargets {
                what: Ref::Target(0),
                to: NewTargets::Choose,
                onto: None,
            },
        ] => {
            let changed: Vec<String> = variants(ability)
                .iter()
                .filter(|text| text.contains(": You may choose new targets for target "))
                .map(|text| {
                    text.replacen(
                        ": You may choose new targets for target ",
                        ": You may change the targets of target ",
                        1,
                    )
                })
                .collect();
            texts.extend(changed);
        }
        [
            Effect::LockOrUnlockDoor {
                what: Ref::Target(0),
            },
        ] => lock_or_unlock(ability, texts),
        [
            Effect::TimeTravel { who: Ref::You },
            Effect::TimeTravel { who: Ref::You },
            Effect::TimeTravel { who: Ref::You },
        ] => with_sentence(ability, "Time travel three times.", texts),
        _ => {}
    }
}

/// The words of a part of a cost no other writer says: "Collect evidence 3", "Return three snow lands you control to
/// their owner's hand".
fn cost_words(item: &CostItem) -> Option<String> {
    match item {
        CostItem::CollectEvidence(total) => Some(format!("Collect evidence {total}")),
        CostItem::ReturnToHand {
            filter,
            count: crate::value::Amount::Fixed(3),
        } if filter.0 == [Pred::Supertype(Supertype::Snow), Pred::Type(CardType::Land)] => {
            Some("Return three snow lands you control to their owner's hand".to_owned())
        }
        _ => None,
    }
}

/// An activated ability with one such part: its wordings with a stand-in part (`{Q}`, or `{T}` when the cost has no tap)
/// whose symbol is then replaced by the words of the part.
fn costs_said(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { cost, .. } = ability else {
        return;
    };
    let Some((at, words)) = cost
        .0
        .iter()
        .enumerate()
        .find_map(|(at, item)| cost_words(item).map(|words| (at, words)))
    else {
        return;
    };
    let (stand, symbol) = if cost.0.contains(&CostItem::Tap) {
        (CostItem::Untap, "{Q}")
    } else {
        (CostItem::Tap, "{T}")
    };
    let mut plain = ability.clone();
    let Ability::Activated { cost, .. } = &mut plain else {
        return;
    };
    if let Some(item) = cost.0.get_mut(at) {
        *item = stand;
    }
    let mut said = Vec::new();
    for text in variants(&plain) {
        if let Some((head, rest)) = text.split_once(": ")
            && head.contains(symbol)
        {
            said.push(format!("{}: {rest}", head.replacen(symbol, &words, 1)));
        }
    }
    // A mana ability followed by a counter on its source is worded by `add_mana_shapes`.
    let mut shaped = Vec::new();
    add_mana_shapes(&plain, &mut shaped);
    let more: Vec<String> = shaped
        .iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(": ")?;
            head.contains(symbol)
                .then(|| format!("{}: {rest}", head.replacen(symbol, &words, 1)))
        })
        .collect();
    texts.extend(said);
    texts.extend(more);
}

/// The wordings this file gives a mana ability with more than its mana: a counter put on the source after the mana.
fn add_mana_shapes(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated { effects, .. } = ability else {
        return;
    };
    if let [
        Effect::AddMana { who: Ref::You, .. },
        then @ Effect::PutCounters { .. },
    ] = effects.as_slice()
    {
        let then = then.clone();
        let mut plain = ability.clone();
        let Ability::Activated { effects, .. } = &mut plain else {
            return;
        };
        effects.pop();
        let step = Ability::Spell {
            targets: Vec::new(),
            effects: vec![then],
        };
        let steps: Vec<String> = super::super::super::render::written_variants(&step);
        for text in variants(&plain) {
            for step in &steps {
                texts.push(format!("{text} {step}"));
            }
        }
    }
}

/// "{3}, {T}, Sacrifice this artifact: Lock or unlock a door of target Room you control. Activate only as a sorcery."
fn lock_or_unlock(ability: &Ability, texts: &mut Vec<String>) {
    let room =
        serde_json::json!([{"what": {"Object": [{"Subtype": "Room"}, {"ControlledBy": "You"}]}}]);
    if serde_json::to_value(ability.targets()).ok().as_ref() == Some(&room) {
        with_sentence(
            ability,
            "Lock or unlock a door of target Room you control.",
            texts,
        );
    }
}

/// The wordings of the ability, its targets and effects left out, with `sentence` in place of what it does.
fn with_sentence(ability: &Ability, sentence: &str, texts: &mut Vec<String>) {
    let mut plain = ability.clone();
    let Ability::Activated {
        targets, effects, ..
    } = &mut plain
    else {
        return;
    };
    targets.clear();
    *effects = vec![Effect::Draw {
        bind: None,
        who: Ref::You,
        count: crate::value::Amount::one(),
    }];
    for text in variants(&plain) {
        if text.contains(": Draw a card.") {
            texts.push(text.replacen(": Draw a card.", &format!(": {sentence}"), 1));
        }
    }
}

/// The ability with its effects one plain "Add ..." of the same mana, as the cost and the mana words of each of its
/// wordings ("{T}", "{G}{G}{G}"), mapped through `sentence`.
fn with_mana(ability: &Ability, texts: &mut Vec<String>, sentence: impl Fn(&str, &str) -> String) {
    let mut plain = ability.clone();
    let Ability::Activated { effects, .. } = &mut plain else {
        return;
    };
    let Some(Effect::AddMana {
        who,
        mana,
        times,
        only,
        lasts,
        when_spent,
    }) = effects
        .iter()
        .find(|e| matches!(e, Effect::AddMana { .. }))
        .cloned()
    else {
        return;
    };
    let _ = (who, only, when_spent);
    *effects = vec![Effect::AddMana {
        who: Ref::You,
        mana,
        times,
        only: Vec::new(),
        lasts,
        when_spent: None,
    }];
    for text in variants(&plain) {
        if let Some((head, rest)) = text.split_once(": Add ")
            && let Some(mana) = rest.strip_suffix('.')
        {
            texts.push(sentence(head, mana));
        }
    }
}

/// "{T}: Choose a player. That player adds {G}{G}{G}." and "{T}: A player of your choice adds {C}."; one mana of any
/// color is one "they choose".
fn chosen_player(ability: &Ability, texts: &mut Vec<String>) {
    let any = matches!(
        ability.effects().get(1),
        Some(Effect::AddMana {
            mana: Produce::AnyColor(_),
            ..
        })
    );
    with_mana(ability, texts, |head, mana| {
        let mana = if any {
            format!("{mana} they choose")
        } else {
            mana.to_owned()
        };
        format!("{head}: Choose a player. That player adds {mana}.")
    });
    with_mana(ability, texts, |head, mana| {
        let mana = if any {
            format!("{mana} they choose")
        } else {
            mana.to_owned()
        };
        format!("{head}: A player of your choice adds {mana}.")
    });
}

/// "{T}, Exile a card from your graveyard: Add {R}. When you do, this creature deals 1 damage to each opponent."
fn when_you_do(ability: &Ability, then: &[Effect], texts: &mut Vec<String>) {
    for body in super::w4c_bodies::bodies(ability, then) {
        with_mana(ability, texts, |head, mana| {
            format!("{head}: Add {mana}. When you do, {body}.")
        });
    }
}

/// The words of two spend uses that name a special action beside a spell: "foretell a card from your hand or cast an
/// instant or sorcery spell", "cast face-down spells or to turn creatures face up".
fn rider(only: &[SpendOn]) -> Option<&'static str> {
    let instant_or_sorcery = SpendFilter {
        types: vec![CardType::Instant, CardType::Sorcery],
        ..SpendFilter::default()
    };
    let face_down = SpendFilter {
        facts: vec![SpendFact::FaceDown],
        ..SpendFilter::default()
    };
    match only {
        [
            SpendOn::Special(SpecialAction::Foretell),
            SpendOn::Spell(spell),
        ] if *spell == instant_or_sorcery => {
            Some("foretell a card from your hand or cast an instant or sorcery spell")
        }
        [
            SpendOn::Spell(spell),
            SpendOn::Special(SpecialAction::TurnFaceUp),
        ] if *spell == face_down => Some("cast face-down spells or to turn creatures face up"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the r28-1 wordings read back for their abilities and a changed mana, player, rider or target does not.
    use rstest::rstest;

    use crate::ability::Ability;
    use crate::compile::read::render::written_variants;

    fn written(json: &str) -> Vec<String> {
        let ability: Ability = serde_json::from_str(json).unwrap();
        written_variants(&ability)
    }

    const CHIMES: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Choose":{"from":"EachPlayer","as":"player"}},{"AddMana":{"who":{"Bound":"player"},"mana":{"Fixed":["Colorless"]}}}]}}"#;
    const SEARCHLIGHT: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Choose":{"from":"EachPlayer","as":"player"}},{"AddMana":{"who":{"Bound":"player"},"mana":{"AnyColor":1}}}]}}"#;
    const SHAUN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}},{"Reflexive":{"effects":[{"Mill":{"count":2}}]}}]}}"#;
    const KARFELL: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue"]},"only":[{"Special":"Foretell"},{"Spell":{"types":["Instant","Sorcery"]}}]}}]}}"#;
    const GOSSIP: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Red","Green"]},"only":[{"Spell":{"facts":["FaceDown"]}},{"Special":"TurnFaceUp"}]}}]}}"#;
    const MARINA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":7,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Move":{"what":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Enchantment"}]},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#;
    const KEYS: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap","SacrificeSelf"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Subtype":"Room"},{"ControlledBy":"You"}]}}],"effects":[{"LockOrUnlockDoor":{"what":{"Target":0}}}]}}"#;
    const KEYS_ANY_ROOM: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap","SacrificeSelf"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Subtype":"Room"}]}}],"effects":[{"LockOrUnlockDoor":{"what":{"Target":0}}}]}}"#;
    const CRYPTEX: &str = r#"{"Activated":{"cost":["Tap",{"CollectEvidence":3}],"effects":[{"AddMana":{"mana":{"AnyColor":1}}},{"PutCounters":{"on":"This","counter":{"Other":"unlock"}}}]}}"#;
    const KRAKEN: &str = r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":[{"Supertype":"Snow"},{"Type":"Land"}],"count":3}}],"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#;
    const KRAKEN_TWO: &str = r#"{"Activated":{"cost":[{"ReturnToHand":{"filter":[{"Supertype":"Snow"},{"Type":"Land"}],"count":2}}],"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#;
    const GRASP: &str = r#"{"Activated":{"cost":[{"Waterbend":5}],"effects":[{"Move":{"what":"Attached","to":{"Library":{"place":"Shuffled"}}}}]}}"#;
    const GRASP_TOP: &str = r#"{"Activated":{"cost":[{"Waterbend":5}],"effects":[{"Move":{"what":"Attached","to":{"Library":{"place":"Top"}}}}]}}"#;
    const DOCTOR: &str = r#"{"Activated":{"cost":[{"Mana":"{7}"}],"timing":"Sorcery","effects":[{"TimeTravel":{}},{"TimeTravel":{}},{"TimeTravel":{}}]}}"#;
    const DOCTOR_TWICE: &str = r#"{"Activated":{"cost":[{"Mana":"{7}"}],"timing":"Sorcery","effects":[{"TimeTravel":{}},{"TimeTravel":{}}]}}"#;
    const FLECTO: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"ChangeTargets":{"what":{"Target":0},"to":"Choose"}}]}}"#;

    #[rstest]
    #[case::chimes(CHIMES, "{T}: A player of your choice adds {C}.", true)]
    #[case::chimes_chosen(CHIMES, "{T}: Choose a player. That player adds {C}.", true)]
    #[case::chimes_other_mana(CHIMES, "{T}: A player of your choice adds {G}.", false)]
    #[case::searchlight(
        SEARCHLIGHT,
        "{T}: Choose a player. That player adds one mana of any color they choose.",
        true
    )]
    #[case::searchlight_unchosen(
        SEARCHLIGHT,
        "{T}: Choose a player. That player adds one mana of any color.",
        false
    )]
    #[case::shaun(SHAUN, "{T}: Add {C}. When you do, mill two cards.", true)]
    #[case::shaun_other_count(SHAUN, "{T}: Add {C}. When you do, mill three cards.", false)]
    #[case::karfell(
        KARFELL,
        "{T}: Add {U}. Spend this mana only to foretell a card from your hand or cast an instant or sorcery spell.",
        true
    )]
    #[case::karfell_other_mana(
        KARFELL,
        "{T}: Add {R}. Spend this mana only to foretell a card from your hand or cast an instant or sorcery spell.",
        false
    )]
    #[case::gossip(
        GOSSIP,
        "{T}: Add {R}{G}. Spend this mana only to cast face-down spells or to turn creatures face up.",
        true
    )]
    #[case::gossip_other_rider(
        GOSSIP,
        "{T}: Add {R}{G}. Spend this mana only to cast face-down spells.",
        false
    )]
    #[case::marina(
        MARINA,
        "When ~ enters, reveal the top seven cards of your library. Put all enchantment cards from among them into your hand and the rest on the bottom of your library in a random order.",
        true
    )]
    #[case::marina_other_type(
        MARINA,
        "When ~ enters, reveal the top seven cards of your library. Put all artifact cards from among them into your hand and the rest on the bottom of your library in a random order.",
        false
    )]
    #[case::cryptex(
        CRYPTEX,
        "{T}, Collect evidence 3: Add one mana of any color. Put an unlock counter on ~.",
        true
    )]
    #[case::cryptex_other_total(
        CRYPTEX,
        "{T}, Collect evidence 4: Add one mana of any color. Put an unlock counter on ~.",
        false
    )]
    #[case::kraken(
        KRAKEN,
        "Return three snow lands you control to their owner's hand: Return ~ to its owner's hand.",
        true
    )]
    #[case::kraken_two(
        KRAKEN_TWO,
        "Return three snow lands you control to their owner's hand: Return ~ to its owner's hand.",
        false
    )]
    #[case::grasp(
        GRASP,
        "Waterbend {5}: Enchanted creature's owner shuffles it into their library.",
        true
    )]
    #[case::grasp_on_top(
        GRASP_TOP,
        "Waterbend {5}: Enchanted creature's owner shuffles it into their library.",
        false
    )]
    #[case::doctor(
        DOCTOR,
        "{7}: Time travel three times. Activate only as a sorcery.",
        true
    )]
    #[case::doctor_twice(
        DOCTOR_TWICE,
        "{7}: Time travel three times. Activate only as a sorcery.",
        false
    )]
    #[case::keys(
        KEYS,
        "{3}, {T}, Sacrifice ~: Lock or unlock a door of target Room you control. Activate only as a sorcery.",
        true
    )]
    #[case::keys_any_room(
        KEYS_ANY_ROOM,
        "{3}, {T}, Sacrifice ~: Lock or unlock a door of target Room you control. Activate only as a sorcery.",
        false
    )]
    #[case::flectomancer(
        FLECTO,
        "Sacrifice ~: You may change the targets of target instant or sorcery spell.",
        true
    )]
    #[case::flectomancer_other_spell(
        FLECTO,
        "Sacrifice ~: You may change the targets of target instant spell.",
        false
    )]
    fn reads_back_only_as_written(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
        let written = written(json);
        assert_eq!(
            written.iter().any(|text| text == clause),
            reads,
            "{written:?}"
        );
    }
}
