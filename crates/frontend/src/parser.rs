use std::collections::{HashMap, HashSet};

use snafu::Snafu;

use crate::{
    Block, ComptimeDirective, ComptimeKind, DartLibrary, Enum, EnumVariant, Expr, ExternFunction,
    Field, FieldValidation, Function, Import, MatchArm, Operator, Parameter, Pattern, Program,
    SourceSpan, Statement, Struct, Type, ValidationRule,
};

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("Unexpected character '{character}'"))]
    UnexpectedCharacter { offset: usize, character: char },
    #[snafu(display("Unterminated string literal"))]
    UnterminatedString { offset: usize },
    #[snafu(display("Unsupported string escape '\\{escape}'"))]
    InvalidEscape { offset: usize, escape: char },
    #[snafu(display("Expected {expected}"))]
    Expected { offset: usize, expected: String },
    #[snafu(display("Expected identifier"))]
    ExpectedIdentifier { offset: usize },
    #[snafu(display("Duplicate parameter '{name}'"))]
    DuplicateParameter { offset: usize, name: String },
    #[snafu(display("Expression nesting limit exceeded"))]
    NestingLimit { offset: usize },
    #[snafu(display("Integer is outside the Int range"))]
    IntegerOutOfRange { offset: usize },
    #[snafu(display("Float literal is outside the finite Float range"))]
    FloatOutOfRange { offset: usize },
    #[snafu(display("Unknown variable '{name}'"))]
    UnknownVariable { offset: usize, name: String },
    #[snafu(display("Unknown function '{name}'"))]
    UnknownFunction { offset: usize, name: String },
    #[snafu(display("'{name}' expects {expected} argument(s), received {actual}"))]
    WrongArgumentCount {
        offset: usize,
        name: String,
        expected: usize,
        actual: usize,
    },
    #[snafu(display("Duplicate pattern binding '{name}'"))]
    DuplicatePatternBinding { offset: usize, name: String },
    #[snafu(display("Unknown pattern variant '{name}'"))]
    UnknownPatternVariant { offset: usize, name: String },
    #[snafu(display("A match expression requires at least one arm"))]
    EmptyMatch { offset: usize },
    #[snafu(display("Expected an expression"))]
    ExpectedExpression { offset: usize },
    #[snafu(display("Duplicate function '{name}'"))]
    DuplicateFunction { offset: usize, name: String },
    #[snafu(display("Unknown type '{name}'"))]
    UnknownType { offset: usize, name: String },
    #[snafu(display("Type '{name}' expects {expected} type argument(s), received {actual}"))]
    WrongTypeArgumentCount {
        offset: usize,
        name: String,
        expected: usize,
        actual: usize,
    },
    #[snafu(display("Unknown comptime operation '{name}'"))]
    UnknownComptimeOperation { offset: usize, name: String },
    #[snafu(display("Duplicate struct field '{name}'"))]
    DuplicateField { offset: usize, name: String },
    #[snafu(display("Duplicate struct '{name}'"))]
    DuplicateStruct { offset: usize, name: String },
    #[snafu(display("Duplicate enum or type '{name}'"))]
    DuplicateEnum { offset: usize, name: String },
    #[snafu(display("Duplicate enum variant '{name}'"))]
    DuplicateVariant { offset: usize, name: String },
    #[snafu(display("A try block must contain at least one '?' propagation"))]
    UselessTry { offset: usize },
    #[snafu(display("A try block cannot mix Option and Result propagation"))]
    MixedTry { offset: usize },
    #[snafu(display("Cannot propagate {actual} from a {expected} boundary"))]
    InvalidPropagation {
        offset: usize,
        actual: String,
        expected: String,
    },
    #[snafu(display("Unknown field '{field}' on struct '{name}'"))]
    UnknownStructField {
        offset: usize,
        name: String,
        field: String,
    },
    #[snafu(display("Struct '{name}' is missing field(s): {fields}"))]
    MissingStructFields {
        offset: usize,
        name: String,
        fields: String,
    },
    #[snafu(display("Struct '{name}' requires named construction: {name} {{ field: value }}"))]
    PositionalStructConstruction { offset: usize, name: String },
    #[snafu(display("Duplicate import alias '{name}'"))]
    DuplicateImportAlias { offset: usize, name: String },
    #[snafu(display("Unknown imported module '{name}'"))]
    UnknownModule { offset: usize, name: String },
    #[snafu(display("Module '{module}' has no function '{name}'"))]
    UnknownModuleFunction {
        offset: usize,
        module: String,
        name: String,
    },
    #[snafu(display("'await' is only valid inside an async function"))]
    AwaitOutsideAsync { offset: usize },
    #[snafu(display("Prefix 'await' is not supported; write 'expression.await'"))]
    PrefixAwait { offset: usize },
    #[snafu(display("Test function '{name}' cannot be public"))]
    PublicTest { offset: usize, name: String },
    #[snafu(display("Test function '{name}' cannot have parameters"))]
    TestParameters { offset: usize, name: String },
    #[snafu(display("Test function '{name}' must return Bool"))]
    TestResult { offset: usize, name: String },
    #[snafu(display("'let mut' currently requires a single identifier"))]
    MutablePattern { offset: usize },
    #[snafu(display("Cannot assign to immutable binding '{name}'"))]
    ImmutableAssignment { offset: usize, name: String },
}

impl Error {
    pub fn offset(&self) -> usize {
        match self {
            Self::UnexpectedCharacter { offset, .. }
            | Self::UnterminatedString { offset }
            | Self::InvalidEscape { offset, .. }
            | Self::Expected { offset, .. }
            | Self::ExpectedIdentifier { offset }
            | Self::DuplicateParameter { offset, .. }
            | Self::NestingLimit { offset }
            | Self::IntegerOutOfRange { offset }
            | Self::FloatOutOfRange { offset }
            | Self::UnknownVariable { offset, .. }
            | Self::UnknownFunction { offset, .. }
            | Self::WrongArgumentCount { offset, .. }
            | Self::DuplicatePatternBinding { offset, .. }
            | Self::UnknownPatternVariant { offset, .. }
            | Self::EmptyMatch { offset }
            | Self::ExpectedExpression { offset }
            | Self::DuplicateFunction { offset, .. }
            | Self::UnknownType { offset, .. }
            | Self::WrongTypeArgumentCount { offset, .. }
            | Self::UnknownComptimeOperation { offset, .. }
            | Self::DuplicateField { offset, .. }
            | Self::DuplicateStruct { offset, .. }
            | Self::DuplicateEnum { offset, .. }
            | Self::DuplicateVariant { offset, .. }
            | Self::UselessTry { offset }
            | Self::MixedTry { offset }
            | Self::InvalidPropagation { offset, .. }
            | Self::UnknownStructField { offset, .. }
            | Self::MissingStructFields { offset, .. }
            | Self::PositionalStructConstruction { offset, .. }
            | Self::DuplicateImportAlias { offset, .. }
            | Self::UnknownModule { offset, .. }
            | Self::UnknownModuleFunction { offset, .. } => *offset,
            Self::AwaitOutsideAsync { offset }
            | Self::PrefixAwait { offset }
            | Self::PublicTest { offset, .. }
            | Self::TestParameters { offset, .. }
            | Self::TestResult { offset, .. } => *offset,
            Self::MutablePattern { offset } | Self::ImmutableAssignment { offset, .. } => *offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Name(String),
    Number(String),
    String(String),
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    LeftAngle,
    RightAngle,
    Colon,
    Comma,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Equal,
    Semicolon,
    Question,
    Dot,
    DotDot,
    Hash,
    Pipe,
    End,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    offset: usize,
    end: usize,
}

fn lex(source: &str) -> Result<Vec<Token>, Error> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let begin = at;
        if bytes[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if bytes[at..].starts_with(b"//") {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            continue;
        }
        let kind = match bytes[at] {
            b'(' => Kind::LeftParen,
            b')' => Kind::RightParen,
            b'{' => Kind::LeftBrace,
            b'}' => Kind::RightBrace,
            b'[' => Kind::LeftBracket,
            b']' => Kind::RightBracket,
            b'<' => Kind::LeftAngle,
            b'>' => Kind::RightAngle,
            b':' => Kind::Colon,
            b',' => Kind::Comma,
            b'+' => Kind::Plus,
            b'*' => Kind::Star,
            b';' => Kind::Semicolon,
            b'?' => Kind::Question,
            b'#' => Kind::Hash,
            b'|' if bytes[at..].starts_with(b"|>") => {
                at += 1;
                Kind::Pipe
            }
            b'.' if bytes[at..].starts_with(b"..") => {
                at += 1;
                Kind::DotDot
            }
            b'.' => Kind::Dot,
            b'=' if bytes[at..].starts_with(b"=>") => {
                at += 1;
                Kind::FatArrow
            }
            b'=' => Kind::Equal,
            b'-' if bytes[at..].starts_with(b"->") => {
                at += 1;
                Kind::Arrow
            }
            b'-' => Kind::Minus,
            b'"' => {
                at += 1;
                let mut value = String::new();
                loop {
                    if at >= bytes.len() {
                        return Err(Error::UnterminatedString { offset: begin });
                    }
                    match bytes[at] {
                        b'"' => break,
                        b'\\' => {
                            at += 1;
                            if at >= bytes.len() {
                                return Err(Error::UnterminatedString { offset: begin });
                            }
                            let escaped = match bytes[at] {
                                b'"' => '"',
                                b'\\' => '\\',
                                b'n' => '\n',
                                b'r' => '\r',
                                b't' => '\t',
                                byte => {
                                    return Err(Error::InvalidEscape {
                                        offset: at - 1,
                                        escape: char::from(byte),
                                    });
                                }
                            };
                            value.push(escaped);
                        }
                        byte if byte.is_ascii() => value.push(char::from(byte)),
                        _ => {
                            let character = source[at..].chars().next().unwrap();
                            value.push(character);
                            at += character.len_utf8() - 1;
                        }
                    }
                    at += 1;
                }
                Kind::String(value)
            }
            value if value.is_ascii_digit() => {
                while at < bytes.len() && bytes[at].is_ascii_digit() {
                    at += 1;
                }
                if at + 1 < bytes.len() && bytes[at] == b'.' && bytes[at + 1].is_ascii_digit() {
                    at += 1;
                    while at < bytes.len() && bytes[at].is_ascii_digit() {
                        at += 1;
                    }
                }
                if at < bytes.len() && matches!(bytes[at], b'e' | b'E') {
                    at += 1;
                    if at < bytes.len() && matches!(bytes[at], b'+' | b'-') {
                        at += 1;
                    }
                    while at < bytes.len() && bytes[at].is_ascii_digit() {
                        at += 1;
                    }
                }
                tokens.push(Token {
                    kind: Kind::Number(source[begin..at].into()),
                    offset: begin,
                    end: at,
                });
                continue;
            }
            value if value.is_ascii_alphabetic() || value == b'_' => {
                while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                    at += 1;
                }
                tokens.push(Token {
                    kind: Kind::Name(source[begin..at].into()),
                    offset: begin,
                    end: at,
                });
                continue;
            }
            _ => {
                return Err(Error::UnexpectedCharacter {
                    offset: at,
                    character: source[at..].chars().next().unwrap(),
                });
            }
        };
        at += 1;
        tokens.push(Token {
            kind,
            offset: begin,
            end: at,
        });
    }
    tokens.push(Token {
        kind: Kind::End,
        offset: source.len(),
        end: source.len(),
    });
    Ok(tokens)
}

fn parse_number(source: &str, offset: usize) -> Result<Expr, Error> {
    if source.contains(['.', 'e', 'E']) {
        let value = source
            .parse::<f64>()
            .map_err(|_| Error::FloatOutOfRange { offset })?;
        if !value.is_finite() {
            return Err(Error::FloatOutOfRange { offset });
        }
        Ok(Expr::Float(value))
    } else {
        source
            .parse::<i64>()
            .map(Expr::Integer)
            .map_err(|_| Error::IntegerOutOfRange { offset })
    }
}

fn parse_number_pattern(source: &str, offset: usize) -> Result<Pattern, Error> {
    Ok(match parse_number(source, offset)? {
        Expr::Integer(value) => Pattern::Integer(value),
        Expr::Float(value) => Pattern::Float(value),
        _ => unreachable!("numeric parser only produces numeric expressions"),
    })
}

