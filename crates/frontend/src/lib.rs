//! Syntax and early semantic checks for Oche's Dart-shaped language.

mod ast;
mod parser;

pub use ast::{
    Block, COMPTIME_IR_TYPES, ComptimeDirective, ComptimeKind, DartLibrary, Enum, EnumVariant,
    Expr, ExternFunction, Field, FieldValidation, Function, INTRINSIC_FUNCTIONS, Import, MatchArm,
    Operator, Parameter, Pattern, Program, SourceSpan, Statement, Struct, Type, ValidationRule,
    intrinsic_signature, is_comptime_ir_type,
};
pub use parser::{Error, ParseOutput, parse, parse_recovering, validate_linked};
