//! Text-changing effects (CR 612): "change the text of target spell or permanent by replacing all instances of one color
//! word with another" (Sleight of Mind, Mind Bend, Magical Hack, Artificial Evolution).
//!
//! A text change is a modification of layer 3 (`Modification::ChangeText`): the layers rewrite the abilities and the
//! keywords an object has and the subtypes of its type line before any later layer reads them, so a static ability, a
//! trigger, a protection or a landwalk reads the new word. A word is a color word, a basic land type or a creature type,
//! changed only where the rules use it as one: a value of the rules that names that color or subtype. Mana symbols
//! are not words (CR 612.2): what lies under a mana or cost entry of the rules stays as it is, and so does a card name.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::Subtype;
use mtg_core::color::Color;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One word a text change replaces or writes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TextWord {
    /// A color word ("black").
    Color(Color),
    /// A basic land type or a creature type ("Swamp", "Goblin").
    Subtype(Subtype),
}

impl TextWord {
    /// The value the rules hold for the word.
    fn value(&self) -> Value {
        match self {
            TextWord::Color(color) => serde_json::to_value(color).unwrap_or(Value::Null),
            TextWord::Subtype(subtype) => Value::String(subtype.0.clone()),
        }
    }

    /// Whether both words are of one kind: two colors, two subtypes (CR 612.2 replaces a word with one of its kind).
    pub fn same_kind(&self, other: &TextWord) -> bool {
        matches!(
            (self, other),
            (TextWord::Color(_), TextWord::Color(_)) | (TextWord::Subtype(_), TextWord::Subtype(_))
        )
    }
}

/// The keys of the rules whose values are no words: mana, costs and names.
fn is_no_word(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("mana") || lower.contains("cost") || lower.contains("name") || lower == "produce"
}

/// Replaces every value equal to `from` with `to` in the tree, outside the entries that hold no words.
fn rewrite(value: &mut Value, from: &Value, to: &Value) -> bool {
    match value {
        Value::String(_) if value == from => {
            *value = to.clone();
            true
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, item| rewrite(item, from, to) || changed),
        Value::Object(entries) => entries
            .iter_mut()
            .filter(|(key, _)| !is_no_word(key))
            .fold(false, |changed, (_, item)| {
                rewrite(item, from, to) || changed
            }),
        _ => false,
    }
}

/// `item` with every instance of `from` replaced by `to`, or `None` when it has no such instance (or does not read back).
fn changed<T: Serialize + for<'de> Deserialize<'de>>(
    item: &T,
    from: &TextWord,
    to: &TextWord,
) -> Option<T> {
    if !from.same_kind(to) || from == to {
        return None;
    }
    let mut tree = serde_json::to_value(item).ok()?;
    if !rewrite(&mut tree, &from.value(), &to.value()) {
        return None;
    }
    serde_json::from_value(tree).ok()
}

/// The ability with `from` replaced by `to`; the same `Arc` when the word does not occur in it.
pub(crate) fn ability(body: &Arc<Ability>, from: &TextWord, to: &TextWord) -> Arc<Ability> {
    changed(&**body, from, to).map_or_else(|| Arc::clone(body), Arc::new)
}

/// The keyword with `from` replaced by `to` ("swampwalk" to "islandwalk", "protection from black" to "from green").
pub(crate) fn keyword(keyword: &Keyword, from: &TextWord, to: &TextWord) -> Keyword {
    changed(keyword, from, to).unwrap_or_else(|| keyword.clone())
}

/// The subtypes of a type line with `from` replaced by `to`, each once.
pub(crate) fn subtypes(subtypes: &mut Vec<Subtype>, from: &TextWord, to: &TextWord) {
    let (TextWord::Subtype(from), TextWord::Subtype(to)) = (from, to) else {
        return;
    };
    if from == to || !subtypes.contains(from) {
        return;
    }
    subtypes.retain(|held| held != from && held != to);
    subtypes.push(to.clone());
}

#[cfg(test)]
mod tests {
    //! Goal: a text change replaces a color word, a basic land type or a creature type wherever the rules use it as one (a selector, a protection, a landwalk, a type line) and nowhere else (a mana symbol, a cost, a name), only with a word of its own kind, and leaves an ability without the word as the very same rules.
    use rstest::rstest;

    use super::*;

    fn rules(json: &str) -> Arc<Ability> {
        Arc::new(serde_json::from_str(json).unwrap())
    }

    const BLACK: TextWord = TextWord::Color(Color::Black);
    const GREEN: TextWord = TextWord::Color(Color::Green);

    fn land(name: &str) -> TextWord {
        TextWord::Subtype(Subtype(name.into()))
    }

    #[rstest]
    #[case::a_selector_color(
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Color":"Black"}}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        BLACK, GREEN,
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Color":"Green"}}]},"mods":[{"AddKeyword":"Flying"}]}}"#
    )]
    #[case::a_land_type_selector(
        r#"{"Static":{"affected":{"Each":[{"Subtype":"Swamp"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        land("Swamp"), land("Island"),
        r#"{"Static":{"affected":{"Each":[{"Subtype":"Island"}]},"mods":[{"AddKeyword":"Flying"}]}}"#
    )]
    fn the_word_is_replaced_where_the_rules_use_it(
        #[case] before: &str,
        #[case] from: TextWord,
        #[case] to: TextWord,
        #[case] after: &str,
    ) {
        assert_eq!(ability(&rules(before), &from, &to), rules(after));
    }

    #[rstest]
    #[case::mana_is_no_word(r#"{"Activated":{"cost":[{"Mana":"{B}"}, "Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Black"]}}}]}}"#, BLACK, GREEN)]
    #[case::another_kind(
        r#"{"Static":{"affected":{"Each":[{"Color":"Black"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        BLACK,
        land("Island")
    )]
    #[case::the_same_word(
        r#"{"Static":{"affected":{"Each":[{"Color":"Black"}]},"mods":[{"AddKeyword":"Flying"}]}}"#,
        BLACK,
        BLACK
    )]
    #[case::no_instance(r#"{"Keyword":"Flying"}"#, BLACK, GREEN)]
    fn nothing_else_changes(#[case] before: &str, #[case] from: TextWord, #[case] to: TextWord) {
        let body = rules(before);
        assert!(Arc::ptr_eq(&ability(&body, &from, &to), &body));
    }

    #[rstest]
    #[case::landwalk(
        r#"{"Landwalk":[{"Subtype":"Swamp"}]}"#,
        land("Swamp"),
        land("Island"),
        r#"{"Landwalk":[{"Subtype":"Island"}]}"#
    )]
    #[case::protection(
        r#"{"Protection":[{"Color":"Black"}]}"#,
        BLACK,
        GREEN,
        r#"{"Protection":[{"Color":"Green"}]}"#
    )]
    fn a_keyword_reads_the_new_word(
        #[case] before: &str,
        #[case] from: TextWord,
        #[case] to: TextWord,
        #[case] after: &str,
    ) {
        let before: Keyword = serde_json::from_str(before).unwrap();
        let after: Keyword = serde_json::from_str(after).unwrap();
        assert_eq!(keyword(&before, &from, &to), after);
    }

    #[test]
    fn a_type_line_changes_its_subtype_once() {
        let mut held = vec![Subtype("Goblin".into()), Subtype("Elf".into())];
        subtypes(&mut held, &land("Goblin"), &land("Elf"));
        assert_eq!(held, vec![Subtype("Elf".into())]);
        subtypes(&mut held, &BLACK, &GREEN);
        assert_eq!(held, vec![Subtype("Elf".into())]);
    }
}