fn located_pattern(start: usize, end: usize, value: Pattern) -> Pattern {
    Pattern::Located {
        span: SourceSpan { start, end },
        value: Box::new(value),
    }
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn recover_top_level(&mut self, declaration_start: usize) {
        if self.at <= declaration_start && self.token().kind != Kind::End {
            self.at += 1;
        }
        while self.token().kind != Kind::End {
            if self.token().kind == Kind::Hash
                || matches!(
                &self.token().kind,
                Kind::Name(name)
                    if matches!(name.as_str(), "fn" | "async" | "const" | "struct" | "enum" | "comptime" | "import" | "dart")
                )
            {
                break;
            }
            self.at += 1;
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.at]
    }

    fn consume(&mut self, kind: Kind) -> bool {
        if self.token().kind == kind {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: Kind) -> Result<(), Error> {
        if self.consume(kind.clone()) {
            Ok(())
        } else {
            Err(Error::Expected {
                offset: self.token().offset,
                expected: format!("{kind:?}"),
            })
        }
    }

    fn keyword(&mut self, name: &str) -> Result<(), Error> {
        self.expect(Kind::Name(name.into()))
    }

    fn name(&mut self) -> Result<String, Error> {
        match self.token().kind.clone() {
            Kind::Name(name)
                if !matches!(
                    name.as_str(),
                    "fn" | "struct"
                        | "enum"
                        | "comptime"
                        | "import"
                        | "as"
                        | "dart"
                        | "let"
                        | "mut"
                        | "try"
                        | "pub"
                        | "async"
                        | "const"
                        | "await"
                        | "if"
                        | "match"
                        | "true"
                        | "false"
                ) =>
            {
                self.at += 1;
                Ok(name)
            }
            _ => Err(Error::ExpectedIdentifier {
                offset: self.token().offset,
            }),
        }
    }

    fn ty(&mut self) -> Result<(usize, Type), Error> {
        let offset = self.token().offset;
        let name = self.name()?;
        let mut arguments = Vec::new();
        if self.consume(Kind::LeftAngle) {
            loop {
                arguments.push(self.ty()?.1);
                if self.consume(Kind::RightAngle) {
                    break;
                }
                self.expect(Kind::Comma)?;
            }
        }
        Ok((offset, Type::Named { name, arguments }))
    }

    fn function(&mut self) -> Result<Function, Error> {
        let start = self.token().offset;
        let is_public = self.consume(Kind::Name("pub".into()));
        let is_test = self.consume(Kind::Name("test".into()));
        let is_const = self.consume(Kind::Name("const".into()));
        let is_async = self.consume(Kind::Name("async".into()));
        self.keyword("fn")?;
        let name = self.name()?;
        self.expect(Kind::LeftParen)?;
        let mut parameters = Vec::new();
        if !self.consume(Kind::RightParen) {
            loop {
                let parameter_offset = self.token().offset;
                let parameter_name = self.name()?;
                if parameters
                    .iter()
                    .any(|parameter: &Parameter| parameter.name == parameter_name)
                {
                    return Err(Error::DuplicateParameter {
                        offset: parameter_offset,
                        name: parameter_name,
                    });
                }
                self.expect(Kind::Colon)?;
                let (_, ty) = self.ty()?;
                parameters.push(Parameter {
                    span: SourceSpan {
                        start: parameter_offset,
                        end: self.tokens[self.at.saturating_sub(1)].end,
                    },
                    name: parameter_name,
                    ty,
                });
                if self.consume(Kind::RightParen) {
                    break;
                }
                self.expect(Kind::Comma)?;
            }
        }
        self.expect(Kind::Arrow)?;
        let (_, result) = self.ty()?;
        self.expect(Kind::LeftBrace)?;
        let block = self.block(0)?;
        let body = if block.statements.is_empty() {
            *block.tail
        } else {
            Expr::Block(block)
        };
        if is_test && is_public {
            return Err(Error::PublicTest {
                offset: start,
                name,
            });
        }
        if is_test && !parameters.is_empty() {
            return Err(Error::TestParameters {
                offset: start,
                name,
            });
        }
        if is_test
            && !matches!(
                &result,
                Type::Named { name, arguments } if name == "Bool" && arguments.is_empty()
            )
        {
            return Err(Error::TestResult {
                offset: start,
                name,
            });
        }
        Ok(Function {
            span: SourceSpan {
                start,
                end: self.tokens[self.at.saturating_sub(1)].end,
            },
            is_public,
            is_test,
            is_const,
            is_async,
            name,
            parameters,
            result,
            body,
        })
    }

    fn enumeration(&mut self) -> Result<Enum, Error> {
        let start = self.token().offset;
        self.keyword("enum")?;
        let name = self.name()?;
        let mut type_parameters = Vec::new();
        if self.consume(Kind::LeftAngle) {
            loop {
                let parameter_offset = self.token().offset;
                let parameter = self.name()?;
                if type_parameters.contains(&parameter) {
                    return Err(Error::DuplicateParameter {
                        offset: parameter_offset,
                        name: parameter,
                    });
                }
                type_parameters.push(parameter);
                if self.consume(Kind::RightAngle) {
                    break;
                }
                self.expect(Kind::Comma)?;
            }
        }
        self.expect(Kind::LeftBrace)?;
        let mut variants = Vec::new();
        while !self.consume(Kind::RightBrace) {
            let variant_offset = self.token().offset;
            let variant_name = self.name()?;
            if variants
                .iter()
                .any(|variant: &EnumVariant| variant.name == variant_name)
            {
                return Err(Error::DuplicateVariant {
                    offset: variant_offset,
                    name: variant_name,
                });
            }
            let mut fields = Vec::new();
            if self.consume(Kind::LeftParen) && !self.consume(Kind::RightParen) {
                loop {
                    fields.push(self.ty()?.1);
                    if self.consume(Kind::RightParen) {
                        break;
                    }
                    self.expect(Kind::Comma)?;
                }
            }
            variants.push(EnumVariant {
                span: Some(SourceSpan {
                    start: variant_offset,
                    end: self.tokens[self.at.saturating_sub(1)].end,
                }),
                name: variant_name,
                fields,
            });
            if self.consume(Kind::RightBrace) {
                break;
            }
            self.expect(Kind::Comma)?;
        }
        Ok(Enum {
            span: Some(SourceSpan {
                start,
                end: self.tokens[self.at.saturating_sub(1)].end,
            }),
            name,
            type_parameters,
            variants,
        })
    }

    fn import(&mut self) -> Result<Import, Error> {
        let start = self.token().offset;
        self.keyword("import")?;
        let Kind::String(path) = self.token().kind.clone() else {
            return Err(Error::Expected {
                offset: self.token().offset,
                expected: "import path string".into(),
            });
        };
        self.at += 1;
        self.keyword("as")?;
        let alias = self.name()?;
        self.consume(Kind::Semicolon);
        let end = self.tokens[self.at.saturating_sub(1)].end;
        Ok(Import {
            span: SourceSpan { start, end },
            path,
            alias,
        })
    }

    fn dart_library(&mut self) -> Result<DartLibrary, Error> {
        let start = self.token().offset;
        self.keyword("dart")?;
        let Kind::String(uri) = self.token().kind.clone() else {
            return Err(Error::Expected {
                offset: self.token().offset,
                expected: "Dart library URI string".into(),
            });
        };
        self.at += 1;
        self.expect(Kind::LeftBrace)?;
        let mut functions = Vec::new();
        while !self.consume(Kind::RightBrace) {
            let offset = self.token().offset;
            self.keyword("fn")?;
            let name = self.name()?;
            self.expect(Kind::LeftParen)?;
            let mut parameters = Vec::new();
            if !self.consume(Kind::RightParen) {
                loop {
                    let parameter_offset = self.token().offset;
                    let parameter_name = self.name()?;
                    if parameters
                        .iter()
                        .any(|parameter: &Parameter| parameter.name == parameter_name)
                    {
                        return Err(Error::DuplicateParameter {
                            offset,
                            name: parameter_name,
                        });
                    }
                    self.expect(Kind::Colon)?;
                    let ty = self.ty()?.1;
                    parameters.push(Parameter {
                        span: SourceSpan {
                            start: parameter_offset,
                            end: self.tokens[self.at.saturating_sub(1)].end,
                        },
                        name: parameter_name,
                        ty,
                    });
                    if self.consume(Kind::RightParen) {
                        break;
                    }
                    self.expect(Kind::Comma)?;
                }
            }
            self.expect(Kind::Arrow)?;
            let result = self.ty()?.1;
            self.consume(Kind::Semicolon);
            if functions
                .iter()
                .any(|function: &ExternFunction| function.name == name)
            {
                return Err(Error::DuplicateFunction { offset, name });
            }
            functions.push(ExternFunction {
                span: SourceSpan {
                    start: offset,
                    end: self.tokens[self.at.saturating_sub(1)].end,
                },
                name,
                parameters,
                result,
            });
        }
        Ok(DartLibrary {
            span: SourceSpan {
                start,
                end: self.tokens[self.at.saturating_sub(1)].end,
            },
            uri,
            namespace: String::new(),
            functions,
        })
    }

    fn block(&mut self, depth: usize) -> Result<Block, Error> {
        let mut statements = Vec::new();
        loop {
            if self.token().kind == Kind::RightBrace {
                return Err(Error::ExpectedExpression {
                    offset: self.token().offset,
                });
            }
            if self.token().kind == Kind::Name("let".into()) {
                self.at += 1;
                let mutable = self.consume(Kind::Name("mut".into()));
                let pattern = self.pattern()?;
                if mutable && !matches!(pattern.unlocated(), Pattern::Binding(_)) {
                    return Err(Error::MutablePattern {
                        offset: pattern
                            .span()
                            .map_or(self.token().offset, |span| span.start),
                    });
                }
                self.expect(Kind::Equal)?;
                let value = self.expression(0, depth + 1)?;
                self.expect(Kind::Semicolon)?;
                statements.push(Statement::Let {
                    mutable,
                    pattern,
                    value,
                });
                continue;
            }
            if let Kind::Name(name) = self.token().kind.clone()
                && self
                    .tokens
                    .get(self.at + 1)
                    .is_some_and(|token| token.kind == Kind::Equal)
            {
                self.at += 2;
                let value = self.expression(0, depth + 1)?;
                self.expect(Kind::Semicolon)?;
                statements.push(Statement::Assign { name, value });
                continue;
            }
            let expression = self.expression(0, depth + 1)?;
            if self.consume(Kind::Semicolon) {
                statements.push(Statement::Expression(expression));
                continue;
            }
            self.expect(Kind::RightBrace)?;
            return Ok(Block {
                statements,
                tail: Box::new(expression),
            });
        }
    }

    fn structure(&mut self) -> Result<Struct, Error> {
        let start = self.token().offset;
        self.keyword("struct")?;
        let name = self.name()?;
        self.expect(Kind::LeftBrace)?;
        let mut fields = Vec::new();
        while !self.consume(Kind::RightBrace) {
            let field_offset = self.token().offset;
            let field_name = self.name()?;
            if fields.iter().any(|field: &Field| field.name == field_name) {
                return Err(Error::DuplicateField {
                    offset: field_offset,
                    name: field_name,
                });
            }
            self.expect(Kind::Colon)?;
            let (_, ty) = self.ty()?;
            fields.push(Field {
                span: SourceSpan {
                    start: field_offset,
                    end: self.tokens[self.at.saturating_sub(1)].end,
                },
                name: field_name,
                ty,
            });
            if self.consume(Kind::RightBrace) {
                break;
            }
            self.expect(Kind::Comma)?;
        }
        Ok(Struct {
            span: SourceSpan {
                start,
                end: self.tokens[self.at.saturating_sub(1)].end,
            },
            name,
            fields,
        })
    }

    fn comptime(&mut self) -> Result<ComptimeDirective, Error> {
        let offset = self.token().offset;
        self.keyword("comptime")?;
        let operation = self.name()?;
        self.expect(Kind::LeftParen)?;
        let target = self.name()?;
        let kind = match operation.as_str() {
            "jsonEncode" => {
                self.expect(Kind::RightParen)?;
                ComptimeKind::Generate {
                    function: "_oche_generate_json_encode".into(),
                }
            }
            "jsonDecode" => {
                self.expect(Kind::RightParen)?;
                ComptimeKind::Generate {
                    function: "_oche_generate_json_decode".into(),
                }
            }
            "validate" => {
                let mut validations = Vec::new();
                while self.consume(Kind::Comma) {
                    let field = self.name()?;
                    self.expect(Kind::Colon)?;
                    let rule = self.name()?;
                    let rule = match rule.as_str() {
                        "non_empty" => ValidationRule::NonEmpty,
                        "range" => {
                            self.expect(Kind::LeftParen)?;
                            let minimum = self.signed_integer()?;
                            self.expect(Kind::Comma)?;
                            let maximum = self.signed_integer()?;
                            self.expect(Kind::RightParen)?;
                            ValidationRule::Range { minimum, maximum }
                        }
                        _ => {
                            return Err(Error::UnknownComptimeOperation {
                                offset: self.token().offset,
                                name: rule,
                            });
                        }
                    };
                    validations.push(FieldValidation { field, rule });
                }
                self.expect(Kind::RightParen)?;
                ComptimeKind::Validate(validations)
            }
            _ => {
                self.expect(Kind::RightParen)?;
                ComptimeKind::Generate {
                    function: operation,
                }
            }
        };
        Ok(ComptimeDirective {
            span: SourceSpan {
                start: offset,
                end: self.tokens[self.at.saturating_sub(1)].end,
            },
            target,
            kind,
        })
    }

    fn signed_integer(&mut self) -> Result<i64, Error> {
        let negative = self.consume(Kind::Minus);
        let Kind::Number(number) = self.token().kind.clone() else {
            return Err(Error::Expected {
                offset: self.token().offset,
                expected: "integer".into(),
            });
        };
        let text = if negative {
            format!("-{number}")
        } else {
            number
        };
        let value = text.parse::<i64>().map_err(|_| Error::IntegerOutOfRange {
            offset: self.token().offset,
        })?;
        self.at += 1;
        Ok(value)
    }

    fn expression(&mut self, minimum: u8, depth: usize) -> Result<Expr, Error> {
        let start = self.token().offset;
        if depth > 128 {
            return Err(Error::NestingLimit {
                offset: self.token().offset,
            });
        }
        let mut left = if self.consume(Kind::Minus) {
            if let Kind::Number(number) = &self.token().kind {
                let value = parse_number(&format!("-{number}"), self.token().offset)?;
                self.at += 1;
                value
            } else {
                Expr::Negate(Box::new(self.expression(3, depth + 1)?))
            }
        } else if self.token().kind == Kind::Name("await".into()) {
            return Err(Error::PrefixAwait {
                offset: self.token().offset,
            });
        } else if self.token().kind == Kind::Name("comptime".into()) {
            self.at += 1;
            Expr::Comptime(Box::new(self.expression(3, depth + 1)?))
        } else if self.token().kind == Kind::Name("type".into()) {
            self.at += 1;
            let (_, ty) = self.ty()?;
            Expr::TypeValue(ty)
        } else if self.consume(Kind::LeftParen) {
            let expression = self.expression(0, depth + 1)?;
            self.expect(Kind::RightParen)?;
            expression
        } else if self.token().kind == Kind::Name("try".into()) {
            let offset = self.token().offset;
            self.at += 1;
            self.expect(Kind::LeftBrace)?;
            let block = self.block(depth + 1)?;
            if !block_contains_propagate(&block) {
                return Err(Error::UselessTry { offset });
            }
            Expr::Try(block)
        } else if self.token().kind == Kind::Name("match".into()) {
            let offset = self.token().offset;
            self.at += 1;
            let value = self.expression(0, depth + 1)?;
            self.expect(Kind::LeftBrace)?;
            let mut arms = Vec::new();
            while !self.consume(Kind::RightBrace) {
                let arm_offset = self.token().offset;
                let pattern = self.pattern()?;
                let guard = if self.consume(Kind::Name("if".into())) {
                    Some(self.expression(0, depth + 1)?)
                } else {
                    None
                };
                self.expect(Kind::FatArrow)?;
                let body = self.expression(0, depth + 1)?;
                arms.push(MatchArm {
                    offset: arm_offset,
                    pattern,
                    guard,
                    body,
                });
                if !self.consume(Kind::Comma) {
                    self.expect(Kind::RightBrace)?;
                    break;
                }
            }
            if arms.is_empty() {
                return Err(Error::EmptyMatch {
                    offset: self.token().offset,
                });
            }
            Expr::Match {
                offset,
                value: Box::new(value),
                arms,
            }
        } else {
            self.primary(depth)?
        };

        loop {
            if self.consume(Kind::Question) {
                left = Expr::Propagate(Box::new(left));
            } else if self.consume(Kind::Dot) {
                if self.consume(Kind::Name("await".into())) {
                    left = Expr::Await(Box::new(left));
                } else {
                    let name = self.name()?;
                    if self.token().kind == Kind::LeftParen {
                        let arguments = self.arguments(depth)?;
                        left = Expr::DotCall {
                            receiver: Box::new(left),
                            name,
                            arguments,
                        };
                    } else {
                        left = Expr::Field {
                            value: Box::new(left),
                            name,
                        };
                    }
                }
            } else {
                break;
            }
        }

        loop {
            if self.token().kind == Kind::Pipe && minimum == 0 {
                self.at += 1;
                let right = self.expression(1, depth + 1)?;
                left = pipe_expression(left, right, start)?;
                continue;
            }
            let (operator, precedence) = match self.token().kind {
                Kind::Plus => (Operator::Add, 1),
                Kind::Minus => (Operator::Subtract, 1),
                Kind::Star => (Operator::Multiply, 2),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.at += 1;
            let right = self.expression(precedence + 1, depth + 1)?;
            left = Expr::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            };
            while self.consume(Kind::Question) {
                left = Expr::Propagate(Box::new(left));
            }
        }
        let end = self.tokens[self.at.saturating_sub(1)].end;
        Ok(Expr::Located {
            span: SourceSpan { start, end },
            value: Box::new(left),
        })
    }

    fn primary(&mut self, depth: usize) -> Result<Expr, Error> {
        match self.token().kind.clone() {
            Kind::LeftBracket => {
                self.at += 1;
                let mut values = Vec::new();
                while !self.consume(Kind::RightBracket) {
                    values.push(self.expression(0, depth + 1)?);
                    if self.consume(Kind::RightBracket) {
                        break;
                    }
                    self.expect(Kind::Comma)?;
                }
                Ok(Expr::List(values))
            }
            Kind::Number(number) => {
                let value = parse_number(&number, self.token().offset)?;
                self.at += 1;
                Ok(value)
            }
            Kind::String(value) => {
                self.at += 1;
                Ok(Expr::String(value))
            }
            Kind::Name(name) if name == "true" || name == "false" => {
                self.at += 1;
                Ok(Expr::Boolean(name == "true"))
            }
            Kind::Name(name) => {
                self.at += 1;
                if name.chars().next().is_some_and(char::is_uppercase)
                    && self.consume(Kind::LeftBrace)
                {
                    let mut fields = Vec::new();
                    while !self.consume(Kind::RightBrace) {
                        let field_offset = self.token().offset;
                        let field = self.name()?;
                        if fields
                            .iter()
                            .any(|(existing, _): &(String, Expr)| *existing == field)
                        {
                            return Err(Error::DuplicateField {
                                offset: field_offset,
                                name: field,
                            });
                        }
                        self.expect(Kind::Colon)?;
                        let value = self.expression(0, depth + 1)?;
                        fields.push((field, value));
                        if self.consume(Kind::RightBrace) {
                            break;
                        }
                        self.expect(Kind::Comma)?;
                    }
                    Ok(Expr::StructLiteral { name, fields })
                } else if self.token().kind == Kind::LeftParen {
                    let arguments = self.arguments(depth)?;
                    Ok(Expr::Call {
                        callee: name,
                        arguments,
                    })
                } else if name == "None" {
                    Ok(Expr::Call {
                        callee: name,
                        arguments: Vec::new(),
                    })
                } else {
                    Ok(Expr::Variable(name))
                }
            }
            _ => Err(Error::ExpectedExpression {
                offset: self.token().offset,
            }),
        }
    }

    fn arguments(&mut self, depth: usize) -> Result<Vec<Expr>, Error> {
        self.expect(Kind::LeftParen)?;
        let mut arguments = Vec::new();
        if !self.consume(Kind::RightParen) {
            loop {
                arguments.push(self.expression(0, depth + 1)?);
                if self.consume(Kind::RightParen) {
                    break;
                }
                self.expect(Kind::Comma)?;
            }
        }
        Ok(arguments)
    }

    fn pattern(&mut self) -> Result<Pattern, Error> {
        let start = self.token().offset;
        let pattern = if self.consume(Kind::Minus) {
            let Kind::Number(number) = self.token().kind.clone() else {
                return Err(Error::Expected {
                    offset: self.token().offset,
                    expected: "integer pattern".into(),
                });
            };
            let value = parse_number_pattern(&format!("-{number}"), self.token().offset)?;
            self.at += 1;
            value
        } else {
            match self.token().kind.clone() {
                Kind::Number(number) => {
                    let value = parse_number_pattern(&number, self.token().offset)?;
                    self.at += 1;
                    value
                }
                Kind::String(value) => {
                    self.at += 1;
                    Pattern::String(value)
                }
                Kind::Name(name) if name == "true" || name == "false" => {
                    self.at += 1;
                    Pattern::Boolean(name == "true")
                }
                Kind::Name(name) if name == "_" => {
                    self.at += 1;
                    Pattern::Wildcard
                }
                Kind::Name(name) => {
                    self.at += 1;
                    let is_variant = name.chars().next().is_some_and(char::is_uppercase);
                    if !is_variant {
                        Pattern::Binding(name)
                    } else if self.consume(Kind::LeftBrace) {
                        let mut fields = Vec::new();
                        let mut rest = false;
                        while !self.consume(Kind::RightBrace) {
                            if self.consume(Kind::DotDot) {
                                rest = true;
                                self.consume(Kind::Comma);
                                self.expect(Kind::RightBrace)?;
                                break;
                            }
                            let field_start = self.token().offset;
                            let field_name = self.name()?;
                            let field_pattern = if self.consume(Kind::Colon) {
                                self.pattern()?
                            } else {
                                located_pattern(
                                    field_start,
                                    self.tokens[self.at.saturating_sub(1)].end,
                                    Pattern::Binding(field_name.clone()),
                                )
                            };
                            fields.push((field_name, field_pattern));
                            if self.consume(Kind::RightBrace) {
                                break;
                            }
                            self.expect(Kind::Comma)?;
                        }
                        Pattern::Struct { name, fields, rest }
                    } else {
                        let mut fields = Vec::new();
                        if self.consume(Kind::LeftParen) && !self.consume(Kind::RightParen) {
                            loop {
                                fields.push(self.pattern()?);
                                if self.consume(Kind::RightParen) {
                                    break;
                                }
                                self.expect(Kind::Comma)?;
                            }
                        }
                        Pattern::Variant { name, fields }
                    }
                }
                _ => {
                    return Err(Error::Expected {
                        offset: self.token().offset,
                        expected: "pattern".into(),
                    });
                }
            }
        };
        let end = self.tokens[self.at.saturating_sub(1)].end;
        Ok(located_pattern(start, end, pattern))
    }
}

fn pipe_expression(left: Expr, right: Expr, offset: usize) -> Result<Expr, Error> {
    let right = match right {
        Expr::Located { value, .. } => *value,
        value => value,
    };
    match right {
        Expr::Call {
            callee,
            mut arguments,
        } => {
            arguments.insert(0, left);
            Ok(Expr::Call { callee, arguments })
        }
        Expr::QualifiedCall {
            module,
            name,
            mut arguments,
        } => {
            arguments.insert(0, left);
            Ok(Expr::QualifiedCall {
                module,
                name,
                arguments,
            })
        }
        Expr::DotCall {
            receiver,
            name,
            mut arguments,
        } => {
            arguments.insert(0, left);
            Ok(Expr::DotCall {
                receiver,
                name,
                arguments,
            })
        }
        _ => Err(Error::Expected {
            offset,
            expected: "function call after '|>'".into(),
        }),
    }
}

#[derive(Debug)]
pub struct ParseOutput {
    pub program: Option<Program>,
    pub diagnostics: Vec<Error>,
}

pub fn parse_recovering(source: &str) -> ParseOutput {
    let tokens = match lex(source) {
        Ok(tokens) => tokens,
        Err(error) => {
            return ParseOutput {
                program: None,
                diagnostics: vec![error],
            };
        }
    };
    let mut parser = Parser { tokens, at: 0 };
    let mut functions = Vec::new();
    let mut function_names = HashSet::new();
    let mut structs = Vec::new();
    let mut struct_names = crate::COMPTIME_IR_TYPES
        .into_iter()
        .chain(["JsonValue", "JsonObject"])
        .map(str::to_owned)
        .collect::<HashSet<_>>();
    let mut enums = builtin_enums();
    let mut enum_names = enums
        .iter()
        .map(|enumeration| enumeration.name.clone())
        .collect::<HashSet<_>>();
    let mut variant_names = enums
        .iter()
        .flat_map(|enumeration| enumeration.variants.iter())
        .map(|variant| variant.name.clone())
        .collect::<HashSet<_>>();
    let mut comptime = Vec::new();
    let mut imports = Vec::new();
    let mut import_aliases = HashSet::new();
    let mut dart_libraries = Vec::new();
    let mut diagnostics = Vec::new();
    while parser.token().kind != Kind::End {
        let declaration_start = parser.at;
        let result = (|| -> Result<(), Error> {
            match &parser.token().kind {
                Kind::Name(name)
                    if name == "fn"
                        || name == "async"
                        || name == "const"
                        || name == "pub"
                        || name == "test" =>
                {
                    let function = parser.function()?;
                    if !function_names.insert(function.name.clone()) {
                        return Err(Error::DuplicateFunction {
                            offset: function.span.start,
                            name: function.name,
                        });
                    }
                    functions.push(function);
                }
                Kind::Name(name) if name == "struct" => {
                    let structure = parser.structure()?;
                    if enum_names.contains(&structure.name)
                        || !struct_names.insert(structure.name.clone())
                    {
                        return Err(Error::DuplicateStruct {
                            offset: structure.span.start,
                            name: structure.name,
                        });
                    }
                    structs.push(structure);
                }
                Kind::Name(name) if name == "enum" => {
                    let enumeration = parser.enumeration()?;
                    let offset = enumeration.span.expect("source enum has a span").start;
                    if struct_names.contains(&enumeration.name)
                        || !enum_names.insert(enumeration.name.clone())
                    {
                        return Err(Error::DuplicateEnum {
                            offset,
                            name: enumeration.name,
                        });
                    }
                    for variant in &enumeration.variants {
                        if !variant_names.insert(variant.name.clone()) {
                            return Err(Error::DuplicateVariant {
                                offset: variant.span.expect("source enum variant has a span").start,
                                name: variant.name.clone(),
                            });
                        }
                    }
                    enums.push(enumeration);
                }
                Kind::Name(name) if name == "comptime" => comptime.push(parser.comptime()?),
                Kind::Name(name) if name == "import" => {
                    let offset = parser.token().offset;
                    let import = parser.import()?;
                    if !import_aliases.insert(import.alias.clone()) {
                        return Err(Error::DuplicateImportAlias {
                            offset,
                            name: import.alias,
                        });
                    }
                    imports.push(import);
                }
                Kind::Name(name) if name == "dart" => {
                    dart_libraries.push(parser.dart_library()?);
                }
                _ => {
                    return Err(Error::Expected {
                        offset: parser.token().offset,
                        expected: "top-level declaration".into(),
                    });
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            diagnostics.push(error);
            if diagnostics.len() == 50 {
                break;
            }
            parser.recover_top_level(declaration_start);
        }
    }
    if diagnostics.is_empty() {
        attach_json_call_generators(&imports, &structs, &functions, &mut comptime);
    }
    if diagnostics.is_empty()
        && let Err(error) = attach_json_stdlib(&mut structs, &mut functions, &mut comptime)
    {
        diagnostics.push(error);
    }
    structs.extend(comptime_ir_structs());
    if diagnostics.is_empty()
        && let Err(error) = validate(
            &imports,
            &structs,
            &enums,
            &dart_libraries,
            &comptime,
            &functions,
        )
    {
        diagnostics.push(error);
    }
    ParseOutput {
        program: Some(Program {
            imports,
            dart_libraries,
            structs,
            enums,
            comptime,
            functions,
        }),
        diagnostics,
    }
}

fn attach_json_call_generators(
    imports: &[Import],
    structs: &[Struct],
    functions: &[Function],
    comptime: &mut Vec<ComptimeDirective>,
) {
    let json_aliases = imports
        .iter()
        .filter(|import| matches!(import.path.as_str(), "std/json" | "std/json.oche"))
        .map(|import| import.alias.as_str())
        .collect::<HashSet<_>>();
    if json_aliases.is_empty() {
        return;
    }
    let mut requested = comptime
        .iter()
        .filter_map(|directive| match &directive.kind {
            ComptimeKind::Generate { function } => {
                Some((function.clone(), directive.target.clone()))
            }
            ComptimeKind::Validate(_) => None,
        })
        .collect::<HashSet<_>>();
    for function in functions {
        collect_json_call_generators(&function.body, &json_aliases, &mut requested, comptime);
    }
    let direct_requests = comptime
        .iter()
        .filter_map(|directive| match &directive.kind {
            ComptimeKind::Generate { function }
                if matches!(
                    function.as_str(),
                    "_oche_generate_json_encode" | "_oche_generate_json_decode"
                ) =>
            {
                Some((function.clone(), directive.target.clone(), directive.span))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (generator, target, span) in direct_requests {
        let mut dependencies = Vec::new();
        collect_json_struct_dependencies(
            &Type::Named {
                name: target.clone(),
                arguments: Vec::new(),
            },
            structs,
            &mut HashSet::new(),
            &mut dependencies,
        );
        for dependency in dependencies {
            if dependency == target {
                continue;
            }
            let key = (generator.clone(), dependency.clone());
            if requested.insert(key) {
                comptime.push(ComptimeDirective {
                    span,
                    target: dependency,
                    kind: ComptimeKind::Generate {
                        function: generator.clone(),
                    },
                });
            }
        }
    }
}

fn collect_json_struct_dependencies(
    ty: &Type,
    structs: &[Struct],
    visited: &mut HashSet<String>,
    dependencies: &mut Vec<String>,
) {
    let Type::Named { name, arguments } = ty;
    for argument in arguments {
        collect_json_struct_dependencies(argument, structs, visited, dependencies);
    }
    let Some(structure) = structs.iter().find(|structure| structure.name == *name) else {
        return;
    };
    if !visited.insert(name.clone()) {
        return;
    }
    dependencies.push(name.clone());
    for field in &structure.fields {
        collect_json_struct_dependencies(&field.ty, structs, visited, dependencies);
    }
}

fn collect_json_call_generators(
    expression: &Expr,
    aliases: &HashSet<&str>,
    requested: &mut HashSet<(String, String)>,
    comptime: &mut Vec<ComptimeDirective>,
) {
    match expression.unlocated() {
        Expr::QualifiedCall {
            module,
            name,
            arguments: call_arguments,
        } => {
            request_json_generator(
                expression,
                module,
                name,
                call_arguments,
                aliases,
                requested,
                comptime,
            );
            for argument in call_arguments {
                collect_json_call_generators(argument, aliases, requested, comptime);
            }
        }
        Expr::Call { arguments, .. } => {
            for argument in arguments {
                collect_json_call_generators(argument, aliases, requested, comptime);
            }
        }
        Expr::DotCall {
            receiver,
            name,
            arguments,
        } => {
            if let Expr::Variable(module) = receiver.unlocated() {
                request_json_generator(
                    expression, module, name, arguments, aliases, requested, comptime,
                );
            }
            collect_json_call_generators(receiver, aliases, requested, comptime);
            for argument in arguments {
                collect_json_call_generators(argument, aliases, requested, comptime);
            }
        }
        Expr::List(values) => {
            for value in values {
                collect_json_call_generators(value, aliases, requested, comptime);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_json_call_generators(value, aliases, requested, comptime);
            }
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value)
        | Expr::Propagate(value) => {
            collect_json_call_generators(value, aliases, requested, comptime);
        }
        Expr::Binary { left, right, .. } => {
            collect_json_call_generators(left, aliases, requested, comptime);
            collect_json_call_generators(right, aliases, requested, comptime);
        }
        Expr::Match { value, arms, .. } => {
            collect_json_call_generators(value, aliases, requested, comptime);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_json_call_generators(guard, aliases, requested, comptime);
                }
                collect_json_call_generators(&arm.body, aliases, requested, comptime);
            }
        }
        Expr::Block(block) | Expr::Try(block) => {
            for statement in &block.statements {
                let value = match statement {
                    Statement::Let { value, .. }
                    | Statement::Assign { value, .. }
                    | Statement::Expression(value) => value,
                };
                collect_json_call_generators(value, aliases, requested, comptime);
            }
            collect_json_call_generators(&block.tail, aliases, requested, comptime);
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_)
        | Expr::Located { .. } => {}
    }
}

fn request_json_generator(
    expression: &Expr,
    module: &str,
    name: &str,
    arguments: &[Expr],
    aliases: &HashSet<&str>,
    requested: &mut HashSet<(String, String)>,
    comptime: &mut Vec<ComptimeDirective>,
) {
    let generator = match name {
        "encode" | "toValue" => Some("_oche_generate_json_encode"),
        "decode" | "fromValue" => Some("_oche_generate_json_decode"),
        _ => None,
    };
    if aliases.contains(module)
        && let Some(generator) = generator
        && let Some(Expr::TypeValue(Type::Named {
            name: target,
            arguments: type_arguments,
        })) = arguments.first().map(Expr::unlocated)
        && type_arguments.is_empty()
        && !matches!(
            target.as_str(),
            "Int" | "Float" | "Bool" | "String" | "JsonValue" | "JsonObject"
        )
    {
        let key = (generator.to_owned(), target.clone());
        if requested.insert(key) {
            comptime.push(ComptimeDirective {
                span: expression
                    .span()
                    .unwrap_or(function_span_fallback(expression)),
                target: target.clone(),
                kind: ComptimeKind::Generate {
                    function: generator.into(),
                },
            });
        }
    }
}

fn function_span_fallback(expression: &Expr) -> SourceSpan {
    SourceSpan::point(expression.offset().unwrap_or(0))
}

fn attach_json_stdlib(
    structs: &mut Vec<Struct>,
    functions: &mut Vec<Function>,
    comptime: &mut [ComptimeDirective],
) -> Result<(), Error> {
    let Some(offset) = comptime.iter().find_map(|directive| {
        matches!(
            &directive.kind,
            ComptimeKind::Generate { function }
                if function == "_oche_generate_json_encode"
                    || function == "_oche_generate_json_decode"
        )
        .then_some(directive.span.start)
    }) else {
        return Ok(());
    };
    let mut library = parse(include_str!("../../../stdlib/json.oche"))
        .expect("the bundled JSON comptime library must parse");
    library
        .structs
        .retain(|structure| !crate::is_comptime_ir_type(&structure.name));
    if let Some(structure) = library
        .structs
        .iter()
        .find(|library| structs.iter().any(|user| user.name == library.name))
    {
        return Err(Error::DuplicateStruct {
            offset,
            name: structure.name.clone(),
        });
    }
    if let Some(function) = library
        .functions
        .iter()
        .find(|library| functions.iter().any(|user| user.name == library.name))
    {
        return Err(Error::DuplicateFunction {
            offset,
            name: function.name.clone(),
        });
    }
    structs.extend(library.structs);
    functions.extend(library.functions);
    Ok(())
}

pub fn parse(source: &str) -> Result<Program, Error> {
    let mut output = parse_recovering(source);
    if !output.diagnostics.is_empty() {
        return Err(output.diagnostics.remove(0));
    }
    Ok(output
        .program
        .expect("a successfully lexed source always produces a program"))
}

/// Resolve calls through imported module aliases after declaration files have been linked.
pub fn validate_linked(program: &Program) -> Result<(), Error> {
    fn builtin_function_arity(
        program: &Program,
        module: &str,
        name: &str,
    ) -> Option<Option<usize>> {
        let import = program
            .imports
            .iter()
            .find(|import| import.alias == module)?;
        match import.path.as_str() {
            "std/json" | "std/json.oche" => Some(match name {
                "parse" | "stringify" | "array" | "fromInt" | "fromFloat" | "fromBool"
                | "fromString" | "asObject" | "asArray" | "asInt" | "asFloat" | "asBool"
                | "asString" => Some(1),
                "object" | "field" | "encode" | "decode" | "toValue" | "fromValue" => Some(2),
                _ => None,
            }),
            "std/isolate" | "std/isolate.oche" => Some((name == "run").then_some(1)),
            _ => None,
        }
    }

    fn expression(program: &Program, value: &Expr, offset: usize) -> Result<(), Error> {
        match value {
            Expr::Located { span, value } => expression(program, value, span.start)?,
            Expr::QualifiedCall {
                module,
                name,
                arguments,
            } => {
                if let Some(expected) = builtin_function_arity(program, module, name) {
                    let expected = expected.ok_or_else(|| Error::UnknownModuleFunction {
                        offset,
                        module: module.clone(),
                        name: name.clone(),
                    })?;
                    if arguments.len() != expected {
                        return Err(Error::WrongArgumentCount {
                            offset,
                            name: format!("{module}.{name}"),
                            expected,
                            actual: arguments.len(),
                        });
                    }
                    for argument in arguments {
                        expression(program, argument, offset)?;
                    }
                    return Ok(());
                }
                let library = program
                    .dart_libraries
                    .iter()
                    .find(|library| library.namespace == *module)
                    .ok_or_else(|| Error::UnknownModule {
                        offset,
                        name: module.clone(),
                    })?;
                let function = library
                    .functions
                    .iter()
                    .find(|function| function.name == *name)
                    .ok_or_else(|| Error::UnknownModuleFunction {
                        offset,
                        module: module.clone(),
                        name: name.clone(),
                    })?;
                if arguments.len() != function.parameters.len() {
                    return Err(Error::WrongArgumentCount {
                        offset,
                        name: format!("{module}.{name}"),
                        expected: function.parameters.len(),
                        actual: arguments.len(),
                    });
                }
                for argument in arguments {
                    expression(program, argument, offset)?;
                }
            }
            Expr::Call { arguments, .. } => {
                for argument in arguments {
                    expression(program, argument, offset)?;
                }
            }
            Expr::DotCall {
                receiver,
                name,
                arguments,
            } => {
                if let Expr::Variable(module) = receiver.unlocated()
                    && let Some(expected) = builtin_function_arity(program, module, name)
                {
                    let expected = expected.ok_or_else(|| Error::UnknownModuleFunction {
                        offset,
                        module: module.clone(),
                        name: name.clone(),
                    })?;
                    if arguments.len() != expected {
                        return Err(Error::WrongArgumentCount {
                            offset,
                            name: format!("{module}.{name}"),
                            expected,
                            actual: arguments.len(),
                        });
                    }
                } else if let Expr::Variable(module) = receiver.unlocated()
                    && let Some(library) = program
                        .dart_libraries
                        .iter()
                        .find(|library| library.namespace == *module)
                {
                    let function = library
                        .functions
                        .iter()
                        .find(|function| function.name == *name)
                        .ok_or_else(|| Error::UnknownModuleFunction {
                            offset,
                            module: module.clone(),
                            name: name.clone(),
                        })?;
                    if arguments.len() != function.parameters.len() {
                        return Err(Error::WrongArgumentCount {
                            offset,
                            name: format!("{module}.{name}"),
                            expected: function.parameters.len(),
                            actual: arguments.len(),
                        });
                    }
                } else {
                    expression(program, receiver, offset)?;
                }
                for argument in arguments {
                    expression(program, argument, offset)?;
                }
            }
            Expr::List(values) => {
                for value in values {
                    expression(program, value, offset)?;
                }
            }
            Expr::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    expression(program, value, offset)?;
                }
            }
            Expr::Field { value, .. }
            | Expr::Negate(value)
            | Expr::Await(value)
            | Expr::Comptime(value)
            | Expr::Propagate(value) => {
                expression(program, value, offset)?;
            }
            Expr::Binary { left, right, .. } => {
                expression(program, left, offset)?;
                expression(program, right, offset)?;
            }
            Expr::Match { value, arms, .. } => {
                expression(program, value, offset)?;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        expression(program, guard, arm.offset)?;
                    }
                    expression(program, &arm.body, arm.offset)?;
                }
            }
            Expr::Block(block) | Expr::Try(block) => {
                for statement in &block.statements {
                    let value = match statement {
                        Statement::Let { value, .. }
                        | Statement::Assign { value, .. }
                        | Statement::Expression(value) => value,
                    };
                    expression(program, value, offset)?;
                }
                expression(program, &block.tail, offset)?;
            }
            Expr::Integer(_)
            | Expr::Float(_)
            | Expr::Boolean(_)
            | Expr::String(_)
            | Expr::TypeValue(_)
            | Expr::Variable(_) => {}
        }
        Ok(())
    }

    for function in &program.functions {
        expression(program, &function.body, function.span.start)?;
    }
    Ok(())
}

fn builtin_enums() -> Vec<Enum> {
    let generic = |name: &str| Type::Named {
        name: name.into(),
        arguments: Vec::new(),
    };
    vec![
        Enum {
            span: None,
            name: "Option".into(),
            type_parameters: vec!["T".into()],
            variants: vec![
                EnumVariant {
                    span: None,
                    name: "Some".into(),
                    fields: vec![generic("T")],
                },
                EnumVariant {
                    span: None,
                    name: "None".into(),
                    fields: Vec::new(),
                },
            ],
        },
        Enum {
            span: None,
            name: "Result".into(),
            type_parameters: vec!["T".into(), "E".into()],
            variants: vec![
                EnumVariant {
                    span: None,
                    name: "Ok".into(),
                    fields: vec![generic("T")],
                },
                EnumVariant {
                    span: None,
                    name: "Err".into(),
                    fields: vec![generic("E")],
                },
            ],
        },
    ]
}

fn comptime_ir_structs() -> Vec<Struct> {
    fn named(name: &str) -> Type {
        Type::Named {
            name: name.into(),
            arguments: Vec::new(),
        }
    }
    fn list(element: Type) -> Type {
        Type::Named {
            name: "List".into(),
            arguments: vec![element],
        }
    }
    fn structure(name: &str, fields: &[(&str, Type)]) -> Struct {
        Struct {
            span: SourceSpan::point(0),
            name: name.into(),
            fields: fields
                .iter()
                .map(|(name, ty)| Field {
                    span: SourceSpan::point(0),
                    name: (*name).into(),
                    ty: ty.clone(),
                })
                .collect(),
        }
    }
    vec![
        structure(
            "ComptimeDeclarations",
            &[("functions", list(named("ComptimeFunction")))],
        ),
        structure(
            "ComptimeFunction",
            &[
                ("name", named("String")),
                ("parameters", list(named("ComptimeParameter"))),
                ("result", named("Type")),
                ("expressions", list(named("ComptimeExpression"))),
                ("root", named("Int")),
            ],
        ),
        structure(
            "ComptimeParameter",
            &[("name", named("String")), ("ty", named("Type"))],
        ),
        structure(
            "ComptimeExpression",
            &[
                ("kind", named("String")),
                ("text", named("String")),
                ("ty", named("Type")),
                ("children", list(named("Int"))),
            ],
        ),
    ]
}

fn validate(
    imports: &[Import],
    structs: &[Struct],
    enums: &[Enum],
    dart_libraries: &[DartLibrary],
    comptime: &[ComptimeDirective],
    functions: &[Function],
) -> Result<(), Error> {
    let type_arities = structs
        .iter()
        .map(|structure| (structure.name.as_str(), 0))
        .chain(
            enums
                .iter()
                .map(|enumeration| (enumeration.name.as_str(), enumeration.type_parameters.len())),
        )
        .chain(std::iter::once(("Future", 1)))
        .chain(std::iter::once(("List", 1)))
        .collect::<HashMap<_, _>>();
    for structure in structs {
        for field in &structure.fields {
            validate_type(&field.ty, field.span.start, &type_arities, &HashSet::new())?;
        }
    }
    for enumeration in enums {
        let parameters = enumeration
            .type_parameters
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        for variant in &enumeration.variants {
            for field in &variant.fields {
                validate_type(
                    field,
                    variant.span.map_or(0, |span| span.start),
                    &type_arities,
                    &parameters,
                )?;
            }
        }
    }
    for library in dart_libraries {
        for function in &library.functions {
            for parameter in &function.parameters {
                validate_type(
                    &parameter.ty,
                    parameter.span.start,
                    &type_arities,
                    &HashSet::new(),
                )?;
            }
            validate_type(
                &function.result,
                function.span.start,
                &type_arities,
                &HashSet::new(),
            )?;
        }
    }
    let mut signatures = functions
        .iter()
        .map(|function| (function.name.clone(), function.parameters.len()))
        .collect::<HashMap<_, _>>();
    let mut return_types = functions
        .iter()
        .map(|function| (function.name.clone(), function.result.clone()))
        .collect::<HashMap<_, _>>();
    for name in crate::INTRINSIC_FUNCTIONS {
        let (parameters, result) =
            crate::intrinsic_signature(name).expect("declared intrinsic has a signature");
        signatures.insert(name.into(), parameters.len());
        return_types.insert(name.into(), result);
    }
    for enumeration in enums {
        for variant in &enumeration.variants {
            signatures.insert(variant.name.clone(), variant.fields.len());
        }
    }
    let import_aliases = imports
        .iter()
        .map(|import| import.alias.as_str())
        .collect::<HashSet<_>>();
    for directive in comptime {
        match directive.kind {
            ComptimeKind::Validate(_) => {
                signatures.insert(format!("{}_validate", directive.target), 1);
                if let Some(structure) = structs
                    .iter()
                    .find(|structure| structure.name == directive.target)
                {
                    return_types.insert(
                        format!("{}_validate", directive.target),
                        result_type(&structure.name),
                    );
                }
            }
            ComptimeKind::Generate { .. } => {
                validate_type(
                    &Type::Named {
                        name: directive.target.clone(),
                        arguments: Vec::new(),
                    },
                    directive.span.start,
                    &type_arities,
                    &HashSet::new(),
                )?;
            }
        }
    }
    if comptime
        .iter()
        .any(|directive| matches!(directive.kind, ComptimeKind::Generate { .. }))
    {
        signatures.insert("*generated*".into(), usize::MAX);
    }
    for function in functions {
        for parameter in &function.parameters {
            validate_type(
                &parameter.ty,
                parameter.span.start,
                &type_arities,
                &HashSet::new(),
            )?;
        }
        validate_type(
            &function.result,
            function.span.start,
            &type_arities,
            &HashSet::new(),
        )?;
        validate_struct_usage(&function.body, structs, function.span.start)?;
        let mut scope = function
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>();
        validate_expression(
            &function.body,
            &signatures,
            enums,
            &import_aliases,
            &mut scope,
            function.span.start,
        )?;
        validate_propagation(
            &function.body,
            &return_types,
            Some(function.result.name()),
            function.span.start,
        )?;
        let mut bindings = function
            .parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), false))
            .collect::<Vec<_>>();
        validate_mutation(&function.body, &mut bindings, function.span.start)?;
        if !function.is_async
            && let Some(await_span) = function.body.first_await_span()
        {
            return Err(Error::AwaitOutsideAsync {
                offset: await_span.start,
            });
        }
    }
    Ok(())
}

