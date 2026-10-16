//! Comment-preserving, width-aware formatting for Oche source files.

use pretty::RcDoc;

type Doc = RcDoc<'static, ()>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Atom,
    Comment,
    Open(char),
    Close(char),
    Comma,
    Colon,
    Semicolon,
    Operator,
    Dot,
    DotDot,
    Question,
    Hash,
}

#[derive(Clone, Debug)]
struct Token {
    text: String,
    kind: Kind,
    newline_before: bool,
    blank_before: bool,
}

#[derive(Clone, Debug)]
enum Item {
    Token(Token),
    Group {
        open: char,
        close: char,
        items: Vec<Item>,
    },
}

#[derive(Clone, Copy)]
enum Class {
    Atom,
    Operator,
    Prefix,
    Dot,
    Suffix,
    Group(char),
}

/// Format a syntactically valid Oche source file at the requested display width.
///
/// Parsing first makes formatting fail safely instead of rewriting malformed input.
pub fn format(source: &str, width: usize) -> Result<String, oche_frontend::Error> {
    oche_frontend::parse(source)?;
    let tokens = lex(source);
    let mut at = 0;
    let items = tree(&tokens, &mut at, None);
    let document = render_items(&items, true).append(RcDoc::hardline());
    let mut output = Vec::new();
    document
        .render(width.max(20), &mut output)
        .expect("rendering into a byte vector cannot fail");
    Ok(String::from_utf8(output).expect("formatter only emits UTF-8 source text"))
}

fn lex(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    let mut previous_end = 0;
    while at < bytes.len() {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at == bytes.len() {
            break;
        }
        let trivia = &source[previous_end..at];
        let newline_count = trivia.bytes().filter(|byte| *byte == b'\n').count();
        let begin = at;
        let kind = if bytes[at..].starts_with(b"//") {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            Kind::Comment
        } else if bytes[at] == b'"' {
            at += 1;
            while at < bytes.len() {
                if bytes[at] == b'\\' {
                    at += 2;
                } else if bytes[at] == b'"' {
                    at += 1;
                    break;
                } else {
                    at += source[at..].chars().next().unwrap().len_utf8();
                }
            }
            Kind::Atom
        } else if bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_' {
            at += 1;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            Kind::Atom
        } else {
            let (length, kind) = match &bytes[at..] {
                rest if rest.starts_with(b"->")
                    || rest.starts_with(b"=>")
                    || rest.starts_with(b"|>") =>
                {
                    (2, Kind::Operator)
                }
                rest if rest.starts_with(b"..") => (2, Kind::DotDot),
                _ => match bytes[at] {
                    b'(' | b'{' | b'<' | b'[' => (1, Kind::Open(char::from(bytes[at]))),
                    b')' | b'}' | b'>' | b']' => (1, Kind::Close(char::from(bytes[at]))),
                    b',' => (1, Kind::Comma),
                    b':' => (1, Kind::Colon),
                    b';' => (1, Kind::Semicolon),
                    b'+' | b'-' | b'*' | b'=' => (1, Kind::Operator),
                    b'.' => (1, Kind::Dot),
                    b'?' => (1, Kind::Question),
                    b'#' => (1, Kind::Hash),
                    _ => unreachable!("the frontend accepted an unknown token"),
                },
            };
            at += length;
            kind
        };
        tokens.push(Token {
            text: source[begin..at].to_string(),
            kind,
            newline_before: newline_count > 0,
            blank_before: newline_count > 1,
        });
        previous_end = at;
    }
    tokens
}

fn tree(tokens: &[Token], at: &mut usize, closing: Option<char>) -> Vec<Item> {
    let mut items = Vec::new();
    while let Some(token) = tokens.get(*at) {
        match token.kind {
            Kind::Open(open) => {
                *at += 1;
                let close = match open {
                    '(' => ')',
                    '{' => '}',
                    '<' => '>',
                    '[' => ']',
                    _ => unreachable!(),
                };
                let children = tree(tokens, at, Some(close));
                items.push(Item::Group {
                    open,
                    close,
                    items: children,
                });
            }
            Kind::Close(close) if Some(close) == closing => {
                *at += 1;
                break;
            }
            Kind::Close(_) => unreachable!("the frontend accepted unbalanced delimiters"),
            _ => {
                items.push(Item::Token(token.clone()));
                *at += 1;
            }
        }
    }
    items
}

