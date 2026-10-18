//! The canonical layout of rules files: JSON with short values kept on one line.
//!
//! `serde_json`'s pretty printer puts every array element on its own line, which makes a card of
//! twenty lines a hundred. This keeps the key order serde wrote and puts a value on one line when it
//! fits in [`WIDTH`] columns; longer ones are split one entry per line.

/// The widest a line may get before a value is split.
pub const WIDTH: usize = 100;

enum Node {
    Scalar(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn string(&mut self) -> Option<String> {
        let start = self.at;
        self.at += 1;
        while let Some(byte) = self.peek() {
            self.at += 1;
            match byte {
                b'\\' => self.at += 1,
                b'"' => return String::from_utf8(self.text[start..self.at].to_vec()).ok(),
                _ => {}
            }
        }
        None
    }

    fn node(&mut self) -> Option<Node> {
        match self.peek()? {
            b'"' => self.string().map(Node::Scalar),
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                while self.peek()? != b']' {
                    items.push(self.node()?);
                    if self.peek()? == b',' {
                        self.at += 1;
                    }
                }
                self.at += 1;
                Some(Node::Array(items))
            }
            b'{' => {
                self.at += 1;
                let mut entries = Vec::new();
                while self.peek()? != b'}' {
                    let key = self.string()?;
                    self.at += 1;
                    entries.push((key, self.node()?));
                    if self.peek()? == b',' {
                        self.at += 1;
                    }
                }
                self.at += 1;
                Some(Node::Object(entries))
            }
            _ => {
                let start = self.at;
                while self
                    .peek()
                    .is_some_and(|b| !matches!(b, b',' | b']' | b'}'))
                {
                    self.at += 1;
                }
                String::from_utf8(self.text[start..self.at].to_vec())
                    .ok()
                    .map(Node::Scalar)
            }
        }
    }
}

fn flat(node: &Node) -> String {
    match node {
        Node::Scalar(raw) => raw.clone(),
        Node::Array(items) if items.is_empty() => "[]".into(),
        Node::Array(items) => {
            format!(
                "[{}]",
                items.iter().map(flat).collect::<Vec<_>>().join(", ")
            )
        }
        Node::Object(entries) if entries.is_empty() => "{}".into(),
        Node::Object(entries) => format!(
            "{{ {} }}",
            entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", flat(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render(node: &Node, indent: usize, used: usize, out: &mut String) {
    let line = flat(node);
    if used + line.chars().count() <= WIDTH || matches!(node, Node::Scalar(_)) {
        out.push_str(&line);
        return;
    }
    let inner = indent + 2;
    let pad = " ".repeat(inner);
    match node {
        Node::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad);
                render(item, inner, inner, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&" ".repeat(indent));
            out.push(']');
        }
        Node::Object(entries) => {
            out.push_str("{\n");
            for (i, (key, value)) in entries.iter().enumerate() {
                out.push_str(&pad);
                out.push_str(key);
                out.push_str(": ");
                render(value, inner, inner + key.chars().count() + 2, out);
                out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
            }
            out.push_str(&" ".repeat(indent));
            out.push('}');
        }
        Node::Scalar(_) => {}
    }
}

/// Lays out compact JSON text (as `serde_json::to_string` writes it) in the canonical form;
/// text that is not valid JSON is returned unchanged.
pub fn pretty(compact: &str) -> String {
    let mut parser = Parser {
        text: compact.as_bytes(),
        at: 0,
    };
    match parser.node() {
        Some(node) if parser.at == compact.len() => {
            let mut out = String::new();
            render(&node, 0, 0, &mut out);
            out.push('\n');
            out
        }
        _ => compact.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the canonical layout keeps short values on a line, splits long ones one entry per line, keeps key order, and never changes the data.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::scalar("1", "1\n")]
    #[case::empty(r#"{"a":[],"b":{}}"#, "{ \"a\": [], \"b\": {} }\n")]
    #[case::short_object(r#"{"Keyword":"Flying"}"#, "{ \"Keyword\": \"Flying\" }\n")]
    #[case::short_array(r#"["This",{"Type":"Land"}]"#, "[\"This\", { \"Type\": \"Land\" }]\n")]
    #[case::text_with_punctuation(r#"{"t":"a, b: {c}"}"#, "{ \"t\": \"a, b: {c}\" }\n")]
    #[case::escapes(r#"["a\"]b","\\"]"#, "[\"a\\\"]b\", \"\\\\\"]\n")]
    fn short_values_stay_on_one_line(#[case] compact: &str, #[case] expected: &str) {
        assert_eq!(pretty(compact), expected);
    }

    #[test]
    fn long_values_split_one_entry_per_line_in_key_order() {
        let long = "x".repeat(40);
        let compact =
            format!(r#"{{"zeta":"{long}","alpha":["{long}","{long}","{long}"],"mid":1}}"#);
        let text = pretty(&compact);
        let expected = format!(
            "{{\n  \"zeta\": \"{long}\",\n  \"alpha\": [\n    \"{long}\",\n    \"{long}\",\n    \"{long}\"\n  ],\n  \"mid\": 1\n}}\n"
        );
        assert_eq!(text, expected);
        assert!(text.lines().all(|l| l.chars().count() <= WIDTH));
    }

    #[test]
    fn layout_never_changes_the_data_and_is_a_fixed_point() {
        let long = "y".repeat(45);
        let compact = format!(
            r#"{{"a":[{{"k":"{long}"}},{{"k":"{long}"}},[1,2,3]],"é":"ü—−","n":-1.5e3,"t":true,"z":null}}"#
        );
        let text = pretty(&compact);
        let before: serde_json::Value = serde_json::from_str(&compact).unwrap();
        let after: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(before, after);
        assert_eq!(pretty(&compact), text);
    }

    #[test]
    fn text_that_is_not_json_is_left_alone() {
        assert_eq!(pretty("{\"a\":"), "{\"a\":");
        assert_eq!(pretty("[1] x"), "[1] x");
    }
}
