use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub const fn point(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<Import>,
    pub dart_libraries: Vec<DartLibrary>,
    pub structs: Vec<Struct>,
    pub enums: Vec<Enum>,
    pub comptime: Vec<ComptimeDirective>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub span: SourceSpan,
    pub path: String,
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DartLibrary {
    pub span: SourceSpan,
    pub uri: String,
    pub namespace: String,
    pub functions: Vec<ExternFunction>,
}

impl Program {
    pub fn into_dart_definition(mut self, namespace: String) -> Option<DartLibrary> {
        if !self.imports.is_empty()
            || self
                .structs
                .iter()
                .any(|structure| !is_comptime_ir_type(&structure.name))
            || self.enums.len() != 2
            || !self.comptime.is_empty()
            || !self.functions.is_empty()
            || self.dart_libraries.len() != 1
        {
            return None;
        }
        let mut library = self.dart_libraries.pop().unwrap();
        library.namespace = namespace;
        Some(library)
    }
}

pub const COMPTIME_IR_TYPES: [&str; 4] = [
    "ComptimeDeclarations",
    "ComptimeFunction",
    "ComptimeParameter",
    "ComptimeExpression",
];

pub fn is_comptime_ir_type(name: &str) -> bool {
    COMPTIME_IR_TYPES.contains(&name)
}

pub const INTRINSIC_FUNCTIONS: [&str; 16] = [
    "_oche_type",
    "_oche_json_parse",
    "_oche_json_stringify",
    "_oche_json_object",
    "_oche_json_array",
    "_oche_json_from_int",
    "_oche_json_from_float",
    "_oche_json_from_bool",
    "_oche_json_from_string",
    "_oche_json_as_object",
    "_oche_json_as_array",
    "_oche_json_as_int",
    "_oche_json_as_float",
    "_oche_json_as_bool",
    "_oche_json_as_string",
    "_oche_json_field",
];

pub fn intrinsic_signature(name: &str) -> Option<(Vec<Type>, Type)> {
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
    fn result(success: Type) -> Type {
        Type::Named {
            name: "Result".into(),
            arguments: vec![success, named("String")],
        }
    }
    let json = named("JsonValue");
    let object = named("JsonObject");
    Some(match name {
        "_oche_type" => (vec![named("String"), list(named("Type"))], named("Type")),
        "_oche_json_parse" => (vec![named("String")], result(json)),
        "_oche_json_stringify" => (vec![json], named("String")),
        "_oche_json_object" => (vec![list(named("String")), list(json)], named("JsonValue")),
        "_oche_json_array" => (vec![list(json)], named("JsonValue")),
        "_oche_json_from_int" => (vec![named("Int")], json),
        "_oche_json_from_float" => (vec![named("Float")], json),
        "_oche_json_from_bool" => (vec![named("Bool")], json),
        "_oche_json_from_string" => (vec![named("String")], json),
        "_oche_json_as_object" => (vec![json], result(object)),
        "_oche_json_as_array" => (vec![json], result(list(named("JsonValue")))),
        "_oche_json_as_int" => (vec![json], result(named("Int"))),
        "_oche_json_as_float" => (vec![json], result(named("Float"))),
        "_oche_json_as_bool" => (vec![json], result(named("Bool"))),
        "_oche_json_as_string" => (vec![json], result(named("String"))),
        "_oche_json_field" => (vec![object, named("String")], result(json)),
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExternFunction {
    pub span: SourceSpan,
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub result: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub span: Option<SourceSpan>,
    pub name: String,
    pub type_parameters: Vec<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub span: Option<SourceSpan>,
    pub name: String,
    pub fields: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub span: SourceSpan,
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub span: SourceSpan,
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComptimeDirective {
    pub span: SourceSpan,
    pub target: String,
    pub kind: ComptimeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeKind {
    Validate(Vec<FieldValidation>),
    Generate { function: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldValidation {
    pub field: String,
    pub rule: ValidationRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationRule {
    NonEmpty,
    Range { minimum: i64, maximum: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub span: SourceSpan,
    pub is_public: bool,
    pub is_test: bool,
    pub is_const: bool,
    pub is_async: bool,
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub result: Type,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub span: SourceSpan,
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Named { name: String, arguments: Vec<Type> },
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named { name, arguments } => {
                formatter.write_str(name)?;
                if !arguments.is_empty() {
                    formatter.write_str("<")?;
                    for (index, argument) in arguments.iter().enumerate() {
                        if index != 0 {
                            formatter.write_str(", ")?;
                        }
                        argument.fmt(formatter)?;
                    }
                    formatter.write_str(">")?;
                }
                Ok(())
            }
        }
    }
}

impl Type {
    pub fn name(&self) -> &str {
        match self {
            Self::Named { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Located {
        span: SourceSpan,
        value: Box<Expr>,
    },
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    TypeValue(Type),
    List(Vec<Expr>),
    Variable(String),
    Call {
        callee: String,
        arguments: Vec<Expr>,
    },
    QualifiedCall {
        module: String,
        name: String,
        arguments: Vec<Expr>,
    },
    DotCall {
        receiver: Box<Expr>,
        name: String,
        arguments: Vec<Expr>,
    },
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Field {
        value: Box<Expr>,
        name: String,
    },
    Negate(Box<Expr>),
    Await(Box<Expr>),
    Comptime(Box<Expr>),
    Quote(Box<Expr>),
    Splice(Box<Expr>),
    Binary {
        operator: Operator,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Match {
        offset: usize,
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Block(Block),
    Try(Block),
    Propagate(Box<Expr>),
}

impl Expr {
    pub fn unlocated(&self) -> &Self {
        match self {
            Self::Located { value, .. } => value.unlocated(),
            value => value,
        }
    }

    pub fn offset(&self) -> Option<usize> {
        self.span().map(|span| span.start)
    }

    pub fn span(&self) -> Option<SourceSpan> {
        match self {
            Self::Located { span, .. } => Some(*span),
            _ => None,
        }
    }

    pub fn first_await_span(&self) -> Option<SourceSpan> {
        match self {
            Self::Located { span, value } => {
                if matches!(value.unlocated(), Self::Await(_)) {
                    Some(*span)
                } else {
                    value.first_await_span()
                }
            }
            Self::Await(value) => value.span().or_else(|| value.first_await_span()),
            Self::Field { value, .. }
            | Self::Negate(value)
            | Self::Comptime(value)
            | Self::Quote(value)
            | Self::Splice(value)
            | Self::Propagate(value) => value.first_await_span(),
            Self::Binary { left, right, .. } => {
                left.first_await_span().or_else(|| right.first_await_span())
            }
            Self::Call { arguments, .. } | Self::QualifiedCall { arguments, .. } => {
                arguments.iter().find_map(Self::first_await_span)
            }
            Self::DotCall {
                receiver,
                arguments,
                ..
            } => receiver
                .first_await_span()
                .or_else(|| arguments.iter().find_map(Self::first_await_span)),
            Self::List(values) => values.iter().find_map(Self::first_await_span),
            Self::StructLiteral { fields, .. } => fields
                .iter()
                .find_map(|(_, value)| value.first_await_span()),
            Self::Match { value, arms, .. } => value.first_await_span().or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(Self::first_await_span)
                        .or_else(|| arm.body.first_await_span())
                })
            }),
            Self::Block(block) | Self::Try(block) => block
                .statements
                .iter()
                .find_map(|statement| match statement {
                    Statement::Let { value, .. }
                    | Statement::Assign { value, .. }
                    | Statement::Expression(value) => value.first_await_span(),
                })
                .or_else(|| block.tail.first_await_span()),
            Self::Integer(_)
            | Self::Float(_)
            | Self::Boolean(_)
            | Self::String(_)
            | Self::TypeValue(_)
            | Self::Variable(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub tail: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Let {
        mutable: bool,
        pattern: Pattern,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
    },
    Expression(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub offset: usize,
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Located {
        span: SourceSpan,
        value: Box<Pattern>,
    },
    Wildcard,
    Binding(String),
    Variant {
        name: String,
        fields: Vec<Pattern>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Pattern)>,
        rest: bool,
    },
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
}

impl Pattern {
    pub fn unlocated(&self) -> &Self {
        match self {
            Self::Located { value, .. } => value.unlocated(),
            pattern => pattern,
        }
    }

    pub fn span(&self) -> Option<SourceSpan> {
        match self {
            Self::Located { span, .. } => Some(*span),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
}
