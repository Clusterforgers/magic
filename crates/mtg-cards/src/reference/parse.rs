//! The type definitions of a source file as data: the public structs and enums with their serde names, defaults and doc lines.

use super::lex::{Tok, lex};

/// A public struct or enum.
#[derive(Debug, Clone)]
pub(super) struct TypeDef {
    pub name: String,
    pub doc: String,
    pub body: Body,
}

/// What a type is made of, as its JSON shows it.
#[derive(Debug, Clone)]
pub(super) enum Body {
    /// An enum: a variant is a string when it has no payload, else an object with one key.
    Enum(Vec<Variant>),
    /// An object with named fields.
    Struct(Vec<Field>),
    /// A newtype or tuple struct: the inner type, or an array of the types.
    Tuple(Vec<String>),
    /// A unit struct.
    Unit,
    /// A type written as a string (`try_from = "String"`).
    Text,
}

/// A variant of an enum.
#[derive(Debug, Clone)]
pub(super) struct Variant {
    pub name: String,
    pub doc: String,
    pub shape: Shape,
}

/// The payload of a variant.
#[derive(Debug, Clone)]
pub(super) enum Shape {
    Unit,
    Tuple(Vec<String>),
    Fields(Vec<Field>),
}

/// A named field.
#[derive(Debug, Clone)]
pub(super) struct Field {
    pub name: String,
    pub ty: String,
    /// The field may be left out.
    pub optional: bool,
    /// The value a left-out field has when it is not the type's own default.
    pub default: Option<String>,
}

/// The public structs and enums of a source file, in order.
pub(super) fn type_defs(source: &str) -> Vec<TypeDef> {
    let mut parser = Parser {
        toks: lex(source),
        at: 0,
    };
    let mut found = Vec::new();
    while parser.at < parser.toks.len() {
        let attrs = parser.attrs();
        let public = parser.visibility();
        match parser.peek_ident() {
            Some("struct" | "enum") => {
                if let Some(def) = parser.type_def(&attrs)
                    && public
                    && !attrs.test
                {
                    found.push(def);
                }
            }
            _ => parser.skip_item(),
        }
    }
    found
}

/// The attributes and doc lines before an item.
#[derive(Debug, Default)]
struct Attrs {
    docs: Vec<String>,
    serde: Vec<(String, Option<String>)>,
    test: bool,
}

impl Attrs {
    fn flag(&self, key: &str) -> bool {
        self.serde.iter().any(|(name, _)| name == key)
    }

    fn value(&self, key: &str) -> Option<&str> {
        self.serde
            .iter()
            .find(|(name, _)| name == key)
            .and_then(|(_, value)| value.as_deref())
    }

    /// The doc comment up to the end of its first sentence.
    fn doc(&self) -> String {
        first_sentence(&self.docs.join(" "))
    }
}

