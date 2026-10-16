//! Typed, resolved high-level IR shared by diagnostics, tooling, and backends.

use std::collections::{HashMap, HashSet};

use oche_frontend::{
    Block, ComptimeKind, Expr, Operator, Pattern, Program, SourceSpan, Statement, Type,
};
use snafu::Snafu;

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("Type mismatch in {context}: expected {expected}, received {actual}"))]
    TypeMismatch {
        context: String,
        expected: Type,
        actual: Type,
    },
    #[snafu(display("Cannot infer the type of {context}"))]
    CannotInfer { context: String },
    #[snafu(display("Type '{receiver}' has no field '{field}'"))]
    UnknownField { receiver: Type, field: String },
    #[snafu(display("Operator '{operator}' does not accept {operand}"))]
    InvalidOperand {
        operator: &'static str,
        operand: Type,
    },
    #[snafu(display("The '?' operator requires Option or Result, received {actual}"))]
    InvalidPropagation { actual: Type },
    #[snafu(display("'await' is only valid inside an async function"))]
    AwaitOutsideAsync,
    #[snafu(display("The 'await' operator requires Future, received {actual}"))]
    InvalidAwait { actual: Type },
    #[snafu(display("Pattern '{pattern}' does not match {subject}"))]
    InvalidPattern { pattern: String, subject: Type },
    #[snafu(display("Non-exhaustive match on {subject}; missing pattern {missing}"))]
    NonExhaustiveMatch { subject: Type, missing: String },
    #[snafu(display("Match arm is unreachable because previous patterns cover it"))]
    RedundantMatchArm,
    #[snafu(display("{source}"))]
    Located {
        span: SourceSpan,
        source: Box<Error>,
    },
    #[snafu(display("Unknown qualified function '{module}.{name}'"))]
    UnknownQualifiedFunction { module: String, name: String },
    #[snafu(display(
        "Function '{module}.{name}' exposes type '{ty}' which cannot cross an Oche module boundary"
    ))]
    UnsupportedModuleBoundaryType {
        module: String,
        name: String,
        ty: Type,
    },
    #[snafu(display("Type '{receiver}' has no method '{method}'"))]
    UnknownMethod { receiver: Type, method: String },
    #[snafu(display("{callable} expects {expected} argument(s), received {actual}"))]
    ArgumentCount {
        callable: String,
        expected: usize,
        actual: usize,
    },
    #[snafu(display("JSON derivation for '{target}' does not support field type '{ty}'"))]
    UnsupportedJsonFieldType { target: String, ty: Type },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId(usize);

