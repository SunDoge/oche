//! Lower checked Oche into deterministic Dart.

use std::fmt::Write;

use oche_comptime::{Output as ComptimeOutput, ResolvedRule};
use oche_frontend::{Operator, Pattern, Program, Type, ValidationRule};
use oche_hir::{
    Callable, LinkedPackage, ModuleId, SyntheticOrigin, TypeProperty, TypedBlock, TypedExpr,
    TypedExprKind, TypedPackage, TypedProgram, TypedStatement,
};
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Type checking failed: {source}"))]
    Typecheck { source: oche_hir::Error },
    #[snafu(display("Compile-time evaluation failed: {source}"))]
    Comptime { source: oche_comptime::Error },
    #[snafu(display("Value-level comptime expression at byte {offset} has not been evaluated"))]
    UnevaluatedComptime { offset: usize },
    #[snafu(display(
        "Function '{function}' cannot be exported to JavaScript because type '{ty}' is not supported at the module boundary"
    ))]
    UnsupportedJavaScriptExport { function: String, ty: Type },
    #[snafu(display(
        "Function '{function}' cannot be exported through the JIT RPC boundary because type '{ty}' is not supported"
    ))]
    UnsupportedJitRpcExport { function: String, ty: Type },
    #[snafu(display("std/isolate is only supported by the Dart Native target"))]
    UnsupportedJavaScriptIsolate,
}

pub fn emit_dart(program: &Program) -> Result<String, Error> {
    ensure_comptime_evaluated(program)?;
    let typed = oche_hir::lower(program).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(program).context(ComptimeSnafu)?;
    let mut output = emit_program(program, &typed, &comptime);
    emit_cli_main(&mut output, ModuleId::ROOT, program);
    Ok(output)
}

pub fn emit_dart_comptime(program: &Program) -> Result<String, Error> {
    let requests = oche_comptime::vm_requests(program).context(ComptimeSnafu)?;
    let mut evaluator = program.clone();
    evaluator.functions.retain(|function| function.is_const);
    let typed = oche_hir::lower(&evaluator).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(&evaluator).context(ComptimeSnafu)?;
    let mut output = emit_program(&evaluator, &typed, &comptime);
    output.push_str("void main(List<String> arguments) {\n  switch (arguments[0]) {\n");
    for request in requests {
        writeln!(output, "    case '{}':", request.id).unwrap();
        write!(
            output,
            "      final result = {}(",
            function_name(
                ModuleId::ROOT,
                evaluator
                    .functions
                    .iter()
                    .position(|function| function.name == request.name)
                    .expect("comptime requests target retained const functions"),
            )
        )
        .unwrap();
        for (index, argument) in request.arguments.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            emit_comptime_value(&mut output, &evaluator, argument);
        }
        output.push_str(");\n      print(jsonEncode(");
        emit_comptime_json_value(&mut output, &evaluator, &request.result, "result", 0);
        output.push_str("));\n");
        output.push_str("      return;\n");
    }
    output.push_str(
        "    default:\n      throw ArgumentError('unknown comptime function');\n  }\n}\n",
    );
    Ok(output)
}

fn emit_comptime_value(output: &mut String, program: &Program, value: &oche_comptime::Value) {
    match value {
        oche_comptime::Value::Int(value) => write!(output, "{value}").unwrap(),
        oche_comptime::Value::Float(value) => emit_float(output, *value),
        oche_comptime::Value::Bool(value) => write!(output, "{value}").unwrap(),
        oche_comptime::Value::String(value) => emit_string(output, value),
        oche_comptime::Value::Type(value) => emit_reflected_type(output, program, value),
        oche_comptime::Value::List(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                emit_comptime_value(output, program, value);
            }
            output.push(']');
        }
        oche_comptime::Value::Struct { name, fields } => {
            let struct_index = program
                .structs
                .iter()
                .position(|structure| structure.name == *name)
                .expect("comptime validates struct value types");
            write!(output, "{}(", struct_name(ModuleId::ROOT, struct_index)).unwrap();
            for (index, (field_name, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                debug_assert_eq!(
                    program.structs[struct_index].fields[index].name,
                    *field_name
                );
                write!(output, "f{index}: ").unwrap();
                emit_comptime_value(output, program, value);
            }
            output.push(')');
        }
    }
}

fn emit_reflected_type(output: &mut String, program: &Program, ty: &Type) {
    emit_reflected_type_inner(output, program, ty, &mut Vec::new());
}

fn emit_reflected_type_inner(
    output: &mut String,
    program: &Program,
    ty: &Type,
    visiting: &mut Vec<String>,
) {
    let Type::Named { name, arguments } = ty;
    output.push_str("OcheType(");
    emit_string(output, name);
    output.push_str(", [");
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        emit_reflected_type_inner(output, program, argument, visiting);
    }
    output.push_str("], [");
    if !visiting.contains(name)
        && let Some(structure) = program
            .structs
            .iter()
            .find(|structure| structure.name == *name)
    {
        visiting.push(name.clone());
        for (index, field) in structure.fields.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("OcheField(");
            emit_string(output, &field.name);
            output.push_str(", ");
            emit_reflected_type_inner(output, program, &field.ty, visiting);
            output.push(')');
        }
        visiting.pop();
    }
    output.push_str("])");
}

fn emit_comptime_json_value(
    output: &mut String,
    program: &Program,
    ty: &Type,
    value: &str,
    depth: usize,
) {
    let Type::Named { name, arguments } = ty;
    if name == "Type" {
        write!(output, "_ocheTypeToJson({value})").unwrap();
    } else if name == "List" {
        let element = format!("c{depth}");
        write!(output, "({value}).map(({element}) => ").unwrap();
        emit_comptime_json_value(output, program, &arguments[0], &element, depth + 1);
        output.push_str(").toList()");
    } else if let Some(struct_index) = program
        .structs
        .iter()
        .position(|structure| structure.name == *name)
    {
        output.push_str("<String, Object?>{");
        for (field_index, field) in program.structs[struct_index].fields.iter().enumerate() {
            if field_index != 0 {
                output.push_str(", ");
            }
            emit_string(output, &field.name);
            output.push_str(": ");
            emit_comptime_json_value(
                output,
                program,
                &field.ty,
                &format!("({value}).f{field_index}"),
                depth,
            );
        }
        output.push('}');
    } else {
        output.push_str(value);
    }
}

pub fn emit_dart_package(package: &LinkedPackage) -> Result<String, Error> {
    let mut output = emit_package_core(package)?;
    emit_cli_main(
        &mut output,
        package.root,
        &package.module(package.root).program,
    );
    Ok(output)
}

fn emit_package_core(package: &LinkedPackage) -> Result<String, Error> {
    for module in &package.modules {
        ensure_comptime_evaluated(&module.program)?;
    }
    let typed = oche_hir::lower_package(package).context(TypecheckSnafu)?;
    let comptime = package
        .modules
        .iter()
        .map(|module| oche_comptime::evaluate(&module.program).context(ComptimeSnafu))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(emit_package_program(package, &typed, &comptime))
}

fn ensure_comptime_evaluated(program: &Program) -> Result<(), Error> {
    let requests = oche_comptime::vm_requests(program).context(ComptimeSnafu)?;
    if let Some(request) = requests.first() {
        return Err(Error::UnevaluatedComptime {
            offset: request.offset,
        });
    }
    Ok(())
}

fn emit_cli_main(output: &mut String, module: ModuleId, program: &Program) {
    output.push_str("Future<void> main(List<String> arguments) async {\n");
    output.push_str("  if (arguments.isEmpty) {\n");
    output.push_str("    throw ArgumentError('expected a Oche function name');\n");
    output.push_str("  }\n");
    output.push_str("  switch (arguments[0]) {\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        writeln!(output, "    case '{}':", function.name).unwrap();
        writeln!(
            output,
            "      if (arguments.length != {}) {{",
            function.parameters.len() + 1
        )
        .unwrap();
        writeln!(
            output,
            "        throw ArgumentError('{} expects {} argument(s)');",
            function.name,
            function.parameters.len()
        )
        .unwrap();
        output.push_str("      }\n");
        output.push_str("      print(");
        if function.is_async {
            output.push_str("await ");
        }
        write!(output, "{}(", function_name(module, function_index)).unwrap();
        for (parameter_index, parameter) in function.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push_str(", ");
            }
            emit_cli_argument(output, &parameter.ty, parameter_index + 1, &function.name);
        }
        output.push_str("));\n");
        output.push_str("      return;\n");
    }
    output.push_str("    default:\n");
    output.push_str("      throw ArgumentError('unknown Oche function: ' + arguments[0]);\n");
    output.push_str("  }\n");
    output.push_str("}\n");
}