fn validate_mutation(
    expression: &Expr,
    bindings: &mut Vec<(String, bool)>,
    offset: usize,
) -> Result<(), Error> {
    macro_rules! visit {
        ($value:expr, $at:expr) => {
            validate_mutation($value, bindings, $at)
        };
    }
    match expression {
        Expr::Located { span, value } => visit!(value, span.start),
        Expr::Block(block) | Expr::Try(block) => {
            let original = bindings.len();
            for statement in &block.statements {
                match statement {
                    Statement::Let {
                        mutable,
                        pattern,
                        value,
                    } => {
                        visit!(value, offset)?;
                        if let Pattern::Binding(name) = pattern.unlocated() {
                            bindings.push((name.clone(), *mutable));
                        }
                    }
                    Statement::Assign { name, value } => {
                        let mutable = bindings
                            .iter()
                            .rev()
                            .find(|(binding, _)| binding == name)
                            .is_some_and(|(_, mutable)| *mutable);
                        if !mutable {
                            return Err(Error::ImmutableAssignment {
                                offset,
                                name: name.clone(),
                            });
                        }
                        visit!(value, offset)?;
                    }
                    Statement::Expression(value) => visit!(value, offset)?,
                }
            }
            visit!(&block.tail, offset)?;
            bindings.truncate(original);
            Ok(())
        }
        Expr::List(values)
        | Expr::Call {
            arguments: values, ..
        } => {
            for value in values {
                visit!(value, offset)?;
            }
            Ok(())
        }
        Expr::QualifiedCall { arguments, .. } => {
            for value in arguments {
                visit!(value, offset)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            visit!(receiver, offset)?;
            for value in arguments {
                visit!(value, offset)?;
            }
            Ok(())
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                visit!(value, offset)?;
            }
            Ok(())
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value)
        | Expr::Propagate(value) => visit!(value, offset),
        Expr::Binary { left, right, .. } => {
            visit!(left, offset)?;
            visit!(right, offset)
        }
        Expr::Match { value, arms, .. } => {
            visit!(value, offset)?;
            for arm in arms {
                let original = bindings.len();
                let mut names = Vec::new();
                collect_pattern_names(&arm.pattern, &mut names);
                bindings.extend(names.into_iter().map(|name| (name, false)));
                if let Some(guard) = &arm.guard {
                    visit!(guard, arm.offset)?;
                }
                visit!(&arm.body, arm.offset)?;
                bindings.truncate(original);
            }
            Ok(())
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => Ok(()),
    }
}

fn collect_pattern_names(pattern: &Pattern, names: &mut Vec<String>) {
    match pattern.unlocated() {
        Pattern::Binding(name) => names.push(name.clone()),
        Pattern::Variant { fields, .. } => {
            for field in fields {
                collect_pattern_names(field, names);
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, pattern) in fields {
                collect_pattern_names(pattern, names);
            }
        }
        _ => {}
    }
}

fn result_type(success: &str) -> Type {
    Type::Named {
        name: "Result".into(),
        arguments: vec![
            Type::Named {
                name: success.into(),
                arguments: Vec::new(),
            },
            Type::Named {
                name: "String".into(),
                arguments: Vec::new(),
            },
        ],
    }
}

fn validate_type(
    ty: &Type,
    offset: usize,
    types: &HashMap<&str, usize>,
    parameters: &HashSet<&str>,
) -> Result<(), Error> {
    let Type::Named { name, arguments } = ty;
    let expected = match name.as_str() {
        "Int" | "Float" | "Bool" | "String" | "Type" | "Field" | "JsonValue" | "JsonObject" => 0,
        _ if parameters.contains(name.as_str()) => 0,
        _ if types.contains_key(name.as_str()) => types[name.as_str()],
        _ => {
            return Err(Error::UnknownType {
                offset,
                name: name.clone(),
            });
        }
    };
    if arguments.len() != expected {
        return Err(Error::WrongTypeArgumentCount {
            offset,
            name: name.clone(),
            expected,
            actual: arguments.len(),
        });
    }
    for argument in arguments {
        validate_type(argument, offset, types, parameters)?;
    }
    Ok(())
}

fn validate_expression(
    expression: &Expr,
    functions: &HashMap<String, usize>,
    enums: &[Enum],
    imports: &HashSet<&str>,
    scope: &mut Vec<String>,
    offset: usize,
) -> Result<(), Error> {
    match expression {
        Expr::Located { span, value } => {
            validate_expression(value, functions, enums, imports, scope, span.start)
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_) => Ok(()),
        Expr::List(values) => {
            for value in values {
                validate_expression(value, functions, enums, imports, scope, offset)?;
            }
            Ok(())
        }
        Expr::Variable(name) => {
            if scope.contains(name) {
                Ok(())
            } else {
                Err(Error::UnknownVariable {
                    offset,
                    name: name.clone(),
                })
            }
        }
        Expr::Call { callee, arguments } => {
            let expected = match callee.as_str() {
                "Some" | "Ok" | "Err" => Some(1),
                "None" => Some(0),
                _ => functions.get(callee.as_str()).copied(),
            };
            let Some(expected) = expected else {
                if functions.contains_key("*generated*") {
                    for argument in arguments {
                        validate_expression(argument, functions, enums, imports, scope, offset)?;
                    }
                    return Ok(());
                }
                return Err(Error::UnknownFunction {
                    offset,
                    name: callee.clone(),
                });
            };
            if arguments.len() != expected {
                return Err(Error::WrongArgumentCount {
                    offset,
                    name: callee.clone(),
                    expected,
                    actual: arguments.len(),
                });
            }
            for argument in arguments {
                validate_expression(argument, functions, enums, imports, scope, offset)?;
            }
            Ok(())
        }
        Expr::QualifiedCall {
            module, arguments, ..
        } => {
            if !imports.contains(module.as_str()) {
                return Err(Error::UnknownModule {
                    offset,
                    name: module.clone(),
                });
            }
            for argument in arguments {
                validate_expression(argument, functions, enums, imports, scope, offset)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            let namespace_receiver = matches!(
                receiver.unlocated(),
                Expr::Variable(name)
                    if imports.contains(name.as_str())
                        || name.chars().next().is_some_and(char::is_uppercase)
            );
            if !namespace_receiver {
                validate_expression(receiver, functions, enums, imports, scope, offset)?;
            }
            for argument in arguments {
                validate_expression(argument, functions, enums, imports, scope, offset)?;
            }
            Ok(())
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_expression(value, functions, enums, imports, scope, offset)?;
            }
            Ok(())
        }
        Expr::Field { value, .. } => {
            validate_expression(value, functions, enums, imports, scope, offset)
        }
        Expr::Negate(operand)
        | Expr::Await(operand)
        | Expr::Comptime(operand)
        | Expr::Propagate(operand) => {
            validate_expression(operand, functions, enums, imports, scope, offset)
        }
        Expr::Binary { left, right, .. } => {
            validate_expression(left, functions, enums, imports, scope, offset)?;
            validate_expression(right, functions, enums, imports, scope, offset)
        }
        Expr::Match { value, arms, .. } => {
            validate_expression(value, functions, enums, imports, scope, offset)?;
            for arm in arms {
                let original_length = scope.len();
                collect_bindings(&arm.pattern, enums, scope, arm.offset)?;
                if let Some(guard) = &arm.guard {
                    validate_expression(guard, functions, enums, imports, scope, arm.offset)?;
                }
                validate_expression(&arm.body, functions, enums, imports, scope, offset)?;
                scope.truncate(original_length);
            }
            Ok(())
        }
        Expr::Block(block) | Expr::Try(block) => {
            let original_length = scope.len();
            for statement in &block.statements {
                match statement {
                    Statement::Let { pattern, value, .. } => {
                        validate_expression(value, functions, enums, imports, scope, offset)?;
                        collect_bindings(pattern, enums, scope, offset)?;
                    }
                    Statement::Assign { name, value } => {
                        if !scope.contains(name) {
                            return Err(Error::UnknownVariable {
                                offset,
                                name: name.clone(),
                            });
                        }
                        validate_expression(value, functions, enums, imports, scope, offset)?;
                    }
                    Statement::Expression(expression) => {
                        validate_expression(expression, functions, enums, imports, scope, offset)?;
                    }
                }
            }
            validate_expression(&block.tail, functions, enums, imports, scope, offset)?;
            scope.truncate(original_length);
            Ok(())
        }
    }
}

fn collect_bindings(
    pattern: &Pattern,
    enums: &[Enum],
    scope: &mut Vec<String>,
    offset: usize,
) -> Result<(), Error> {
    match pattern {
        Pattern::Located { span, value } => collect_bindings(value, enums, scope, span.start),
        Pattern::Wildcard
        | Pattern::Integer(_)
        | Pattern::Float(_)
        | Pattern::Boolean(_)
        | Pattern::String(_) => Ok(()),
        Pattern::Binding(name) => {
            if scope.contains(name) {
                return Err(Error::DuplicatePatternBinding {
                    offset,
                    name: name.clone(),
                });
            }
            scope.push(name.clone());
            Ok(())
        }
        Pattern::Variant { name, fields } => {
            let expected = enums
                .iter()
                .flat_map(|enumeration| &enumeration.variants)
                .find(|variant| variant.name == *name)
                .map(|variant| variant.fields.len())
                .ok_or_else(|| Error::UnknownPatternVariant {
                    offset,
                    name: name.clone(),
                })?;
            if fields.len() != expected {
                return Err(Error::WrongArgumentCount {
                    offset,
                    name: name.clone(),
                    expected,
                    actual: fields.len(),
                });
            }
            for field in fields {
                collect_bindings(field, enums, scope, offset)?;
            }
            Ok(())
        }
        Pattern::Struct { fields, .. } => {
            for (_, field) in fields {
                collect_bindings(field, enums, scope, offset)?;
            }
            Ok(())
        }
    }
}

fn validate_struct_usage(
    expression: &Expr,
    structs: &[Struct],
    offset: usize,
) -> Result<(), Error> {
    fn validate_pattern(pattern: &Pattern, structs: &[Struct], offset: usize) -> Result<(), Error> {
        match pattern {
            Pattern::Located { span, value } => validate_pattern(value, structs, span.start),
            Pattern::Struct { name, fields, rest } => {
                let structure = structs
                    .iter()
                    .find(|structure| structure.name == *name)
                    .ok_or_else(|| Error::UnknownType {
                        offset,
                        name: name.clone(),
                    })?;
                let mut seen = HashSet::new();
                for (field, pattern) in fields {
                    if !seen.insert(field.as_str()) {
                        return Err(Error::DuplicateField {
                            offset,
                            name: field.clone(),
                        });
                    }
                    if !structure.fields.iter().any(|item| item.name == *field) {
                        return Err(Error::UnknownStructField {
                            offset,
                            name: name.clone(),
                            field: field.clone(),
                        });
                    }
                    validate_pattern(pattern, structs, offset)?;
                }
                if !rest {
                    let missing = structure
                        .fields
                        .iter()
                        .filter(|field| !seen.contains(field.name.as_str()))
                        .map(|field| field.name.clone())
                        .collect::<Vec<_>>();
                    if !missing.is_empty() {
                        return Err(Error::MissingStructFields {
                            offset,
                            name: name.clone(),
                            fields: missing.join(", "),
                        });
                    }
                }
                Ok(())
            }
            Pattern::Variant { fields, .. } => {
                for field in fields {
                    validate_pattern(field, structs, offset)?;
                }
                Ok(())
            }
            Pattern::Wildcard
            | Pattern::Binding(_)
            | Pattern::Integer(_)
            | Pattern::Float(_)
            | Pattern::Boolean(_)
            | Pattern::String(_) => Ok(()),
        }
    }

    match expression {
        Expr::Located { span, value } => validate_struct_usage(value, structs, span.start),
        Expr::StructLiteral { name, fields } => {
            let structure = structs
                .iter()
                .find(|structure| structure.name == *name)
                .ok_or_else(|| Error::UnknownType {
                    offset,
                    name: name.clone(),
                })?;
            for (field, value) in fields {
                if !structure.fields.iter().any(|item| item.name == *field) {
                    return Err(Error::UnknownStructField {
                        offset,
                        name: name.clone(),
                        field: field.clone(),
                    });
                }
                validate_struct_usage(value, structs, offset)?;
            }
            let missing = structure
                .fields
                .iter()
                .filter(|field| !fields.iter().any(|(name, _)| *name == field.name))
                .map(|field| field.name.clone())
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(Error::MissingStructFields {
                    offset,
                    name: name.clone(),
                    fields: missing.join(", "),
                });
            }
            Ok(())
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value)
        | Expr::Propagate(value) => validate_struct_usage(value, structs, offset),
        Expr::Call { callee, arguments } => {
            if structs.iter().any(|structure| structure.name == *callee) {
                return Err(Error::PositionalStructConstruction {
                    offset,
                    name: callee.clone(),
                });
            }
            for argument in arguments {
                validate_struct_usage(argument, structs, offset)?;
            }
            Ok(())
        }
        Expr::QualifiedCall { arguments, .. } => {
            for argument in arguments {
                validate_struct_usage(argument, structs, offset)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            validate_struct_usage(receiver, structs, offset)?;
            for argument in arguments {
                validate_struct_usage(argument, structs, offset)?;
            }
            Ok(())
        }
        Expr::List(values) => {
            for value in values {
                validate_struct_usage(value, structs, offset)?;
            }
            Ok(())
        }
        Expr::Binary { left, right, .. } => {
            validate_struct_usage(left, structs, offset)?;
            validate_struct_usage(right, structs, offset)
        }
        Expr::Match { value, arms, .. } => {
            validate_struct_usage(value, structs, offset)?;
            for arm in arms {
                validate_pattern(&arm.pattern, structs, arm.offset)?;
                if let Some(guard) = &arm.guard {
                    validate_struct_usage(guard, structs, arm.offset)?;
                }
                validate_struct_usage(&arm.body, structs, offset)?;
            }
            Ok(())
        }
        Expr::Block(block) | Expr::Try(block) => {
            for statement in &block.statements {
                match statement {
                    Statement::Let { pattern, value, .. } => {
                        validate_pattern(pattern, structs, offset)?;
                        validate_struct_usage(value, structs, offset)?;
                    }
                    Statement::Assign { value, .. } => {
                        validate_struct_usage(value, structs, offset)?;
                    }
                    Statement::Expression(value) => {
                        validate_struct_usage(value, structs, offset)?;
                    }
                }
            }
            validate_struct_usage(&block.tail, structs, offset)
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => Ok(()),
    }
}

fn validate_propagation(
    expression: &Expr,
    returns: &HashMap<String, Type>,
    expected: Option<&str>,
    offset: usize,
) -> Result<(), Error> {
    match expression {
        Expr::Located { span, value } => validate_propagation(value, returns, expected, span.start),
        Expr::Propagate(value) => {
            if let Some(actual) = propagation_carrier(value, returns)
                && expected != Some(actual)
            {
                return Err(Error::InvalidPropagation {
                    offset,
                    actual: actual.into(),
                    expected: expected.unwrap_or("non-Option/Result function").into(),
                });
            }
            validate_propagation(value, returns, expected, offset)
        }
        Expr::Try(block) => {
            let mut carriers = HashSet::new();
            collect_block_carriers(block, returns, &mut carriers);
            if carriers.len() > 1 {
                return Err(Error::MixedTry { offset });
            }
            let local = carriers.iter().next().copied().unwrap_or("Result");
            validate_block_propagation(block, returns, Some(local), offset)
        }
        Expr::Block(block) => validate_block_propagation(block, returns, expected, offset),
        Expr::Call { arguments, .. } | Expr::QualifiedCall { arguments, .. } => {
            for argument in arguments {
                validate_propagation(argument, returns, expected, offset)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            validate_propagation(receiver, returns, expected, offset)?;
            for argument in arguments {
                validate_propagation(argument, returns, expected, offset)?;
            }
            Ok(())
        }
        Expr::List(values) => {
            for value in values {
                validate_propagation(value, returns, expected, offset)?;
            }
            Ok(())
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_propagation(value, returns, expected, offset)?;
            }
            Ok(())
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value) => validate_propagation(value, returns, expected, offset),
        Expr::Binary { left, right, .. } => {
            validate_propagation(left, returns, expected, offset)?;
            validate_propagation(right, returns, expected, offset)
        }
        Expr::Match { value, arms, .. } => {
            validate_propagation(value, returns, expected, offset)?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    validate_propagation(guard, returns, expected, offset)?;
                }
                validate_propagation(&arm.body, returns, expected, offset)?;
            }
            Ok(())
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => Ok(()),
    }
}

fn validate_block_propagation(
    block: &Block,
    returns: &HashMap<String, Type>,
    expected: Option<&str>,
    offset: usize,
) -> Result<(), Error> {
    for statement in &block.statements {
        let value = match statement {
            Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value) => value,
        };
        validate_propagation(value, returns, expected, offset)?;
    }
    validate_propagation(&block.tail, returns, expected, offset)
}

