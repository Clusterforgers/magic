//! r47-02 wordings: a name chosen, then the top cards of the library revealed, the cards with that name put into the
//! hand and the rest into the graveyard ("Choose a nonland card name, then reveal the top four cards of your library.
//! Put all cards with the chosen name from among them into your hand and the rest into your graveyard."), written from
//! the twin with one plain step, whose cost or trigger another writer words.

use serde_json::{Value, json};

use super::super::super::amounts::count_word;
use crate::ability::Ability;

/// Adds the r47-02 wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    r4702_named_from_top(ability, texts);
    r4702_lose_land_types(ability, texts);
    r4702_emblem_of_chosen_type(ability, texts);
}

/// "Choose a creature type. You get an emblem with "Creatures you control of the chosen type get +3/+3 ..."": the type
/// chosen in the resolution is the one the emblem's abilities name, written from the twin whose emblem reads the chosen
/// type of its source.
fn r4702_emblem_of_chosen_type(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(effects) = value
        .as_object_mut()
        .and_then(|kinds| kinds.values_mut().next())
        .and_then(|body| body.get_mut("effects"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let [choose, emblem] = effects.as_slice() else {
        return;
    };
    let Some(name) = choose
        .get("ChooseValue")
        .filter(|choice| {
            choice.get("kind") == Some(&json!("CreatureType")) && choice.get("who").is_none()
        })
        .and_then(|choice| choice.get("as"))
        .and_then(Value::as_str)
    else {
        return;
    };
    if emblem.get("CreateEmblem").is_none() {
        return;
    }
    let chosen = json!({ "ChosenAs": name }).to_string();
    let emblem_text = emblem.to_string();
    if !emblem_text.contains(&chosen) {
        return;
    }
    let Ok(twin_emblem) =
        serde_json::from_str::<Value>(&emblem_text.replace(&chosen, "\"ChosenSubtype\""))
    else {
        return;
    };
    *effects = vec![twin_emblem];
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("You get an emblem").count() == 1)
        .map(|text| {
            text.replacen(
                "You get an emblem",
                "Choose a creature type. You get an emblem",
                1,
            )
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// "Lands your opponents control with the chosen name lose all land types and abilities, and they gain "{T}: Add one
/// mana of any color."": a group of lands loses its land types and abilities and gains one ability, written from the
/// twin that only gains it.
fn r4702_lose_land_types(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(mods) = value
        .get_mut("Static")
        .and_then(|body| body.get_mut("mods"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let [lose_types, lose_abilities, gain] = mods.as_slice() else {
        return;
    };
    if *lose_types != json!({ "SetLandTypes": [] })
        || *lose_abilities != json!("LoseAbilities")
        || gain.get("GrantAbility").is_none()
    {
        return;
    }
    let gain = gain.clone();
    *mods = vec![gain];
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches(" have \"").count() == 1)
        .map(|text| {
            text.replacen(
                " have \"",
                " lose all land types and abilities, and they gain \"",
                1,
            )
        })
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// The steps of a name chosen and the top cards sorted by it: the words after the cost, or `None` for other steps.
fn r4702_named_steps(effects: &Value) -> Option<String> {
    let [choose, look, reveal, kept, rest] = effects.as_array()?.as_slice() else {
        return None;
    };
    let choose = choose.get("ChooseValue")?;
    let name = choose.get("as")?.as_str()?;
    if choose.get("who").is_some() {
        return None;
    }
    let named = match choose.get("kind")?.get("CardName")? {
        filter if *filter == json!([]) => "a card name",
        filter if *filter == json!([{ "Not": { "Type": "Land" } }]) => "a nonland card name",
        _ => return None,
    };
    let look = look.get("LookAtTop")?;
    let top = look.get("as")?.as_str()?;
    let count = look
        .get("count")?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())?;
    (look.as_object()?.len() == 2).then_some(())?;
    (*reveal == json!({ "Reveal": { "what": { "Bound": top } } })).then_some(())?;
    let wanted = json!({ "Move": {
        "what": { "Each": [{ "Is": { "Bound": top } }, { "ChosenAs": name }] },
        "to": "Hand"
    } });
    (*kept == wanted).then_some(())?;
    (*rest == json!({ "Move": { "what": { "Bound": top }, "to": "Graveyard" } })).then_some(())?;
    let cards = if count == 1 {
        "the top card".to_owned()
    } else {
        format!("the top {} cards", count_word(count)?)
    };
    Some(format!(
        "Choose {named}, then reveal {cards} of your library. Put all cards with the chosen name from among them into your hand and the rest into your graveyard"
    ))
}

/// "+1: Choose a nonland card name, then reveal the top four cards of your library. Put all cards with the chosen name
/// from among them into your hand and the rest into your graveyard."
fn r4702_named_from_top(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut value) = serde_json::to_value(ability) else {
        return;
    };
    let Some(body) = value
        .as_object_mut()
        .and_then(|kinds| kinds.values_mut().next())
    else {
        return;
    };
    let Some(steps) = body.get("effects").and_then(r4702_named_steps) else {
        return;
    };
    body["effects"] = json!([{ "GainLife": { "amount": 7 } }]);
    let Ok(twin) = serde_json::from_value::<Ability>(value) else {
        return;
    };
    let found: Vec<String> = crate::compile::written_variants(&twin)
        .into_iter()
        .filter(|text| text.matches("You gain 7 life").count() == 1)
        .map(|text| text.replacen("You gain 7 life", &steps, 1))
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

#[cfg(test)]
mod tests {
    //! Goal: the steps of a name chosen and the top cards sorted by it read back with the cost of the twin, and a near
    //! miss (another zone for the rest, any name for "nonland", the cards kept without the name) does not.
    use super::*;
    use rstest::rstest;

    const TAMIYO: &str = r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"ChooseValue":{"kind":{"CardName":[{"Not":{"Type":"Land"}}]},"as":"name"}},{"LookAtTop":{"count":4,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Move":{"what":{"Each":[{"Is":{"Bound":"top"}},{"ChosenAs":"name"}]},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":"Graveyard"}}]}}"#;
    const GOLD: &str = "+1: Choose a nonland card name, then reveal the top four cards of your library. Put all cards with the chosen name from among them into your hand and the rest into your graveyard.";

    fn written(json: &str) -> Vec<String> {
        let ability: Ability = serde_json::from_str(json).unwrap();
        let mut texts = Vec::new();
        add(&ability, &mut texts);
        texts
    }

    const OKO: &str = r#"{"Loyalty":{"cost":{"Minus":6},"effects":[{"ChooseValue":{"kind":"CreatureType","as":"type"}},{"CreateEmblem":{"abilities":[{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"ChosenAs":"type"}]},"mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":"Vigilance"},{"AddKeyword":"Hexproof"}]}}]}}]}}"#;
    const OKO_GOLD: &str = "−6: Choose a creature type. You get an emblem with \"Creatures you control of the chosen type get +3/+3 and have vigilance and hexproof.\"";

    #[rstest]
    #[case::as_printed(OKO, true)]
    #[case::another_type_chosen(&OKO.replacen(r#""ChosenAs":"type""#, r#""ChosenAs":"other""#, 1), false)]
    #[case::a_color_chosen(&OKO.replacen(r#""CreatureType""#, r#""Color""#, 1), false)]
    #[case::bigger(&OKO.replacen(r#""power":3"#, r#""power":4"#, 1), false)]
    fn r4702_oko_emblem(#[case] json: &str, #[case] reads: bool) {
        let texts = written(json);
        assert_eq!(
            texts
                .iter()
                .any(|text| crate::compile::read::tokens::same_text(text, OKO_GOLD)),
            reads,
            "{texts:?}"
        );
    }

    #[test]
    fn r4702_tamiyo_reads_back() {
        let texts = written(TAMIYO);
        assert!(
            texts
                .iter()
                .any(|text| crate::compile::read::tokens::same_text(text, GOLD)),
            "{texts:?}"
        );
    }

    #[rstest]
    #[case::the_rest_to_hand(r#""to":"Graveyard""#, r#""to":"Hand""#)]
    #[case::any_name(r#"[{"Not":{"Type":"Land"}}]"#, "[]")]
    #[case::kept_without_the_name(r#",{"ChosenAs":"name"}"#, "")]
    #[case::three_cards(r#""count":4"#, r#""count":3"#)]
    fn r4702_a_near_miss_does_not_read_back(#[case] from: &str, #[case] to: &str) {
        let json = TAMIYO.replacen(from, to, 1);
        assert_ne!(json, TAMIYO);
        assert!(
            written(&json)
                .iter()
                .all(|text| !crate::compile::read::tokens::same_text(text, GOLD))
        );
    }
}