/// Emit a Dart VM JIT entry point for the tests declared in an Oche source file.
pub fn emit_dart_tests(program: &Program) -> Result<String, Error> {
    ensure_comptime_evaluated(program)?;
    let typed = oche_hir::lower(program).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(program).context(ComptimeSnafu)?;
    let mut output = emit_program(program, &typed, &comptime).replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:io';\n",
        1,
    );
    emit_tests_main(&mut output, ModuleId::ROOT, program);
    Ok(output)
}

pub fn emit_dart_tests_package(package: &LinkedPackage) -> Result<String, Error> {
    let mut output = emit_package_core(package)?.replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:io';\n",
        1,
    );
    emit_tests_main(
        &mut output,
        package.root,
        &package.module(package.root).program,
    );
    Ok(output)
}

fn emit_tests_main(output: &mut String, module: ModuleId, program: &Program) {
    output.push_str("Future<void> main() async {\n");
    output.push_str("  var passed = 0;\n  var failed = 0;\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        if !function.is_test {
            continue;
        }
        output.push_str("  try {\n    final result = ");
        if function.is_async {
            output.push_str("await ");
        }
        writeln!(output, "{}();", function_name(module, function_index)).unwrap();
        output.push_str("    if (result) {\n");
        writeln!(output, "      print('PASS {}');", function.name).unwrap();
        output.push_str("      passed++;\n    } else {\n");
        writeln!(output, "      print('FAIL {}');", function.name).unwrap();
        output.push_str("      failed++;\n    }\n");
        output.push_str("  } catch (error) {\n");
        writeln!(output, "    print('FAIL {}');", function.name).unwrap();
        output.push_str("    print('  $error');\n    failed++;\n  }\n");
    }
    output.push_str("  print('');\n");
    output.push_str("  print('$passed passed; $failed failed');\n");
    output.push_str("  if (failed != 0) exitCode = 1;\n");
    output.push_str("}\n");
}

pub fn emit_dart_module(program: &Program) -> Result<String, Error> {
    ensure_comptime_evaluated(program)?;
    let typed = oche_hir::lower(program).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(program).context(ComptimeSnafu)?;
    let mut output = emit_program(program, &typed, &comptime);
    emit_module_registration(&mut output, ModuleId::ROOT, program);
    Ok(output)
}

pub fn emit_dart_module_package(package: &LinkedPackage) -> Result<String, Error> {
    let mut output = emit_package_core(package)?;
    emit_module_registration(
        &mut output,
        package.root,
        &package.module(package.root).program,
    );
    Ok(output)
}

fn emit_module_registration(output: &mut String, module: ModuleId, program: &Program) {
    output.push_str("@pragma('vm:entry-point')\n");
    output.push_str("void register(void Function(String, Function) addTool) {\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        writeln!(
            output,
            "  addTool('{}', {});",
            function.name,
            function_name(module, function_index)
        )
        .unwrap();
    }
    output.push_str("}\n\nvoid main() {}\n");
}

/// Emit a persistent newline-delimited JSON host for public Oche functions.
/// The generated program is intended to run as a Dart VM JIT process.
pub fn emit_dart_jit_rpc(program: &Program) -> Result<String, Error> {
    ensure_comptime_evaluated(program)?;
    let typed = oche_hir::lower(program).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(program).context(ComptimeSnafu)?;
    validate_jit_rpc_exports(program)?;

    let mut output = emit_program(program, &typed, &comptime).replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:io';\n",
        1,
    );
    emit_jit_rpc_main(&mut output, ModuleId::ROOT, program);
    Ok(output)
}

pub fn emit_dart_jit_rpc_package(package: &LinkedPackage) -> Result<String, Error> {
    let program = &package.module(package.root).program;
    validate_jit_rpc_exports(program)?;
    let mut output = emit_package_core(package)?.replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:io';\n",
        1,
    );
    emit_jit_rpc_main(&mut output, package.root, program);
    Ok(output)
}

fn validate_jit_rpc_exports(program: &Program) -> Result<(), Error> {
    for function in program
        .functions
        .iter()
        .filter(|function| function.is_public)
    {
        for parameter in &function.parameters {
            ensure_jit_rpc_boundary_type(&function.name, &parameter.ty)?;
        }
        ensure_jit_rpc_boundary_type(&function.name, &function.result)?;
    }
    Ok(())
}

fn emit_jit_rpc_main(output: &mut String, module: ModuleId, program: &Program) {
    output.push_str("Future<void> main() async {\n");
    output.push_str(
        "  await for (final line in stdin.transform(utf8.decoder).transform(const LineSplitter())) {\n",
    );
    output.push_str("    Object? requestId;\n");
    output.push_str("    try {\n");
    output.push_str("      final Object? decodedRequest = jsonDecode(line);\n");
    output.push_str("      if (decodedRequest is! Map<String, Object?>) {\n");
    output.push_str("        throw const FormatException('request must be a JSON object');\n");
    output.push_str("      }\n");
    output.push_str("      final request = decodedRequest;\n");
    output.push_str("      requestId = request['id'];\n");
    output.push_str("      final method = request['method'];\n");
    output.push_str("      final args = request['args'];\n");
    output.push_str("      if (method is! String || args is! List<Object?>) {\n");
    output.push_str("        throw const FormatException('request requires method and args');\n");
    output.push_str("      }\n");
    output.push_str("      Object? value;\n");
    output.push_str("      switch (method) {\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        if !function.is_public {
            continue;
        }
        writeln!(output, "        case '{}':", function.name).unwrap();
        writeln!(
            output,
            "          if (args.length != {}) throw ArgumentError('{} expects {} argument(s)');",
            function.parameters.len(),
            function.name,
            function.parameters.len()
        )
        .unwrap();
        output.push_str("          value = ");
        if function.is_async {
            output.push_str("await ");
        }
        write!(output, "{}(", function_name(module, function_index)).unwrap();
        for (parameter_index, parameter) in function.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push_str(", ");
            }
            emit_jit_rpc_argument(output, &parameter.ty, parameter_index);
        }
        output.push_str(");\n");
        output.push_str("          break;\n");
    }
    output.push_str("        default:\n");
    output.push_str("          throw ArgumentError('unknown Oche function: $method');\n");
    output.push_str("      }\n");
    output.push_str(
        "      stdout.writeln(jsonEncode({'id': requestId, 'ok': true, 'value': value}));\n",
    );
    output.push_str("    } catch (error) {\n");
    output.push_str(
        "      stdout.writeln(jsonEncode({'id': requestId, 'ok': false, 'error': error.toString()}));\n",
    );
    output.push_str("    }\n");
    output.push_str("    await stdout.flush();\n");
    output.push_str("  }\n");
    output.push_str("}\n");
}

fn ensure_jit_rpc_boundary_type(function: &str, ty: &Type) -> Result<(), Error> {
    let Type::Named { name, arguments } = ty;
    if matches!(name.as_str(), "Int" | "Float" | "Bool" | "String") && arguments.is_empty() {
        Ok(())
    } else {
        Err(Error::UnsupportedJitRpcExport {
            function: function.into(),
            ty: ty.clone(),
        })
    }
}

fn emit_jit_rpc_argument(output: &mut String, ty: &Type, parameter_index: usize) {
    match ty.name() {
        "Int" => write!(output, "args[{parameter_index}] as int").unwrap(),
        "Float" => write!(output, "(args[{parameter_index}] as num).toDouble()").unwrap(),
        "Bool" => write!(output, "args[{parameter_index}] as bool").unwrap(),
        "String" => write!(output, "args[{parameter_index}] as String").unwrap(),
        _ => unreachable!("JIT RPC boundary types were validated"),
    }
}

/// Emit a Dart web entry point that publishes Oche functions through a single
/// JavaScript object. The ESM wrapper generated by the CLI reads this object.
pub fn emit_dart_js_module(program: &Program) -> Result<String, Error> {
    ensure_comptime_evaluated(program)?;
    let typed = oche_hir::lower(program).context(TypecheckSnafu)?;
    let comptime = oche_comptime::evaluate(program).context(ComptimeSnafu)?;
    validate_js_exports(program)?;

    let mut output = emit_program(program, &typed, &comptime).replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:js_interop';\n",
        1,
    );
    emit_js_main(&mut output, ModuleId::ROOT, program);
    Ok(output)
}

pub fn emit_dart_js_module_package(package: &LinkedPackage) -> Result<String, Error> {
    let program = &package.module(package.root).program;
    for module in &package.modules {
        validate_js_exports(&module.program)?;
    }
    let mut output = emit_package_core(package)?.replacen(
        "import 'dart:convert';\n",
        "import 'dart:convert';\nimport 'dart:js_interop';\n",
        1,
    );
    emit_js_main(&mut output, package.root, program);
    Ok(output)
}

