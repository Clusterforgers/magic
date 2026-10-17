//! Parser for Magic Online `.dek` XML decks.

use roxmltree::{Document, Node, ParsingOptions};

use super::line::{check_count, check_name};
use super::types::{DeckEntry, LineError, ParsedDeck, Section};

/// Cap on XML nodes so hostile uploads cannot exhaust memory.
const MAX_XML_NODES: u32 = 100_000;

/// Real `.dek` files nest two levels; roxmltree overflows the stack on deep documents.
const MAX_XML_DEPTH: usize = 8;

/// Index just past the first `needle` at or after `from`, if any.
fn after(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| from + at + needle.len())
}

/// Index just past the `>` that ends the tag starting at `from`, skipping quoted attribute values.
fn tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, &byte) in bytes.get(from..)?.iter().enumerate() {
        match (quote, byte) {
            (Some(open), b) if b == open => quote = None,
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'>') => return Some(from + offset + 1),
            _ => {}
        }
    }
    None
}

/// Whether element nesting stays within `limit`; comments, CDATA and quoted values are skipped like a parser would.
fn nesting_within(input: &str, limit: usize) -> bool {
    let bytes = input.as_bytes();
    let (mut depth, mut pos) = (0usize, 0usize);
    while let Some(offset) = bytes
        .get(pos..)
        .and_then(|rest| rest.iter().position(|&b| b == b'<'))
    {
        let start = pos + offset;
        let rest = &bytes[start..];
        let next = if rest.starts_with(b"<!--") {
            after(bytes, start + 4, b"-->")
        } else if rest.starts_with(b"<![CDATA[") {
            after(bytes, start + 9, b"]]>")
        } else if rest.starts_with(b"<?") {
            after(bytes, start + 2, b"?>")
        } else if rest.starts_with(b"<!") {
            after(bytes, start + 2, b">")
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            after(bytes, start + 2, b">")
        } else {
            let end = tag_end(bytes, start + 1);
            let self_closing = end.is_some_and(|e| bytes[e - 2] == b'/');
            if !self_closing {
                depth += 1;
                if depth > limit {
                    return false;
                }
            }
            end
        };
        match next {
            Some(next) => pos = next,
            None => return true,
        }
    }
    true
}

fn line_of(doc: &Document<'_>, node: Node<'_, '_>) -> usize {
    doc.text_pos_at(node.range().start).row as usize
}

fn entry_from(node: Node<'_, '_>) -> Result<DeckEntry, String> {
    let quantity = node
        .attribute("Quantity")
        .ok_or("<Cards> element has no Quantity")?
        .trim();
    if quantity.is_empty() || !quantity.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Quantity {quantity:?} is not a whole number"));
    }
    let count = check_count(quantity).map_err(|e| e.to_string())?;
    let name = node
        .attribute("Name")
        .ok_or("<Cards> element has no Name")?;
    let name = check_name(name).map_err(|e| e.to_string())?;
    let section = match node.attribute("Sideboard").map(str::trim) {
        None => Section::Main,
        Some(flag) if flag.eq_ignore_ascii_case("false") => Section::Main,
        Some(flag) if flag.eq_ignore_ascii_case("true") => Section::Sideboard,
        Some(flag) => return Err(format!("Sideboard value {flag:?} is not true or false")),
    };
    Ok(DeckEntry::new(count, name, section))
}

