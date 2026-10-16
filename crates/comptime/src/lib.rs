//! Deterministic compile-time derivation for schema-backed code generation.

use std::collections::{HashMap, HashSet};

use oche_frontend::{ComptimeKind, Program, Type, ValidationRule};
use snafu::Snafu;

fn named(name: &str) -> Type {
    Type::Named {
        name: name.into(),
        arguments: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub validators: Vec<ValidatorDerive>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorDerive {
    pub struct_index: usize,
    pub rules: Vec<ResolvedRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRule {
    pub field_index: usize,
    pub rule: ValidationRule,
}

#[derive(Debug, PartialEq, Snafu)]
pub enum Error {
    #[snafu(display("Unknown comptime target '{target}'"))]
    UnknownTarget { offset: usize, target: String },
    #[snafu(display("Duplicate comptime {operation} derivation for '{target}'"))]
    DuplicateDerive {
        offset: usize,
        operation: &'static str,
        target: String,
    },
    #[snafu(display("Unknown field '{field}' on '{target}'"))]
    UnknownField {
        offset: usize,
        target: String,
        field: String,
    },
    #[snafu(display("Duplicate validation rule for '{target}.{field}'"))]
    DuplicateRule {
        offset: usize,
        target: String,
        field: String,
    },
    #[snafu(display("Validation rule '{rule}' cannot be applied to {target}.{field}: {ty}"))]
    IncompatibleRule {
        offset: usize,
        target: String,
        field: String,
        ty: String,
        rule: &'static str,
    },
    #[snafu(display("Invalid range for '{target}.{field}': {minimum} is greater than {maximum}"))]
    InvalidRange {
        offset: usize,
        target: String,
        field: String,
        minimum: i64,
        maximum: i64,
    },
    #[snafu(display("Invalid comptime expression: {message}"))]
    InvalidExpression { offset: usize, message: String },
}

impl Error {
    pub fn offset(&self) -> usize {
        match self {
            Self::UnknownTarget { offset, .. }
            | Self::DuplicateDerive { offset, .. }
            | Self::UnknownField { offset, .. }
            | Self::DuplicateRule { offset, .. }
            | Self::IncompatibleRule { offset, .. }
            | Self::InvalidRange { offset, .. }
            | Self::InvalidExpression { offset, .. } => *offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Type(Type),
    List(Vec<Value>),
    Struct {
        name: String,
        fields: Vec<(String, Value)>,
    },
}

impl Value {
    fn from_literal(
        expression: &oche_frontend::Expr,
        expected: &Type,
        structs: &HashMap<&str, &oche_frontend::Struct>,
    ) -> Option<Self> {
        use oche_frontend::Expr;
        let Type::Named { name, arguments } = expected;
        match (name.as_str(), arguments.as_slice(), expression.unlocated()) {
            ("Int", [], Expr::Integer(value)) => Some(Self::Int(*value)),
            ("Float", [], Expr::Float(value)) => Some(Self::Float(*value)),
            ("Bool", [], Expr::Boolean(value)) => Some(Self::Bool(*value)),
            ("String", [], Expr::String(value)) => Some(Self::String(value.clone())),
            ("Type", [], Expr::TypeValue(value)) => Some(Self::Type(value.clone())),
            ("List", [element], Expr::List(values)) => values
                .iter()
                .map(|value| Self::from_literal(value, element, structs))
                .collect::<Option<Vec<_>>>()
                .map(Self::List),
            (
                _,
                [],
                Expr::StructLiteral {
                    name: actual,
                    fields,
                },
            ) if actual == name => {
                let structure = structs.get(name.as_str())?;
                let fields = structure
                    .fields
                    .iter()
                    .map(|field| {
                        let value = fields
                            .iter()
                            .find_map(|(name, value)| (name == &field.name).then_some(value))?;
                        Some((
                            field.name.clone(),
                            Self::from_literal(value, &field.ty, structs)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Self::Struct {
                    name: name.clone(),
                    fields,
                })
            }
            _ => None,
        }
    }

    fn to_expr(&self) -> oche_frontend::Expr {
        use oche_frontend::Expr;
        match self {
            Self::Int(value) => Expr::Integer(*value),
            Self::Float(value) => Expr::Float(*value),
            Self::Bool(value) => Expr::Boolean(*value),
            Self::String(value) => Expr::String(value.clone()),
            Self::Type(value) => Expr::TypeValue(value.clone()),
            Self::List(values) => Expr::List(values.iter().map(Self::to_expr).collect()),
            Self::Struct { name, fields } => Expr::StructLiteral {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value.to_expr()))
                    .collect(),
            },
        }
    }
}

fn is_comptime_value_type(
    ty: &Type,
    structs: &HashMap<&str, &oche_frontend::Struct>,
    visiting: &mut HashSet<String>,
) -> bool {
    let Type::Named { name, arguments } = ty;
    (matches!(name.as_str(), "Int" | "Float" | "Bool" | "String" | "Type") && arguments.is_empty())
        || (name == "List"
            && arguments.len() == 1
            && is_comptime_value_type(&arguments[0], structs, visiting))
        || (arguments.is_empty()
            && structs.get(name.as_str()).is_some_and(|structure| {
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let supported = structure
                    .fields
                    .iter()
                    .all(|field| is_comptime_value_type(&field.ty, structs, visiting));
                visiting.remove(name);
                supported
            }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct VmRequest {
    pub id: usize,
    pub function: usize,
    pub name: String,
    pub arguments: Vec<Value>,
    pub result: Type,
    pub offset: usize,
}

/// Validate both bootstrap generators and value-level comptime expressions without
/// executing the latter. Editors use this to report the same static boundary
/// errors as the command-line compiler while keeping analysis side-effect free.
pub fn validate(program: &Program) -> Result<(), Error> {
    vm_requests(program)?;
    evaluate(program)?;
    Ok(())
}

pub fn vm_requests(program: &Program) -> Result<Vec<VmRequest>, Error> {
    let functions = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.name.as_str(), (index, function)))
        .collect::<HashMap<_, _>>();
    let structs = program
        .structs
        .iter()
        .map(|structure| (structure.name.as_str(), structure))
        .collect::<HashMap<_, _>>();
    let mut requests = Vec::new();
    for directive in &program.comptime {
        let ComptimeKind::Generate { function: name } = &directive.kind else {
            continue;
        };
        if matches!(
            name.as_str(),
            "_oche_generate_json_encode" | "_oche_generate_json_decode"
        ) {
            continue;
        }
        let Some(&(index, function)) = functions.get(name.as_str()) else {
            return Err(Error::InvalidExpression {
                offset: directive.span.start,
                message: format!("unknown comptime generator '{name}'"),
            });
        };
        if !function.is_const || function.is_async {
            return Err(Error::InvalidExpression {
                offset: directive.span.start,
                message: format!("'{name}' must be a synchronous const fn"),
            });
        }
        if function.parameters.len() != 1 || function.parameters[0].ty != named("Type") {
            return Err(Error::InvalidExpression {
                offset: directive.span.start,
                message: format!("'{name}' must accept exactly one Type parameter"),
            });
        }
        if function.result != named("ComptimeDeclarations") {
            return Err(Error::InvalidExpression {
                offset: directive.span.start,
                message: format!("'{name}' must return ComptimeDeclarations"),
            });
        }
        requests.push(VmRequest {
            id: requests.len(),
            function: index,
            name: name.clone(),
            arguments: vec![Value::Type(named(&directive.target))],
            result: named("ComptimeDeclarations"),
            offset: directive.span.start,
        });
    }
    for function in &program.functions {
        collect_vm_requests(
            &function.body,
            function.span.start,
            &functions,
            &structs,
            &mut requests,
        )?;
    }
    Ok(requests)
}

fn collect_vm_requests(
    expression: &oche_frontend::Expr,
    inherited_offset: usize,
    functions: &HashMap<&str, (usize, &oche_frontend::Function)>,
    structs: &HashMap<&str, &oche_frontend::Struct>,
    requests: &mut Vec<VmRequest>,
) -> Result<(), Error> {
    use oche_frontend::{Expr, Statement};
    let offset = expression.offset().unwrap_or(inherited_offset);
    match expression.unlocated() {
        Expr::Comptime(value) => {
            let Expr::Call { callee, arguments } = value.unlocated() else {
                return Err(Error::InvalidExpression {
                    offset,
                    message: "expected a direct const function call".into(),
                });
            };
            let Some(&(index, function)) = functions.get(callee.as_str()) else {
                return Err(Error::InvalidExpression {
                    offset,
                    message: format!("unknown function '{callee}'"),
                });
            };
            if !function.is_const || function.is_async {
                return Err(Error::InvalidExpression {
                    offset,
                    message: format!("'{callee}' must be a synchronous const fn"),
                });
            }
            if arguments.len() != function.parameters.len() {
                return Err(Error::InvalidExpression {
                    offset,
                    message: format!(
                        "'{callee}' expects {} comptime argument(s), received {}",
                        function.parameters.len(),
                        arguments.len()
                    ),
                });
            }
            let mut argument_values = Vec::with_capacity(arguments.len());
            for (argument_index, (argument, parameter)) in
                arguments.iter().zip(&function.parameters).enumerate()
            {
                let Some(value) = Value::from_literal(argument, &parameter.ty, structs) else {
                    return Err(Error::InvalidExpression {
                        offset: argument.offset().unwrap_or(offset),
                        message: format!(
                            "argument {} to '{callee}' must be a comptime value literal",
                            argument_index + 1
                        ),
                    });
                };
                argument_values.push(value);
            }
            if !is_comptime_value_type(&function.result, structs, &mut HashSet::new()) {
                return Err(Error::InvalidExpression {
                    offset,
                    message: format!("'{callee}' must return a comptime value type"),
                });
            }
            requests.push(VmRequest {
                id: requests.len(),
                function: index,
                name: callee.clone(),
                arguments: argument_values,
                result: function.result.clone(),
                offset,
            });
            Ok(())
        }
        Expr::Located { .. } => unreachable!("unlocated removes location wrappers"),
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Quote(value)
        | Expr::Splice(value)
        | Expr::Propagate(value) => {
            collect_vm_requests(value, offset, functions, structs, requests)
        }
        Expr::Binary { left, right, .. } => {
            collect_vm_requests(left, offset, functions, structs, requests)?;
            collect_vm_requests(right, offset, functions, structs, requests)
        }
        Expr::Call { arguments, .. } | Expr::QualifiedCall { arguments, .. } => {
            for argument in arguments {
                collect_vm_requests(argument, offset, functions, structs, requests)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            collect_vm_requests(receiver, offset, functions, structs, requests)?;
            for argument in arguments {
                collect_vm_requests(argument, offset, functions, structs, requests)?;
            }
            Ok(())
        }
        Expr::List(values) => {
            for value in values {
                collect_vm_requests(value, offset, functions, structs, requests)?;
            }
            Ok(())
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_vm_requests(value, offset, functions, structs, requests)?;
            }
            Ok(())
        }
        Expr::Match { value, arms, .. } => {
            collect_vm_requests(value, offset, functions, structs, requests)?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_vm_requests(guard, arm.offset, functions, structs, requests)?;
                }
                collect_vm_requests(&arm.body, arm.offset, functions, structs, requests)?;
            }
            Ok(())
        }
        Expr::Block(block) | Expr::Try(block) => {
            for statement in &block.statements {
                let value = match statement {
                    Statement::Let { value, .. }
                    | Statement::Assign { value, .. }
                    | Statement::Expression(value) => value,
                };
                collect_vm_requests(value, offset, functions, structs, requests)?;
            }
            collect_vm_requests(&block.tail, offset, functions, structs, requests)
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => Ok(()),
    }
}

pub fn apply_vm_values(
    program: &mut Program,
    results: &HashMap<usize, Value>,
) -> Result<(), Error> {
    let has_json_stdlib = program
        .functions
        .iter()
        .any(|function| function.name.starts_with("_oche_generate_json_"));
    let mut next_request = 0;
    let mut generated = Vec::new();
    for directive in &program.comptime {
        let ComptimeKind::Generate {
            function: generator,
        } = &directive.kind
        else {
            continue;
        };
        let request = next_request;
        next_request += 1;
        let value = results
            .get(&request)
            .ok_or_else(|| Error::InvalidExpression {
                offset: directive.span.start,
                message: format!("missing Dart VM result for comptime generator '{generator}'"),
            })?;
        generated.extend(decode_declarations(value, directive.span)?);
    }
    let source_functions = program.functions.len();
    for function in &mut program.functions[..source_functions] {
        replace_vm_values(
            &mut function.body,
            results,
            &mut next_request,
            function.span.start,
        )?;
    }
    let mut names = program
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect::<HashSet<_>>();
    for function in &generated {
        if !names.insert(function.name.clone()) {
            return Err(Error::InvalidExpression {
                offset: function.span.start,
                message: format!("generated duplicate function '{}'", function.name),
            });
        }
    }
    program.functions.extend(generated);
    if has_json_stdlib {
        program.functions.retain(|function| {
            !function.name.starts_with("_oche_ct_")
                && !function.name.starts_with("_oche_generate_json_")
        });
        program
            .structs
            .retain(|structure| !structure.name.starts_with("OcheInternal"));
    }
    program
        .comptime
        .retain(|directive| !matches!(directive.kind, ComptimeKind::Generate { .. }));
    Ok(())
}

struct GeneratedExpression {
    kind: String,
    text: String,
    children: Vec<usize>,
}

fn decode_declarations(
    value: &Value,
    span: oche_frontend::SourceSpan,
) -> Result<Vec<oche_frontend::Function>, Error> {
    let fields = value_struct(value, "ComptimeDeclarations", span)?;
    let functions = value_field(fields, "functions", span)?;
    let Value::List(functions) = functions else {
        return invalid_value(span, "ComptimeDeclarations.functions must be a List");
    };
    functions
        .iter()
        .map(|value| decode_function(value, span))
        .collect()
}

fn decode_function(
    value: &Value,
    span: oche_frontend::SourceSpan,
) -> Result<oche_frontend::Function, Error> {
    let fields = value_struct(value, "ComptimeFunction", span)?;
    let name = value_string(value_field(fields, "name", span)?, span)?;
    let result = value_type(value_field(fields, "result", span)?, span)?;
    let parameters = match value_field(fields, "parameters", span)? {
        Value::List(values) => values
            .iter()
            .map(|value| {
                let fields = value_struct(value, "ComptimeParameter", span)?;
                Ok(oche_frontend::Parameter {
                    span,
                    name: value_string(value_field(fields, "name", span)?, span)?,
                    ty: value_type(value_field(fields, "ty", span)?, span)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?,
        _ => return invalid_value(span, "ComptimeFunction.parameters must be a List"),
    };
    let expressions = match value_field(fields, "expressions", span)? {
        Value::List(values) => values
            .iter()
            .map(|value| {
                let fields = value_struct(value, "ComptimeExpression", span)?;
                let children = match value_field(fields, "children", span)? {
                    Value::List(values) => values
                        .iter()
                        .map(|value| match value {
                            Value::Int(index) => {
                                usize::try_from(*index).map_err(|_| Error::InvalidExpression {
                                    offset: span.start,
                                    message: "generated expression index must be non-negative"
                                        .into(),
                                })
                            }
                            _ => Err(Error::InvalidExpression {
                                offset: span.start,
                                message: "generated expression children must contain Int indices"
                                    .into(),
                            }),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => {
                        return invalid_value(span, "ComptimeExpression.children must be a List");
                    }
                };
                Ok(GeneratedExpression {
                    kind: value_string(value_field(fields, "kind", span)?, span)?,
                    text: value_string(value_field(fields, "text", span)?, span)?,
                    children,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?,
        _ => return invalid_value(span, "ComptimeFunction.expressions must be a List"),
    };
    let root = value_int(value_field(fields, "root", span)?, span)?;
    let root = usize::try_from(root).map_err(|_| Error::InvalidExpression {
        offset: span.start,
        message: "generated function root must be non-negative".into(),
    })?;
    let body = build_generated_expression(root, &expressions, &mut HashSet::new(), span)?;
    Ok(oche_frontend::Function {
        span,
        is_public: false,
        is_test: false,
        is_const: false,
        is_async: false,
        name,
        parameters,
        result,
        body,
    })
}

fn build_generated_expression(
    index: usize,
    expressions: &[GeneratedExpression],
    visiting: &mut HashSet<usize>,
    span: oche_frontend::SourceSpan,
) -> Result<oche_frontend::Expr, Error> {
    use oche_frontend::{Expr, Operator};
    let node = expressions
        .get(index)
        .ok_or_else(|| Error::InvalidExpression {
            offset: span.start,
            message: format!("generated expression index {index} is out of bounds"),
        })?;
    if !visiting.insert(index) {
        return invalid_value(span, "generated expression graph contains a cycle");
    }
    if node.kind == "struct" {
        let mut fields = Vec::with_capacity(node.children.len());
        for field_index in &node.children {
            let field = expressions
                .get(*field_index)
                .ok_or_else(|| Error::InvalidExpression {
                    offset: span.start,
                    message: format!("generated expression index {field_index} is out of bounds"),
                })?;
            if field.kind != "named" || field.children.len() != 1 {
                return invalid_value(
                    span,
                    "generated struct children must be named nodes with one value",
                );
            }
            fields.push((
                field.text.clone(),
                build_generated_expression(field.children[0], expressions, visiting, span)?,
            ));
        }
        visiting.remove(&index);
        return Ok(oche_frontend::Expr::Located {
            span,
            value: Box::new(oche_frontend::Expr::StructLiteral {
                name: node.text.clone(),
                fields,
            }),
        });
    }
    if matches!(node.kind.as_str(), "block" | "try") {
        let (tail, statements) =
            node.children
                .split_last()
                .ok_or_else(|| Error::InvalidExpression {
                    offset: span.start,
                    message: "generated block requires a tail expression".into(),
                })?;
        let statements = statements
            .iter()
            .map(|statement| {
                let statement =
                    expressions
                        .get(*statement)
                        .ok_or_else(|| Error::InvalidExpression {
                            offset: span.start,
                            message: "generated statement index is out of bounds".into(),
                        })?;
                if statement.children.len() != 1 {
                    return invalid_value(span, "generated statement requires one value");
                }
                let value =
                    build_generated_expression(statement.children[0], expressions, visiting, span)?;
                Ok(match statement.kind.as_str() {
                    "let" => oche_frontend::Statement::Let {
                        mutable: false,
                        pattern: oche_frontend::Pattern::Binding(statement.text.clone()),
                        value,
                    },
                    "expression_statement" => oche_frontend::Statement::Expression(value),
                    "assign" => oche_frontend::Statement::Assign {
                        name: statement.text.clone(),
                        value,
                    },
                    _ => {
                        return invalid_value(
                            span,
                            "generated block child must be a statement node",
                        );
                    }
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let tail = build_generated_expression(*tail, expressions, visiting, span)?;
        visiting.remove(&index);
        let block = oche_frontend::Block {
            statements,
            tail: Box::new(tail),
        };
        return Ok(oche_frontend::Expr::Located {
            span,
            value: Box::new(if node.kind == "try" {
                oche_frontend::Expr::Try(block)
            } else {
                oche_frontend::Expr::Block(block)
            }),
        });
    }
    if node.kind == "match" {
        let (value, arms) =
            node.children
                .split_first()
                .ok_or_else(|| Error::InvalidExpression {
                    offset: span.start,
                    message: "generated match requires a value and at least one arm".into(),
                })?;
        if arms.is_empty() {
            return invalid_value(span, "generated match requires at least one arm");
        }
        let value = build_generated_expression(*value, expressions, visiting, span)?;
        let arms = arms
            .iter()
            .map(|arm_index| {
                let arm = expressions.get(*arm_index).ok_or_else(|| {
                    Error::InvalidExpression {
                        offset: span.start,
                        message: "generated match arm index is out of bounds".into(),
                    }
                })?;
                if arm.kind != "arm" || !(arm.children.len() == 2 || arm.children.len() == 3) {
                    return invalid_value(
                        span,
                        "generated match children must be arm nodes with a pattern, optional guard, and body",
                    );
                }
                let pattern =
                    build_generated_pattern(arm.children[0], expressions, visiting, span)?;
                let (guard, body) = if arm.children.len() == 3 {
                    (
                        Some(build_generated_expression(
                            arm.children[1],
                            expressions,
                            visiting,
                            span,
                        )?),
                        arm.children[2],
                    )
                } else {
                    (None, arm.children[1])
                };
                Ok(oche_frontend::MatchArm {
                    offset: span.start,
                    pattern,
                    guard,
                    body: build_generated_expression(body, expressions, visiting, span)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        visiting.remove(&index);
        return Ok(oche_frontend::Expr::Located {
            span,
            value: Box::new(oche_frontend::Expr::Match {
                offset: span.start,
                value: Box::new(value),
                arms,
            }),
        });
    }
    let children = node
        .children
        .iter()
        .map(|index| build_generated_expression(*index, expressions, visiting, span))
        .collect::<Result<Vec<_>, _>>()?;
    visiting.remove(&index);
    let expression = match node.kind.as_str() {
        "int" => Expr::Integer(node.text.parse().map_err(|_| Error::InvalidExpression {
            offset: span.start,
            message: format!("invalid generated Int literal '{}'", node.text),
        })?),
        "float" => Expr::Float(node.text.parse().map_err(|_| Error::InvalidExpression {
            offset: span.start,
            message: format!("invalid generated Float literal '{}'", node.text),
        })?),
        "bool" => match node.text.as_str() {
            "true" => Expr::Boolean(true),
            "false" => Expr::Boolean(false),
            _ => return invalid_value(span, "generated Bool literal must be true or false"),
        },
        "string" => Expr::String(node.text.clone()),
        "variable" if children.is_empty() => Expr::Variable(node.text.clone()),
        "call" => Expr::Call {
            callee: node.text.clone(),
            arguments: children,
        },
        "dot_call" if !children.is_empty() => {
            let mut children = children.into_iter();
            Expr::DotCall {
                receiver: Box::new(children.next().unwrap()),
                name: node.text.clone(),
                arguments: children.collect(),
            }
        }
        "field" if children.len() == 1 => Expr::Field {
            value: Box::new(children.into_iter().next().unwrap()),
            name: node.text.clone(),
        },
        "list" => Expr::List(children),
        "negate" if children.len() == 1 => {
            Expr::Negate(Box::new(children.into_iter().next().unwrap()))
        }
        "propagate" if children.len() == 1 => {
            Expr::Propagate(Box::new(children.into_iter().next().unwrap()))
        }
        "add" | "subtract" | "multiply" if children.len() == 2 => {
            let mut children = children.into_iter();
            Expr::Binary {
                operator: match node.kind.as_str() {
                    "add" => Operator::Add,
                    "subtract" => Operator::Subtract,
                    "multiply" => Operator::Multiply,
                    _ => unreachable!(),
                },
                left: Box::new(children.next().unwrap()),
                right: Box::new(children.next().unwrap()),
            }
        }
        _ => {
            return Err(Error::InvalidExpression {
                offset: span.start,
                message: format!(
                    "unsupported generated expression '{}' with {} child(ren)",
                    node.kind,
                    children.len()
                ),
            });
        }
    };
    Ok(Expr::Located {
        span,
        value: Box::new(expression),
    })
}

fn build_generated_pattern(
    index: usize,
    expressions: &[GeneratedExpression],
    visiting: &mut HashSet<usize>,
    span: oche_frontend::SourceSpan,
) -> Result<oche_frontend::Pattern, Error> {
    let node = expressions
        .get(index)
        .ok_or_else(|| Error::InvalidExpression {
            offset: span.start,
            message: format!("generated pattern index {index} is out of bounds"),
        })?;
    if !visiting.insert(index) {
        return invalid_value(span, "generated expression graph contains a cycle");
    }
    let pattern = match node.kind.as_str() {
        "pattern_binding" if node.children.is_empty() => {
            oche_frontend::Pattern::Binding(node.text.clone())
        }
        "pattern_wildcard" if node.children.is_empty() => oche_frontend::Pattern::Wildcard,
        "pattern_variant" => {
            let fields = node
                .children
                .iter()
                .map(|child| build_generated_pattern(*child, expressions, visiting, span))
                .collect::<Result<Vec<_>, _>>()?;
            oche_frontend::Pattern::Variant {
                name: node.text.clone(),
                fields,
            }
        }
        _ => {
            return Err(Error::InvalidExpression {
                offset: span.start,
                message: format!("unsupported generated pattern '{}'", node.kind),
            });
        }
    };
    visiting.remove(&index);
    Ok(oche_frontend::Pattern::Located {
        span,
        value: Box::new(pattern),
    })
}

fn value_struct<'a>(
    value: &'a Value,
    expected: &str,
    span: oche_frontend::SourceSpan,
) -> Result<&'a [(String, Value)], Error> {
    match value {
        Value::Struct { name, fields } if name == expected => Ok(fields),
        _ => invalid_value(span, &format!("expected generated {expected} value")),
    }
}

fn value_field<'a>(
    fields: &'a [(String, Value)],
    name: &str,
    span: oche_frontend::SourceSpan,
) -> Result<&'a Value, Error> {
    fields
        .iter()
        .find_map(|(field, value)| (field == name).then_some(value))
        .ok_or_else(|| Error::InvalidExpression {
            offset: span.start,
            message: format!("generated value is missing field '{name}'"),
        })
}

fn value_string(value: &Value, span: oche_frontend::SourceSpan) -> Result<String, Error> {
    match value {
        Value::String(value) => Ok(value.clone()),
        _ => invalid_value(span, "expected generated String value"),
    }
}

fn value_type(value: &Value, span: oche_frontend::SourceSpan) -> Result<Type, Error> {
    match value {
        Value::Type(value) => Ok(value.clone()),
        _ => invalid_value(span, "expected generated Type value"),
    }
}

fn value_int(value: &Value, span: oche_frontend::SourceSpan) -> Result<i64, Error> {
    match value {
        Value::Int(value) => Ok(*value),
        _ => invalid_value(span, "expected generated Int value"),
    }
}

fn invalid_value<T>(span: oche_frontend::SourceSpan, message: &str) -> Result<T, Error> {
    Err(Error::InvalidExpression {
        offset: span.start,
        message: message.into(),
    })
}

fn replace_vm_values(
    expression: &mut oche_frontend::Expr,
    results: &HashMap<usize, Value>,
    next_request: &mut usize,
    inherited_offset: usize,
) -> Result<(), Error> {
    use oche_frontend::{Expr, Statement};
    let offset = expression.offset().unwrap_or(inherited_offset);
    match expression {
        Expr::Located { value, .. } => replace_vm_values(value, results, next_request, offset),
        Expr::Comptime(value) => {
            let Expr::Call { callee, .. } = value.unlocated() else {
                return Err(Error::InvalidExpression {
                    offset,
                    message: "expected a direct const function call".into(),
                });
            };
            let request = *next_request;
            *next_request += 1;
            let Some(value) = results.get(&request) else {
                return Err(Error::InvalidExpression {
                    offset,
                    message: format!("missing Dart VM result for comptime call to '{callee}'"),
                });
            };
            *expression = value.to_expr();
            Ok(())
        }
        Expr::Field { value, .. }
        | Expr::Negate(value)
        | Expr::Await(value)
        | Expr::Quote(value)
        | Expr::Splice(value)
        | Expr::Propagate(value) => replace_vm_values(value, results, next_request, offset),
        Expr::Binary { left, right, .. } => {
            replace_vm_values(left, results, next_request, offset)?;
            replace_vm_values(right, results, next_request, offset)
        }
        Expr::Call { arguments, .. } | Expr::QualifiedCall { arguments, .. } => {
            for argument in arguments {
                replace_vm_values(argument, results, next_request, offset)?;
            }
            Ok(())
        }
        Expr::DotCall {
            receiver,
            arguments,
            ..
        } => {
            replace_vm_values(receiver, results, next_request, offset)?;
            for argument in arguments {
                replace_vm_values(argument, results, next_request, offset)?;
            }
            Ok(())
        }
        Expr::List(values) => {
            for value in values {
                replace_vm_values(value, results, next_request, offset)?;
            }
            Ok(())
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                replace_vm_values(value, results, next_request, offset)?;
            }
            Ok(())
        }
        Expr::Match { value, arms, .. } => {
            replace_vm_values(value, results, next_request, offset)?;
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    replace_vm_values(guard, results, next_request, arm.offset)?;
                }
                replace_vm_values(&mut arm.body, results, next_request, arm.offset)?;
            }
            Ok(())
        }
        Expr::Block(block) | Expr::Try(block) => {
            for statement in &mut block.statements {
                let value = match statement {
                    Statement::Let { value, .. }
                    | Statement::Assign { value, .. }
                    | Statement::Expression(value) => value,
                };
                replace_vm_values(value, results, next_request, offset)?;
            }
            replace_vm_values(&mut block.tail, results, next_request, offset)
        }
        Expr::Integer(_)
        | Expr::Float(_)
        | Expr::Boolean(_)
        | Expr::String(_)
        | Expr::TypeValue(_)
        | Expr::Variable(_) => Ok(()),
    }
}

pub fn evaluate(program: &Program) -> Result<Output, Error> {
    let structs = program
        .structs
        .iter()
        .enumerate()
        .map(|(index, structure)| (structure.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut validators = Vec::new();
    let mut seen_validate = HashSet::new();

    for directive in &program.comptime {
        if matches!(directive.kind, ComptimeKind::Generate { .. }) {
            continue;
        }
        let Some(&struct_index) = structs.get(directive.target.as_str()) else {
            return Err(Error::UnknownTarget {
                offset: directive.span.start,
                target: directive.target.clone(),
            });
        };
        let structure = &program.structs[struct_index];
        match &directive.kind {
            ComptimeKind::Validate(validations) => {
                if !seen_validate.insert(struct_index) {
                    return Err(Error::DuplicateDerive {
                        offset: directive.span.start,
                        operation: "validate",
                        target: directive.target.clone(),
                    });
                }
                let mut seen_fields = HashSet::new();
                let mut rules = Vec::new();
                for validation in validations {
                    let Some(field_index) = structure
                        .fields
                        .iter()
                        .position(|field| field.name == validation.field)
                    else {
                        return Err(Error::UnknownField {
                            offset: directive.span.start,
                            target: directive.target.clone(),
                            field: validation.field.clone(),
                        });
                    };
                    if !seen_fields.insert(field_index) {
                        return Err(Error::DuplicateRule {
                            offset: directive.span.start,
                            target: directive.target.clone(),
                            field: validation.field.clone(),
                        });
                    }
                    let field = &structure.fields[field_index];
                    match &validation.rule {
                        ValidationRule::NonEmpty if type_name(&field.ty) != "String" => {
                            return Err(Error::IncompatibleRule {
                                offset: directive.span.start,
                                target: directive.target.clone(),
                                field: field.name.clone(),
                                ty: field.ty.to_string(),
                                rule: "non_empty",
                            });
                        }
                        ValidationRule::Range { minimum, maximum } => {
                            if type_name(&field.ty) != "Int" {
                                return Err(Error::IncompatibleRule {
                                    offset: directive.span.start,
                                    target: directive.target.clone(),
                                    field: field.name.clone(),
                                    ty: field.ty.to_string(),
                                    rule: "range",
                                });
                            }
                            if minimum > maximum {
                                return Err(Error::InvalidRange {
                                    offset: directive.span.start,
                                    target: directive.target.clone(),
                                    field: field.name.clone(),
                                    minimum: *minimum,
                                    maximum: *maximum,
                                });
                            }
                        }
                        _ => {}
                    }
                    rules.push(ResolvedRule {
                        field_index,
                        rule: validation.rule.clone(),
                    });
                }
                validators.push(ValidatorDerive {
                    struct_index,
                    rules,
                });
            }
            ComptimeKind::Generate { .. } => unreachable!("handled above"),
        }
    }
    Ok(Output { validators })
}

fn type_name(ty: &Type) -> &str {
    let Type::Named { name, .. } = ty;
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_json_derivation_out_of_the_vm_comptime_queue() {
        let program = oche_frontend::parse(
            "struct User { name: String, age: Int, tags: List<List<String>> }
             comptime jsonEncode(User)
             comptime jsonDecode(User)
             comptime validate(User, name: non_empty, age: range(0, 130))",
        )
        .unwrap();
        let requests = vm_requests(&program).unwrap();
        assert!(requests.is_empty());
        let output = evaluate(&program).unwrap();
        assert_eq!(output.validators[0].rules.len(), 2);
    }

    #[test]
    fn leaves_each_json_direction_to_compiler_owned_derivation() {
        let encode = oche_frontend::parse(
            "struct Sample { score: Float, values: List<Float> }
             comptime jsonEncode(Sample)",
        )
        .unwrap();
        let encode_requests = vm_requests(&encode).unwrap();
        assert!(encode_requests.is_empty());

        let decode = oche_frontend::parse(
            "struct Sample { score: Float, values: List<Float> }
             comptime jsonDecode(Sample)",
        )
        .unwrap();
        let decode_requests = vm_requests(&decode).unwrap();
        assert!(decode_requests.is_empty());
    }

    #[test]
    fn leaves_nested_json_derivation_out_of_vm_comptime() {
        let program = oche_frontend::parse(
            "struct Child { value: Int }
             struct Parent { primary: Child, children: List<Child> }
             comptime jsonEncode(Child)
             comptime jsonDecode(Child)
             comptime jsonEncode(Parent)
             comptime jsonDecode(Parent)",
        )
        .unwrap();
        let requests = vm_requests(&program).unwrap();
        assert!(requests.is_empty());
    }

    #[test]
    fn rejects_invalid_schema_rules() {
        let program = oche_frontend::parse(
            "struct User { age: Int }
             comptime validate(User, age: non_empty)",
        )
        .unwrap();
        assert!(
            evaluate(&program)
                .unwrap_err()
                .to_string()
                .contains("cannot be applied")
        );
    }

    #[test]
    fn collects_and_applies_vm_comptime_values() {
        let mut program = oche_frontend::parse(
            "const fn scale(value: Int, factor: Int) -> Int { value * factor }
             fn first() -> Int { comptime scale(16, 2) }
             fn second() -> Int { comptime scale(10, 3) }",
        )
        .unwrap();
        let requests = vm_requests(&program).unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].id, 0);
        assert_eq!(requests[0].function, 0);
        assert_eq!(requests[0].name, "scale");
        assert_eq!(requests[0].arguments, [Value::Int(16), Value::Int(2)]);
        assert_eq!(requests[1].id, 1);
        assert_eq!(requests[1].arguments, [Value::Int(10), Value::Int(3)]);
        assert_eq!(requests[0].result.to_string(), "Int");

        apply_vm_values(
            &mut program,
            &HashMap::from([(0, Value::Int(32)), (1, Value::Int(30))]),
        )
        .unwrap();
        assert!(vm_requests(&program).unwrap().is_empty());
        assert!(matches!(
            program.functions[1].body.unlocated(),
            oche_frontend::Expr::Integer(32)
        ));
        assert!(matches!(
            program.functions[2].body.unlocated(),
            oche_frontend::Expr::Integer(30)
        ));
    }

    #[test]
    fn restricts_the_initial_vm_comptime_boundary() {
        for (source, message) in [
            (
                "fn value() -> Int { 1 } fn result() -> Int { comptime value() }",
                "must be a synchronous const fn",
            ),
            (
                "const fn value(input: Int) -> Int { input } fn result(input: Int) -> Int { comptime value(input) }",
                "must be a comptime value literal",
            ),
            (
                "const fn value() -> Option<Int> { Some(1) } fn result() -> Option<Int> { comptime value() }",
                "must return a comptime value type",
            ),
        ] {
            let program = oche_frontend::parse(source).unwrap();
            assert!(
                vm_requests(&program)
                    .unwrap_err()
                    .to_string()
                    .contains(message)
            );
        }
    }

    #[test]
    fn carries_recursive_list_values_through_the_vm_protocol() {
        let mut program = oche_frontend::parse(
            "const fn extend(values: List<Int>) -> List<Int> { values + [3] }
             fn result() -> List<Int> { comptime extend([1, 2]) }",
        )
        .unwrap();
        let requests = vm_requests(&program).unwrap();
        assert_eq!(
            requests[0].arguments,
            [Value::List(vec![Value::Int(1), Value::Int(2)])]
        );
        assert_eq!(requests[0].result.to_string(), "List<Int>");

        apply_vm_values(
            &mut program,
            &HashMap::from([(
                0,
                Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            )]),
        )
        .unwrap();
        assert!(matches!(
            program.functions[1].body.unlocated(),
            oche_frontend::Expr::List(values)
                if matches!(values.as_slice(), [oche_frontend::Expr::Integer(1), oche_frontend::Expr::Integer(2), oche_frontend::Expr::Integer(3)])
        ));
    }

    #[test]
    fn carries_named_struct_values_through_the_vm_protocol() {
        let mut program = oche_frontend::parse(
            "struct Config { name: String, limits: List<Int> }
             const fn configured(name: String) -> Config {
               Config { name: name, limits: [1, 2] }
             }
             fn result() -> Config { comptime configured(\"agent\") }",
        )
        .unwrap();
        let requests = vm_requests(&program).unwrap();
        assert_eq!(requests[0].result.to_string(), "Config");
        let value = Value::Struct {
            name: "Config".into(),
            fields: vec![
                ("name".into(), Value::String("agent".into())),
                (
                    "limits".into(),
                    Value::List(vec![Value::Int(1), Value::Int(2)]),
                ),
            ],
        };
        apply_vm_values(&mut program, &HashMap::from([(0, value)])).unwrap();
        assert!(matches!(
            program.functions[1].body.unlocated(),
            oche_frontend::Expr::StructLiteral { name, fields }
                if name == "Config" && fields.len() == 2
        ));
    }
}