fn validate_js_exports(program: &Program) -> Result<(), Error> {
    if program
        .imports
        .iter()
        .any(|import| matches!(import.path.as_str(), "std/isolate" | "std/isolate.oche"))
    {
        return UnsupportedJavaScriptIsolateSnafu.fail();
    }
    for function in program
        .functions
        .iter()
        .filter(|function| function.is_public)
    {
        for parameter in &function.parameters {
            ensure_js_boundary_type(&function.name, &parameter.ty)?;
        }
        ensure_js_boundary_type(&function.name, &function.result)?;
    }
    Ok(())
}

fn emit_js_main(output: &mut String, module: ModuleId, program: &Program) {
    output.push_str("@JS('__ocheExports')\nexternal set ocheExports(JSObject value);\n\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        if !function.is_public {
            continue;
        }
        if function.is_async {
            output.push_str("JSPromise<");
            emit_js_type(output, &function.result);
            output.push('>');
        } else {
            emit_js_type(output, &function.result);
        }
        write!(output, " ocheJsFunction{function_index}(").unwrap();
        for (parameter_index, parameter) in function.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push_str(", ");
            }
            emit_js_type(output, &parameter.ty);
            write!(output, " p{parameter_index}").unwrap();
        }
        output.push_str(") => ");
        let mut call = format!("{}(", function_name(module, function_index));
        for (parameter_index, parameter) in function.parameters.iter().enumerate() {
            if parameter_index != 0 {
                call.push_str(", ");
            }
            emit_js_to_dart(&mut call, &parameter.ty, parameter_index);
        }
        call.push(')');
        if function.is_async {
            output.push_str(&call);
            output.push_str(".then((value) => ");
            emit_dart_to_js(output, &function.result, "value");
            output.push_str(").toJS;\n\n");
        } else {
            emit_dart_to_js(output, &function.result, &call);
            output.push_str(";\n\n");
        }
    }
    output.push_str("void main() {\n  ocheExports = <String, Object?>{\n");
    for (function_index, function) in program.functions.iter().enumerate() {
        if !function.is_public {
            continue;
        }
        writeln!(
            output,
            "    '{}': ocheJsFunction{function_index}.toJS,",
            function.name
        )
        .unwrap();
    }
    output.push_str("  }.jsify()! as JSObject;\n}\n");
}

fn ensure_js_boundary_type(function: &str, ty: &Type) -> Result<(), Error> {
    let Type::Named { name, arguments } = ty;
    if matches!(name.as_str(), "Int" | "Float" | "Bool" | "String") && arguments.is_empty() {
        Ok(())
    } else {
        Err(Error::UnsupportedJavaScriptExport {
            function: function.into(),
            ty: ty.clone(),
        })
    }
}

fn emit_js_type(output: &mut String, ty: &Type) {
    output.push_str(match ty.name() {
        "Int" => "JSNumber",
        "Float" => "JSNumber",
        "Bool" => "JSBoolean",
        "String" => "JSString",
        _ => unreachable!("JavaScript boundary types were validated"),
    });
}

fn emit_js_to_dart(output: &mut String, ty: &Type, parameter_index: usize) {
    write!(output, "p{parameter_index}").unwrap();
    output.push_str(match ty.name() {
        "Int" => ".toDartInt",
        "Float" => ".toDartDouble",
        "Bool" | "String" => ".toDart",
        _ => unreachable!("JavaScript boundary types were validated"),
    });
}

fn emit_dart_to_js(output: &mut String, ty: &Type, expression: &str) {
    output.push('(');
    output.push_str(expression);
    output.push(')');
    output.push_str(match ty.name() {
        "Int" | "Float" | "Bool" | "String" => ".toJS",
        _ => unreachable!("JavaScript boundary types were validated"),
    });
}

struct EmissionModule<'a> {
    id: ModuleId,
    program: &'a Program,
    typed: &'a TypedProgram,
    comptime: &'a ComptimeOutput,
}

fn emit_program(program: &Program, typed: &TypedProgram, comptime: &ComptimeOutput) -> String {
    assert_eq!(typed.module, ModuleId::ROOT);
    emit_programs(
        ModuleId::ROOT,
        &[EmissionModule {
            id: ModuleId::ROOT,
            program,
            typed,
            comptime,
        }],
    )
}

fn emit_package_program(
    package: &LinkedPackage,
    typed: &TypedPackage,
    comptime: &[ComptimeOutput],
) -> String {
    assert_eq!(package.root, typed.root);
    assert_eq!(package.modules.len(), typed.modules.len());
    assert_eq!(package.modules.len(), comptime.len());
    let modules = package
        .modules
        .iter()
        .zip(&typed.modules)
        .zip(comptime)
        .enumerate()
        .map(|(index, ((module, typed), comptime))| EmissionModule {
            id: ModuleId::new(index),
            program: &module.program,
            typed,
            comptime,
        })
        .collect::<Vec<_>>();
    emit_programs(package.root, &modules)
}