/// Parses a `.dek` document; an unreadable document yields a single error and no entries.
pub fn parse_dek(input: &str) -> ParsedDeck {
    let text = input.trim_start_matches('\u{feff}');
    if !nesting_within(text, MAX_XML_DEPTH) {
        return ParsedDeck {
            entries: Vec::new(),
            errors: vec![LineError {
                line: 1,
                message: format!("XML is nested more than {MAX_XML_DEPTH} levels deep"),
            }],
            ..ParsedDeck::default()
        };
    }
    let options = ParsingOptions {
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    let doc = match Document::parse_with_options(text, options) {
        Ok(doc) => doc,
        Err(error) => {
            return ParsedDeck {
                entries: Vec::new(),
                errors: vec![LineError {
                    line: error.pos().row as usize,
                    message: format!("invalid XML: {error}"),
                }],
                ..ParsedDeck::default()
            };
        }
    };

    let root = doc.root_element();
    let mut deck = ParsedDeck::default();
    if !root.has_tag_name("Deck") {
        deck.errors.push(LineError {
            line: line_of(&doc, root),
            message: format!(
                "expected a <Deck> root element, found <{}>",
                root.tag_name().name()
            ),
        });
        return deck;
    }
    for node in root.children().filter(|n| n.has_tag_name("Cards")) {
        match entry_from(node) {
            Ok(entry) => deck.entries.push(entry),
            Err(message) => deck.errors.push(LineError {
                line: line_of(&doc, node),
                message,
            }),
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    //! Goal: MTGO .dek XML yields the same entries as the text formats, tolerating real-world quirks and failing softly on broken documents.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn wrap(body: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Deck xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n  <NetDeckID>0</NetDeckID>\n{body}\n</Deck>"
        )
    }

    fn summary(deck: &ParsedDeck) -> Vec<(u32, &str, Section)> {
        deck.entries
            .iter()
            .map(|e| (e.count, e.name.as_str(), e.section))
            .collect()
    }

    #[rstest]
    #[case::main(r#"<Cards CatID="1" Quantity="4" Sideboard="false" Name="Lightning Bolt" Annotation="0" />"#, (4, "Lightning Bolt", Section::Main))]
    #[case::sideboard(r#"<Cards CatID="1" Quantity="2" Sideboard="true" Name="Pyroblast" />"#, (2, "Pyroblast", Section::Sideboard))]
    #[case::capitalised_bool(r#"<Cards Quantity="2" Sideboard="True" Name="Pyroblast" />"#, (2, "Pyroblast", Section::Sideboard))]
    #[case::missing_sideboard_attr(r#"<Cards Quantity="1" Name="Island" />"#, (1, "Island", Section::Main))]
    #[case::entity_escapes(r#"<Cards Quantity="1" Name="Look at Me, I&apos;m R&amp;D" />"#, (1, "Look at Me, I'm R&D", Section::Main))]
    #[case::numeric_entity(r#"<Cards Quantity="1" Name="Lim-D&#251;l&apos;s Vault" />"#, (1, "Lim-Dûl's Vault", Section::Main))]
    #[case::unicode_raw(r#"<Cards Quantity="1" Name="Æther Vial" />"#, (1, "Æther Vial", Section::Main))]
    #[case::split_card(r#"<Cards Quantity="1" Name="Fire // Ice" />"#, (1, "Fire // Ice", Section::Main))]
    #[case::open_close_tags(r#"<Cards Quantity="3" Name="Island"></Cards>"#, (3, "Island", Section::Main))]
    #[case::unknown_attrs_ignored(r#"<Cards Quantity="3" Name="Island" Foo="bar" CatID="999" />"#, (3, "Island", Section::Main))]
    fn reads_card_elements(#[case] body: &str, #[case] expected: (u32, &str, Section)) {
        let deck = parse_dek(&wrap(body));
        assert!(deck.errors.is_empty(), "{:?}", deck.errors);
        assert_eq!(summary(&deck), [expected]);
    }

    #[test]
    fn keeps_document_order_across_sections() {
        let body = r#"<Cards Quantity="1" Sideboard="true" Name="A" /><Cards Quantity="1" Name="B" /><Cards Quantity="1" Sideboard="true" Name="C" />"#;
        let deck = parse_dek(&wrap(body));
        assert_eq!(
            summary(&deck),
            [
                (1, "A", Section::Sideboard),
                (1, "B", Section::Main),
                (1, "C", Section::Sideboard)
            ]
        );
    }

    #[rstest]
    #[case::no_quantity(r#"<Cards Name="Island" />"#, "Quantity")]
    #[case::no_name(r#"<Cards Quantity="1" />"#, "Name")]
    #[case::zero(r#"<Cards Quantity="0" Name="Island" />"#, "at least 1")]
    #[case::huge(r#"<Cards Quantity="100000" Name="Island" />"#, "too large")]
    #[case::overflow(r#"<Cards Quantity="99999999999999" Name="Island" />"#, "too large")]
    #[case::not_a_number(r#"<Cards Quantity="four" Name="Island" />"#, "whole number")]
    #[case::negative(r#"<Cards Quantity="-1" Name="Island" />"#, "whole number")]
    #[case::empty_quantity(r#"<Cards Quantity="" Name="Island" />"#, "whole number")]
    #[case::empty_name(r#"<Cards Quantity="1" Name="  " />"#, "missing card name")]
    #[case::bad_flag(
        r#"<Cards Quantity="1" Sideboard="maybe" Name="Island" />"#,
        "true or false"
    )]
    fn bad_card_elements_are_per_entry_errors(#[case] body: &str, #[case] fragment: &str) {
        let good = r#"<Cards Quantity="1" Name="Forest" />"#;
        let deck = parse_dek(&wrap(&format!("{body}\n{good}")));
        assert_eq!(summary(&deck), [(1, "Forest", Section::Main)]);
        assert_eq!(deck.errors.len(), 1, "{:?}", deck.errors);
        assert!(
            deck.errors[0].message.contains(fragment),
            "{:?}",
            deck.errors
        );
        assert_eq!(deck.errors[0].line, 4);
    }

    #[rstest]
    #[case::empty("")]
    #[case::not_xml("4 Lightning Bolt")]
    #[case::unclosed("<Deck><Cards Quantity=\"1\" Name=\"A\" />")]
    #[case::mismatched("<Deck><Cards Quantity=\"1\" Name=\"A\" /></Dek>")]
    #[case::unknown_entity(r#"<Deck><Cards Quantity="1" Name="&bogus;" /></Deck>"#)]
    #[case::dtd(r#"<!DOCTYPE d [<!ENTITY a "b">]><Deck><Cards Quantity="1" Name="&a;" /></Deck>"#)]
    #[case::wrong_root(r#"<Deckk><Cards Quantity="1" Name="A" /></Deckk>"#)]
    #[case::truncated_attr("<Deck><Cards Quantity=\"1")]
    fn broken_documents_give_one_error_and_no_entries(#[case] input: &str) {
        let deck = parse_dek(input);
        assert_eq!(deck.entries, Vec::new());
        assert_eq!(deck.errors.len(), 1, "{:?}", deck.errors);
    }

    #[rstest]
    #[case::flat("<Deck><Cards Quantity=\"1\" Name=\"A\"/></Deck>", true)]
    #[case::exactly_at_limit(&format!("{}{}", "<a>".repeat(MAX_XML_DEPTH), "</a>".repeat(MAX_XML_DEPTH)), true)]
    #[case::one_over(&format!("{}{}", "<a>".repeat(MAX_XML_DEPTH + 1), "</a>".repeat(MAX_XML_DEPTH + 1)), false)]
    #[case::siblings_do_not_accumulate(&"<a></a>".repeat(10_000), true)]
    #[case::self_closing_siblings(&"<a/>".repeat(10_000), true)]
    #[case::self_closing_with_attrs(&"<a b=\"c\" />".repeat(10_000), true)]
    #[case::unclosed_bomb(&"<a>".repeat(10_000), false)]
    #[case::comment_cannot_hide_depth(&"<a><!-- </a></a></a></a></a></a></a></a></a></a> -->".repeat(20), false)]
    #[case::cdata_cannot_hide_depth(&"<a><![CDATA[</a></a></a></a></a></a></a></a></a></a>]]>".repeat(20), false)]
    #[case::pi_cannot_hide_depth(&"<a><?x </a></a></a></a></a></a></a></a></a></a> ?>".repeat(20), false)]
    #[case::attribute_gt_cannot_close_tag(
        "<a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\"><a b=\">\">",
        false
    )]
    #[case::single_quoted_attribute(
        "<a b='/'><a b='/'><a b='/'><a b='/'><a b='/'><a b='/'><a b='/'><a b='/'><a b='/'>",
        false
    )]
    #[case::comment_with_tags_inside_is_ignored(
        "<a><!-- <b><b><b><b><b><b><b><b><b><b><b> --></a>",
        true
    )]
    #[case::unterminated_comment("<a><!-- <b><b><b><b><b><b><b><b><b><b><b>", true)]
    #[case::unterminated_tag("<a><b c=\"<d><d><d><d><d><d><d><d><d><d>", true)]
    #[case::stray_closers_do_not_go_negative("</a></a></a><a>", true)]
    #[case::no_markup("just text", true)]
    #[case::empty("", true)]
    fn nesting_guard(#[case] xml: &str, #[case] within: bool) {
        assert_eq!(nesting_within(xml, MAX_XML_DEPTH), within);
    }

    #[test]
    fn deeply_nested_documents_are_refused_instead_of_overflowing_the_stack() {
        for input in [
            "<Deck>".repeat(100_000),
            format!("{}{}", "<a>".repeat(50_000), "</a>".repeat(50_000)),
        ] {
            let deck = parse_dek(&input);
            assert_eq!(deck.entries, Vec::new());
            assert_eq!(deck.errors.len(), 1);
            assert!(
                deck.errors[0].message.contains("nested"),
                "{:?}",
                deck.errors
            );
        }
    }

    #[test]
    fn leading_bom_is_ignored() {
        let input = format!(
            "\u{feff}{}",
            wrap(r#"<Cards Quantity="1" Name="Island" />"#)
        );
        assert_eq!(parse_dek(&input).entries.len(), 1);
    }

    #[test]
    fn billion_laughs_is_refused() {
        let input = r#"<!DOCTYPE lolz [<!ENTITY lol "lol"><!ENTITY lol2 "&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;&lol;">]><Deck><Cards Quantity="1" Name="&lol2;" /></Deck>"#;
        let deck = parse_dek(input);
        assert_eq!(deck.entries, Vec::new());
        assert_eq!(deck.errors.len(), 1);
    }

    /// A random element tree rendered as XML, with its true depth.
    fn xml_tree() -> impl Strategy<Value = (String, usize)> {
        let leaf = prop_oneof![
            Just(("<a/>".to_owned(), 0usize)),
            Just(("<a x=\"/\"></a>".to_owned(), 1)),
            Just(("<!-- <b><b><b> --><a/>".to_owned(), 0)),
            Just(("<a><![CDATA[</a></a>]]></a>".to_owned(), 1)),
        ];
        leaf.prop_recursive(14, 64, 3, |inner| {
            prop::collection::vec(inner, 0..3).prop_map(|children| {
                let depth = 1 + children.iter().map(|c| c.1).max().unwrap_or(0);
                let body: String = children.into_iter().map(|c| c.0).collect();
                (format!("<n k='>'>{body}</n>"), depth)
            })
        })
    }

    proptest! {
        #[test]
        fn nesting_guard_agrees_with_the_true_depth((xml, depth) in xml_tree(), limit in 1usize..16) {
            prop_assert_eq!(nesting_within(&xml, limit), depth <= limit);
        }

        #[test]
        fn arbitrary_input_never_panics(input in "\\PC{0,200}") {
            let _ = parse_dek(&input);
        }

        #[test]
        fn xml_escaped_names_roundtrip(name in "[A-Za-z][A-Za-z0-9 ,'&<>\"!?:.\u{e9}\u{fb}\u{c6}-]{0,30}[A-Za-z]", count in 1u32..=999) {
            let escaped = name
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");
            let deck = parse_dek(&wrap(&format!(r#"<Cards Quantity="{count}" Name="{escaped}" />"#)));
            prop_assert_eq!(deck.errors, Vec::new());
            prop_assert_eq!(deck.entries, vec![DeckEntry::new(count, name, Section::Main)]);
        }
    }
}
