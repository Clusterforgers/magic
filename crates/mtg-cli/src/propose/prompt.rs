//! The prompts: the instructions with the DSL reference (the same for every card), one card with its nearest examples,
//! and the repair message that carries the checker's diagnosis.

use std::fmt::Write as _;

use mtg_cards::PrintedCard;
use mtg_cards::proposal::{ClauseState, Report, Verdict};

use super::examples::Example;
use super::model::{Message, Request, Role};

/// The marker of the first line of a card's prompt; the scripted client finds the card by it.
pub const CARD_MARKER: &str = "Card: ";

/// The most problems or clauses a repair message shows.
const DIAGNOSIS_LIMIT: usize = 3;

/// The most tokens an answer may have: a card with many abilities is a few thousand.
pub const MAX_ANSWER_TOKENS: u32 = 8192;

const INSTRUCTIONS: &str = "\
You write the rules of one Magic: The Gathering card as JSON for a rules engine, from the card's oracle text.

How your answer is used:
- Reply with one JSON object and nothing else: {\"faces\":[{\"abilities\":[...]}]} with one entry of \"faces\" for each face of the card, in the printed order, and the abilities of a face in the order of its text. Do not write oracle_id, names, impl_status or fingerprints; they come from the card.
- A checker reads each ability back as a sentence and compares it with the card's text. It accepts rules only when they are valid in the reference below and every ability reads back as a sentence of the card. Follow the shapes of the examples.
- The oracle text is the only ground truth. Write one ability for each sentence the reference can say, a keyword line as {\"Keyword\": ...} abilities. When the reference has no construct for a sentence, write no ability for it: the table plays that sentence by hand. Never invent a variant or a field, and never change the card's meaning to fit.
- Reminder text in parentheses is not a sentence of its own.
";

/// The system prompt: the instructions, then the reference.
pub fn system_prompt(reference: &str) -> String {
    format!("{INSTRUCTIONS}\n{reference}")
}

/// The part of the system prompt before the reference, for a dry run that does not print the reference.
pub fn instructions() -> &'static str {
    INSTRUCTIONS
}

/// The heading of the Forge hint section: what the lines are and how they may be used.
pub const FORGE_HINT_HEADING: &str = "Forge script of this card (another engine's reading of the same card). Use it to find the right effects, costs, triggers and conditions for the oracle text above; it is not a format to copy, it can differ from the oracle text, and the oracle text stays the only ground truth. Your answer is written in the DSL of the reference only:";

/// The first message about a card: its text, then the nearest solved examples, then the request.
pub fn card_prompt(card: &PrintedCard, examples: &[&Example]) -> String {
    hinted_card_prompt(card, examples, None)
}

/// [`card_prompt`] with the Forge hint (the script lines of the card) as a section of its own after the card's text and
/// before the examples.
pub fn hinted_card_prompt(card: &PrintedCard, examples: &[&Example], hint: Option<&str>) -> String {
    let mut text = format!("{CARD_MARKER}{}\nLayout: {}\n", card.name, card.layout);
    if !card.keywords.is_empty() {
        let _ = writeln!(text, "Keywords (Scryfall): {}", card.keywords.join(", "));
    }
    let many = card.faces.len() > 1;
    for (at, face) in card.faces.iter().enumerate() {
        if many {
            let _ = writeln!(
                text,
                "\nFace {} of {}: {}",
                at + 1,
                card.faces.len(),
                face.name
            );
        } else {
            text.push('\n');
        }
        if !face.mana_cost.is_empty() {
            let _ = writeln!(text, "Mana cost: {}", face.mana_cost);
        }
        let _ = writeln!(text, "Type line: {}", face.type_line);
        for (label, value) in [
            (
                "Power/toughness",
                stats(face.power.as_deref(), face.toughness.as_deref()),
            ),
            ("Loyalty", face.loyalty.clone()),
            ("Defense", face.defense.clone()),
        ] {
            if let Some(value) = value {
                let _ = writeln!(text, "{label}: {value}");
            }
        }
        let _ = writeln!(text, "Oracle text:\n{}", face.oracle_text);
    }
    if let Some(hint) = hint {
        let _ = write!(text, "\n{FORGE_HINT_HEADING}\n{hint}\n");
    }
    if !examples.is_empty() {
        text.push_str("\nSolved examples with close wording (oracle text, then the rules that are accepted for it):\n");
        for example in examples {
            let _ = write!(
                text,
                "\nExample: {} ({})\nOracle text:\n{}\nRules: {}\n",
                example.name, example.type_line, example.oracle_text, example.rules
            );
        }
    }
    let _ = write!(
        text,
        "\nWrite the rules of {} now. Reply with the JSON object only.",
        card.name
    );
    text
}

fn stats(power: Option<&str>, toughness: Option<&str>) -> Option<String> {
    Some(format!("{}/{}", power?, toughness?))
}

/// The card a request is about, found by the marker line of its first message.
pub fn card_of(request: &Request) -> Option<&str> {
    request
        .messages
        .iter()
        .find(|message| message.role == Role::User)?
        .text
        .lines()
        .next()?
        .strip_prefix(CARD_MARKER)
}

/// The request for a card: the system prompt and the card's message.
pub fn first_request(model: &str, system: &str, card_message: String) -> Request {
    Request {
        model: model.to_owned(),
        system: system.to_owned(),
        messages: vec![Message::user(card_message)],
        max_tokens: MAX_ANSWER_TOKENS,
    }
}

/// The request for the repair round: the first request, the model's answer and what the checker said.
pub fn repair_request(first: &Request, answer: &str, report: &Report) -> Request {
    let mut request = first.clone();
    request.messages.push(Message::assistant(answer));
    request.messages.push(Message::user(repair_message(report)));
    request
}

/// What the checker found wrong, for the model: the verdict and the first problems or the first clause that differs.
pub fn repair_message(report: &Report) -> String {
    format!(
        "The checker rejected these rules: {}.\n{}\nFix the rules so that every ability is valid and reads back as its sentence of the card's text, \
         and reply with the complete corrected JSON object only.",
        report.verdict().word(),
        diagnosis(report)
    )
}

/// The first problems of an invalid proposal, or the first clause that differs (or ability without a clause) of a mismatch.
pub fn diagnosis(report: &Report) -> String {
    let mut lines = Vec::new();
    match report.verdict() {
        Verdict::Invalid => {
            for problem in report.problems.iter().take(DIAGNOSIS_LIMIT) {
                lines.push(if problem.path.is_empty() {
                    format!("- {}", problem.message)
                } else {
                    format!("- at {}: {}", problem.path, problem.message)
                });
            }
            let more = report.problems.len().saturating_sub(DIAGNOSIS_LIMIT);
            if more > 0 {
                lines.push(format!("- and {more} more problems"));
            }
        }
        Verdict::Mismatch => {
            let differing = report
                .clauses
                .iter()
                .filter_map(|clause| match &clause.state {
                    ClauseState::Differs { ability, written } => Some((clause, ability, written)),
                    _ => None,
                });
            for (clause, ability, written) in differing.take(1) {
                lines.push(format!(
                    "- ability {ability} reads back as \"{written}\" but the card says \"{}\"",
                    clause.text.replace('\n', " / ")
                ));
            }
            for stray in report.strays.iter().take(1) {
                lines.push(format!(
                    "- ability {} ({}) stands for no sentence of the card's text; remove it or put the sentence it stands for in the abilities",
                    stray.ability, stray.construct
                ));
            }
        }
        Verdict::Unchecked | Verdict::Accepted => {}
    }
    lines.join("\n")
}

/// The JSON object of an answer: the text between the first `{` and the last `}`, so a code fence or a sentence around it
/// does not matter.
pub fn extract_json(answer: &str) -> &str {
    match (answer.find('{'), answer.rfind('}')) {
        (Some(start), Some(end)) if start < end => &answer[start..=end],
        _ => answer.trim(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a card's prompt names the card, its faces, the examples and the request in a fixed layout, the JSON is found
    //! inside fences, and the repair message carries the verdict and the first thing the checker found wrong.
    use mtg_cards::PrintedFace;
    use rstest::rstest;

    use super::*;

    fn dragon() -> PrintedCard {
        PrintedCard {
            oracle_id: "id".into(),
            name: "Shivan Dragon".into(),
            keywords: vec!["Flying".into()],
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: "Shivan Dragon".into(),
                mana_cost: "{4}{R}{R}".into(),
                type_line: "Creature — Dragon".into(),
                oracle_text: "Flying\n{R}: This creature gets +1/+0 until end of turn.".into(),
                power: Some("5".into()),
                toughness: Some("5".into()),
                ..PrintedFace::default()
            }],
        }
    }

    #[test]
    fn a_card_prompt_has_the_card_the_examples_and_the_request() {
        let example = Example::plain("Wind Drake", "Creature — Drake", "Flying", "{\"faces\":[]}");
        let text = card_prompt(&dragon(), &[&example]);
        assert!(
            text.starts_with("Card: Shivan Dragon\nLayout: normal\nKeywords (Scryfall): Flying\n")
        );
        for part in [
            "Mana cost: {4}{R}{R}",
            "Type line: Creature — Dragon",
            "Power/toughness: 5/5",
            "Oracle text:\nFlying\n{R}: This creature gets +1/+0 until end of turn.",
            "Example: Wind Drake (Creature — Drake)",
            "Rules: {\"faces\":[]}",
        ] {
            assert!(text.contains(part), "missing {part:?} in\n{text}");
        }
        assert!(
            text.ends_with(
                "Write the rules of Shivan Dragon now. Reply with the JSON object only."
            )
        );
    }

    #[test]
    fn the_forge_hint_is_a_section_of_its_own_between_the_card_and_the_examples_and_only_when_given()
     {
        let example = Example::plain("Wind Drake", "Creature — Drake", "Flying", "{\"faces\":[]}");
        let plain = card_prompt(&dragon(), &[&example]);
        assert!(!plain.contains("Forge"), "{plain}");
        assert_eq!(hinted_card_prompt(&dragon(), &[&example], None), plain);

        let hint = "K:Flying\nA:AB$ Pump | Cost$ R";
        let text = hinted_card_prompt(&dragon(), &[&example], Some(hint));
        let at = |part: &str| {
            text.find(part)
                .unwrap_or_else(|| panic!("missing {part:?} in\n{text}"))
        };
        let order = [
            at("Card: Shivan Dragon"),
            at("Oracle text:\nFlying"),
            at("Forge script of this card"),
            at(hint),
            at("Solved examples with close wording"),
            at("Write the rules of Shivan Dragon now."),
        ];
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "{order:?}\n{text}"
        );
        for stated in [
            "another engine's reading of the same card",
            "not a format to copy",
            "the only ground truth",
            "DSL of the reference only",
        ] {
            assert!(FORGE_HINT_HEADING.contains(stated), "{stated}");
        }
        assert!(text.ends_with("Reply with the JSON object only."));
        let alone = hinted_card_prompt(&dragon(), &[], Some(hint));
        assert!(
            alone.contains(hint) && !alone.contains("Solved examples"),
            "{alone}"
        );
    }

    #[test]
    fn each_face_of_a_card_has_its_own_block_and_loyalty_is_shown() {
        let mut card = dragon();
        card.name = "Front // Back".into();
        card.faces.push(PrintedFace {
            name: "Back".into(),
            type_line: "Legendary Planeswalker — Chandra".into(),
            oracle_text: "+1: Add {R}{R}.".into(),
            loyalty: Some("4".into()),
            ..PrintedFace::default()
        });
        let text = card_prompt(&card, &[]);
        assert!(
            text.contains("\nFace 1 of 2: Shivan Dragon\nMana cost: {4}{R}{R}\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "\nFace 2 of 2: Back\nType line: Legendary Planeswalker — Chandra\nLoyalty: 4\n"
            ),
            "{text}"
        );
        assert!(!text.contains("examples with close wording"));
    }

    #[test]
    fn the_card_of_a_request_is_found_by_its_marker() {
        let request = first_request("m", "s", card_prompt(&dragon(), &[]));
        assert_eq!(card_of(&request), Some("Shivan Dragon"));
        assert_eq!(card_of(&first_request("m", "s", "no marker".into())), None);
    }

    #[rstest]
    #[case::bare("{\"a\":1}", "{\"a\":1}")]
    #[case::fenced("```json\n{\"a\":{\"b\":2}}\n```", "{\"a\":{\"b\":2}}")]
    #[case::sentence("Here you go: {\"a\":1} Hope it helps.", "{\"a\":1}")]
    #[case::none("no json", "no json")]
    fn the_json_object_is_found_inside_the_answer(#[case] answer: &str, #[case] expected: &str) {
        assert_eq!(extract_json(answer), expected);
    }
}