fn emit_programs(root: ModuleId, modules: &[EmissionModule<'_>]) -> String {
    let mut output = String::from(
        "// Generated by Oche. Do not edit.\n\
import 'dart:convert';\n",
    );
    let has_isolate = modules.iter().any(|module| {
        module.typed.functions.iter().any(|function| {
            typed_expression_contains(&function.body, TypedExpressionNeedle::IsolateRun)
        })
    });
    if has_isolate {
        output.push_str("import 'dart:isolate';\n");
    }
    output.push('\n');
    let mut has_interop = false;
    for module in modules {
        assert_eq!(module.typed.module, module.id);
        for (index, library) in module.program.dart_libraries.iter().enumerate() {
            output.push_str("import ");
            emit_string(&mut output, &library.uri);
            writeln!(output, " as {};", interop_name(module.id, index)).unwrap();
            has_interop = true;
        }
    }
    if has_interop {
        output.push('\n');
    }
    output.push_str(
        "final class OcheType {\n\
  const OcheType(this.name, this.arguments, this.fields);\n\
  final String name;\n\
  final List<OcheType> arguments;\n\
  final List<OcheField> fields;\n\
}\n\
final class OcheField {\n\
  const OcheField(this.name, this.ty);\n\
  final String name;\n\
  final OcheType ty;\n\
}\n\
Object? _ocheTypeToJson(OcheType value) => <String, Object?>{\n\
  'name': value.name,\n\
  'arguments': value.arguments.map(_ocheTypeToJson).toList(),\n\
};\n\n",
    );
    let root_module = &modules[root.index()];
    emit_enums(&mut output, root, root_module.program, true);
    for module in modules {
        if module.id != root {
            emit_enums(&mut output, module.id, module.program, false);
        }
    }
    output.push_str(
        "OcheType _oche_type(String name, List<OcheType> arguments) =>\n\
  OcheType(name, arguments, const []);\n\
OcheResult<Object?, String> _oche_json_parse(String source) {\n\
  try { return OcheOk<Object?, String>(jsonDecode(source)); }\n\
  on FormatException { return OcheErr<Object?, String>('Invalid JSON'); }\n\
}\n\
String _oche_json_stringify(Object? value) => jsonEncode(value);\n\
Object? _oche_json_object(List<String> names, List<Object?> values) =>\n\
  Map<String, Object?>.fromIterables(names, values);\n\
Object? _oche_json_array(List<Object?> values) => values;\n\
Object? _oche_json_from_int(int value) => value;\n\
Object? _oche_json_from_float(double value) => value;\n\
Object? _oche_json_from_bool(bool value) => value;\n\
Object? _oche_json_from_string(String value) => value;\n\
OcheResult<Map<String, Object?>, String> _oche_json_as_object(Object? value) =>\n\
  value is Map<String, Object?>\n\
    ? OcheOk<Map<String, Object?>, String>(value)\n\
    : OcheErr<Map<String, Object?>, String>('Expected a JSON object');\n\
OcheResult<List<Object?>, String> _oche_json_as_array(Object? value) =>\n\
  value is List<Object?>\n\
    ? OcheOk<List<Object?>, String>(value)\n\
    : OcheErr<List<Object?>, String>('Expected a JSON array');\n\
OcheResult<int, String> _oche_json_as_int(Object? value) => value is int\n\
  ? OcheOk<int, String>(value)\n\
  : OcheErr<int, String>('Expected a JSON integer');\n\
OcheResult<double, String> _oche_json_as_float(Object? value) => value is double\n\
  ? OcheOk<double, String>(value)\n\
  : OcheErr<double, String>('Expected a JSON float');\n\
OcheResult<bool, String> _oche_json_as_bool(Object? value) => value is bool\n\
  ? OcheOk<bool, String>(value)\n\
  : OcheErr<bool, String>('Expected a JSON boolean');\n\
OcheResult<String, String> _oche_json_as_string(Object? value) => value is String\n\
  ? OcheOk<String, String>(value)\n\
  : OcheErr<String, String>('Expected a JSON string');\n\
OcheResult<Object?, String> _oche_json_field(\n\
  Map<String, Object?> object, String name) => object.containsKey(name)\n\
    ? OcheOk<Object?, String>(object[name])\n\
    : OcheErr<Object?, String>('Missing JSON field: $name');\n\n",
    );
    let uses_propagation_runtime = modules.iter().any(|module| {
        module.typed.functions.iter().any(|function| {
            typed_expression_contains(&function.body, TypedExpressionNeedle::PropagationRuntime)
        })
    });
    if uses_propagation_runtime {
        output.push_str(
            "final class _OcheOptionPropagation { const _OcheOptionPropagation(); }\n\
final class _OcheResultPropagation {\n\
  const _OcheResultPropagation(this.error);\n\
  final Object? error;\n\
}\n\
extension _OcheOptionUnwrap<T> on OcheOption<T> {\n\
  T get _ocheUnwrap => switch (this) {\n\
    OcheSome(value: final value) => value,\n\
    OcheNone() => throw const _OcheOptionPropagation(),\n\
  };\n\
}\n\
extension _OcheResultUnwrap<T, E> on OcheResult<T, E> {\n\
  T get _ocheUnwrap => switch (this) {\n\
    OcheOk(value: final value) => value,\n\
    OcheErr(error: final error) => throw _OcheResultPropagation(error),\n\
  };\n\
}\n\
OcheResult<T, E> _ocheTryResult<T, E>(T Function() body) {\n\
  try { return OcheOk<T, E>(body()); }\n\
  on _OcheResultPropagation catch (propagation) {\n\
    return OcheErr<T, E>(propagation.error as E);\n\
  }\n\
}\n\
OcheOption<T> _ocheTryOption<T>(T Function() body) {\n\
  try { return OcheSome<T>(body()); }\n\
  on _OcheOptionPropagation { return OcheNone<T>(); }\n\
}\n\
Future<OcheResult<T, E>> _ocheTryResultAsync<T, E>(Future<T> Function() body) async {\n\
  try { return OcheOk<T, E>(await body()); }\n\
  on _OcheResultPropagation catch (propagation) {\n\
    return OcheErr<T, E>(propagation.error as E);\n\
  }\n\
}\n\
Future<OcheOption<T>> _ocheTryOptionAsync<T>(Future<T> Function() body) async {\n\
  try { return OcheSome<T>(await body()); }\n\
  on _OcheOptionPropagation { return OcheNone<T>(); }\n\
}\n\n",
        );
    }
    output.push_str(
        "OcheOption<T> _ocheListLast<T>(List<T> values) =>\n\
  values.isEmpty ? OcheNone<T>() : OcheSome<T>(values.last);\n\n",
    );
    output.push_str(
        "OcheOption<T> _ocheListFirst<T>(List<T> values) =>\n\
  values.isEmpty ? OcheNone<T>() : OcheSome<T>(values.first);\n\n\
List<T> _ocheListRest<T>(List<T> values) =>\n\
  values.isEmpty ? <T>[] : values.sublist(1);\n\n",
    );
    for module in modules {
        emit_structs(&mut output, module.id, module.program);
        emit_comptime(&mut output, module.id, module.program, module.comptime);
        emit_functions(&mut output, module.program, module.typed);
    }
    output
}

fn emit_functions(output: &mut String, program: &Program, typed: &TypedProgram) {
    for typed_function in &typed.functions {
        assert_eq!(typed_function.id.module, typed.module);
        let function_index = typed_function.id.index;
        let function = &program.functions[function_index];
        output.push_str("@pragma('vm:entry-point')\n");
        if function.is_async {
            output.push_str("Future<");
        }
        emit_type(output, typed.module, program, &function.result);
        if function.is_async {
            output.push('>');
        }
        write!(output, " {}(", function_name(typed.module, function_index)).unwrap();
        let mut environment = Vec::new();
        for (parameter_index, parameter) in function.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push_str(", ");
            }
            emit_type(output, typed.module, program, &parameter.ty);
            write!(output, " p{parameter_index}").unwrap();
            environment.push((parameter.name.clone(), format!("p{parameter_index}")));
        }
        if function.is_async {
            output.push_str(") async {\n");
        } else {
            output.push_str(") {\n");
        }
        let can_propagate = typed_expression_contains(
            &typed_function.body,
            TypedExpressionNeedle::EscapingPropagation,
        );
        if can_propagate {
            output.push_str("  try {\n    return ");
        } else {
            output.push_str("  return ");
        }
        let mut next_binding = 0;
        emit_expression(
            output,
            program,
            typed,
            &environment,
            &mut next_binding,
            &typed_function.body,
        );
        if can_propagate {
            let Type::Named { arguments, .. } = &function.result;
            match function.result.name() {
                "Result" => {
                    output.push_str(
                        ";\n  } on _OcheResultPropagation catch (propagation) {\n    return OcheErr<",
                    );
                    emit_type(output, typed.module, program, &arguments[0]);
                    output.push_str(", ");
                    emit_type(output, typed.module, program, &arguments[1]);
                    output.push_str(">(propagation.error as ");
                    emit_type(output, typed.module, program, &arguments[1]);
                    output.push_str(");\n  }\n}\n\n");
                }
                "Option" => output.push_str(
                    ";\n  } on _OcheOptionPropagation {\n    return OcheNone();\n  }\n}\n\n",
                ),
                _ => unreachable!("escaping propagation has a Result or Option boundary"),
            }
        } else {
            output.push_str(";\n}\n\n");
        }
    }
}

fn function_name(module: ModuleId, index: usize) -> String {
    if module == ModuleId::ROOT {
        format!("ocheFunction{index}")
    } else {
        format!("ocheModule{}Function{index}", module.index())
    }
}

fn struct_name(module: ModuleId, index: usize) -> String {
    if module == ModuleId::ROOT {
        format!("OcheStruct{index}")
    } else {
        format!("OcheModule{}Struct{index}", module.index())
    }
}

fn enum_name(module: ModuleId, index: usize) -> String {
    match index {
        0 => "OcheOption".into(),
        1 => "OcheResult".into(),
        _ if module == ModuleId::ROOT => format!("OcheEnum{}", index - 2),
        _ => format!("OcheModule{}Enum{}", module.index(), index - 2),
    }
}

fn variant_name(module: ModuleId, enum_index: usize, variant_index: usize) -> String {
    match (enum_index, variant_index) {
        (0, 0) => "OcheSome".into(),
        (0, 1) => "OcheNone".into(),
        (1, 0) => "OcheOk".into(),
        (1, 1) => "OcheErr".into(),
        _ if module == ModuleId::ROOT => {
            format!("OcheEnum{}Variant{variant_index}", enum_index - 2)
        }
        _ => format!(
            "OcheModule{}Enum{}Variant{variant_index}",
            module.index(),
            enum_index - 2
        ),
    }
}

fn interop_name(module: ModuleId, library: usize) -> String {
    if module == ModuleId::ROOT {
        format!("ocheInterop{library}")
    } else {
        format!("ocheModule{}Interop{library}", module.index())
    }
}

fn generated_name(module: ModuleId, root_stem: &str, module_stem: &str, index: usize) -> String {
    if module == ModuleId::ROOT {
        format!("{root_stem}{index}")
    } else {
        format!("{module_stem}M{}I{index}", module.index())
    }
}

fn json_to_name(module: ModuleId, index: usize) -> String {
    generated_name(module, "ocheJsonTo", "ocheModuleJsonTo", index)
}

fn json_to_value_name(module: ModuleId, index: usize) -> String {
    generated_name(module, "ocheJsonToValue", "ocheModuleJsonToValue", index)
}

fn json_from_name(module: ModuleId, index: usize) -> String {
    generated_name(module, "ocheJsonFrom", "ocheModuleJsonFrom", index)
}

fn json_from_value_name(module: ModuleId, index: usize) -> String {
    generated_name(
        module,
        "ocheJsonFromValue",
        "ocheModuleJsonFromValue",
        index,
    )
}

fn validate_name(module: ModuleId, index: usize) -> String {
    generated_name(module, "ocheValidate", "ocheModuleValidate", index)
}

fn variant_field_name(enum_index: usize, variant_index: usize, field_index: usize) -> String {
    match (enum_index, variant_index, field_index) {
        (0, 0, 0) | (1, 0, 0) => "value".into(),
        (1, 1, 0) => "error".into(),
        _ => format!("f{field_index}"),
    }
}