fn propagation_carrier<'a>(
    expression: &Expr,
    returns: &'a HashMap<String, Type>,
) -> Option<&'a str> {
    let expression = expression.unlocated();
    let expression = match expression {
        Expr::Await(value) => value.unlocated(),
        expression => expression,
    };
    let Expr::Call { callee, .. } = expression else {
        return None;
    };
    let name = returns.get(callee)?.name();
    matches!(name, "Option" | "Result").then_some(name)
}

fn collect_block_carriers<'a>(
    block: &Block,
    returns: &'a HashMap<String, Type>,
    carriers: &mut HashSet<&'a str>,
) {
    for statement in &block.statements {
        let value = match statement {
            Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value) => value,
        };
        collect_expression_carriers(value, returns, carriers);
    }
    collect_expression_carriers(&block.tail, returns, carriers);
}

fn collect_expression_carriers<'a>(
    expression: &Expr,
    returns: &'a HashMap<String, Type>,
    carriers: &mut HashSet<&'a str>,
) {
    match expression {
        Expr::Located { value, .. } => collect_expression_carriers(value, returns, carriers),
        Expr::Propagate(value) => {
            if let Some(carrier) = propagation_carrier(value, returns) {
                carriers.insert(carrier);
            }
        }
        Expr::Try(_) => {}
        Expr::Block(block) => collect_block_carriers(block, returns, carriers),
        Expr::Call { arguments, .. } | Expr::QualifiedCall { arguments, .. } => {
            for argument in arguments {
                collect_expression_carriers(argument, returns, carriers);
            }
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            collect_expression_carriers(receiver, returns, carriers);
            for argument in arguments {
                collect_expression_carriers(argument, returns, carriers);
            }
        }
        Expr::List(values) => {
            for value in values {
                collect_expression_carriers(value, returns, carriers);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expression_carriers(value, returns, carriers);
            }
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value) => {
            collect_expression_carriers(value, returns, carriers);
        }
        Expr::Binary { left, right, .. } => {
            collect_expression_carriers(left, returns, carriers);
            collect_expression_carriers(right, returns, carriers);
        }
        Expr::Match { value, arms, .. } => {
            collect_expression_carriers(value, returns, carriers);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expression_carriers(guard, returns, carriers);
                }
                collect_expression_carriers(&arm.body, returns, carriers);
            }
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => {}
    }
}