/// The first sentence of a doc comment: up to a full stop followed by a space or the end.
fn first_sentence(text: &str) -> String {
    let mut end = text.len();
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '.' && chars.peek().is_none_or(|(_, next)| next.is_whitespace()) {
            end = at;
            break;
        }
    }
    text[..end].trim().to_owned()
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn peek_ident(&self) -> Option<&str> {
        match self.peek() {
            Some(Tok::Ident(name)) => Some(name),
            _ => None,
        }
    }

    fn is_punct(&self, c: char) -> bool {
        self.peek() == Some(&Tok::Punct(c))
    }

    fn eat_punct(&mut self, c: char) -> bool {
        let found = self.is_punct(c);
        if found {
            self.at += 1;
        }
        found
    }

    /// Skips a bracketed group whose opening token is next, nested groups included.
    fn skip_group(&mut self) {
        let mut depth = 0usize;
        while let Some(tok) = self.peek() {
            match tok {
                Tok::Punct('(' | '[' | '{') => depth += 1,
                Tok::Punct(')' | ']' | '}') => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.at += 1;
            if depth == 0 {
                return;
            }
        }
    }

    /// Reads doc lines and `#[...]` attributes.
    fn attrs(&mut self) -> Attrs {
        let mut attrs = Attrs::default();
        loop {
            match self.peek() {
                Some(Tok::Doc(line)) => {
                    attrs.docs.push(line.clone());
                    self.at += 1;
                }
                Some(Tok::Punct('#')) => {
                    self.at += 1;
                    self.eat_punct('!');
                    let start = self.at;
                    self.skip_group();
                    self.read_attr(start, &mut attrs);
                }
                _ => return attrs,
            }
        }
    }

    /// Reads the attribute whose `[ ... ]` group starts at `start`.
    fn read_attr(&self, start: usize, attrs: &mut Attrs) {
        let inner = self
            .toks
            .get(start + 1..self.at.saturating_sub(1))
            .unwrap_or(&[]);
        match inner.first() {
            Some(Tok::Ident(name)) if name == "serde" => {
                let args = inner.get(2..inner.len().saturating_sub(1)).unwrap_or(&[]);
                attrs.serde.extend(serde_args(args));
            }
            Some(Tok::Ident(name)) if name == "cfg" => {
                attrs.test |= inner.contains(&Tok::Ident("test".to_owned()));
            }
            _ => {}
        }
    }

    /// Reads `pub`, `pub(crate)` and the like; true only for a plain `pub`.
    fn visibility(&mut self) -> bool {
        if self.peek_ident() != Some("pub") {
            return false;
        }
        self.at += 1;
        if self.is_punct('(') {
            self.skip_group();
            return false;
        }
        true
    }

    /// Skips one item that is not a struct or an enum.
    fn skip_item(&mut self) {
        let start = self.at;
        while let Some(tok) = self.peek() {
            match tok {
                Tok::Punct(';') => {
                    self.at += 1;
                    return;
                }
                Tok::Punct('{') => {
                    self.skip_group();
                    return;
                }
                Tok::Punct('(' | '[') => self.skip_group(),
                _ => self.at += 1,
            }
        }
        if self.at == start {
            self.at += 1;
        }
    }

    fn type_def(&mut self, attrs: &Attrs) -> Option<TypeDef> {
        let is_enum = self.peek_ident() == Some("enum");
        self.at += 1;
        let Some(Tok::Ident(name)) = self.peek().cloned() else {
            return None;
        };
        self.at += 1;
        if self.is_punct('<') {
            self.skip_angles();
        }
        let case = attrs.value("rename_all").map(str::to_owned);
        let body = if attrs.value("try_from").is_some() || attrs.value("into").is_some() {
            self.skip_item();
            Body::Text
        } else if is_enum {
            self.enum_body(case.as_deref())
        } else if self.is_punct('{') {
            Body::Struct(self.fields(case.as_deref()))
        } else if self.is_punct('(') {
            let types = self.tuple_types();
            self.eat_punct(';');
            Body::Tuple(types)
        } else {
            self.eat_punct(';');
            Body::Unit
        };
        Some(TypeDef {
            name,
            doc: attrs.doc(),
            body,
        })
    }

    fn skip_angles(&mut self) {
        let mut depth = 0usize;
        while let Some(tok) = self.peek() {
            match tok {
                Tok::Punct('<') => depth += 1,
                Tok::Punct('>') => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.at += 1;
            if depth == 0 {
                return;
            }
        }
    }

    fn enum_body(&mut self, case: Option<&str>) -> Body {
        let mut variants = Vec::new();
        if !self.eat_punct('{') {
            return Body::Enum(variants);
        }
        while self.peek().is_some() && !self.is_punct('}') {
            let attrs = self.attrs();
            let Some(Tok::Ident(name)) = self.peek().cloned() else {
                self.at += 1;
                continue;
            };
            self.at += 1;
            let shape = if self.is_punct('(') {
                Shape::Tuple(self.tuple_types())
            } else if self.is_punct('{') {
                Shape::Fields(self.fields(None))
            } else {
                Shape::Unit
            };
            // A discriminant: `= 3`.
            while self.peek().is_some() && !self.is_punct(',') && !self.is_punct('}') {
                self.at += 1;
            }
            self.eat_punct(',');
            if attrs.flag("skip") {
                continue;
            }
            let name = attrs
                .value("rename")
                .map_or_else(|| apply_case(&name, case), str::to_owned);
            variants.push(Variant {
                name,
                doc: attrs.doc(),
                shape,
            });
        }
        self.eat_punct('}');
        Body::Enum(variants)
    }

    /// The types of a tuple payload: `( A, B )`.
    fn tuple_types(&mut self) -> Vec<String> {
        let mut types = Vec::new();
        self.eat_punct('(');
        while self.peek().is_some() && !self.is_punct(')') {
            self.attrs();
            self.visibility();
            types.push(self.ty());
            self.eat_punct(',');
        }
        self.eat_punct(')');
        types
    }

    /// The named fields of `{ ... }`.
    fn fields(&mut self, case: Option<&str>) -> Vec<Field> {
        let mut fields = Vec::new();
        self.eat_punct('{');
        while self.peek().is_some() && !self.is_punct('}') {
            let attrs = self.attrs();
            self.visibility();
            let Some(Tok::Ident(name)) = self.peek().cloned() else {
                self.at += 1;
                continue;
            };
            self.at += 1;
            self.eat_punct(':');
            let ty = self.ty();
            self.eat_punct(',');
            if attrs.flag("skip") {
                continue;
            }
            let (ty, option) = match ty.strip_suffix('?') {
                Some(inner) => (inner.to_owned(), true),
                None => (ty, false),
            };
            let default = attrs.value("default").map(default_text);
            fields.push(Field {
                name: attrs
                    .value("rename")
                    .map_or_else(|| apply_case(&name, case), str::to_owned),
                ty,
                optional: option || attrs.flag("default"),
                default,
            });
        }
        self.eat_punct('}');
        fields
    }

    /// A type, rendered as the DSL shows it: `Vec<T>` is `[T]`, `Option<T>` is `T?`, boxes vanish.
    fn ty(&mut self) -> String {
        match self.peek().cloned() {
            Some(Tok::Punct('&')) => {
                self.at += 1;
                if self.peek() == Some(&Tok::Lifetime) {
                    self.at += 1;
                }
                self.ty()
            }
            Some(Tok::Punct('(')) => {
                let items = self.tuple_types();
                format!("({})", items.join(", "))
            }
            Some(Tok::Punct('[')) => {
                self.at += 1;
                let item = self.ty();
                let mut length = String::new();
                if self.eat_punct(';') {
                    if let Some(Tok::Ident(digits)) = self.peek() {
                        length = format!("; {digits}");
                    }
                    self.at += 1;
                }
                self.eat_punct(']');
                format!("[{item}{length}]")
            }
            Some(Tok::Ident(_)) => self.path_type(),
            _ => {
                self.at += 1;
                String::new()
            }
        }
    }

    fn path_type(&mut self) -> String {
        let mut name = String::new();
        while let Some(Tok::Ident(segment)) = self.peek().cloned() {
            name = segment;
            self.at += 1;
            if self.is_punct(':') && self.toks.get(self.at + 1) == Some(&Tok::Punct(':')) {
                self.at += 2;
            } else {
                break;
            }
        }
        let mut args = Vec::new();
        if self.eat_punct('<') {
            while self.peek().is_some() && !self.is_punct('>') {
                if self.peek() == Some(&Tok::Lifetime) {
                    self.at += 1;
                } else {
                    args.push(self.ty());
                }
                self.eat_punct(',');
            }
            self.eat_punct('>');
        }
        match (name.as_str(), args.as_slice()) {
            ("Vec" | "VecDeque" | "BTreeSet" | "HashSet", [item]) => format!("[{item}]"),
            ("Box" | "Rc" | "Arc" | "Cow", [item]) => item.clone(),
            ("Option", [item]) => format!("{item}?"),
            (_, []) => name,
            (_, args) => format!("{name}<{}>", args.join(", ")),
        }
    }
}

/// The `key` and `key = "value"` entries of a `serde(...)` attribute.
fn serde_args(args: &[Tok]) -> Vec<(String, Option<String>)> {
    args.split(|tok| tok == &Tok::Punct(','))
        .filter_map(|entry| match entry {
            [Tok::Ident(key)] => Some((key.clone(), None)),
            [Tok::Ident(key), Tok::Punct('='), Tok::Str(value)] => {
                Some((key.clone(), Some(value.clone())))
            }
            _ => None,
        })
        .collect()
}

/// A default named by a function path (`Amount::one`) as the value it stands for; the type's own default is empty.
fn default_text(path: &str) -> String {
    match path.rsplit("::").next().unwrap_or(path) {
        "one" => "1".to_owned(),
        "two" => "2".to_owned(),
        "yes" => "true".to_owned(),
        other => other.to_owned(),
    }
}

/// A name in the case `rename_all` asks for; other cases are left as written.
fn apply_case(name: &str, case: Option<&str>) -> String {
    match case {
        Some("snake_case") => {
            let mut text = String::new();
            for (at, c) in name.chars().enumerate() {
                if c.is_uppercase() {
                    if at > 0 {
                        text.push('_');
                    }
                    text.extend(c.to_lowercase());
                } else {
                    text.push(c);
                }
            }
            text
        }
        Some("lowercase") => name.to_lowercase(),
        _ => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the scanner reads the shapes the rules types use (serde renames and defaults, nested generics, payloads of every
    //! kind) and skips what is not a public type (private items, test modules, code with braces in strings).
    use rstest::rstest;

    use super::*;

    fn one(source: &str) -> TypeDef {
        let mut defs = type_defs(source);
        assert_eq!(defs.len(), 1, "{defs:?}");
        defs.remove(0)
    }

    fn fields_of(def: &TypeDef) -> Vec<String> {
        let Body::Struct(fields) = &def.body else {
            panic!("not a struct: {def:?}");
        };
        fields
            .iter()
            .map(|f| {
                format!(
                    "{}{}:{}{}",
                    f.name,
                    if f.optional { "?" } else { "" },
                    f.ty,
                    f.default
                        .as_ref()
                        .map_or(String::new(), |d| format!("={d}"))
                )
            })
            .collect()
    }

    #[rstest]
    #[case::vec("pub struct S { pub a: Vec<u32> }", "a:[u32]")]
    #[case::option("pub struct S { a: Option<Box<Ref>> }", "a?:Ref")]
    #[case::path("pub struct S { a: crate::refs::Ref }", "a:Ref")]
    #[case::tuple(
        "pub struct S { a: Vec<(CounterKind, u32)> }",
        "a:[(CounterKind, u32)]"
    )]
    #[case::array("pub struct S { a: [u8; 3] }", "a:[u8; 3]")]
    #[case::reference("pub struct S<'a> { a: &'a str }", "a:str")]
    #[case::default_fn(
        "pub struct S { #[serde(default = \"Amount::one\", skip_serializing_if = \"Amount::is_one\")] a: Amount }",
        "a?:Amount=1"
    )]
    #[case::rename(
        "pub struct S { #[serde(rename = \"as\")] binding: String }",
        "as:String"
    )]
    #[case::skip("pub struct S { #[serde(skip)] hidden: u8, shown: u8 }", "shown:u8")]
    fn fields_are_shown_as_the_json_has_them(#[case] source: &str, #[case] expected: &str) {
        assert_eq!(fields_of(&one(source)).join(","), expected);
    }

    #[test]
    fn variants_of_every_kind_and_their_first_sentence() {
        let def = one(
            "/// Kinds. More text.\n#[derive(Debug)]\npub enum E {\n    /// Nothing. Really.\n    A,\n    B(u32, Ref),\n    #[serde(rename = \"c\")]\n    C { x: u8 },\n    D = 4,\n}",
        );
        assert_eq!(def.doc, "Kinds");
        let Body::Enum(variants) = def.body else {
            panic!("not an enum");
        };
        let shapes: Vec<String> = variants
            .iter()
            .map(|v| {
                let kind = match &v.shape {
                    Shape::Unit => "unit".to_owned(),
                    Shape::Tuple(items) => format!("tuple{}", items.len()),
                    Shape::Fields(fields) => format!("fields{}", fields.len()),
                };
                format!("{}:{kind}:{}", v.name, v.doc)
            })
            .collect();
        assert_eq!(
            shapes,
            ["A:unit:Nothing", "B:tuple2:", "c:fields1:", "D:unit:"]
        );
    }

    #[test]
    fn rename_all_and_strings_that_are_types() {
        let def = one("#[serde(rename_all = \"snake_case\")] pub enum E { FirstStrike, Other }");
        let Body::Enum(variants) = def.body else {
            panic!("not an enum");
        };
        assert_eq!(variants[0].name, "first_strike");
        let text = one("#[serde(try_from = \"String\", into = \"String\")] pub struct M(Vec<u8>);");
        assert!(matches!(text.body, Body::Text));
    }

    #[test]
    fn private_items_tests_and_code_are_skipped() {
        let source = r###"
            use std::fmt::{self, Debug};
            enum Hidden { A }
            pub(crate) struct Crate { a: u8 }
            const BRACE: &str = "}} { not a type";
            const RAW: &str = r##"pub struct Raw { a: u8 }"##;
            fn code() -> char { let c = '}'; let l: &'static str = "x"; c; todo!() }
            impl Foo { pub fn f() {} }
            #[cfg(test)]
            mod tests { pub struct InTests { a: u8 } }
            /* pub struct Commented { a: u8 } */
            // pub struct Line { a: u8 }
            pub struct Shown { a: u8 }
        "###;
        let names: Vec<String> = type_defs(source).into_iter().map(|d| d.name).collect();
        assert_eq!(names, ["Shown"]);
    }

    #[test]
    fn newtypes_and_unit_structs() {
        let tuple = one("pub struct Selector(pub Vec<Pred>);");
        assert!(matches!(&tuple.body, Body::Tuple(items) if items == &["[Pred]"]));
        assert!(matches!(one("pub struct Unit;").body, Body::Unit));
    }
}