fn emit_type_parameters(output: &mut String, parameters: &[String]) {
    if !parameters.is_empty() {
        write!(output, "<{}>", parameters.join(", ")).unwrap();
    }
}

fn emit_enums(output: &mut String, module: ModuleId, program: &Program, include_prelude: bool) {
    for (enum_index, enumeration) in program.enums.iter().enumerate() {
        if enum_index < 2 && !include_prelude {
            continue;
        }
        let base = enum_name(module, enum_index);
        write!(output, "sealed class {base}").unwrap();
        emit_type_parameters(output, &enumeration.type_parameters);
        writeln!(output, " {{ const {base}(); }}").unwrap();
        for (variant_index, variant) in enumeration.variants.iter().enumerate() {
            let variant_class = variant_name(module, enum_index, variant_index);
            write!(output, "final class {variant_class}").unwrap();
            emit_type_parameters(output, &enumeration.type_parameters);
            write!(output, " extends {base}").unwrap();
            emit_type_parameters(output, &enumeration.type_parameters);
            output.push_str(" {\n  const ");
            write!(output, "{variant_class}(").unwrap();
            for field_index in 0..variant.fields.len() {
                if field_index != 0 {
                    output.push_str(", ");
                }
                write!(
                    output,
                    "this.{}",
                    variant_field_name(enum_index, variant_index, field_index)
                )
                .unwrap();
            }
            output.push_str(");\n");
            for (field_index, field) in variant.fields.iter().enumerate() {
                output.push_str("  final ");
                emit_enum_field_type(output, module, program, field, enumeration);
                writeln!(
                    output,
                    " {};",
                    variant_field_name(enum_index, variant_index, field_index)
                )
                .unwrap();
            }
            output.push_str("}\n");
        }
        output.push('\n');
    }
}

fn emit_enum_field_type(
    output: &mut String,
    module: ModuleId,
    program: &Program,
    ty: &Type,
    enumeration: &oche_frontend::Enum,
) {
    let Type::Named { name, arguments } = ty;
    if enumeration.type_parameters.contains(name) {
        output.push_str(name);
    } else {
        let shallow = Type::Named {
            name: name.clone(),
            arguments: Vec::new(),
        };
        emit_type(output, module, program, &shallow);
        if !arguments.is_empty() {
            output.push('<');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                emit_enum_field_type(output, module, program, argument, enumeration);
            }
            output.push('>');
        }
    }
}

fn emit_structs(output: &mut String, module: ModuleId, program: &Program) {
    for (struct_index, structure) in program.structs.iter().enumerate() {
        let name = struct_name(module, struct_index);
        writeln!(output, "final class {name} {{").unwrap();
        write!(output, "  const {name}({{").unwrap();
        for field_index in 0..structure.fields.len() {
            if field_index != 0 {
                output.push_str(", ");
            }
            write!(output, "required this.f{field_index}").unwrap();
        }
        output.push_str("});\n");
        for (field_index, field) in structure.fields.iter().enumerate() {
            output.push_str("  final ");
            emit_type(output, module, program, &field.ty);
            writeln!(output, " f{field_index};").unwrap();
            output.push_str("  ");
            emit_type(output, module, program, &field.ty);
            writeln!(
                output,
                " get {} => f{field_index};",
                field_accessor_name(&field.name)
            )
            .unwrap();
        }
        output.push_str("}\n\n");
    }
}