impl ModuleId {
    pub const ROOT: Self = Self(0);

    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleImport {
    pub alias: String,
    pub module: ModuleId,
}

#[derive(Debug)]
pub struct LinkedModule {
    pub program: Program,
    pub imports: Vec<ModuleImport>,
}

#[derive(Debug)]
pub struct LinkedPackage {
    pub root: ModuleId,
    pub modules: Vec<LinkedModule>,
}

impl LinkedPackage {
    pub fn module(&self, id: ModuleId) -> &LinkedModule {
        &self.modules[id.index()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId {
    pub module: ModuleId,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructId {
    pub module: ModuleId,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariantId {
    pub module: ModuleId,
    pub enumeration: usize,
    pub variant: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntheticFunctionId {
    pub module: ModuleId,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalFunctionId {
    pub module: ModuleId,
    pub library: usize,
    pub function: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
    Variant(VariantId),
    Synthetic(SyntheticFunctionId),
    External(ExternalFunctionId),
    Intrinsic(Intrinsic),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    TypeNamed,
    JsonParse,
    JsonStringify,
    JsonObject,
    JsonArray,
    JsonFromInt,
    JsonFromFloat,
    JsonFromBool,
    JsonFromString,
    JsonAsObject,
    JsonAsArray,
    JsonAsInt,
    JsonAsFloat,
    JsonAsBool,
    JsonAsString,
    JsonField,
}

impl Intrinsic {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "_oche_type" => Self::TypeNamed,
            "_oche_json_parse" => Self::JsonParse,
            "_oche_json_stringify" => Self::JsonStringify,
            "_oche_json_object" => Self::JsonObject,
            "_oche_json_array" => Self::JsonArray,
            "_oche_json_from_int" => Self::JsonFromInt,
            "_oche_json_from_float" => Self::JsonFromFloat,
            "_oche_json_from_bool" => Self::JsonFromBool,
            "_oche_json_from_string" => Self::JsonFromString,
            "_oche_json_as_object" => Self::JsonAsObject,
            "_oche_json_as_array" => Self::JsonAsArray,
            "_oche_json_as_int" => Self::JsonAsInt,
            "_oche_json_as_float" => Self::JsonAsFloat,
            "_oche_json_as_bool" => Self::JsonAsBool,
            "_oche_json_as_string" => Self::JsonAsString,
            "_oche_json_field" => Self::JsonField,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::TypeNamed => "_oche_type",
            Self::JsonParse => "_oche_json_parse",
            Self::JsonStringify => "_oche_json_stringify",
            Self::JsonObject => "_oche_json_object",
            Self::JsonArray => "_oche_json_array",
            Self::JsonFromInt => "_oche_json_from_int",
            Self::JsonFromFloat => "_oche_json_from_float",
            Self::JsonFromBool => "_oche_json_from_bool",
            Self::JsonFromString => "_oche_json_from_string",
            Self::JsonAsObject => "_oche_json_as_object",
            Self::JsonAsArray => "_oche_json_as_array",
            Self::JsonAsInt => "_oche_json_as_int",
            Self::JsonAsFloat => "_oche_json_as_float",
            Self::JsonAsBool => "_oche_json_as_bool",
            Self::JsonAsString => "_oche_json_as_string",
            Self::JsonField => "_oche_json_field",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntheticOrigin {
    JsonEncode,
    JsonEncodeValue,
    JsonDecode,
    JsonDecodeValue,
    Validate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticFunction {
    pub name: String,
    pub parameters: Vec<Type>,
    pub result: Type,
    pub origin: SyntheticOrigin,
    pub structure: StructId,
    pub target: String,
}

#[derive(Debug, PartialEq)]
pub struct TypedProgram {
    pub module: ModuleId,
    pub functions: Vec<TypedFunction>,
    pub synthetic_functions: Vec<SyntheticFunction>,
}

#[derive(Debug, PartialEq)]
pub struct TypedPackage {
    pub root: ModuleId,
    pub modules: Vec<TypedProgram>,
}

#[derive(Debug, PartialEq)]
pub struct TypedFunction {
    pub id: FunctionId,
    pub body: TypedExpr,
}

#[derive(Debug, PartialEq)]
pub struct TypedExpr {
    pub span: Option<SourceSpan>,
    pub ty: Type,
    pub kind: TypedExprKind,
}

#[derive(Debug, PartialEq)]
pub enum TypedExprKind {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    TypeValue(Type),
    List(Vec<TypedExpr>),
    ListLength(Box<TypedExpr>),
    ListFirst(Box<TypedExpr>),
    ListLast(Box<TypedExpr>),
    ListRest(Box<TypedExpr>),
    TypeProperty {
        value: Box<TypedExpr>,
        property: TypeProperty,
    },
    Variable(String),
    Call {
        target: Callable,
        arguments: Vec<TypedExpr>,
    },
    StructLiteral {
        structure: StructId,
        fields: Vec<(usize, TypedExpr)>,
    },
    Field {
        value: Box<TypedExpr>,
        structure: StructId,
        field: usize,
    },
    Negate(Box<TypedExpr>),
    Await(Box<TypedExpr>),
    IsolateRun(Box<TypedExpr>),
    Comptime(Box<TypedExpr>),
    Binary {
        operator: Operator,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Match {
        value: Box<TypedExpr>,
        arms: Vec<TypedMatchArm>,
    },
    Block(TypedBlock),
    Try(TypedBlock),
    Propagate(Box<TypedExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeProperty {
    Name,
    Arguments,
    Fields,
    FieldType,
}

#[derive(Debug, PartialEq)]
pub struct TypedBlock {
    pub statements: Vec<TypedStatement>,
    pub tail: Box<TypedExpr>,
}

#[derive(Debug, PartialEq)]
pub enum TypedStatement {
    Let {
        mutable: bool,
        pattern: Pattern,
        value: TypedExpr,
    },
    Assign {
        name: String,
        value: TypedExpr,
    },
    Expression(TypedExpr),
}

#[derive(Debug, PartialEq)]
pub struct TypedMatchArm {
    pub offset: usize,
    pub pattern: Pattern,
    pub guard: Option<TypedExpr>,
    pub body: TypedExpr,
}

#[derive(Debug)]
pub struct LocatedError {
    pub function: FunctionId,
    pub span: Option<SourceSpan>,
    pub source: Box<Error>,
}

impl Error {
    fn at(self, span: SourceSpan) -> Self {
        if matches!(self, Self::Located { .. }) {
            self
        } else {
            Self::Located {
                span,
                source: Box::new(self),
            }
        }
    }

    fn at_offset(self, offset: usize) -> Self {
        self.at(SourceSpan::point(offset))
    }

    fn into_unlocated(self) -> (Option<SourceSpan>, Self) {
        match self {
            Self::Located { span, source } => (Some(span), *source),
            source => (None, source),
        }
    }
}

pub fn lower(program: &Program) -> Result<TypedProgram, Error> {
    lower_located(program).map_err(|error| *error.source)
}

pub fn lower_located(program: &Program) -> Result<TypedProgram, LocatedError> {
    lower_module_located(ModuleId::ROOT, program)
}

pub fn lower_module(module: ModuleId, program: &Program) -> Result<TypedProgram, Error> {
    lower_module_located(module, program).map_err(|error| *error.source)
}

pub fn lower_module_located(
    module: ModuleId,
    program: &Program,
) -> Result<TypedProgram, LocatedError> {
    lower_module_with_package(module, program, None)
}

pub fn lower_package(package: &LinkedPackage) -> Result<TypedPackage, Error> {
    lower_package_located(package).map_err(|error| *error.source)
}

pub fn lower_package_located(package: &LinkedPackage) -> Result<TypedPackage, LocatedError> {
    let mut modules = Vec::with_capacity(package.modules.len());
    for (index, linked) in package.modules.iter().enumerate() {
        modules.push(lower_module_with_package(
            ModuleId::new(index),
            &linked.program,
            Some(package),
        )?);
    }
    Ok(TypedPackage {
        root: package.root,
        modules,
    })
}

fn lower_module_with_package(
    module: ModuleId,
    program: &Program,
    package: Option<&LinkedPackage>,
) -> Result<TypedProgram, LocatedError> {
    validate_json_derives(module, program)?;
    let synthetic_functions = synthetic_functions(module, program);
    let checker = Checker {
        module,
        program,
        synthetic: &synthetic_functions,
        package,
    };
    let mut functions = Vec::with_capacity(program.functions.len());
    for (source_index, function) in program.functions.iter().enumerate() {
        let id = FunctionId {
            module,
            index: source_index,
        };
        if !function.is_async
            && let Some(await_span) = function.body.first_await_span()
        {
            return Err(LocatedError {
                function: id,
                span: Some(await_span),
                source: Box::new(Error::AwaitOutsideAsync),
            });
        }
        let mut scope = function
            .parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
            .collect::<HashMap<_, _>>();
        let body = checker
            .expression(&function.body, Some(&function.result), &mut scope)
            .map_err(|source| {
                let (span, source) = source.into_unlocated();
                LocatedError {
                    function: id,
                    span,
                    source: Box::new(source),
                }
            })?;
        checker
            .expect(
                &function.result,
                &body.ty,
                &format!("return from {}", function.name),
            )
            .map_err(|source| LocatedError {
                function: id,
                span: body.span,
                source: Box::new(source),
            })?;
        functions.push(TypedFunction { id, body });
    }
    Ok(TypedProgram {
        module,
        functions,
        synthetic_functions,
    })
}

fn validate_json_derives(module: ModuleId, program: &Program) -> Result<(), LocatedError> {
    for directive in &program.comptime {
        let ComptimeKind::Generate { function } = &directive.kind else {
            continue;
        };
        if !matches!(
            function.as_str(),
            "_oche_generate_json_encode" | "_oche_generate_json_decode"
        ) {
            continue;
        }
        let Some(structure) = program
            .structs
            .iter()
            .find(|structure| structure.name == directive.target)
        else {
            continue;
        };
        for field in &structure.fields {
            if !json_type_supported(&field.ty, program, &mut HashSet::new()) {
                return Err(LocatedError {
                    function: FunctionId { module, index: 0 },
                    span: Some(field.span),
                    source: Box::new(Error::UnsupportedJsonFieldType {
                        target: directive.target.clone(),
                        ty: field.ty.clone(),
                    }),
                });
            }
        }
    }
    Ok(())
}

fn json_type_supported(ty: &Type, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Type::Named { name, arguments } = ty;
    match (name.as_str(), arguments.as_slice()) {
        ("Int" | "Float" | "Bool" | "String" | "JsonValue", []) => true,
        ("List", [element]) => json_type_supported(element, program, visiting),
        (_, []) => {
            let Some(structure) = program
                .structs
                .iter()
                .find(|structure| structure.name == *name)
            else {
                return false;
            };
            if !visiting.insert(name.clone()) {
                return false;
            }
            let supported = structure
                .fields
                .iter()
                .all(|field| json_type_supported(&field.ty, program, visiting));
            visiting.remove(name);
            supported
        }
        _ => false,
    }
}

fn named(name: &str) -> Type {
    Type::Named {
        name: name.into(),
        arguments: Vec::new(),
    }
}

fn module_boundary_type(ty: &Type) -> bool {
    let Type::Named { name, arguments } = ty;
    let arity = match name.as_str() {
        "Int" | "Float" | "Bool" | "String" => 0,
        "Future" | "List" | "Option" => 1,
        "Result" => 2,
        _ => return false,
    };
    arguments.len() == arity && arguments.iter().all(module_boundary_type)
}

fn result(success: Type) -> Type {
    Type::Named {
        name: "Result".into(),
        arguments: vec![success, named("String")],
    }
}

fn json_encode_value_function(target: &str) -> String {
    format!("_oche_generated_json_encode_value_{target}")
}

fn json_encode_function(target: &str) -> String {
    format!("_oche_generated_json_encode_{target}")
}

fn json_decode_value_function(target: &str) -> String {
    format!("_oche_generated_json_decode_value_{target}")
}

fn json_decode_function(target: &str) -> String {
    format!("_oche_generated_json_decode_{target}")
}

fn synthetic_functions(module: ModuleId, program: &Program) -> Vec<SyntheticFunction> {
    let mut functions = Vec::new();
    for directive in &program.comptime {
        let json_generator = match &directive.kind {
            ComptimeKind::Generate { function } if function == "_oche_generate_json_encode" => {
                Some((true, false))
            }
            ComptimeKind::Generate { function } if function == "_oche_generate_json_decode" => {
                Some((false, true))
            }
            _ => None,
        };
        if let Some((encode, decode)) = json_generator {
            let structure = comptime_target(program, directive);
            let target = named(&directive.target);
            if encode {
                functions.push(SyntheticFunction {
                    name: json_encode_value_function(&directive.target),
                    parameters: vec![target.clone()],
                    result: named("JsonValue"),
                    origin: SyntheticOrigin::JsonEncodeValue,
                    structure: StructId {
                        module,
                        index: structure,
                    },
                    target: directive.target.clone(),
                });
                functions.push(SyntheticFunction {
                    name: json_encode_function(&directive.target),
                    parameters: vec![target.clone()],
                    result: named("String"),
                    origin: SyntheticOrigin::JsonEncode,
                    structure: StructId {
                        module,
                        index: structure,
                    },
                    target: directive.target.clone(),
                });
            }
            if decode {
                functions.push(SyntheticFunction {
                    name: json_decode_value_function(&directive.target),
                    parameters: vec![named("JsonValue")],
                    result: result(target.clone()),
                    origin: SyntheticOrigin::JsonDecodeValue,
                    structure: StructId {
                        module,
                        index: structure,
                    },
                    target: directive.target.clone(),
                });
                functions.push(SyntheticFunction {
                    name: json_decode_function(&directive.target),
                    parameters: vec![named("String")],
                    result: result(target),
                    origin: SyntheticOrigin::JsonDecode,
                    structure: StructId {
                        module,
                        index: structure,
                    },
                    target: directive.target.clone(),
                });
            }
            continue;
        }
        match &directive.kind {
            ComptimeKind::Validate(_) => {
                let structure = comptime_target(program, directive);
                let target = named(&directive.target);
                functions.push(SyntheticFunction {
                    name: format!("{}_validate", directive.target),
                    parameters: vec![target.clone()],
                    result: result(target),
                    origin: SyntheticOrigin::Validate,
                    structure: StructId {
                        module,
                        index: structure,
                    },
                    target: directive.target.clone(),
                });
            }
            ComptimeKind::Generate { .. } => {}
        }
    }
    functions
}

fn comptime_target(program: &Program, directive: &oche_frontend::ComptimeDirective) -> usize {
    program
        .structs
        .iter()
        .position(|structure| structure.name == directive.target)
        .expect("schema comptime targets are validated by the frontend")
}

struct Checker<'a> {
    module: ModuleId,
    program: &'a Program,
    synthetic: &'a [SyntheticFunction],
    package: Option<&'a LinkedPackage>,
}

impl Checker<'_> {
    fn has_callable(&self, name: &str) -> bool {
        self.program.functions.iter().any(|item| item.name == name)
            || self.synthetic.iter().any(|item| item.name == name)
    }

    fn expect(&self, expected: &Type, actual: &Type, context: &str) -> Result<(), Error> {
        if expected == actual {
            Ok(())
        } else {
            Err(Error::TypeMismatch {
                context: context.into(),
                expected: expected.clone(),
                actual: actual.clone(),
            })
        }
    }

    fn expression(
        &self,
        expression: &Expr,
        expected: Option<&Type>,
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        if let Expr::Located { span, value } = expression {
            let mut typed = self
                .expression(value, expected, scope)
                .map_err(|source| source.at(*span))?;
            if typed.span.is_none() {
                typed.span = Some(*span);
            }
            return Ok(typed);
        }
        let typed = match expression {
            Expr::Located { .. } => unreachable!("located expressions return before lowering"),
            Expr::Integer(value) => leaf(named("Int"), TypedExprKind::Integer(*value)),
            Expr::Float(value) => leaf(named("Float"), TypedExprKind::Float(*value)),
            Expr::Boolean(value) => leaf(named("Bool"), TypedExprKind::Boolean(*value)),
            Expr::String(value) => leaf(named("String"), TypedExprKind::String(value.clone())),
            Expr::TypeValue(value) => leaf(named("Type"), TypedExprKind::TypeValue(value.clone())),
            Expr::List(values) => {
                let expected_element = expected.and_then(|ty| {
                    (ty.name() == "List" && ty.arguments().len() == 1).then(|| &ty.arguments()[0])
                });
                let mut typed_values = Vec::with_capacity(values.len());
                let mut element_ty = expected_element.cloned();
                for value in values {
                    let typed_value = self.expression(value, element_ty.as_ref(), scope)?;
                    if element_ty.is_none() {
                        element_ty = Some(typed_value.ty.clone());
                    }
                    typed_values.push(typed_value);
                }
                let element_ty = element_ty.ok_or_else(|| Error::CannotInfer {
                    context: "empty list without an expected List type".into(),
                })?;
                leaf(
                    Type::Named {
                        name: "List".into(),
                        arguments: vec![element_ty],
                    },
                    TypedExprKind::List(typed_values),
                )
            }
            Expr::Variable(name) => leaf(
                scope.get(name).cloned().ok_or_else(|| Error::CannotInfer {
                    context: format!("variable '{name}'"),
                })?,
                TypedExprKind::Variable(name.clone()),
            ),
            Expr::Call { callee, arguments } => self.call(callee, arguments, expected, scope)?,
            Expr::QualifiedCall {
                module,
                name,
                arguments,
            } => self.qualified_call(module, name, arguments, scope)?,
            Expr::DotCall {
                receiver,
                name,
                arguments,
            } => {
                if let Expr::Variable(module) = receiver.as_ref()
                    && (self
                        .program
                        .dart_libraries
                        .iter()
                        .any(|library| library.namespace == *module)
                        || self.imported_module(module).is_some()
                        || self.is_std_json_namespace(module)
                        || self.is_std_isolate_namespace(module))
                {
                    return self.qualified_call(module, name, arguments, scope);
                }
                if let Expr::Variable(owner) = receiver.as_ref()
                    && self
                        .program
                        .structs
                        .iter()
                        .any(|structure| structure.name == *owner)
                {
                    let callable = if name == "fromJson" {
                        json_decode_function(owner)
                    } else {
                        format!("{owner}_{name}")
                    };
                    if self.has_callable(&callable) {
                        return self.call(&callable, arguments, expected, scope);
                    }
                }
                let receiver_expression = (**receiver).clone();
                let receiver = self.expression(receiver, None, scope)?;
                if receiver.ty.name() == "List"
                    && matches!(name.as_str(), "len" | "first" | "last" | "rest")
                {
                    if !arguments.is_empty() {
                        return Err(Error::ArgumentCount {
                            callable: format!("List.{name}"),
                            expected: 0,
                            actual: arguments.len(),
                        });
                    }
                    match name.as_str() {
                        "len" => leaf(named("Int"), TypedExprKind::ListLength(Box::new(receiver))),
                        "rest" => leaf(
                            receiver.ty.clone(),
                            TypedExprKind::ListRest(Box::new(receiver)),
                        ),
                        "first" | "last" => {
                            let element = receiver.ty.arguments()[0].clone();
                            leaf(
                                Type::Named {
                                    name: "Option".into(),
                                    arguments: vec![element],
                                },
                                if name == "first" {
                                    TypedExprKind::ListFirst(Box::new(receiver))
                                } else {
                                    TypedExprKind::ListLast(Box::new(receiver))
                                },
                            )
                        }
                        _ => unreachable!(),
                    }
                } else {
                    let callable = if name == "toJson" {
                        json_encode_function(receiver.ty.name())
                    } else {
                        format!("{}_{name}", receiver.ty.name())
                    };
                    if !self.has_callable(&callable) {
                        return Err(Error::UnknownMethod {
                            receiver: receiver.ty,
                            method: name.clone(),
                        });
                    }
                    let mut method_arguments = Vec::with_capacity(arguments.len() + 1);
                    method_arguments.push(receiver_expression);
                    method_arguments.extend(arguments.iter().cloned());
                    self.call(&callable, &method_arguments, expected, scope)?
                }
            }
            Expr::StructLiteral { name, fields } => {
                let structure = self
                    .program
                    .structs
                    .iter()
                    .position(|item| item.name == *name)
                    .ok_or_else(|| Error::CannotInfer {
                        context: format!("struct '{name}'"),
                    })?;
                let mut typed_fields = Vec::with_capacity(fields.len());
                for (field_name, value) in fields {
                    let field = self.program.structs[structure]
                        .fields
                        .iter()
                        .position(|item| item.name == *field_name)
                        .expect("struct fields were validated by the frontend");
                    let field_ty = &self.program.structs[structure].fields[field].ty;
                    typed_fields.push((field, self.expression(value, Some(field_ty), scope)?));
                }
                leaf(
                    named(name),
                    TypedExprKind::StructLiteral {
                        structure: StructId {
                            module: self.module,
                            index: structure,
                        },
                        fields: typed_fields,
                    },
                )
            }
            Expr::Field { value, name } => {
                let value = self.expression(value, None, scope)?;
                if matches!(value.ty.name(), "Type" | "Field") {
                    let (ty, property) = match (value.ty.name(), name.as_str()) {
                        ("Type", "name") | ("Field", "name") => {
                            (named("String"), TypeProperty::Name)
                        }
                        ("Type", "arguments") => (
                            Type::Named {
                                name: "List".into(),
                                arguments: vec![named("Type")],
                            },
                            TypeProperty::Arguments,
                        ),
                        ("Type", "fields") => (
                            Type::Named {
                                name: "List".into(),
                                arguments: vec![named("Field")],
                            },
                            TypeProperty::Fields,
                        ),
                        ("Field", "ty") => (named("Type"), TypeProperty::FieldType),
                        _ => {
                            return Err(Error::UnknownField {
                                receiver: value.ty,
                                field: name.clone(),
                            });
                        }
                    };
                    return Ok(leaf(
                        ty,
                        TypedExprKind::TypeProperty {
                            value: Box::new(value),
                            property,
                        },
                    ));
                }
                let structure = self
                    .program
                    .structs
                    .iter()
                    .position(|item| item.name == value.ty.name())
                    .ok_or_else(|| Error::UnknownField {
                        receiver: value.ty.clone(),
                        field: name.clone(),
                    })?;
                let field = self.program.structs[structure]
                    .fields
                    .iter()
                    .position(|item| item.name == *name)
                    .ok_or_else(|| Error::UnknownField {
                        receiver: value.ty.clone(),
                        field: name.clone(),
                    })?;
                leaf(
                    self.program.structs[structure].fields[field].ty.clone(),
                    TypedExprKind::Field {
                        value: Box::new(value),
                        structure: StructId {
                            module: self.module,
                            index: structure,
                        },
                        field,
                    },
                )
            }
            Expr::Negate(value) => {
                let value = self.expression(value, None, scope)?;
                if !matches!(value.ty.name(), "Int" | "Float") {
                    return Err(Error::InvalidOperand {
                        operator: "-",
                        operand: value.ty,
                    });
                }
                leaf(value.ty.clone(), TypedExprKind::Negate(Box::new(value)))
            }
            Expr::Await(value) => {
                let value = self.expression(value, None, scope)?;
                if value.ty.name() != "Future" || value.ty.arguments().len() != 1 {
                    return Err(Error::InvalidAwait {
                        actual: value.ty.clone(),
                    });
                }
                leaf(
                    value.ty.arguments()[0].clone(),
                    TypedExprKind::Await(Box::new(value)),
                )
            }
            Expr::Comptime(value) => {
                let value = self.expression(value, expected, scope)?;
                leaf(value.ty.clone(), TypedExprKind::Comptime(Box::new(value)))
            }
            Expr::Quote(value) | Expr::Splice(value) => self.expression(value, expected, scope)?,
            Expr::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.expression(left, None, scope)?;
                if !matches!(left.ty.name(), "Int" | "Float")
                    && !(*operator == Operator::Add && matches!(left.ty.name(), "String" | "List"))
                {
                    return Err(Error::InvalidOperand {
                        operator: operator_text(*operator),
                        operand: left.ty,
                    });
                }
                let right = self.expression(right, Some(&left.ty), scope)?;
                leaf(
                    left.ty.clone(),
                    TypedExprKind::Binary {
                        operator: *operator,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                )
            }
            Expr::Match {
                offset,
                value,
                arms,
            } => {
                let value = self.expression(value, None, scope)?;
                let mut typed_arms = Vec::with_capacity(arms.len());
                let mut result_ty = expected.cloned();
                for arm in arms {
                    let mut arm_scope = scope.clone();
                    self.pattern(&arm.pattern, &value.ty, &mut arm_scope)
                        .map_err(|source| source.at_offset(arm.offset))?;
                    let guard = arm
                        .guard
                        .as_ref()
                        .map(|guard| self.expression(guard, Some(&named("Bool")), &mut arm_scope))
                        .transpose()
                        .map_err(|source| source.at_offset(arm.offset))?;
                    let body = self
                        .expression(&arm.body, result_ty.as_ref(), &mut arm_scope)
                        .map_err(|source| source.at_offset(arm.offset))?;
                    if let Some(ty) = &result_ty {
                        self.expect(ty, &body.ty, "match arm")?;
                    } else {
                        result_ty = Some(body.ty.clone());
                    }
                    typed_arms.push(TypedMatchArm {
                        offset: arm.offset,
                        pattern: clone_pattern(&arm.pattern),
                        guard,
                        body,
                    });
                }
                let mut rows = Vec::with_capacity(arms.len());
                for arm in arms {
                    let candidate = vec![&arm.pattern];
                    if !self.useful(&rows, &candidate, std::slice::from_ref(&value.ty)) {
                        return Err(match arm.pattern.span() {
                            Some(span) => Error::RedundantMatchArm.at(span),
                            None => Error::RedundantMatchArm.at_offset(arm.offset),
                        });
                    }
                    if arm.guard.is_none() {
                        rows.push(candidate);
                    }
                }
                if let Some(mut witness) = self.uncovered(&rows, std::slice::from_ref(&value.ty)) {
                    return Err(Error::NonExhaustiveMatch {
                        subject: value.ty,
                        missing: witness.remove(0),
                    }
                    .at_offset(*offset));
                }
                leaf(
                    result_ty.ok_or_else(|| Error::CannotInfer {
                        context: "empty match".into(),
                    })?,
                    TypedExprKind::Match {
                        value: Box::new(value),
                        arms: typed_arms,
                    },
                )
            }
            Expr::Block(block) => {
                let block = self.block(block, expected, scope)?;
                leaf(block.tail.ty.clone(), TypedExprKind::Block(block))
            }
            Expr::Try(block) => {
                let carrier = expected.ok_or_else(|| Error::CannotInfer {
                    context: "try block without an expected Result or Option".into(),
                })?;
                let success = carrier
                    .arguments()
                    .first()
                    .ok_or_else(|| Error::CannotInfer {
                        context: "try block carrier".into(),
                    })?;
                if !matches!(carrier.name(), "Option" | "Result") {
                    return Err(Error::InvalidPropagation {
                        actual: carrier.clone(),
                    });
                }
                let block = self.block(block, Some(success), scope)?;
                leaf(carrier.clone(), TypedExprKind::Try(block))
            }
            Expr::Propagate(value) => {
                let value = self.expression(value, None, scope)?;
                if !matches!(value.ty.name(), "Option" | "Result") {
                    return Err(Error::InvalidPropagation {
                        actual: value.ty.clone(),
                    });
                }
                let success = value.ty.arguments().first().cloned().ok_or_else(|| {
                    Error::InvalidPropagation {
                        actual: value.ty.clone(),
                    }
                })?;
                leaf(success, TypedExprKind::Propagate(Box::new(value)))
            }
        };
        if let Some(expected) = expected {
            self.expect(expected, &typed.ty, "expression")?;
        }
        Ok(typed)
    }

    fn block(
        &self,
        block: &Block,
        expected: Option<&Type>,
        parent: &mut HashMap<String, Type>,
    ) -> Result<TypedBlock, Error> {
        let mut scope = parent.clone();
        let mut statements = Vec::with_capacity(block.statements.len());
        for statement in &block.statements {
            match statement {
                Statement::Let {
                    mutable,
                    pattern,
                    value,
                } => {
                    let value = self.expression(value, None, &mut scope)?;
                    self.pattern(pattern, &value.ty, &mut scope)?;
                    statements.push(TypedStatement::Let {
                        mutable: *mutable,
                        pattern: clone_pattern(pattern),
                        value,
                    });
                }
                Statement::Assign { name, value } => {
                    let ty = scope.get(name).cloned().ok_or_else(|| Error::CannotInfer {
                        context: format!("assignment to '{name}'"),
                    })?;
                    let value = self.expression(value, Some(&ty), &mut scope)?;
                    statements.push(TypedStatement::Assign {
                        name: name.clone(),
                        value,
                    });
                }
                Statement::Expression(value) => statements.push(TypedStatement::Expression(
                    self.expression(value, None, &mut scope)?,
                )),
            }
        }
        let tail = self.expression(&block.tail, expected, &mut scope)?;
        Ok(TypedBlock {
            statements,
            tail: Box::new(tail),
        })
    }

    fn call(
        &self,
        name: &str,
        arguments: &[Expr],
        expected: Option<&Type>,
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        if let Some(index) = self
            .program
            .functions
            .iter()
            .position(|item| item.name == name)
        {
            let function = &self.program.functions[index];
            return self.typed_call(
                Callable::Function(FunctionId {
                    module: self.module,
                    index,
                }),
                name,
                &function
                    .parameters
                    .iter()
                    .map(|item| item.ty.clone())
                    .collect::<Vec<_>>(),
                if function.is_async {
                    Type::Named {
                        name: "Future".into(),
                        arguments: vec![function.result.clone()],
                    }
                } else {
                    function.result.clone()
                },
                arguments,
                scope,
            );
        }
        if let Some((enumeration, variant)) =
            self.program
                .enums
                .iter()
                .enumerate()
                .find_map(|(enum_index, item)| {
                    item.variants
                        .iter()
                        .position(|variant| variant.name == name)
                        .map(|variant| (enum_index, variant))
                })
        {
            let enumeration_def = &self.program.enums[enumeration];
            let variant_def = &enumeration_def.variants[variant];
            if arguments.len() != variant_def.fields.len() {
                return Err(Error::ArgumentCount {
                    callable: name.into(),
                    expected: variant_def.fields.len(),
                    actual: arguments.len(),
                });
            }
            if let Some(result) = expected.filter(|ty| ty.name() == enumeration_def.name) {
                let substitutions = enumeration_def
                    .type_parameters
                    .iter()
                    .zip(result.arguments())
                    .map(|(name, ty)| (name.as_str(), ty))
                    .collect::<HashMap<_, _>>();
                let parameters = variant_def
                    .fields
                    .iter()
                    .map(|ty| substitute(ty, &substitutions))
                    .collect::<Vec<_>>();
                return self.typed_call(
                    Callable::Variant(VariantId {
                        module: self.module,
                        enumeration,
                        variant,
                    }),
                    name,
                    &parameters,
                    result.clone(),
                    arguments,
                    scope,
                );
            }
            let typed_arguments = arguments
                .iter()
                .map(|argument| self.expression(argument, None, scope))
                .collect::<Result<Vec<_>, _>>()?;
            let parameters = enumeration_def
                .type_parameters
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut inferred = HashMap::new();
            for (declared, argument) in variant_def.fields.iter().zip(&typed_arguments) {
                infer_type_parameters(declared, &argument.ty, &parameters, &mut inferred)?;
            }
            let result = Type::Named {
                name: enumeration_def.name.clone(),
                arguments: parameters
                    .iter()
                    .map(|parameter| {
                        inferred
                            .get(parameter)
                            .cloned()
                            .ok_or_else(|| Error::CannotInfer {
                                context: format!(
                                    "type parameter '{parameter}' in call to '{name}'"
                                ),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            };
            let substitutions = enumeration_def
                .type_parameters
                .iter()
                .zip(result.arguments())
                .map(|(name, ty)| (name.as_str(), ty))
                .collect::<HashMap<_, _>>();
            let resolved_parameters = variant_def
                .fields
                .iter()
                .map(|ty| substitute(ty, &substitutions))
                .collect::<Vec<_>>();
            for (parameter, argument) in resolved_parameters.iter().zip(&typed_arguments) {
                self.expect(parameter, &argument.ty, &format!("argument to {name}"))?;
            }
            return Ok(leaf(
                result,
                TypedExprKind::Call {
                    target: Callable::Variant(VariantId {
                        module: self.module,
                        enumeration,
                        variant,
                    }),
                    arguments: typed_arguments,
                },
            ));
        }
        if let Some(index) = self.synthetic.iter().position(|item| item.name == name) {
            let function = &self.synthetic[index];
            return self.typed_call(
                Callable::Synthetic(SyntheticFunctionId {
                    module: self.module,
                    index,
                }),
                name,
                &function.parameters,
                function.result.clone(),
                arguments,
                scope,
            );
        }
        if let Some(intrinsic) = Intrinsic::from_name(name) {
            let (parameters, result) = oche_frontend::intrinsic_signature(name)
                .expect("HIR intrinsics match frontend signatures");
            return self.typed_call(
                Callable::Intrinsic(intrinsic),
                name,
                &parameters,
                result,
                arguments,
                scope,
            );
        }
        Err(Error::CannotInfer {
            context: format!("call to '{name}'"),
        })
    }

    fn qualified_call(
        &self,
        module: &str,
        name: &str,
        arguments: &[Expr],
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        if self.is_std_json_namespace(module) {
            return self.json_call(module, name, arguments, scope);
        }
        if self.is_std_isolate_namespace(module) {
            return self.isolate_call(module, name, arguments, scope);
        }
        if let Some((library, function)) =
            self.program
                .dart_libraries
                .iter()
                .enumerate()
                .find_map(|(library_index, library)| {
                    (library.namespace == module).then(|| {
                        library
                            .functions
                            .iter()
                            .position(|function| function.name == name)
                            .map(|function| (library_index, function))
                    })?
                })
        {
            let definition = &self.program.dart_libraries[library].functions[function];
            return self.typed_call(
                Callable::External(ExternalFunctionId {
                    module: self.module,
                    library,
                    function,
                }),
                &format!("{module}.{name}"),
                &definition
                    .parameters
                    .iter()
                    .map(|item| item.ty.clone())
                    .collect::<Vec<_>>(),
                definition.result.clone(),
                arguments,
                scope,
            );
        }

        if let Some(target_module) = self.imported_module(module) {
            let target_program = &self
                .package
                .expect("an imported module requires a linked package")
                .module(target_module)
                .program;
            if let Some((function, definition)) = target_program
                .functions
                .iter()
                .enumerate()
                .find(|(_, function)| function.is_public && function.name == name)
            {
                for ty in definition
                    .parameters
                    .iter()
                    .map(|parameter| &parameter.ty)
                    .chain(std::iter::once(&definition.result))
                {
                    if !module_boundary_type(ty) {
                        return Err(Error::UnsupportedModuleBoundaryType {
                            module: module.into(),
                            name: name.into(),
                            ty: ty.clone(),
                        });
                    }
                }
                let result = if definition.is_async {
                    Type::Named {
                        name: "Future".into(),
                        arguments: vec![definition.result.clone()],
                    }
                } else {
                    definition.result.clone()
                };
                return self.typed_call(
                    Callable::Function(FunctionId {
                        module: target_module,
                        index: function,
                    }),
                    &format!("{module}.{name}"),
                    &definition
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty.clone())
                        .collect::<Vec<_>>(),
                    result,
                    arguments,
                    scope,
                );
            }
        }

        Err(Error::UnknownQualifiedFunction {
            module: module.into(),
            name: name.into(),
        })
    }

    fn is_std_json_namespace(&self, alias: &str) -> bool {
        self.program.imports.iter().any(|import| {
            import.alias == alias && matches!(import.path.as_str(), "std/json" | "std/json.oche")
        })
    }

    fn is_std_isolate_namespace(&self, alias: &str) -> bool {
        self.program.imports.iter().any(|import| {
            import.alias == alias
                && matches!(import.path.as_str(), "std/isolate" | "std/isolate.oche")
        })
    }

    fn isolate_call(
        &self,
        module: &str,
        name: &str,
        arguments: &[Expr],
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        if name != "run" {
            return Err(Error::UnknownQualifiedFunction {
                module: module.into(),
                name: name.into(),
            });
        }
        if arguments.len() != 1 {
            return Err(Error::ArgumentCount {
                callable: format!("{module}.{name}"),
                expected: 1,
                actual: arguments.len(),
            });
        }
        let value = self.expression(&arguments[0], None, scope)?;
        let result = if value.ty.name() == "Future" && value.ty.arguments().len() == 1 {
            value.ty.arguments()[0].clone()
        } else {
            value.ty.clone()
        };
        Ok(leaf(
            Type::Named {
                name: "Future".into(),
                arguments: vec![result],
            },
            TypedExprKind::IsolateRun(Box::new(value)),
        ))
    }

    fn json_call(
        &self,
        module: &str,
        name: &str,
        arguments: &[Expr],
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        let qualified = format!("{module}.{name}");
        match name {
            "parse" | "stringify" | "object" | "array" | "fromInt" | "fromFloat" | "fromBool"
            | "fromString" | "asObject" | "asArray" | "asInt" | "asFloat" | "asBool"
            | "asString" | "field" => {
                let intrinsic_name = match name {
                    "parse" => "_oche_json_parse",
                    "stringify" => "_oche_json_stringify",
                    "object" => "_oche_json_object",
                    "array" => "_oche_json_array",
                    "fromInt" => "_oche_json_from_int",
                    "fromFloat" => "_oche_json_from_float",
                    "fromBool" => "_oche_json_from_bool",
                    "fromString" => "_oche_json_from_string",
                    "asObject" => "_oche_json_as_object",
                    "asArray" => "_oche_json_as_array",
                    "asInt" => "_oche_json_as_int",
                    "asFloat" => "_oche_json_as_float",
                    "asBool" => "_oche_json_as_bool",
                    "asString" => "_oche_json_as_string",
                    "field" => "_oche_json_field",
                    _ => unreachable!(),
                };
                let intrinsic = Intrinsic::from_name(intrinsic_name)
                    .expect("public JSON operations map to frontend intrinsics");
                let (parameters, result) = oche_frontend::intrinsic_signature(intrinsic_name)
                    .expect("public JSON operations map to frontend signatures");
                self.typed_call(
                    Callable::Intrinsic(intrinsic),
                    &qualified,
                    &parameters,
                    result,
                    arguments,
                    scope,
                )
            }
            "encode" | "decode" | "toValue" | "fromValue" => {
                if arguments.len() != 2 {
                    return Err(Error::ArgumentCount {
                        callable: qualified,
                        expected: 2,
                        actual: arguments.len(),
                    });
                }
                let Expr::TypeValue(Type::Named {
                    name: target,
                    arguments: type_arguments,
                }) = arguments[0].unlocated()
                else {
                    return Err(Error::CannotInfer {
                        context: format!("type argument in call to '{qualified}'"),
                    });
                };
                if !type_arguments.is_empty() {
                    return Err(Error::CannotInfer {
                        context: format!("generic type argument in call to '{qualified}'"),
                    });
                }
                if target == "JsonValue" {
                    return match name {
                        "encode" => self.json_call(module, "stringify", &arguments[1..], scope),
                        "decode" => self.json_call(module, "parse", &arguments[1..], scope),
                        "toValue" | "fromValue" => {
                            self.expression(&arguments[1], Some(&named("JsonValue")), scope)
                        }
                        _ => unreachable!(),
                    };
                }
                let generated = match name {
                    "encode" => json_encode_function(target),
                    "decode" => json_decode_function(target),
                    "toValue" => json_encode_value_function(target),
                    "fromValue" => json_decode_value_function(target),
                    _ => unreachable!(),
                };
                self.call(&generated, &arguments[1..], None, scope)
            }
            _ => Err(Error::UnknownQualifiedFunction {
                module: module.into(),
                name: name.into(),
            }),
        }
    }

    fn imported_module(&self, alias: &str) -> Option<ModuleId> {
        let package = self.package?;
        package
            .module(self.module)
            .imports
            .iter()
            .find(|import| import.alias == alias)
            .map(|import| import.module)
    }

    fn typed_call(
        &self,
        target: Callable,
        name: &str,
        parameters: &[Type],
        result: Type,
        arguments: &[Expr],
        scope: &mut HashMap<String, Type>,
    ) -> Result<TypedExpr, Error> {
        if arguments.len() != parameters.len() {
            return Err(Error::ArgumentCount {
                callable: name.into(),
                expected: parameters.len(),
                actual: arguments.len(),
            });
        }
        let arguments = arguments
            .iter()
            .zip(parameters)
            .map(|(argument, parameter)| self.expression(argument, Some(parameter), scope))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(leaf(result, TypedExprKind::Call { target, arguments }))
    }

    fn pattern(
        &self,
        pattern: &Pattern,
        subject: &Type,
        scope: &mut HashMap<String, Type>,
    ) -> Result<(), Error> {
        if let Pattern::Located { span, value } = pattern {
            return self
                .pattern(value, subject, scope)
                .map_err(|source| source.at(*span));
        }
        match pattern {
            Pattern::Located { .. } => unreachable!("located patterns return before checking"),
            Pattern::Wildcard => Ok(()),
            Pattern::Binding(name) => {
                scope.insert(name.clone(), subject.clone());
                Ok(())
            }
            Pattern::Integer(_) => self.expect(&named("Int"), subject, "integer pattern"),
            Pattern::Float(_) => self.expect(&named("Float"), subject, "float pattern"),
            Pattern::Boolean(_) => self.expect(&named("Bool"), subject, "boolean pattern"),
            Pattern::String(_) => self.expect(&named("String"), subject, "string pattern"),
            Pattern::Struct { name, fields, .. } => {
                if subject.name() != name {
                    return Err(Error::InvalidPattern {
                        pattern: name.clone(),
                        subject: subject.clone(),
                    });
                }
                let structure = self
                    .program
                    .structs
                    .iter()
                    .find(|item| item.name == *name)
                    .expect("struct patterns were validated by the frontend");
                for (name, pattern) in fields {
                    let ty = &structure
                        .fields
                        .iter()
                        .find(|item| item.name == *name)
                        .expect("struct fields were validated by the frontend")
                        .ty;
                    self.pattern(pattern, ty, scope)?;
                }
                Ok(())
            }
            Pattern::Variant { name, fields } => {
                let enumeration = self
                    .program
                    .enums
                    .iter()
                    .find(|item| item.name == subject.name())
                    .ok_or_else(|| Error::InvalidPattern {
                        pattern: name.clone(),
                        subject: subject.clone(),
                    })?;
                let variant = enumeration
                    .variants
                    .iter()
                    .find(|item| item.name == *name)
                    .ok_or_else(|| Error::InvalidPattern {
                        pattern: name.clone(),
                        subject: subject.clone(),
                    })?;
                let substitutions = enumeration
                    .type_parameters
                    .iter()
                    .zip(subject.arguments())
                    .map(|(name, ty)| (name.as_str(), ty))
                    .collect::<HashMap<_, _>>();
                for (pattern, ty) in fields.iter().zip(&variant.fields) {
                    self.pattern(pattern, &substitute(ty, &substitutions), scope)?;
                }
                Ok(())
            }
        }
    }

    fn uncovered(&self, rows: &[Vec<&Pattern>], types: &[Type]) -> Option<Vec<String>> {
        let Some((ty, remaining_types)) = types.split_first() else {
            return rows.is_empty().then(Vec::new);
        };
        let constructors = self.constructors(ty);
        if constructors.is_empty() {
            let defaults = rows
                .iter()
                .filter(|row| row.first().is_some_and(|pattern| is_catch_all(pattern)))
                .map(|row| row[1..].to_vec())
                .collect::<Vec<_>>();
            return self
                .uncovered(&defaults, remaining_types)
                .map(|mut witness| {
                    witness.insert(0, "_".into());
                    witness
                });
        }
        for constructor in constructors {
            let argument_types = self.constructor_argument_types(constructor, ty);
            let mut specialized_types = argument_types.clone();
            specialized_types.extend_from_slice(remaining_types);
            let specialized_rows = rows
                .iter()
                .filter_map(|row| self.specialize(row, constructor))
                .collect::<Vec<_>>();
            if let Some(mut witness) = self.uncovered(&specialized_rows, &specialized_types) {
                let remaining = witness.split_off(argument_types.len());
                let head = self.format_constructor(constructor, &witness);
                let mut result = vec![head];
                result.extend(remaining);
                return Some(result);
            }
        }
        None
    }

    fn useful(&self, rows: &[Vec<&Pattern>], candidate: &[&Pattern], types: &[Type]) -> bool {
        let Some((pattern, candidate_tail)) = candidate.split_first() else {
            return rows.is_empty();
        };
        let (ty, remaining_types) = types
            .split_first()
            .expect("a pattern column always has a corresponding type");
        let constructors = self.constructors(ty);
        if is_catch_all(pattern) {
            if constructors.is_empty() {
                let defaults = rows
                    .iter()
                    .filter(|row| row.first().is_some_and(|pattern| is_catch_all(pattern)))
                    .map(|row| row[1..].to_vec())
                    .collect::<Vec<_>>();
                return self.useful(&defaults, candidate_tail, remaining_types);
            }
            return constructors.into_iter().any(|constructor| {
                let argument_types = self.constructor_argument_types(constructor, ty);
                let mut specialized_types = argument_types;
                specialized_types.extend_from_slice(remaining_types);
                let specialized_rows = rows
                    .iter()
                    .filter_map(|row| self.specialize(row, constructor))
                    .collect::<Vec<_>>();
                let specialized_candidate = self
                    .specialize(candidate, constructor)
                    .expect("a catch-all pattern specializes to every constructor");
                self.useful(
                    &specialized_rows,
                    &specialized_candidate,
                    &specialized_types,
                )
            });
        }
        if let Some(constructor) = self.pattern_constructor(pattern) {
            let argument_types = self.constructor_argument_types(constructor, ty);
            let mut specialized_types = argument_types;
            specialized_types.extend_from_slice(remaining_types);
            let specialized_rows = rows
                .iter()
                .filter_map(|row| self.specialize(row, constructor))
                .collect::<Vec<_>>();
            let specialized_candidate = self
                .specialize(candidate, constructor)
                .expect("a constructor pattern specializes to itself");
            return self.useful(
                &specialized_rows,
                &specialized_candidate,
                &specialized_types,
            );
        }
        let specialized_rows = rows
            .iter()
            .filter_map(|row| specialize_literal(row, pattern))
            .collect::<Vec<_>>();
        self.useful(&specialized_rows, candidate_tail, remaining_types)
    }

    fn constructors(&self, ty: &Type) -> Vec<CoverageConstructor> {
        if ty.name() == "Bool" {
            return vec![
                CoverageConstructor::Bool(false),
                CoverageConstructor::Bool(true),
            ];
        }
        if let Some(enumeration) = self
            .program
            .enums
            .iter()
            .position(|enumeration| enumeration.name == ty.name())
        {
            return (0..self.program.enums[enumeration].variants.len())
                .map(|variant| CoverageConstructor::Variant {
                    enumeration,
                    variant,
                })
                .collect();
        }
        self.program
            .structs
            .iter()
            .position(|structure| structure.name == ty.name())
            .map(CoverageConstructor::Struct)
            .into_iter()
            .collect()
    }

    fn pattern_constructor(&self, pattern: &Pattern) -> Option<CoverageConstructor> {
        match pattern.unlocated() {
            Pattern::Located { .. } => unreachable!("unlocated pattern cannot be located"),
            Pattern::Boolean(value) => Some(CoverageConstructor::Bool(*value)),
            Pattern::Variant { name, .. } => {
                self.program
                    .enums
                    .iter()
                    .enumerate()
                    .find_map(|(enumeration, item)| {
                        item.variants
                            .iter()
                            .position(|variant| variant.name == *name)
                            .map(|variant| CoverageConstructor::Variant {
                                enumeration,
                                variant,
                            })
                    })
            }
            Pattern::Struct { name, .. } => self
                .program
                .structs
                .iter()
                .position(|structure| structure.name == *name)
                .map(CoverageConstructor::Struct),
            Pattern::Wildcard
            | Pattern::Binding(_)
            | Pattern::Integer(_)
            | Pattern::Float(_)
            | Pattern::String(_) => None,
        }
    }

    fn constructor_argument_types(&self, constructor: CoverageConstructor, ty: &Type) -> Vec<Type> {
        match constructor {
            CoverageConstructor::Bool(_) => Vec::new(),
            CoverageConstructor::Struct(index) => self.program.structs[index]
                .fields
                .iter()
                .map(|field| field.ty.clone())
                .collect(),
            CoverageConstructor::Variant {
                enumeration,
                variant,
            } => {
                let enumeration = &self.program.enums[enumeration];
                let substitutions = enumeration
                    .type_parameters
                    .iter()
                    .zip(ty.arguments())
                    .map(|(name, ty)| (name.as_str(), ty))
                    .collect::<HashMap<_, _>>();
                enumeration.variants[variant]
                    .fields
                    .iter()
                    .map(|field| substitute(field, &substitutions))
                    .collect()
            }
        }
    }

    fn specialize<'a>(
        &self,
        row: &[&'a Pattern],
        constructor: CoverageConstructor,
    ) -> Option<Vec<&'a Pattern>> {
        let (pattern, tail) = row.split_first()?;
        let pattern = pattern.unlocated();
        let argument_count = match constructor {
            CoverageConstructor::Bool(_) => 0,
            CoverageConstructor::Struct(index) => self.program.structs[index].fields.len(),
            CoverageConstructor::Variant {
                enumeration,
                variant,
            } => self.program.enums[enumeration].variants[variant]
                .fields
                .len(),
        };
        let mut result = match (pattern, constructor) {
            (Pattern::Located { .. }, _) => {
                unreachable!("unlocated pattern cannot be located")
            }
            (Pattern::Wildcard | Pattern::Binding(_), _) => {
                vec![&WILDCARD_PATTERN; argument_count]
            }
            (Pattern::Boolean(value), CoverageConstructor::Bool(expected))
                if *value == expected =>
            {
                Vec::new()
            }
            (
                Pattern::Variant { name, fields },
                CoverageConstructor::Variant {
                    enumeration,
                    variant,
                },
            ) if *name == self.program.enums[enumeration].variants[variant].name => {
                fields.iter().collect()
            }
            (Pattern::Struct { name, fields, .. }, CoverageConstructor::Struct(index))
                if *name == self.program.structs[index].name =>
            {
                self.program.structs[index]
                    .fields
                    .iter()
                    .map(|field| {
                        fields
                            .iter()
                            .find(|(name, _)| *name == field.name)
                            .map_or(&WILDCARD_PATTERN, |(_, pattern)| pattern)
                    })
                    .collect()
            }
            _ => return None,
        };
        result.extend_from_slice(tail);
        Some(result)
    }

    fn format_constructor(&self, constructor: CoverageConstructor, fields: &[String]) -> String {
        match constructor {
            CoverageConstructor::Bool(value) => value.to_string(),
            CoverageConstructor::Variant {
                enumeration,
                variant,
            } => {
                let name = &self.program.enums[enumeration].variants[variant].name;
                if fields.is_empty() {
                    name.clone()
                } else {
                    format!("{name}({})", fields.join(", "))
                }
            }
            CoverageConstructor::Struct(index) => {
                let structure = &self.program.structs[index];
                let fields = structure
                    .fields
                    .iter()
                    .zip(fields)
                    .map(|(field, pattern)| format!("{}: {pattern}", field.name))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} {{ {fields} }}", structure.name)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CoverageConstructor {
    Bool(bool),
    Variant { enumeration: usize, variant: usize },
    Struct(usize),
}

static WILDCARD_PATTERN: Pattern = Pattern::Wildcard;

fn is_catch_all(pattern: &Pattern) -> bool {
    matches!(pattern.unlocated(), Pattern::Wildcard | Pattern::Binding(_))
}

fn specialize_literal<'a>(row: &[&'a Pattern], literal: &Pattern) -> Option<Vec<&'a Pattern>> {
    let (pattern, tail) = row.split_first()?;
    (is_catch_all(pattern) || same_literal(pattern, literal)).then(|| tail.to_vec())
}

fn same_literal(left: &Pattern, right: &Pattern) -> bool {
    match (left.unlocated(), right.unlocated()) {
        (Pattern::Integer(left), Pattern::Integer(right)) => left == right,
        (Pattern::Float(left), Pattern::Float(right)) => left == right,
        (Pattern::String(left), Pattern::String(right)) => left == right,
        _ => false,
    }
}

fn leaf(ty: Type, kind: TypedExprKind) -> TypedExpr {
    TypedExpr {
        span: None,
        ty,
        kind,
    }
}

fn substitute(ty: &Type, substitutions: &HashMap<&str, &Type>) -> Type {
    let Type::Named { name, arguments } = ty;
    if arguments.is_empty()
        && let Some(ty) = substitutions.get(name.as_str())
    {
        return (*ty).clone();
    }
    Type::Named {
        name: name.clone(),
        arguments: arguments
            .iter()
            .map(|argument| substitute(argument, substitutions))
            .collect(),
    }
}

fn infer_type_parameters<'a>(
    declared: &Type,
    actual: &Type,
    parameters: &[&'a str],
    inferred: &mut HashMap<&'a str, Type>,
) -> Result<(), Error> {
    let Type::Named {
        name: declared_name,
        arguments: declared_arguments,
    } = declared;
    if declared_arguments.is_empty()
        && let Some(parameter) = parameters
            .iter()
            .copied()
            .find(|parameter| *parameter == declared_name)
    {
        if let Some(previous) = inferred.get(parameter) {
            if previous != actual {
                return Err(Error::TypeMismatch {
                    context: format!("inference of type parameter '{parameter}'"),
                    expected: previous.clone(),
                    actual: actual.clone(),
                });
            }
        } else {
            inferred.insert(parameter, actual.clone());
        }
        return Ok(());
    }
    let Type::Named {
        name: actual_name,
        arguments: actual_arguments,
    } = actual;
    if declared_name != actual_name || declared_arguments.len() != actual_arguments.len() {
        return Err(Error::TypeMismatch {
            context: "generic argument inference".into(),
            expected: declared.clone(),
            actual: actual.clone(),
        });
    }
    for (declared, actual) in declared_arguments.iter().zip(actual_arguments) {
        infer_type_parameters(declared, actual, parameters, inferred)?;
    }
    Ok(())
}

fn operator_text(operator: Operator) -> &'static str {
    match operator {
        Operator::Add => "+",
        Operator::Subtract => "-",
        Operator::Multiply => "*",
    }
}

fn clone_pattern(pattern: &Pattern) -> Pattern {
    match pattern {
        Pattern::Located { span, value } => Pattern::Located {
            span: *span,
            value: Box::new(clone_pattern(value)),
        },
        Pattern::Wildcard => Pattern::Wildcard,
        Pattern::Binding(name) => Pattern::Binding(name.clone()),
        Pattern::Variant { name, fields } => Pattern::Variant {
            name: name.clone(),
            fields: fields.iter().map(clone_pattern).collect(),
        },
        Pattern::Struct { name, fields, rest } => Pattern::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, pattern)| (name.clone(), clone_pattern(pattern)))
                .collect(),
            rest: *rest,
        },
        Pattern::Integer(value) => Pattern::Integer(*value),
        Pattern::Float(value) => Pattern::Float(*value),
        Pattern::Boolean(value) => Pattern::Boolean(*value),
        Pattern::String(value) => Pattern::String(value.clone()),
    }
}

trait TypeArguments {
    fn arguments(&self) -> &[Type];
}

impl TypeArguments for Type {
    fn arguments(&self) -> &[Type] {
        match self {
            Type::Named { arguments, .. } => arguments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_isolate_run_as_a_future_and_flattens_async_results() {
        let program = oche_frontend::parse(
            r#"
            import "std/isolate" as isolate
            async fn later(value: Int) -> Int { value }
            fn syncWork(value: Int) -> Future<Int> { isolate.run(value + 1) }
            fn asyncWork(value: Int) -> Future<Int> { isolate.run(later(value)) }
            "#,
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        let future_int = Type::Named {
            name: "Future".into(),
            arguments: vec![named("Int")],
        };
        assert_eq!(typed.functions[1].body.ty, future_int);
        assert!(matches!(
            typed.functions[1].body.kind,
            TypedExprKind::IsolateRun(_)
        ));
        assert_eq!(typed.functions[2].body.ty, typed.functions[1].body.ty);
    }

    #[test]
    fn resolves_fields_calls_and_comptime_symbols() {
        let program = oche_frontend::parse(
            r#"
            struct User { name: String, age: Int }
            comptime jsonEncode(User)
            comptime jsonDecode(User)
            fn age(name: String) -> Int { User { name: name, age: 37 }.age }
            fn encode(user: User) -> String { user.toJson() }
            fn decode(source: String) -> Result<User, String> { User.fromJson(source) }
            "#,
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.synthetic_functions.len(), 4);
        assert_eq!(
            typed.synthetic_functions[0].structure,
            StructId {
                module: ModuleId::ROOT,
                index: 0,
            }
        );
        assert_eq!(
            typed.synthetic_functions[0].origin,
            SyntheticOrigin::JsonEncodeValue
        );
        assert_eq!(
            typed.synthetic_functions[1].origin,
            SyntheticOrigin::JsonEncode
        );
        assert_eq!(
            typed.synthetic_functions[2].origin,
            SyntheticOrigin::JsonDecodeValue
        );
        assert_eq!(
            typed.synthetic_functions[3].origin,
            SyntheticOrigin::JsonDecode
        );
        assert_eq!(typed.functions[0].body.ty, named("Int"));
        assert!(matches!(
            typed.functions[0].body.kind,
            TypedExprKind::Field {
                structure: StructId {
                    module: ModuleId::ROOT,
                    index: 0,
                },
                field: 1,
                ..
            }
        ));
        assert!(matches!(
            typed.functions[1].body.kind,
            TypedExprKind::Call {
                target: Callable::Synthetic(SyntheticFunctionId {
                    module: ModuleId::ROOT,
                    index: 1,
                }),
                ..
            }
        ));
        assert!(matches!(
            typed.functions[2].body.kind,
            TypedExprKind::Call {
                target: Callable::Synthetic(SyntheticFunctionId {
                    module: ModuleId::ROOT,
                    index: 3,
                }),
                ..
            }
        ));
    }

    #[test]
    fn types_json_namespace_before_comptime_expansion() {
        let program = oche_frontend::parse(
            r#"
            import "std/json" as json
            struct User { name: String }
            fn encode(user: User) -> String { json.encode(type User, user) }
            fn decode(source: String) -> Result<User, String> {
              json.decode(type User, source)
            }
            fn dynamic(source: String) -> Result<JsonValue, String> { json.parse(source) }
            "#,
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.functions[0].body.ty, named("String"));
        assert_eq!(typed.functions[1].body.ty, result(named("User")));
        assert_eq!(typed.functions[2].body.ty, result(named("JsonValue")));
    }

    #[test]
    fn qualifies_resolved_identities_with_the_module() {
        let program = oche_frontend::parse(
            "struct Boxed { value: Int }
             comptime jsonEncode(Boxed)
             comptime jsonDecode(Boxed)
             enum Item { Value(Int), Empty }
             fn helper(value: Int) -> Int { value }
             fn call(value: Int) -> Int { helper(value) }
             fn construct(value: Int) -> Boxed { Boxed { value: value } }
             fn variant(value: Int) -> Item { Value(value) }",
        )
        .unwrap();
        let module = ModuleId::new(7);
        let typed = lower_module(module, &program).unwrap();
        assert_eq!(typed.module, module);
        assert_eq!(typed.functions[1].id, FunctionId { module, index: 1 });
        assert!(matches!(
            typed.functions[1].body.kind,
            TypedExprKind::Call {
                target: Callable::Function(FunctionId {
                    module: target_module,
                    index: 0,
                }),
                ..
            } if target_module == module
        ));
        assert!(matches!(
            typed.functions[2].body.kind,
            TypedExprKind::StructLiteral {
                structure: StructId {
                    module: target_module,
                    index: 0,
                },
                ..
            } if target_module == module
        ));
        assert!(matches!(
            typed.functions[3].body.kind,
            TypedExprKind::Call {
                target: Callable::Variant(VariantId {
                    module: target_module,
                    enumeration: 2,
                    variant: 0,
                }),
                ..
            } if target_module == module
        ));
        assert_eq!(typed.synthetic_functions[0].structure.module, module);
    }

    #[test]
    fn types_async_calls_and_await() {
        let program = oche_frontend::parse(
            "async fn later(value: Int) -> Int { value + 1 }
             async fn use(value: Int) -> Int { later(value).await }",
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.functions[1].body.ty, named("Int"));
        let TypedExprKind::Await(call) = &typed.functions[1].body.kind else {
            panic!("expected await expression");
        };
        assert_eq!(
            call.ty,
            Type::Named {
                name: "Future".into(),
                arguments: vec![named("Int")],
            }
        );

        let invalid = oche_frontend::parse("async fn bad() -> Int { 1.await }").unwrap();
        assert!(matches!(lower(&invalid), Err(Error::InvalidAwait { .. })));

        let mut outside = oche_frontend::parse("async fn bad() -> Int { bad().await }").unwrap();
        outside.functions[0].is_async = false;
        assert!(matches!(lower(&outside), Err(Error::AwaitOutsideAsync)));
    }

    #[test]
    fn rejects_expression_type_mismatches() {
        let source = "fn wrong() -> Int { \"text\" }";
        let program = oche_frontend::parse(source).unwrap();
        let typed = lower(&program);
        let located = lower_located(&program).unwrap_err();
        assert_eq!(
            located.span,
            Some(SourceSpan {
                start: source.find("\"text\"").unwrap(),
                end: source.find("\"text\"").unwrap() + "\"text\"".len(),
            })
        );
        assert!(matches!(*located.source, Error::TypeMismatch { .. }));
        let error = typed.unwrap_err();
        assert!(error.to_string().contains("expected Int, received String"));
    }

    #[test]
    fn types_float_arithmetic_without_numeric_coercion() {
        let program = oche_frontend::parse(
            "fn scale(value: Float) -> Float { value * 1.5 }
             fn negative(value: Float) -> Float { -value }",
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.functions[0].body.ty, named("Float"));
        assert_eq!(typed.functions[1].body.ty, named("Float"));

        let mixed = oche_frontend::parse("fn bad(value: Float) -> Float { value + 1 }").unwrap();
        assert!(matches!(lower(&mixed), Err(Error::TypeMismatch { .. })));
    }

    #[test]
    fn infers_generic_variants_in_local_bindings() {
        let program = oche_frontend::parse(
            "fn unwrap() -> Int { let value = Some(41); match value { Some(inner) => inner, None => 0 } }",
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        let TypedExprKind::Block(block) = &typed.functions[0].body.kind else {
            panic!("expected a typed block");
        };
        let TypedStatement::Let { value, .. } = &block.statements[0] else {
            panic!("expected a typed let statement");
        };
        assert_eq!(value.ty.to_string(), "Option<Int>");
    }

    #[test]
    fn checks_match_exhaustiveness_with_typed_nested_patterns() {
        let booleans = oche_frontend::parse(
            "fn choose(value: Bool) -> Int { match value { true => 1, false => 0 } }",
        )
        .unwrap();
        lower(&booleans).unwrap();

        let option = oche_frontend::parse(
            "fn choose(value: Option<Bool>) -> Int {
               match value { Some(true) => 1, Some(false) => 0, None => -1 }
             }",
        )
        .unwrap();
        lower(&option).unwrap();

        let incomplete = oche_frontend::parse(
            "enum Pair { Both(Bool, Bool), Empty }
             fn choose(value: Pair) -> Int {
               match value { Both(true, true) => 1, Both(false, false) => 0, Empty => -1 }
             }",
        )
        .unwrap();
        let error = lower(&incomplete).unwrap_err();
        assert!(matches!(
            error,
            Error::NonExhaustiveMatch { ref missing, .. } if missing == "Both(false, true)"
        ));

        let structures = oche_frontend::parse(
            "struct Flags { first: Bool, second: Bool }
             fn choose(value: Flags) -> Int {
               match value {
                 Flags { first: true, .. } => 1,
                 Flags { first: false, .. } => 0,
               }
             }",
        )
        .unwrap();
        lower(&structures).unwrap();

        let integer =
            oche_frontend::parse("fn choose(value: Int) -> Int { match value { 0 => 0, 1 => 1 } }")
                .unwrap();
        assert!(matches!(
            lower(&integer),
            Err(Error::NonExhaustiveMatch { ref missing, .. }) if missing == "_"
        ));
    }

    #[test]
    fn rejects_match_arms_covered_by_previous_patterns() {
        for source in [
            "fn choose(value: Bool) -> Int {
               match value { true => 1, true => 2, false => 0 }
             }",
            "fn choose(value: Option<Bool>) -> Int {
               match value { Some(_) => 1, Some(true) => 2, None => 0 }
             }",
            "struct Flags { first: Bool, second: Bool }
             fn choose(value: Flags) -> Int {
               match value {
                 Flags { first: true, .. } => 1,
                 Flags { first: true, second: false } => 2,
                 _ => 0,
               }
             }",
            "fn choose(value: Int) -> Int { match value { _ => 0, 1 => 1 } }",
        ] {
            let program = oche_frontend::parse(source).unwrap();
            assert!(matches!(lower(&program), Err(Error::RedundantMatchArm)));
        }

        let useful_literals = oche_frontend::parse(
            "fn choose(value: Int) -> Int { match value { 0 => 0, 1 => 1, _ => 2 } }",
        )
        .unwrap();
        lower(&useful_literals).unwrap();
    }

    #[test]
    fn types_guards_and_excludes_them_from_exhaustive_coverage() {
        let valid = oche_frontend::parse(
            "fn allowed(value: Int) -> Bool { true }
             fn choose(value: Option<Int>) -> Int {
               match value { Some(inner) if allowed(inner) => inner, Some(_) => 0, None => -1 }
             }",
        )
        .unwrap();
        let typed = lower(&valid).unwrap();
        let TypedExprKind::Match { arms, .. } = &typed.functions[1].body.kind else {
            panic!("expected typed match");
        };
        assert_eq!(arms[0].guard.as_ref().unwrap().ty, named("Bool"));

        let non_exhaustive = oche_frontend::parse(
            "fn allowed(value: Int) -> Bool { true }
             fn choose(value: Option<Int>) -> Int {
               match value { Some(inner) if allowed(inner) => inner, None => -1 }
             }",
        )
        .unwrap();
        assert!(matches!(
            lower(&non_exhaustive),
            Err(Error::NonExhaustiveMatch { .. })
        ));

        let wrong_source = "fn choose(value: Option<Int>) -> Int {
               match value { Some(inner) if inner => inner, Some(_) => 0, None => -1 }
             }";
        let wrong_type = oche_frontend::parse(wrong_source).unwrap();
        let located = lower_located(&wrong_type).unwrap_err();
        let guard_start = wrong_source.find("if inner").unwrap() + "if ".len();
        assert_eq!(
            located.span,
            Some(SourceSpan {
                start: guard_start,
                end: guard_start + "inner".len(),
            })
        );
        assert!(matches!(
            lower(&wrong_type),
            Err(Error::TypeMismatch { .. })
        ));

        let redundant = oche_frontend::parse(
            "fn allowed(value: Int) -> Bool { true }
             fn choose(value: Option<Int>) -> Int {
               match value { Some(_) => 0, Some(inner) if allowed(inner) => inner, None => -1 }
             }",
        )
        .unwrap();
        assert!(matches!(lower(&redundant), Err(Error::RedundantMatchArm)));
    }

    #[test]
    fn infers_and_checks_list_elements() {
        let program = oche_frontend::parse(
            "fn inferred() -> List<Int> { [1, 2, 3] }
             fn empty() -> List<String> { [] }",
        )
        .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.functions[0].body.ty.to_string(), "List<Int>");
        assert!(matches!(
            &typed.functions[1].body.kind,
            TypedExprKind::List(values) if values.is_empty()
        ));

        let mixed = oche_frontend::parse("fn mixed() -> List<Int> { [1, true] }").unwrap();
        assert!(matches!(lower(&mixed), Err(Error::TypeMismatch { .. })));

        let untyped = oche_frontend::parse("fn bad() -> Int { let values = []; 1 }").unwrap();
        assert!(matches!(lower(&untyped), Err(Error::CannotInfer { .. })));
    }

    #[test]
    fn types_list_concatenation_without_dynamic_values() {
        let program =
            oche_frontend::parse("fn append(values: List<Int>) -> List<Int> { values + [3] }")
                .unwrap();
        let typed = lower(&program).unwrap();
        assert_eq!(typed.functions[0].body.ty.to_string(), "List<Int>");

        let mixed =
            oche_frontend::parse("fn bad(values: List<Int>) -> List<Int> { values + [true] }")
                .unwrap();
        assert!(matches!(lower(&mixed), Err(Error::TypeMismatch { .. })));

        let length =
            oche_frontend::parse("fn count(values: List<String>) -> Int { values.len() }").unwrap();
        let typed = lower(&length).unwrap();
        assert!(matches!(
            typed.functions[0].body.kind,
            TypedExprKind::ListLength(_)
        ));

        let last = oche_frontend::parse(
            "fn last(values: List<String>) -> Option<String> { values.last() }",
        )
        .unwrap();
        let typed = lower(&last).unwrap();
        assert_eq!(typed.functions[0].body.ty.to_string(), "Option<String>");
        assert!(matches!(
            typed.functions[0].body.kind,
            TypedExprKind::ListLast(_)
        ));

        let traversal = oche_frontend::parse(
            "fn first(values: List<String>) -> Option<String> { values.first() }
             fn rest(values: List<String>) -> List<String> { values.rest() }",
        )
        .unwrap();
        let typed = lower(&traversal).unwrap();
        assert!(matches!(
            typed.functions[0].body.kind,
            TypedExprKind::ListFirst(_)
        ));
        assert!(matches!(
            typed.functions[1].body.kind,
            TypedExprKind::ListRest(_)
        ));

        let invalid = oche_frontend::parse("fn bad(value: Int) -> Int { value.len() }").unwrap();
        assert!(matches!(lower(&invalid), Err(Error::UnknownMethod { .. })));
    }

    #[test]
    fn resolves_public_functions_across_linked_modules() {
        let root = oche_frontend::parse(
            "import \"math.oche\" as math\nfn answer() -> Int { math.double(21) }",
        )
        .unwrap();
        let math =
            oche_frontend::parse("pub fn double(value: Int) -> Int { value + value }").unwrap();
        let package = LinkedPackage {
            root: ModuleId::ROOT,
            modules: vec![
                LinkedModule {
                    program: root,
                    imports: vec![ModuleImport {
                        alias: "math".into(),
                        module: ModuleId::new(1),
                    }],
                },
                LinkedModule {
                    program: math,
                    imports: Vec::new(),
                },
            ],
        };

        let typed = lower_package(&package).unwrap();
        assert_eq!(typed.root, ModuleId::ROOT);
        assert_eq!(typed.modules.len(), 2);
        assert!(matches!(
            typed.modules[0].functions[0].body.kind,
            TypedExprKind::Call {
                target: Callable::Function(FunctionId { module, index: 0 }),
                ..
            } if module == ModuleId::new(1)
        ));
    }

    #[test]
    fn hides_private_module_functions_and_rejects_local_data_at_the_boundary() {
        let root = oche_frontend::parse(
            "import \"dependency.oche\" as dependency\nfn use() -> Int { dependency.hidden() }",
        )
        .unwrap();
        let dependency = oche_frontend::parse("fn hidden() -> Int { 1 }").unwrap();
        let package = LinkedPackage {
            root: ModuleId::ROOT,
            modules: vec![
                LinkedModule {
                    program: root,
                    imports: vec![ModuleImport {
                        alias: "dependency".into(),
                        module: ModuleId::new(1),
                    }],
                },
                LinkedModule {
                    program: dependency,
                    imports: Vec::new(),
                },
            ],
        };
        assert!(matches!(
            lower_package(&package),
            Err(Error::UnknownQualifiedFunction { .. })
        ));

        let root = oche_frontend::parse(
            "import \"dependency.oche\" as dependency\nfn use() -> Int { dependency.user_count() }",
        )
        .unwrap();
        let dependency = oche_frontend::parse(
            "struct User { name: String }\npub fn user_count(user: User) -> Int { 1 }",
        )
        .unwrap();
        let package = LinkedPackage {
            root: ModuleId::ROOT,
            modules: vec![
                LinkedModule {
                    program: root,
                    imports: vec![ModuleImport {
                        alias: "dependency".into(),
                        module: ModuleId::new(1),
                    }],
                },
                LinkedModule {
                    program: dependency,
                    imports: Vec::new(),
                },
            ],
        };
        assert!(matches!(
            lower_package(&package),
            Err(Error::UnsupportedModuleBoundaryType { .. })
        ));
    }
}