fn render_items(items: &[Item], top_level: bool) -> Doc {
    let mut output = RcDoc::nil();
    let mut previous = None;
    let mut previous_modifier = false;
    let mut attribute_hash = false;
    let mut after_attribute = false;
    let mut emitted = false;
    for (index, item) in items.iter().enumerate() {
        if let Item::Token(token) = item {
            match token.kind {
                Kind::Comma => {
                    output = output.append(",");
                    if index + 1 != items.len() {
                        output = output.append(RcDoc::line());
                    }
                    previous = None;
                    previous_modifier = false;
                    emitted = true;
                    continue;
                }
                Kind::Colon => {
                    output = output.append(": ");
                    previous = None;
                    previous_modifier = false;
                    emitted = true;
                    continue;
                }
                Kind::Semicolon => {
                    output = output.append(";").append(RcDoc::hardline());
                    previous = None;
                    previous_modifier = false;
                    emitted = true;
                    continue;
                }
                Kind::Comment => {
                    if emitted {
                        if previous.is_none() && token.blank_before {
                            output = output.append(RcDoc::hardline());
                        } else if previous.is_some() && token.blank_before {
                            output = output.append(RcDoc::hardline()).append(RcDoc::hardline());
                        } else if previous.is_some() && token.newline_before {
                            output = output.append(RcDoc::hardline());
                        } else if previous.is_some() {
                            output = output.append(" ");
                        }
                    }
                    output = output
                        .append(RcDoc::text(token.text.clone()))
                        .append(RcDoc::hardline());
                    previous = None;
                    previous_modifier = false;
                    emitted = true;
                    continue;
                }
                _ => {}
            }
        }

        let (document, mut class, token) = render_item(item);
        if token.is_some_and(|token| token.text == "-")
            && matches!(previous, None | Some(Class::Operator | Class::Prefix))
        {
            class = Class::Prefix;
        }
        let declaration = top_level
            && token
                .map(|token| {
                    token.kind == Kind::Atom
                        && is_declaration(&token.text)
                        && !((token.text == "async" || token.text == "const" || token.text == "fn")
                            && previous_modifier)
                })
                .unwrap_or(false);
        let attribute_start = top_level && token.is_some_and(|token| token.kind == Kind::Hash);
        if after_attribute {
            output = output.append(RcDoc::hardline());
        } else if emitted && (declaration || attribute_start) && previous.is_some() {
            output = output.append(RcDoc::hardline()).append(RcDoc::hardline());
        } else if emitted && previous.is_none() && token.is_some_and(|token| token.blank_before) {
            output = output.append(RcDoc::hardline());
        } else if previous.is_some_and(|left| needs_space(left, class)) {
            output = output.append(" ");
        }
        output = output.append(document);
        previous = Some(class);
        previous_modifier = token.is_some_and(|token| {
            token.text == "pub"
                || token.text == "test"
                || token.text == "const"
                || token.text == "async"
        });
        after_attribute = attribute_hash && matches!(item, Item::Group { open: '[', .. });
        attribute_hash = token.is_some_and(|token| token.kind == Kind::Hash);
        emitted = true;
    }
    output
}

fn render_item(item: &Item) -> (Doc, Class, Option<&Token>) {
    match item {
        Item::Token(token) => {
            let class = match token.kind {
                Kind::Operator => Class::Operator,
                Kind::Dot | Kind::DotDot => Class::Dot,
                Kind::Question => Class::Suffix,
                Kind::Hash => Class::Prefix,
                Kind::Atom => Class::Atom,
                _ => unreachable!(),
            };
            (RcDoc::text(token.text.clone()), class, Some(token))
        }
        Item::Group { open, close, items } => {
            let content = render_items(items, false);
            let trailing_comma = matches!(
                items.last(),
                Some(Item::Token(Token {
                    kind: Kind::Comma,
                    ..
                }))
            );
            let line = if trailing_comma {
                RcDoc::hardline()
            } else if *open == '{' {
                RcDoc::line()
            } else {
                RcDoc::line_()
            };
            let document = if items.is_empty() {
                RcDoc::text(format!("{open}{close}"))
            } else {
                RcDoc::text(open.to_string())
                    .append(line.clone().append(content).nest(2))
                    .append(line)
                    .append(RcDoc::text(close.to_string()))
                    .group()
            };
            (document, Class::Group(*open), None)
        }
    }
}

fn needs_space(left: Class, right: Class) -> bool {
    match (left, right) {
        (Class::Dot | Class::Prefix, _) | (_, Class::Dot | Class::Suffix) => false,
        (Class::Atom, Class::Group('(' | '<')) => false,
        (_, Class::Group('{')) => true,
        (Class::Operator, _) | (_, Class::Operator) => true,
        _ => true,
    }
}