fn block_contains_propagate(block: &Block) -> bool {
    block.statements.iter().any(|statement| match statement {
        Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value) => expression_contains_propagate(value),
    }) || expression_contains_propagate(&block.tail)
}

fn expression_contains_propagate(expression: &Expr) -> bool {
    match expression {
        Expr::Located { value, .. } => expression_contains_propagate(value),
        Expr::Propagate(_) => true,
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Comptime(value) => expression_contains_propagate(value),
        Expr::Binary { left, right, .. } => {
            expression_contains_propagate(left) || expression_contains_propagate(right)
        }
        Expr::Call { arguments, .. } | Expr::QualifiedCall { arguments, .. } => {
            arguments.iter().any(expression_contains_propagate)
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            expression_contains_propagate(receiver)
                || arguments.iter().any(expression_contains_propagate)
        }
        Expr::List(values) => values.iter().any(expression_contains_propagate),
        Expr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| expression_contains_propagate(value)),
        Expr::Match { value, arms, .. } => {
            expression_contains_propagate(value)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(expression_contains_propagate)
                        || expression_contains_propagate(&arm.body)
                })
        }
        Expr::Block(block) => block_contains_propagate(block),
        Expr::Try(_) => false,
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_function_calls() {
        let source = "fn twice(a: Int) -> Int { a * 2 } fn f(a: Int) -> Int { twice(a - 2) + 3 }";
        let parsed = parse(source).unwrap();
        assert!(matches!(
            parsed.functions[1].body.unlocated(),
            Expr::Binary {
                operator: Operator::Add,
                ..
            }
        ));
        let body = "twice(a - 2) + 3";
        assert_eq!(
            parsed.functions[1].body.span(),
            Some(SourceSpan {
                start: source.find(body).unwrap(),
                end: source.find(body).unwrap() + body.len(),
            })
        );

        let parenthesized = "fn f(a: Int) -> Int { (a + 1) }";
        let parsed = parse(parenthesized).unwrap();
        let body = "(a + 1)";
        assert_eq!(
            parsed.functions[0].body.span(),
            Some(SourceSpan {
                start: parenthesized.find(body).unwrap(),
                end: parenthesized.find(body).unwrap() + body.len(),
            })
        );
    }

    #[test]
    fn pipes_values_into_the_first_call_argument() {
        let source = "fn add(a: Int, b: Int) -> Int { a + b } fn value() -> Int { 1 + 2 |> add(3) |> add(4) }";
        let parsed = parse(source).unwrap();
        let Expr::Call { callee, arguments } = parsed.functions[1].body.unlocated() else {
            panic!("expected the outer pipe to produce a call");
        };
        assert_eq!(callee, "add");
        assert!(matches!(arguments[1].unlocated(), Expr::Integer(4)));
        assert!(matches!(
            arguments[0].unlocated(),
            Expr::Call { callee, arguments }
                if callee == "add"
                    && matches!(arguments[0].unlocated(), Expr::Binary { operator: Operator::Add, .. })
                    && matches!(arguments[1].unlocated(), Expr::Integer(3))
        ));
    }

    #[test]
    fn pipes_require_a_call_on_the_right() {
        let error = parse("fn bad(value: Int) -> Int { value |> value }").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Expected function call after '|>'")
        );
    }

    #[test]
    fn pipes_into_module_qualified_calls() {
        let parsed = parse(
            r#"
            import "ast.d.oche" as ast
            fn wrap(value: Int) -> Int { value |> ast.wrap("try") }
            "#,
        )
        .unwrap();
        assert!(matches!(
            parsed.functions[0].body.unlocated(),
            Expr::DotCall { receiver, name, arguments }
                if matches!(receiver.unlocated(), Expr::Variable(module) if module == "ast")
                    && name == "wrap"
                    && arguments.len() == 2
                    && matches!(arguments[0].unlocated(), Expr::Variable(value) if value == "value")
        ));
    }

    #[test]
    fn parses_value_level_comptime_expressions() {
        let source = "const fn capacity() -> Int { 32 * 1024 }\nfn configured() -> Int { comptime capacity() }";
        let parsed = parse(source).unwrap();
        let Expr::Comptime(value) = parsed.functions[1].body.unlocated() else {
            panic!("expected comptime expression");
        };
        assert!(matches!(
            value.unlocated(),
            Expr::Call { callee, arguments } if callee == "capacity" && arguments.is_empty()
        ));
        assert_eq!(
            parsed.functions[1].body.span(),
            Some(SourceSpan {
                start: source.find("comptime capacity()").unwrap(),
                end: source.find("comptime capacity()").unwrap() + "comptime capacity()".len(),
            })
        );
    }

    #[test]
    fn imports_require_and_preserve_a_namespace() {
        let parsed = parse(
            r#"
            import "tools.d.oche" as tools
            fn greet(name: String) -> String { tools.greet(name) }
            "#,
        )
        .unwrap();
        assert_eq!(parsed.imports[0].path, "tools.d.oche");
        assert_eq!(parsed.imports[0].alias, "tools");
        assert!(matches!(
            parsed.functions[0].body.unlocated(),
            Expr::DotCall { receiver, name, .. }
                if matches!(receiver.unlocated(), Expr::Variable(module) if module == "tools")
                    && name == "greet"
        ));
        assert!(parse("import \"tools.d.oche\"").is_err());
    }

    #[test]
    fn json_namespace_calls_request_only_the_needed_generators() {
        let parsed = parse(
            r#"
            import "std/json" as data
            struct Profile { active: Bool }
            struct User { name: String, profile: Profile }
            fn encode(user: User) -> String { data.encode(type User, user) }
            fn decode(source: String) -> Result<User, String> {
              data.decode(type User, source)
            }
            "#,
        )
        .unwrap();
        let requests = parsed
            .comptime
            .iter()
            .filter_map(|directive| match &directive.kind {
                ComptimeKind::Generate { function } => {
                    Some((function.as_str(), directive.target.as_str()))
                }
                ComptimeKind::Validate(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            requests,
            vec![
                ("_oche_generate_json_encode", "User"),
                ("_oche_generate_json_decode", "User"),
                ("_oche_generate_json_encode", "Profile"),
                ("_oche_generate_json_decode", "Profile"),
            ]
        );
    }

    #[test]
    fn json_namespace_dependency_discovery_handles_recursive_structs() {
        let parsed = parse(
            r#"
            import "std/json" as json
            struct Node { children: List<Node> }
            fn encode(node: Node) -> String { json.encode(type Node, node) }
            "#,
        )
        .unwrap();
        let requests = parsed
            .comptime
            .iter()
            .filter(|directive| matches!(directive.kind, ComptimeKind::Generate { .. }))
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target, "Node");
    }

    #[test]
    fn preserves_top_level_and_member_declaration_spans() {
        let source = r#"struct User { name: String }
enum State { Ready, Data(Int) }
fn greet(user: User) -> String { user.name }
dart "package:tools/tools.dart" { fn echo(value: String) -> String; }
comptime jsonEncode(User)"#;
        let program = parse(source).unwrap();
        let expected = |needle: &str| {
            let start = source.find(needle).unwrap();
            SourceSpan {
                start,
                end: start + needle.len(),
            }
        };
        assert_eq!(
            program.structs[0].span,
            expected("struct User { name: String }")
        );
        assert_eq!(program.structs[0].fields[0].span, expected("name: String"));
        assert_eq!(
            program.enums[2].span,
            Some(expected("enum State { Ready, Data(Int) }"))
        );
        assert_eq!(program.enums[2].variants[0].span, Some(expected("Ready")));
        assert_eq!(
            program.enums[2].variants[1].span,
            Some(expected("Data(Int)"))
        );
        assert_eq!(
            program.comptime[0].span,
            expected("comptime jsonEncode(User)")
        );
        assert_eq!(
            program.functions[0].span,
            expected("fn greet(user: User) -> String { user.name }")
        );
        assert_eq!(
            program.functions[0].parameters[0].span,
            expected("user: User")
        );
        assert_eq!(
            program.dart_libraries[0].span,
            expected("dart \"package:tools/tools.dart\" { fn echo(value: String) -> String; }")
        );
        assert_eq!(
            program.dart_libraries[0].functions[0].span,
            expected("fn echo(value: String) -> String;")
        );
        assert_eq!(
            program.dart_libraries[0].functions[0].parameters[0].span,
            expected("value: String")
        );
    }

    #[test]
    fn option_result_types_and_match_patterns() {
        let parsed = parse(
            r#"
            fn unwrap(value: Option<Int>) -> Int {
              match value { Some(inner) => inner, None => 0 }
            }
            fn result(value: Int) -> Result<Int, String> { Ok(value) }
            "#,
        )
        .unwrap();
        assert!(matches!(
            parsed.functions[0].body.unlocated(),
            Expr::Match { .. }
        ));
        assert_eq!(
            parsed.functions[1].result.to_string(),
            "Result<Int, String>"
        );

        let guarded_source = "fn allowed(value: Int) -> Bool { true }
             fn choose(value: Option<Int>) -> Int {
               match value { Some(inner) if allowed(inner) => inner, Some(_) => 0, None => -1 }
             }";
        let guarded = parse(guarded_source).unwrap();
        let Expr::Match { arms, .. } = guarded.functions[1].body.unlocated() else {
            panic!("expected match expression");
        };
        assert!(arms[0].guard.is_some());
        assert!(arms[1].guard.is_none());
        let pattern = "Some(inner)";
        let start = guarded_source.find(pattern).unwrap();
        assert_eq!(
            arms[0].pattern.span(),
            Some(SourceSpan {
                start,
                end: start + pattern.len(),
            })
        );
        let Pattern::Variant { fields, .. } = arms[0].pattern.unlocated() else {
            panic!("expected variant pattern");
        };
        let inner = guarded_source.find("inner)").unwrap();
        assert_eq!(
            fields[0].span(),
            Some(SourceSpan {
                start: inner,
                end: inner + "inner".len(),
            })
        );
    }

    #[test]
    fn parses_list_types_and_literals() {
        let parsed = parse(
            "fn values() -> List<Int> { [1, 2, 3] }
             fn nested() -> List<List<Int>> { [[1], [2, 3]] }
             fn empty() -> List<String> { [] }",
        )
        .unwrap();
        assert_eq!(parsed.functions[0].result.to_string(), "List<Int>");
        assert!(matches!(
            parsed.functions[1].body.unlocated(),
            Expr::List(values) if values.len() == 2
        ));
        assert!(
            matches!(parsed.functions[2].body.unlocated(), Expr::List(values) if values.is_empty())
        );
        assert!(
            parse("fn bad(value: List) -> Int { 1 }")
                .unwrap_err()
                .to_string()
                .contains("expects 1 type argument")
        );
        let length = parse("fn count(values: List<Int>) -> Int { values.len() }").unwrap();
        assert!(matches!(
            length.functions[0].body.unlocated(),
            Expr::DotCall { name, .. } if name == "len"
        ));
        let strings =
            parse("fn classify(value: String) -> Int { match value { \"agent\" => 1, _ => 0 } }")
                .unwrap();
        assert!(matches!(
            strings.functions[0].body.unlocated(),
            Expr::Match { arms, .. } if matches!(arms[0].pattern.unlocated(), Pattern::String(value) if value == "agent")
        ));
    }

    #[test]
    fn tagged_unions_and_try_blocks() {
        let parsed = parse(
            r#"
            enum Message { Text(String), Stop }
            fn read(value: Int) -> Result<Int, String> { Ok(value) }
            fn combine(value: Int) -> Result<Int, String> {
              try {
                let first = read(value)?;
                first + 1
              }
            }
            fn render(message: Message) -> String {
              match message { Text(value) => value, Stop => "stop" }
            }
            "#,
        )
        .unwrap();
        assert_eq!(parsed.enums[2].name, "Message");
        assert!(matches!(parsed.functions[1].body.unlocated(), Expr::Try(_)));

        let parsed = parse(
            "enum Message { Text(String), Stop } fn f(value: Message) -> String {
               match value { Text(text) => text }
             }",
        )
        .unwrap();
        assert_eq!(parsed.functions.len(), 1);

        let error = parse("fn f() -> Result<Int, String> { try { Ok(1) } }").unwrap_err();
        assert!(error.to_string().contains("at least one '?'"));

        let error = parse(
            "fn result() -> Result<Int, String> { Ok(1) }
             fn option() -> Option<Int> { Some(1) }
             fn mixed() -> Result<Int, String> {
               try { let a = result()?; let b = option()?; a + b }
             }",
        )
        .unwrap_err();
        assert!(error.to_string().contains("cannot mix Option and Result"));
    }

    #[test]
    fn named_struct_literals_fields_and_patterns() {
        let parsed = parse(
            "struct User { name: String, age: Int }
             fn name(input: String) -> String {
               let user = User { age: 37, name: input };
               match user { User { name, .. } => name }
             }
             fn age(input: String) -> Int { User { name: input, age: 37 }.age }",
        )
        .unwrap();
        assert!(matches!(
            parsed.functions[1].body.unlocated(),
            Expr::Field { .. }
        ));

        let error = parse(
            "struct User { name: String, age: Int }
             fn bad() -> User { User { name: \"Ada\" } }",
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing field(s): age"));

        let error = parse(
            "struct User { name: String, age: Int }
             fn user() -> User { User(\"Ada\", 37) }",
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::PositionalStructConstruction { ref name, .. } if name == "User"
        ));
    }

    #[test]
    fn diagnostics_and_integer_boundaries() {
        for (source, message) in [
            ("fn f(a: Int, a: Int) -> Int { a }", "Duplicate parameter"),
            ("fn f() -> Int { missing }", "Unknown variable"),
            (
                "fn f() -> Int { 9223372036854775808 }",
                "outside the Int range",
            ),
            (
                "fn f() -> Int { 1 } fn f() -> Int { 2 }",
                "Duplicate function",
            ),
            ("fn f(x: Option) -> Int { 1 }", "expects 1 type argument"),
            ("fn f() -> Int { nope() }", "Unknown function"),
        ] {
            assert!(parse(source).unwrap_err().to_string().contains(message));
        }
        let parsed = parse("fn f() -> Int { -9223372036854775808 }").unwrap();
        assert_eq!(
            parsed.functions[0].body.unlocated(),
            &Expr::Integer(i64::MIN)
        );

        for (source, needle) in [
            ("fn f() -> Int { missing }", "missing"),
            ("fn f() -> Int { nope() }", "nope"),
            (
                "struct User { name: String } fn f() -> User { User(\"Ada\") }",
                "User(\"Ada\")",
            ),
        ] {
            let error = parse(source).unwrap_err();
            assert_eq!(error.offset(), source.find(needle).unwrap(), "{source}");
        }
    }

    #[test]
    fn json_supports_explicit_comptime_calls() {
        let program = parse(
            "struct User { name: String }
             comptime jsonEncode(User)
             comptime jsonDecode(User)",
        )
        .unwrap();
        assert_eq!(program.comptime.len(), 2);
        assert_eq!(
            program.comptime[0].kind,
            ComptimeKind::Generate {
                function: "_oche_generate_json_encode".into()
            }
        );
        assert_eq!(
            program.comptime[1].kind,
            ComptimeKind::Generate {
                function: "_oche_generate_json_decode".into()
            }
        );
        assert!(
            program
                .functions
                .iter()
                .any(|function| function.name == "_oche_generate_json_encode")
        );

        assert!(parse("#[derive(Json)] struct User { name: String }").is_err());
    }

    #[test]
    fn parses_finite_float_literals_and_patterns() {
        let parsed = parse(
            "fn scale(value: Float) -> Float { value * 1.5e2 }
             fn negative() -> Float { -0.25 }
             fn classify(value: Float) -> String {
               match value { 1.5 => \"one and a half\", _ => \"other\" }
             }",
        )
        .unwrap();
        assert!(matches!(
            parsed.functions[0].body.unlocated(),
            Expr::Binary { right, .. } if matches!(right.unlocated(), Expr::Float(value) if *value == 150.0)
        ));
        assert!(
            matches!(parsed.functions[1].body.unlocated(), Expr::Float(value) if *value == -0.25)
        );
        assert!(matches!(
            parsed.functions[2].body.unlocated(),
            Expr::Match { arms, .. } if matches!(arms[0].pattern.unlocated(), Pattern::Float(value) if *value == 1.5)
        ));
        assert!(matches!(
            parse("fn bad() -> Float { 1e999 }").unwrap_err(),
            Error::FloatOutOfRange { .. }
        ));
    }

    #[test]
    fn recovers_at_top_level_and_reports_multiple_syntax_errors() {
        let output = parse_recovering(
            "fn first( -> Int { 1 }\nfn second( -> String { \"x\" }\nfn good() -> Int { 1 }",
        );
        assert_eq!(output.diagnostics.len(), 2);
        let program = output.program.unwrap();
        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.functions[0].name, "good");
    }

    #[test]
    fn parses_async_functions_and_await() {
        let parsed = parse(
            "pub async fn later(value: Int) -> Int { value + 1 }
             async fn use(value: Int) -> Int { later(value).await + 1 }
             async fn result(value: Int) -> Result<Int, String> { Ok(value) }
             async fn propagate(value: Int) -> Result<Int, String> {
               try { result(value).await? }
             }",
        )
        .unwrap();
        assert!(parsed.functions[0].is_public);
        assert!(parsed.functions[0].is_async);
        assert!(!parsed.functions[1].is_public);
        assert!(matches!(
            parsed.functions[1].body.unlocated(),
            Expr::Binary { left, .. } if matches!(left.unlocated(), Expr::Await(_))
        ));

        let source = "async fn later(value: Int) -> Int { value }
             fn bad() -> Int { later(1).await }";
        let error = parse(source).unwrap_err();
        assert!(matches!(
            error,
            Error::AwaitOutsideAsync { offset } if offset == source.find("later(1).await").unwrap()
        ));

        assert!(matches!(
            parse(
                "async fn later(value: Int) -> Int { value }
                 async fn bad() -> Int { await later(1) }",
            ),
            Err(Error::PrefixAwait { .. })
        ));

        assert!(matches!(
            parsed.functions[3].body.unlocated(),
            Expr::Try(block)
                if matches!(block.tail.unlocated(), Expr::Propagate(value) if matches!(value.unlocated(), Expr::Await(_)))
        ));
    }

    #[test]
    fn parses_checked_sync_and_async_tests() {
        let parsed = parse(
            "test fn arithmetic() -> Bool { true }
             test async fn agent_turn() -> Bool { true }",
        )
        .unwrap();
        assert!(parsed.functions[0].is_test);
        assert!(!parsed.functions[0].is_async);
        assert!(parsed.functions[1].is_test);
        assert!(parsed.functions[1].is_async);

        for (source, expected) in [
            ("pub test fn visible() -> Bool { true }", "cannot be public"),
            (
                "test fn parameterized(value: Int) -> Bool { true }",
                "cannot have parameters",
            ),
            ("test fn wrong() -> Int { 1 }", "must return Bool"),
        ] {
            assert!(parse(source).unwrap_err().to_string().contains(expected));
        }
    }

    #[test]
    fn marks_functions_for_compile_time_evaluation() {
        let parsed = parse(
            "const fn twice(value: Int) -> Int { value * 2 }
             pub const fn four() -> Int { twice(2) }",
        )
        .unwrap();
        assert!(parsed.functions.iter().all(|function| function.is_const));
        assert!(!parsed.functions[0].is_public);
        assert!(parsed.functions[1].is_public);
    }

    #[test]
    fn checks_mutable_local_bindings() {
        let parsed = parse(
            "fn bump(value: Int) -> Int {
               let mut current = value;
               current = current + 1;
               current
             }",
        )
        .unwrap();
        let Expr::Block(block) = &parsed.functions[0].body else {
            panic!("function body should contain statements");
        };
        assert!(matches!(
            block.statements[0],
            Statement::Let { mutable: true, .. }
        ));
        assert!(matches!(block.statements[1], Statement::Assign { .. }));

        let error = parse("fn bad() -> Int { let value = 1; value = 2; value }").unwrap_err();
        assert!(matches!(error, Error::ImmutableAssignment { ref name, .. } if name == "value"));
    }
}