fn field_accessor_name(name: &str) -> String {
    let mut output = String::from("ocheField_");
    for byte in name.bytes() {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

fn emit_comptime(
    output: &mut String,
    module: ModuleId,
    program: &Program,
    comptime: &ComptimeOutput,
) {
    for derive in &comptime.validators {
        emit_validator(output, module, program, derive.struct_index, &derive.rules);
    }
}

fn emit_validator(
    output: &mut String,
    module: ModuleId,
    program: &Program,
    struct_index: usize,
    rules: &[ResolvedRule],
) {
    let structure = &program.structs[struct_index];
    let structure_name = struct_name(module, struct_index);
    let validator = validate_name(module, struct_index);
    writeln!(
        output,
        "OcheResult<{structure_name}, String> {validator}({structure_name} value) {{"
    )
    .unwrap();
    for rule in rules {
        let field = &structure.fields[rule.field_index];
        match rule.rule {
            ValidationRule::NonEmpty => {
                write!(
                    output,
                    "  if (value.f{}.isEmpty) return OcheErr<{structure_name}, String>(",
                    rule.field_index
                )
                .unwrap();
                emit_string(output, &format!("{} must not be empty", field.name));
                output.push_str(");\n");
            }
            ValidationRule::Range { minimum, maximum } => {
                write!(output, "  if (value.f{0} < {minimum} || value.f{0} > {maximum}) return OcheErr<{structure_name}, String>(", rule.field_index).unwrap();
                emit_string(
                    output,
                    &format!("{} must be between {minimum} and {maximum}", field.name),
                );
                output.push_str(");\n");
            }
        }
    }
    writeln!(output, "  return OcheOk<{structure_name}, String>(value);").unwrap();
    output.push_str("}\n\n");
}

fn emit_type(output: &mut String, module: ModuleId, program: &Program, ty: &Type) {
    let Type::Named { name, arguments } = ty;
    match name.as_str() {
        "Int" => output.push_str("int"),
        "Float" => output.push_str("double"),
        "Bool" => output.push_str("bool"),
        "String" => output.push_str("String"),
        "Type" => output.push_str("OcheType"),
        "Field" => output.push_str("OcheField"),
        "JsonValue" => output.push_str("Object?"),
        "JsonObject" => output.push_str("Map<String, Object?>"),
        "Future" => output.push_str("Future"),
        "List" => output.push_str("List"),
        _ => {
            if let Some(index) = program
                .enums
                .iter()
                .position(|enumeration| enumeration.name == *name)
            {
                output.push_str(&enum_name(module, index));
                if !arguments.is_empty() {
                    output.push('<');
                    for (argument_index, argument) in arguments.iter().enumerate() {
                        if argument_index != 0 {
                            output.push_str(", ");
                        }
                        emit_type(output, module, program, argument);
                    }
                    output.push('>');
                }
                return;
            }
            let index = program
                .structs
                .iter()
                .position(|structure| structure.name == *name)
                .expect("types are checked by the frontend");
            output.push_str(&struct_name(module, index));
        }
    }
    if !arguments.is_empty() {
        output.push('<');
        for (index, argument) in arguments.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            emit_type(output, module, program, argument);
        }
        output.push('>');
    }
}

fn emit_cli_argument(output: &mut String, ty: &Type, index: usize, function: &str) {
    let Type::Named { name, .. } = ty;
    match name.as_str() {
        "Int" => write!(output, "int.parse(arguments[{index}])").unwrap(),
        "Float" => write!(output, "double.parse(arguments[{index}])").unwrap(),
        "Bool" => write!(output, "bool.parse(arguments[{index}])").unwrap(),
        "String" => write!(output, "arguments[{index}]").unwrap(),
        "Option" | "Result" => write!(
            output,
            "throw UnsupportedError('CLI cannot decode {name} parameter for {function}')"
        )
        .unwrap(),
        _ => write!(
            output,
            "throw UnsupportedError('CLI cannot decode {name} parameter for {function}')"
        )
        .unwrap(),
    }
}

fn emit_expression(
    output: &mut String,
    program: &Program,
    typed: &TypedProgram,
    environment: &[(String, String)],
    next_binding: &mut usize,
    expression: &TypedExpr,
) {
    match &expression.kind {
        TypedExprKind::Integer(value) => write!(output, "{value}").unwrap(),
        TypedExprKind::Float(value) => emit_float(output, *value),
        TypedExprKind::Boolean(value) => write!(output, "{value}").unwrap(),
        TypedExprKind::String(value) => emit_string(output, value),
        TypedExprKind::TypeValue(value) => emit_reflected_type(output, program, value),
        TypedExprKind::List(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                emit_expression(output, program, typed, environment, next_binding, value);
            }
            output.push(']');
        }
        TypedExprKind::ListLength(value) => {
            output.push('(');
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push_str(").length");
        }
        TypedExprKind::ListFirst(value) => {
            output.push_str("_ocheListFirst(");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::ListLast(value) => {
            output.push_str("_ocheListLast(");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::ListRest(value) => {
            output.push_str("_ocheListRest(");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::TypeProperty { value, property } => {
            output.push('(');
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push_str(match property {
                TypeProperty::Name => ").name",
                TypeProperty::Arguments => ").arguments",
                TypeProperty::Fields => ").fields",
                TypeProperty::FieldType => ").ty",
            });
        }
        TypedExprKind::Variable(name) => output.push_str(
            environment
                .iter()
                .rev()
                .find(|(source, _)| source == name)
                .map(|(_, dart)| dart.as_str())
                .expect("variables are resolved by typed HIR"),
        ),
        TypedExprKind::Call { target, arguments } => {
            if arguments.is_empty() && matches!(target, Callable::Variant { .. }) {
                output.push_str("const ");
            }
            output.push_str(&callable_name(program, typed, target));
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                emit_expression(output, program, typed, environment, next_binding, argument);
            }
            output.push(')');
        }
        TypedExprKind::StructLiteral { structure, fields } => {
            assert_eq!(structure.module, typed.module);
            write!(
                output,
                "{}(",
                struct_name(structure.module, structure.index)
            )
            .unwrap();
            for (index, (field, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                write!(output, "f{field}: ").unwrap();
                emit_expression(output, program, typed, environment, next_binding, value);
            }
            output.push(')');
        }
        TypedExprKind::Field {
            value,
            structure,
            field,
        } => {
            assert_eq!(structure.module, typed.module);
            output.push('(');
            emit_expression(output, program, typed, environment, next_binding, value);
            write!(
                output,
                ").{}",
                field_accessor_name(&program.structs[structure.index].fields[*field].name)
            )
            .unwrap();
        }
        TypedExprKind::Negate(value) => {
            output.push_str("(-");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::Await(value) => {
            output.push_str("(await ");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::IsolateRun(value) => {
            output.push_str("Isolate.run(() => ");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push(')');
        }
        TypedExprKind::Comptime(value) => {
            emit_expression(output, program, typed, environment, next_binding, value);
        }
        TypedExprKind::Binary {
            operator,
            left,
            right,
        } => {
            output.push('(');
            emit_expression(output, program, typed, environment, next_binding, left);
            output.push_str(match operator {
                Operator::Add => " + ",
                Operator::Subtract => " - ",
                Operator::Multiply => " * ",
            });
            emit_expression(output, program, typed, environment, next_binding, right);
            output.push(')');
        }
        TypedExprKind::Match { value, arms } => {
            output.push_str("switch (");
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push_str(") {\n");
            for arm in arms {
                output.push_str("  ");
                let mut arm_environment = environment.to_vec();
                emit_pattern(
                    output,
                    typed.module,
                    program,
                    &arm.pattern,
                    &mut arm_environment,
                    next_binding,
                );
                if let Some(guard) = &arm.guard {
                    output.push_str(" when ");
                    emit_expression(
                        output,
                        program,
                        typed,
                        &arm_environment,
                        next_binding,
                        guard,
                    );
                }
                output.push_str(" => ");
                emit_expression(
                    output,
                    program,
                    typed,
                    &arm_environment,
                    next_binding,
                    &arm.body,
                );
                output.push_str(",\n");
            }
            output.push('}');
        }
        TypedExprKind::Block(block) => {
            emit_block(
                output,
                program,
                typed,
                environment,
                next_binding,
                block,
                false,
            );
        }
        TypedExprKind::Try(block) => {
            let Type::Named { name, arguments } = &expression.ty;
            let is_option = name == "Option";
            let is_async = typed_block_contains_await(block);
            let helper = match (is_option, is_async) {
                (true, true) => "_ocheTryOptionAsync",
                (true, false) => "_ocheTryOption",
                (false, true) => "_ocheTryResultAsync",
                (false, false) => "_ocheTryResult",
            };
            if is_async {
                output.push_str("(await ");
            }
            output.push_str(helper);
            output.push('<');
            let type_arguments = if is_option {
                &arguments[..arguments.len().min(1)]
            } else {
                arguments.as_slice()
            };
            for (index, argument) in type_arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                emit_type(output, typed.module, program, argument);
            }
            output.push('>');
            if is_async {
                output.push_str("(() async ");
            } else {
                output.push_str("(() ");
            }
            emit_block(
                output,
                program,
                typed,
                environment,
                next_binding,
                block,
                true,
            );
            output.push(')');
            if is_async {
                output.push(')');
            }
        }
        TypedExprKind::Propagate(value) => {
            output.push('(');
            emit_expression(output, program, typed, environment, next_binding, value);
            output.push_str(")._ocheUnwrap");
        }
    }
}

fn emit_block(
    output: &mut String,
    program: &Program,
    typed: &TypedProgram,
    environment: &[(String, String)],
    next_binding: &mut usize,
    block: &TypedBlock,
    closure_body: bool,
) {
    if !closure_body {
        output.push_str("(() ");
    }
    output.push_str("{\n");
    let mut block_environment = environment.to_vec();
    for statement in &block.statements {
        output.push_str("  ");
        match statement {
            TypedStatement::Let { .. } => emit_let_statement(
                output,
                program,
                typed,
                statement,
                &mut block_environment,
                next_binding,
            ),
            TypedStatement::Assign { name, value } => {
                let dart = block_environment
                    .iter()
                    .rev()
                    .find(|(source, _)| source == name)
                    .map(|(_, dart)| dart.as_str())
                    .expect("assignments are resolved by typed HIR");
                output.push_str(dart);
                output.push_str(" = ");
                emit_expression(
                    output,
                    program,
                    typed,
                    &block_environment,
                    next_binding,
                    value,
                );
                output.push_str(";\n");
            }
            TypedStatement::Expression(expression) => {
                emit_expression(
                    output,
                    program,
                    typed,
                    &block_environment,
                    next_binding,
                    expression,
                );
                output.push_str(";\n");
            }
        }
    }
    output.push_str("  return ");
    emit_expression(
        output,
        program,
        typed,
        &block_environment,
        next_binding,
        &block.tail,
    );
    output.push_str(";\n}");
    if !closure_body {
        output.push_str(")()");
    }
}

fn emit_let_statement(
    output: &mut String,
    program: &Program,
    typed: &TypedProgram,
    statement: &TypedStatement,
    environment: &mut Vec<(String, String)>,
    next_binding: &mut usize,
) {
    let TypedStatement::Let {
        mutable,
        pattern,
        value,
    } = statement
    else {
        unreachable!("emit_let_statement only accepts let statements");
    };
    let original_environment = environment.clone();
    if let Pattern::Binding(name) = pattern.unlocated() {
        let dart = format!("l{next_binding}");
        *next_binding += 1;
        write!(
            output,
            "{} {dart} = ",
            if *mutable { "var" } else { "final" }
        )
        .unwrap();
        emit_expression(
            output,
            program,
            typed,
            &original_environment,
            next_binding,
            value,
        );
        output.push_str(";\n");
        environment.push((name.clone(), dart));
        return;
    }

    let mut value_output = String::new();
    emit_expression(
        &mut value_output,
        program,
        typed,
        &original_environment,
        next_binding,
        value,
    );
    let mut pattern_output = String::new();
    let mut match_environment = original_environment.clone();
    emit_pattern(
        &mut pattern_output,
        typed.module,
        program,
        pattern,
        &mut match_environment,
        next_binding,
    );
    let captures = &match_environment[original_environment.len()..];
    let destination_start = *next_binding;
    let destinations = (0..captures.len())
        .map(|index| format!("l{}", destination_start + index))
        .collect::<Vec<_>>();
    if !destinations.is_empty() {
        output.push_str("final ");
        if destinations.len() == 1 {
            output.push_str(&destinations[0]);
        } else {
            write!(output, "({})", destinations.join(", ")).unwrap();
        }
        output.push_str(" = ");
    }
    output.push_str("switch (");
    output.push_str(&value_output);
    write!(output, ") {{ {pattern_output} => ").unwrap();
    match captures {
        [] => output.push_str("null"),
        [capture] => output.push_str(&capture.1),
        _ => {
            output.push('(');
            for (index, (_, dart)) in captures.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                output.push_str(dart);
            }
            output.push(')');
        }
    }
    output.push_str(", _ => throw StateError('let pattern did not match') };\n");
    for (index, (source, _)) in captures.iter().enumerate() {
        environment.push((source.clone(), format!("l{}", destination_start + index)));
    }
    *next_binding += captures.len();
}

fn typed_block_contains_await(block: &TypedBlock) -> bool {
    typed_block_contains(block, TypedExpressionNeedle::Await)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypedExpressionNeedle {
    Await,
    IsolateRun,
    PropagationRuntime,
    EscapingPropagation,
}

fn typed_block_contains(block: &TypedBlock, needle: TypedExpressionNeedle) -> bool {
    block.statements.iter().any(|statement| match statement {
        TypedStatement::Let { value, .. }
        | TypedStatement::Assign { value, .. }
        | TypedStatement::Expression(value) => typed_expression_contains(value, needle),
    }) || typed_expression_contains(&block.tail, needle)
}

fn typed_expression_contains(expression: &TypedExpr, needle: TypedExpressionNeedle) -> bool {
    match &expression.kind {
        TypedExprKind::Await(value) => {
            needle == TypedExpressionNeedle::Await || typed_expression_contains(value, needle)
        }
        TypedExprKind::IsolateRun(value) => {
            needle == TypedExpressionNeedle::IsolateRun || typed_expression_contains(value, needle)
        }
        TypedExprKind::Comptime(value) => typed_expression_contains(value, needle),
        TypedExprKind::Propagate(value) => {
            needle != TypedExpressionNeedle::Await || typed_expression_contains(value, needle)
        }
        TypedExprKind::Try(block) => match needle {
            TypedExpressionNeedle::PropagationRuntime => true,
            TypedExpressionNeedle::EscapingPropagation => false,
            TypedExpressionNeedle::Await | TypedExpressionNeedle::IsolateRun => {
                typed_block_contains(block, needle)
            }
        },
        TypedExprKind::ListLength(value)
        | TypedExprKind::ListFirst(value)
        | TypedExprKind::ListLast(value)
        | TypedExprKind::ListRest(value)
        | TypedExprKind::TypeProperty { value, .. }
        | TypedExprKind::Field { value, .. }
        | TypedExprKind::Negate(value) => typed_expression_contains(value, needle),
        TypedExprKind::Binary { left, right, .. } => {
            typed_expression_contains(left, needle) || typed_expression_contains(right, needle)
        }
        TypedExprKind::Call { arguments, .. } | TypedExprKind::List(arguments) => arguments
            .iter()
            .any(|argument| typed_expression_contains(argument, needle)),
        TypedExprKind::StructLiteral { fields, .. } => fields
            .iter()
            .any(|(_, value)| typed_expression_contains(value, needle)),
        TypedExprKind::Match { value, arms } => {
            typed_expression_contains(value, needle)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| typed_expression_contains(guard, needle))
                        || typed_expression_contains(&arm.body, needle)
                })
        }
        TypedExprKind::Block(block) => typed_block_contains(block, needle),
        TypedExprKind::Integer(_)
        | TypedExprKind::Float(_)
        | TypedExprKind::Boolean(_)
        | TypedExprKind::String(_)
        | TypedExprKind::TypeValue(_)
        | TypedExprKind::Variable(_) => false,
    }
}

fn callable_name(program: &Program, typed: &TypedProgram, target: &Callable) -> String {
    match target {
        Callable::Function(id) => function_name(id.module, id.index),
        Callable::Variant(id) => {
            assert_eq!(id.module, typed.module);
            variant_name(id.module, id.enumeration, id.variant)
        }
        Callable::Synthetic(id) => {
            assert_eq!(id.module, typed.module);
            let synthetic = &typed.synthetic_functions[id.index];
            assert_eq!(synthetic.structure.module, typed.module);
            let structure = synthetic.structure.index;
            match synthetic.origin {
                SyntheticOrigin::JsonEncode => json_to_name(id.module, structure),
                SyntheticOrigin::JsonEncodeValue => json_to_value_name(id.module, structure),
                SyntheticOrigin::JsonDecode => json_from_name(id.module, structure),
                SyntheticOrigin::JsonDecodeValue => json_from_value_name(id.module, structure),
                SyntheticOrigin::Validate => validate_name(id.module, structure),
            }
        }
        Callable::External(id) => {
            assert_eq!(id.module, typed.module);
            format!(
                "{}.{}",
                interop_name(id.module, id.library),
                program.dart_libraries[id.library].functions[id.function].name
            )
        }
        Callable::Intrinsic(intrinsic) => intrinsic.name().into(),
    }
}

fn resolve_variant(program: &Program, name: &str) -> (usize, usize) {
    for (enum_index, enumeration) in program.enums.iter().enumerate() {
        if let Some(variant_index) = enumeration
            .variants
            .iter()
            .position(|variant| variant.name == name)
        {
            return (enum_index, variant_index);
        }
    }
    unreachable!("variants are checked by the frontend")
}

fn emit_pattern(
    output: &mut String,
    module: ModuleId,
    program: &Program,
    pattern: &Pattern,
    environment: &mut Vec<(String, String)>,
    next_binding: &mut usize,
) {
    match pattern.unlocated() {
        Pattern::Located { .. } => unreachable!("unlocated pattern cannot be located"),
        Pattern::Wildcard => output.push('_'),
        Pattern::Binding(name) => {
            let dart = format!("m{next_binding}");
            *next_binding += 1;
            write!(output, "final {dart}").unwrap();
            environment.push((name.clone(), dart));
        }
        Pattern::Integer(value) => write!(output, "{value}").unwrap(),
        Pattern::Float(value) => emit_float(output, *value),
        Pattern::Boolean(value) => write!(output, "{value}").unwrap(),
        Pattern::String(value) => emit_string(output, value),
        Pattern::Variant { name, fields } => {
            let (enum_index, variant_index) = resolve_variant(program, name);
            write!(
                output,
                "{}(",
                variant_name(module, enum_index, variant_index)
            )
            .unwrap();
            for (field_index, field) in fields.iter().enumerate() {
                if field_index != 0 {
                    output.push_str(", ");
                }
                write!(
                    output,
                    "{}: ",
                    variant_field_name(enum_index, variant_index, field_index)
                )
                .unwrap();
                emit_pattern(output, module, program, field, environment, next_binding);
            }
            output.push(')');
        }
        Pattern::Struct { name, fields, .. } => {
            let struct_index = program
                .structs
                .iter()
                .position(|structure| structure.name == *name)
                .expect("struct patterns are checked by the frontend");
            write!(output, "{}(", struct_name(module, struct_index)).unwrap();
            for (index, (field_name, field_pattern)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                let field_index = program.structs[struct_index]
                    .fields
                    .iter()
                    .position(|field| field.name == *field_name)
                    .expect("struct fields are checked by the frontend");
                write!(output, "f{field_index}: ").unwrap();
                emit_pattern(
                    output,
                    module,
                    program,
                    field_pattern,
                    environment,
                    next_binding,
                );
            }
            output.push(')');
        }
    }
}

fn emit_string(output: &mut String, value: &str) {
    output.push('\'');
    for character in value.chars() {
        match character {
            '\'' => output.push_str("\\'"),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '$' => output.push_str("\\$"),
            _ => output.push(character),
        }
    }
    output.push('\'');
}

fn emit_float(output: &mut String, value: f64) {
    let rendered = value.to_string();
    output.push_str(&rendered);
    if !rendered.contains(['.', 'e', 'E']) {
        output.push_str(".0");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowers_std_isolate_run_only_for_native_targets() {
        let program = oche_frontend::parse(
            r#"
            import "std/isolate" as isolate
            fn work(value: Int) -> Future<Int> { isolate.run(value * 2) }
            "#,
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("import 'dart:isolate';"));
        assert!(dart.contains("return Isolate.run(() => (p0 * 2));"));
        assert!(matches!(
            emit_dart_js_module(&program),
            Err(Error::UnsupportedJavaScriptIsolate)
        ));
    }

    #[test]
    fn emits_only_the_dedicated_comptime_runner_before_expansion() {
        let program = oche_frontend::parse(
            "const fn scale(value: Int, factor: Int) -> Int { value * factor }
             fn configured() -> Int { comptime scale(16, 2) }",
        )
        .unwrap();
        assert!(matches!(
            emit_dart(&program),
            Err(Error::UnevaluatedComptime { .. })
        ));
        let dart = emit_dart_comptime(&program).unwrap();
        assert!(dart.contains("case '0':"));
        assert!(dart.contains("final result = ocheFunction0(16, 2);"));
        assert!(dart.contains("print(jsonEncode(result));"));
        assert!(!dart.contains("int ocheFunction1()"));
    }

    #[test]
    fn emits_transitively_linked_modules_with_qualified_names() {
        let root = oche_frontend::parse(
            "import \"service.oche\" as service\nfn answer() -> Int { service.answer() }",
        )
        .unwrap();
        let service = oche_frontend::parse(
            "import \"math.oche\" as math\npub fn answer() -> Int { math.double(21) }",
        )
        .unwrap();
        let math =
            oche_frontend::parse("pub fn double(value: Int) -> Int { value + value }").unwrap();
        let package = LinkedPackage {
            root: ModuleId::ROOT,
            modules: vec![
                oche_hir::LinkedModule {
                    program: root,
                    imports: vec![oche_hir::ModuleImport {
                        alias: "service".into(),
                        module: ModuleId::new(1),
                    }],
                },
                oche_hir::LinkedModule {
                    program: service,
                    imports: vec![oche_hir::ModuleImport {
                        alias: "math".into(),
                        module: ModuleId::new(2),
                    }],
                },
                oche_hir::LinkedModule {
                    program: math,
                    imports: Vec::new(),
                },
            ],
        };

        let dart = emit_dart_package(&package).unwrap();
        assert!(dart.contains("int ocheModule2Function0(int p0)"));
        assert!(dart.contains("int ocheModule1Function0()"));
        assert!(dart.contains("return ocheModule2Function0(21);"));
        assert!(dart.contains("return ocheModule1Function0();"));
        assert!(dart.contains("print(ocheFunction0());"));
        assert_eq!(dart.matches("sealed class OcheOption").count(), 1);
    }

    #[test]
    fn lowers_dart_keywords_without_collisions() {
        let program = oche_frontend::parse(
            "fn class(switch: Int, value: Int) -> Int { switch - value * -2 }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("int ocheFunction0(int p0, int p1) {"));
        assert!(dart.contains("return (p0 - (p1 * -2));"));
        assert!(dart.contains("case 'class':"));
        assert!(!dart.contains("int switch"));
    }

    #[test]
    fn lowers_mutable_bindings_to_dart_locals() {
        let program = oche_frontend::parse(
            "fn bump(value: Int) -> Int {
                let mut current = value;
                current = current + 1;
                current
            }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("var l0 = p0;"));
        assert!(dart.contains("l0 = (l0 + 1);"));
        assert!(dart.contains("return l0;"));
    }

    #[test]
    fn omits_propagation_runtime_from_functions_that_cannot_propagate() {
        let plain = oche_frontend::parse("fn answer() -> Int { 42 }").unwrap();
        let dart = emit_dart(&plain).unwrap();
        assert!(!dart.contains("_OchePropagation"));
        assert!(!dart.contains("on _OchePropagation catch"));
        assert!(dart.contains("int ocheFunction0() {\n  return 42;\n}"));

        let fallible = oche_frontend::parse(
            "fn checked(value: Int) -> Result<Int, String> { Ok(value) }
             fn forward(value: Int) -> Result<Int, String> { Ok(checked(value)? + 1) }",
        )
        .unwrap();
        let dart = emit_dart(&fallible).unwrap();
        assert!(dart.contains("final class _OcheResultPropagation"));
        assert!(dart.contains("final class _OcheOptionPropagation"));
        assert_eq!(dart.matches("on _OcheResultPropagation catch").count(), 3);
        assert_eq!(dart.matches("on _OcheOptionPropagation").count(), 2);
    }

    #[test]
    fn lowers_option_match_to_dart_patterns() {
        let program = oche_frontend::parse(
            "fn unwrap(value: Option<Int>) -> Int {
                match value { Some(inner) => inner, None => 0 }
            }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("OcheOption<int> p0"));
        assert!(dart.contains("OcheSome(value: final m0) => m0"));
        assert!(dart.contains("OcheNone() => 0"));
    }

    #[test]
    fn lowers_match_guards_to_dart_when_clauses() {
        let program = oche_frontend::parse(
            "fn allowed(value: Int) -> Bool { true }
             fn choose(value: Option<Int>) -> Int {
               match value { Some(inner) if allowed(inner) => inner, Some(_) => 0, None => -1 }
             }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains(" when ocheFunction0("));
    }

    #[test]
    fn lowers_struct_fields_without_dynamic_dispatch() {
        let program = oche_frontend::parse(
            "struct User { name: String, age: Int }
             fn age(name: String) -> Int { User { name: name, age: 37 }.age }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("int get ocheField_616765 => f1;"));
        assert!(dart.contains(").ocheField_616765"));
        assert!(!dart.contains("dynamic _ocheField"));
    }

    #[test]
    fn lowers_typed_list_literals_to_dart_lists() {
        let program = oche_frontend::parse(
            "fn values() -> List<Int> { [1, 2, 3] }
             fn empty() -> List<String> { [] }
             fn append(values: List<Int>) -> List<Int> { values + [4] }
             fn count(values: List<Int>) -> Int { values.len() }
             fn last(values: List<String>) -> String {
               match values.last() { Some(value) => value, None => \"\" }
             }
             fn first(values: List<String>) -> Option<String> { values.first() }
             fn rest(values: List<String>) -> List<String> { values.rest() }",
        )
        .unwrap();
        let dart = emit_dart(&program).unwrap();
        assert!(dart.contains("List<int> ocheFunction0()"));
        assert!(dart.contains("return [1, 2, 3];"));
        assert!(dart.contains("List<String> ocheFunction1()"));
        assert!(dart.contains("return [];"));
        assert!(dart.contains("return (p0 + [4]);"));
        assert!(dart.contains("return (p0).length;"));
        assert!(dart.contains("switch (_ocheListLast(p0))"));
        assert!(dart.contains("return _ocheListFirst(p0);"));
        assert!(dart.contains("return _ocheListRest(p0);"));
    }

    #[test]
    fn exports_primitive_functions_to_javascript() {
        let program = oche_frontend::parse(
            "pub fn add(left: Int, right: Int) -> Int { left + right }
             pub fn scale(value: Float, factor: Float) -> Float { value * factor }
             pub async fn later(value: String) -> String { value }
             fn internal(value: Option<Int>) -> Int {
               match value { Some(inner) => inner, None => 0 }
             }",
        )
        .unwrap();
        let dart = emit_dart_js_module(&program).unwrap();
        assert!(dart.contains("JSNumber ocheJsFunction0(JSNumber p0, JSNumber p1)"));
        assert!(dart.contains("JSNumber ocheJsFunction1(JSNumber p0, JSNumber p1)"));
        assert!(dart.contains("p0.toDartDouble"));
        assert!(dart.contains("JSPromise<JSString> ocheJsFunction2(JSString p0)"));
        assert!(dart.contains("'add': ocheJsFunction0.toJS"));
        assert!(dart.contains("'scale': ocheJsFunction1.toJS"));
        assert!(dart.contains("'later': ocheJsFunction2.toJS"));

        let unsupported = oche_frontend::parse(
            "pub fn unwrap(value: Option<Int>) -> Int {
               match value { Some(inner) => inner, None => 0 }
             }",
        )
        .unwrap();
        assert!(matches!(
            emit_dart_js_module(&unsupported),
            Err(Error::UnsupportedJavaScriptExport { .. })
        ));
    }

    #[test]
    fn exports_public_functions_to_persistent_jit_rpc() {
        let program = oche_frontend::parse(
            "pub fn add(left: Int, right: Int) -> Int { left + right }
             pub fn scale(value: Float, factor: Float) -> Float { value * factor }
             pub async fn later(value: String) -> String { value }
             fn private(value: Int) -> Int { value }",
        )
        .unwrap();
        let dart = emit_dart_jit_rpc(&program).unwrap();
        assert!(dart.contains("import 'dart:io';"));
        assert!(dart.contains("await for (final line in stdin.transform"));
        assert!(dart.contains("final Object? decodedRequest = jsonDecode(line);"));
        assert!(dart.contains("decodedRequest is! Map<String, Object?>"));
        assert!(dart.contains("args is! List<Object?>"));
        assert!(!dart.contains("dynamic"));
        assert!(dart.contains("case 'add':"));
        assert!(dart.contains("value = ocheFunction0(args[0] as int, args[1] as int);"));
        assert!(dart.contains("(args[0] as num).toDouble()"));
        assert!(dart.contains("value = await ocheFunction2(args[0] as String);"));
        assert!(!dart.contains("case 'private':"));

        let unsupported =
            oche_frontend::parse("pub fn values(value: List<Int>) -> Int { value.len() }").unwrap();
        assert!(matches!(
            emit_dart_jit_rpc(&unsupported),
            Err(Error::UnsupportedJitRpcExport { .. })
        ));
    }

    #[test]
    fn emits_a_jit_test_runner_for_sync_and_async_tests() {
        let program = oche_frontend::parse(
            "fn helper() -> Bool { true }
             test fn sync_case() -> Bool { helper() }
             test async fn async_case() -> Bool { true }",
        )
        .unwrap();
        let dart = emit_dart_tests(&program).unwrap();
        assert!(dart.contains("import 'dart:io';"));
        assert!(dart.contains("final result = ocheFunction1();"));
        assert!(dart.contains("final result = await ocheFunction2();"));
        assert!(dart.contains("print('PASS sync_case');"));
        assert!(dart.contains("if (failed != 0) exitCode = 1;"));
        assert!(!dart.contains("PASS helper"));
    }
}