fn is_declaration(text: &str) -> bool {
    matches!(
        text,
        "pub"
            | "test"
            | "const"
            | "fn"
            | "async"
            | "struct"
            | "enum"
            | "comptime"
            | "import"
            | "dart"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_comments_and_is_idempotent() {
        let source = r#"// user model
struct User{name:String,age:Int}

// generate JSON
comptime jsonEncode(User)

fn name(user:User)->String{user.name} // public entry point
"#;
        let formatted = format(source, 80).unwrap();
        assert!(formatted.contains("// user model"));
        assert!(formatted.contains("// generate JSON"));
        assert!(formatted.contains("// public entry point"));
        assert!(formatted.contains("struct User { name: String, age: Int }"));
        assert!(formatted.contains("comptime jsonEncode(User)"));
        assert!(formatted.contains("fn name(user: User) -> String { user.name }"));
        assert_eq!(format(&formatted, 80).unwrap(), formatted);
    }

    #[test]
    fn keeps_async_attached_to_function_declarations() {
        let source = "pub async fn later(value: Int) -> Int { value }\n\nasync fn use() -> Int { later(1).await }\n";
        let formatted = format(source, 100).unwrap();
        assert!(formatted.starts_with("pub async fn later"));
        assert!(formatted.contains("\n\nasync fn use"));
        assert!(formatted.contains("later(1).await"));
        oche_frontend::parse(&formatted).unwrap();
        assert_eq!(format(&formatted, 100).unwrap(), formatted);
    }

    #[test]
    fn keeps_test_modifiers_attached_to_function_declarations() {
        let source = "test fn works()->Bool{true}\n\ntest async fn later()->Bool{true}\n";
        let formatted = format(source, 100).unwrap();
        assert!(formatted.starts_with("test fn works() -> Bool"));
        assert!(formatted.contains("\n\ntest async fn later() -> Bool"));
        assert_eq!(format(&formatted, 100).unwrap(), formatted);
    }

    #[test]
    fn keeps_const_attached_to_function_declarations() {
        let source = "pub const fn twice(value:Int)->Int{value*2}\n";
        let formatted = format(source, 100).unwrap();
        assert!(formatted.starts_with("pub const fn twice(value: Int) -> Int"));
        assert_eq!(format(&formatted, 100).unwrap(), formatted);
    }

    #[test]
    fn formats_mutable_bindings_and_assignment() {
        let source = "fn bump(value:Int)->Int{let mut current=value;current=current+1;current}\n";
        let formatted = format(source, 100).unwrap();
        assert_eq!(
            formatted,
            "fn bump(value: Int) -> Int {\n  let mut current = value;\n  current = current + 1;\n  current\n}\n"
        );
        oche_frontend::parse(&formatted).unwrap();
        assert_eq!(format(&formatted, 100).unwrap(), formatted);
    }

    #[test]
    fn formats_pipe_chains_as_operators() {
        let source = "fn add(a:Int,b:Int)->Int{a+b}\nfn value()->Int{1|>add(2)|>add(3)}\n";
        let formatted = format(source, 100).unwrap();
        assert!(formatted.contains("1 |> add(2) |> add(3)"));
        assert_eq!(format(&formatted, 100).unwrap(), formatted);
    }

    #[test]
    fn breaks_nested_syntax_at_the_requested_width() {
        let source = "fn choose(value: Result<Int, String>) -> Int { match value { Ok(inner) => inner, Err(_) => 0 } }";
        let formatted = format(source, 35).unwrap();
        assert!(formatted.lines().count() > 4, "{formatted}");
        oche_frontend::parse(&formatted).unwrap();
        assert_eq!(format(&formatted, 35).unwrap(), formatted);
    }

    #[test]
    fn keeps_all_language_examples_canonical_and_parseable() {
        for source in [
            include_str!("../../../examples/arithmetic.oche"),
            include_str!("../../../examples/options.oche"),
            include_str!("../../../examples/comptime_json.oche"),
            include_str!("../../../examples/comptime_json_oche.oche"),
            include_str!("../../../examples/comptime_values.oche"),
            include_str!("../../../examples/comptime_declarations.oche"),
            include_str!("../../../examples/tagged_union_try.oche"),
            include_str!("../../../examples/async.oche"),
            include_str!("../../../examples/js_module.oche"),
            include_str!("../../../examples/lists.oche"),
            include_str!("../../../examples/testing.oche"),
            include_str!("../../../examples/dart_mapping.oche"),
            include_str!("../../../agent/pi_core.oche"),
        ] {
            let formatted = format(source, 100).unwrap();
            assert_eq!(formatted, source);
            oche_frontend::parse(&formatted).unwrap();
        }
    }
}
