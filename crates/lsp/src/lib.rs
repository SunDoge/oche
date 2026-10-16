//! Minimal synchronous Language Server Protocol implementation for Oche.

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use lsp_server::{ErrorCode, Message, Notification, Request, Response};
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use serde_json::{Value, json};
use snafu::{ResultExt, Snafu};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("LSP transport failed: {source}"))]
    Transport { source: std::io::Error },
}

pub fn run(input: impl Read, output: impl Write) -> Result<()> {
    Server::new(input, output).run()
}

struct Server<R, W> {
    input: BufReader<R>,
    output: W,
    documents: HashMap<String, oche_analysis::FileId>,
    analysis: oche_analysis::Database,
    shutdown: bool,
}

struct LinkDiagnostic {
    span: oche_analysis::Span,
    message: String,
}

struct DocumentAnalysis {
    diagnostics: Vec<oche_analysis::Diagnostic>,
    typed: Option<oche_hir::TypedProgram>,
}

impl<R: Read, W: Write> Server<R, W> {
    fn new(input: R, output: W) -> Self {
        Self {
            input: BufReader::new(input),
            output,
            documents: HashMap::new(),
            analysis: oche_analysis::Database::default(),
            shutdown: false,
        }
    }

    fn run(mut self) -> Result<()> {
        while let Some(message) = self.read_message()? {
            if self.handle(message)? {
                break;
            }
        }
        Ok(())
    }

    fn read_message(&mut self) -> Result<Option<Message>> {
        Message::read(&mut self.input).context(TransportSnafu)
    }

    fn send(&mut self, message: Message) -> Result<()> {
        message.write(&mut self.output).context(TransportSnafu)
    }

    fn respond(&mut self, request: Request, result: Value) -> Result<()> {
        self.send(Response::new_ok(request.id, result).into())
    }

    fn handle(&mut self, message: Message) -> Result<bool> {
        let (method, params, request) = match message {
            Message::Request(request) => (
                request.method.clone(),
                request.params.clone(),
                Some(request),
            ),
            Message::Notification(notification) => (notification.method, notification.params, None),
            Message::Response(_) => return Ok(false),
        };
        match method.as_str() {
            "initialize" => {
                if let Some(request) = request {
                    self.respond(
                        request,
                        json!({
                            "capabilities": {
                                "textDocumentSync": {"openClose": true, "change": 1},
                                "hoverProvider": true,
                                "documentFormattingProvider": true
                            },
                            "serverInfo": {"name": "oche", "version": env!("CARGO_PKG_VERSION")}
                        }),
                    )?;
                }
            }
            "shutdown" => {
                self.shutdown = true;
                if let Some(request) = request {
                    self.respond(request, Value::Null)?;
                }
            }
            "exit" => return Ok(true),
            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    params.pointer("/textDocument/uri").and_then(Value::as_str),
                    params.pointer("/textDocument/text").and_then(Value::as_str),
                ) {
                    let file = self.analysis.set(uri, text);
                    self.documents.insert(uri.to_string(), file);
                    self.publish_all_diagnostics()?;
                }
            }
            "textDocument/didChange" => {
                let uri = params.pointer("/textDocument/uri").and_then(Value::as_str);
                let text = params
                    .pointer("/contentChanges/0/text")
                    .and_then(Value::as_str);
                if let (Some(uri), Some(text)) = (uri, text) {
                    let file = self.analysis.set(uri, text);
                    self.documents.insert(uri.to_string(), file);
                    self.publish_all_diagnostics()?;
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) {
                    self.documents.remove(uri);
                    self.analysis.remove(uri);
                    self.notify(
                        "textDocument/publishDiagnostics",
                        json!({"uri": uri, "diagnostics": []}),
                    )?;
                    self.publish_all_diagnostics()?;
                }
            }
            "textDocument/hover" => {
                if let Some(request) = request {
                    let result = self.hover(&params);
                    self.respond(request, result)?;
                }
            }
            "textDocument/formatting" => {
                if let Some(request) = request {
                    self.respond(request, self.formatting(&params))?;
                }
            }
            _ if request.is_some() => {
                let request = request.expect("request checked above");
                self.send(
                    Response::new_err(
                        request.id,
                        ErrorCode::MethodNotFound as i32,
                        "Method not found".into(),
                    )
                    .into(),
                )?;
            }
            _ => {}
        }
        Ok(self.shutdown && method == "exit")
    }

    fn notify(&mut self, method: impl Into<String>, params: Value) -> Result<()> {
        self.send(
            Notification {
                method: method.into(),
                params,
            }
            .into(),
        )
    }

    fn publish_all_diagnostics(&mut self) -> Result<()> {
        let mut uris = self.documents.keys().cloned().collect::<Vec<_>>();
        uris.sort();
        for uri in uris {
            self.publish_diagnostics(&uri)?;
        }
        Ok(())
    }

    fn publish_diagnostics(&mut self, uri: &str) -> Result<()> {
        let Some(file) = self.documents.get(uri).copied() else {
            return Ok(());
        };
        let (analysis, link_diagnostics) = self.analyze_document(uri, file);
        let source = &self.analysis.file(file).expect("document file exists").text;
        let mut diagnostics = analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic_at(source, diagnostic.span, diagnostic.cause.to_string()))
            .collect::<Vec<_>>();
        diagnostics.extend(
            link_diagnostics.iter().map(|diagnostic| {
                diagnostic_at(source, diagnostic.span, diagnostic.message.clone())
            }),
        );
        self.notify(
            "textDocument/publishDiagnostics",
            json!({"uri": uri, "diagnostics": diagnostics}),
        )
    }

    fn analyze_document(
        &mut self,
        uri: &str,
        file: oche_analysis::FileId,
    ) -> (DocumentAnalysis, Vec<LinkDiagnostic>) {
        let parsed = self.analysis.parse(file).expect("document file exists");
        if !parsed.diagnostics.is_empty() {
            let snapshot = self.analysis.analyze(file).expect("document file exists");
            return (
                DocumentAnalysis {
                    diagnostics: snapshot.diagnostics,
                    typed: snapshot.typed,
                },
                Vec::new(),
            );
        }
        let mut builder = EditorPackageBuilder::new(&mut self.analysis);
        let root = match builder.load(uri) {
            Ok(root) => root,
            Err(error) => {
                return (
                    DocumentAnalysis {
                        diagnostics: Vec::new(),
                        typed: None,
                    },
                    vec![LinkDiagnostic {
                        span: error
                            .span
                            .map_or(oche_analysis::Span { start: 0, end: 1 }, analysis_span),
                        message: error.message,
                    }],
                );
            }
        };
        let package = builder.finish(root);
        let mut diagnostics = Vec::new();
        let typed = match oche_hir::lower_package_located(&package.package) {
            Ok(typed) => Some(typed),
            Err(error) => {
                let module = error.function.module;
                if module == root {
                    let source = self.analysis.file(file).expect("document file exists");
                    let span = error.span.map_or_else(
                        || {
                            let offset = package
                                .package
                                .module(root)
                                .program
                                .functions
                                .get(error.function.index)
                                .map_or(0, |function| function.span.start);
                            oche_analysis::Span::at(&source.text, offset)
                        },
                        analysis_span,
                    );
                    diagnostics.push(oche_analysis::Diagnostic {
                        file,
                        span,
                        stage: oche_analysis::Stage::Type,
                        cause: oche_analysis::Cause::Type(*error.source),
                    });
                } else {
                    let span = package.root_import_span_to(module);
                    return (
                        DocumentAnalysis {
                            diagnostics,
                            typed: None,
                        },
                        vec![LinkDiagnostic {
                            span: span
                                .map_or(oche_analysis::Span { start: 0, end: 1 }, analysis_span),
                            message: format!(
                                "Imported module '{}' has a type error: {}",
                                package.uris[module.index()],
                                error.source
                            ),
                        }],
                    );
                }
                None
            }
        };
        let mut link_diagnostics = Vec::new();
        for (index, module) in package.package.modules.iter().enumerate() {
            if let Err(error) = oche_comptime::validate(&module.program) {
                let module = oche_hir::ModuleId::new(index);
                if module == root {
                    let source = self.analysis.file(file).expect("document file exists");
                    diagnostics.push(oche_analysis::Diagnostic {
                        file,
                        span: oche_analysis::Span::at(&source.text, error.offset()),
                        stage: oche_analysis::Stage::Comptime,
                        cause: oche_analysis::Cause::Comptime(error),
                    });
                } else {
                    link_diagnostics.push(LinkDiagnostic {
                        span: package
                            .root_import_span_to(module)
                            .map_or(oche_analysis::Span { start: 0, end: 1 }, analysis_span),
                        message: format!(
                            "Imported module '{}' has a comptime error: {error}",
                            package.uris[index]
                        ),
                    });
                }
            }
        }
        let typed = typed.and_then(|typed| typed.modules.into_iter().nth(root.index()));
        (DocumentAnalysis { diagnostics, typed }, link_diagnostics)
    }

    fn hover(&mut self, params: &Value) -> Value {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return Value::Null;
        };
        let Some(file) = self.documents.get(uri).copied() else {
            return Value::Null;
        };
        let source = &self.analysis.file(file).expect("document file exists").text;
        let line = params.pointer("/position/line").and_then(Value::as_u64);
        let character = params
            .pointer("/position/character")
            .and_then(Value::as_u64);
        let (Some(line), Some(character)) = (line, character) else {
            return Value::Null;
        };
        let Some(offset) = position_to_offset(source, line as usize, character as usize) else {
            return Value::Null;
        };
        let Some(word) = word_at(source, offset).map(str::to_owned) else {
            return Value::Null;
        };
        let associated = associated_synthetic_name(source, offset, &word);
        let description = match word.as_str() {
            "Int" => "Signed 64-bit integer with Dart VM wraparound semantics.",
            "Float" => {
                "IEEE 754 double-precision value mapped to Dart double and JavaScript number."
            }
            "Bool" => "Boolean value.",
            "String" => "UTF-16 Dart string exposed through Oche.",
            "Option" => "Non-nullable tagged optional value: Some(T) or None.",
            "Result" => "Tagged success or failure value: Ok(T) or Err(E).",
            "comptime" => "Evaluate this expression or declaration during compilation.",
            "const" => "Mark a function as eligible for compile-time evaluation.",
            "match" => "Pattern-matching expression.",
            "if" => "Restrict a match arm with a Bool guard.",
            "enum" => "A tagged union lowered to a sealed Dart class family.",
            "import" => "Load declarations under a required module alias.",
            "dart" => "Declare the typed surface of an imported Dart library.",
            "try" => "Capture Option or Result propagation from '?' inside this block.",
            _ => {
                let (snapshot, _) = self.analyze_document(uri, file);
                let Some(typed) = snapshot.typed else {
                    return Value::Null;
                };
                if let Some(hover) = synthetic_hover(&typed, &word) {
                    return hover;
                }
                if let Some(candidate) = associated {
                    return synthetic_hover(&typed, &candidate).unwrap_or(Value::Null);
                }
                return Value::Null;
            }
        };
        json!({"contents": {"kind": "markdown", "value": format!("```oche\n{word}\n```\n\n{description}")}})
    }

    fn formatting(&self, params: &Value) -> Value {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return json!([]);
        };
        let Some(file) = self.documents.get(uri).copied() else {
            return json!([]);
        };
        let source = &self.analysis.file(file).expect("document file exists").text;
        let Ok(formatted) = oche_formatter::format(source, 100) else {
            return json!([]);
        };
        if formatted == *source {
            return json!([]);
        }
        json!([{
            "range": {
                "start": {"line": 0, "character": 0},
                "end": offset_to_position(source, source.len())
            },
            "newText": formatted
        }])
    }
}

struct PackageBuildError {
    span: Option<oche_frontend::SourceSpan>,
    message: String,
}

struct EditorPackageBuilder<'a> {
    analysis: &'a mut oche_analysis::Database,
    ids: HashMap<String, oche_hir::ModuleId>,
    uris: Vec<String>,
    modules: Vec<Option<oche_hir::LinkedModule>>,
    edges: Vec<Vec<(oche_hir::ModuleId, oche_frontend::SourceSpan)>>,
    loading: Vec<bool>,
    stack: Vec<oche_hir::ModuleId>,
}

struct EditorPackage {
    package: oche_hir::LinkedPackage,
    uris: Vec<String>,
    edges: Vec<Vec<(oche_hir::ModuleId, oche_frontend::SourceSpan)>>,
}

impl EditorPackage {
    fn root_import_span_to(&self, target: oche_hir::ModuleId) -> Option<oche_frontend::SourceSpan> {
        let root = self.package.root;
        self.edges[root.index()]
            .iter()
            .find(|(module, _)| self.reaches(*module, target, &mut vec![false; self.edges.len()]))
            .map(|(_, span)| *span)
    }

    fn reaches(
        &self,
        from: oche_hir::ModuleId,
        target: oche_hir::ModuleId,
        visited: &mut [bool],
    ) -> bool {
        if from == target {
            return true;
        }
        if std::mem::replace(&mut visited[from.index()], true) {
            return false;
        }
        self.edges[from.index()]
            .iter()
            .any(|(next, _)| self.reaches(*next, target, visited))
    }
}

impl<'a> EditorPackageBuilder<'a> {
    fn new(analysis: &'a mut oche_analysis::Database) -> Self {
        Self {
            analysis,
            ids: HashMap::new(),
            uris: Vec::new(),
            modules: Vec::new(),
            edges: Vec::new(),
            loading: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn finish(self, root: oche_hir::ModuleId) -> EditorPackage {
        EditorPackage {
            package: oche_hir::LinkedPackage {
                root,
                modules: self
                    .modules
                    .into_iter()
                    .map(|module| module.expect("successful editor linking fills every module"))
                    .collect(),
            },
            uris: self.uris,
            edges: self.edges,
        }
    }

    fn load(&mut self, uri: &str) -> std::result::Result<oche_hir::ModuleId, PackageBuildError> {
        if let Some(id) = self.ids.get(uri).copied() {
            if self.loading[id.index()] {
                let start = self
                    .stack
                    .iter()
                    .position(|candidate| *candidate == id)
                    .unwrap_or(0);
                let chain = self.stack[start..]
                    .iter()
                    .map(|module| self.uris[module.index()].as_str())
                    .chain(std::iter::once(uri))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return Err(PackageBuildError {
                    span: None,
                    message: format!("Circular Oche module import: {chain}"),
                });
            }
            return Ok(id);
        }

        let source = self.source(uri)?;
        let mut program = oche_frontend::parse(&source).map_err(|error| PackageBuildError {
            span: None,
            message: format!("Invalid Oche module '{uri}': {error}"),
        })?;
        let id = oche_hir::ModuleId::new(self.modules.len());
        self.ids.insert(uri.into(), id);
        self.uris.push(uri.into());
        self.modules.push(None);
        self.edges.push(Vec::new());
        self.loading.push(true);
        self.stack.push(id);

        let mut imports = Vec::new();
        for import in program.imports.clone() {
            if matches!(
                import.path.as_str(),
                "std/json" | "std/json.oche" | "std/isolate" | "std/isolate.oche"
            ) {
                continue;
            }
            let Some(imported_uri) = resolve_import_uri(uri, &import.path) else {
                return Err(PackageBuildError {
                    span: Some(import.span),
                    message: format!("Cannot resolve imported module '{}'", import.path),
                });
            };
            if import.path.ends_with(".d.oche") {
                let definition_source =
                    self.source(&imported_uri)
                        .map_err(|error| PackageBuildError {
                            span: Some(import.span),
                            message: format!(
                                "Cannot load imported definition '{}': {}",
                                import.path, error.message
                            ),
                        })?;
                let definition = oche_frontend::parse(&definition_source).map_err(|error| {
                    PackageBuildError {
                        span: Some(import.span),
                        message: format!(
                            "Imported definition '{}' is invalid: {error}",
                            import.path
                        ),
                    }
                })?;
                let Some(library) = definition.into_dart_definition(import.alias) else {
                    return Err(PackageBuildError {
                        span: Some(import.span),
                        message: format!(
                            "Imported definition '{}' must contain exactly one Dart library declaration",
                            import.path
                        ),
                    });
                };
                program.dart_libraries.push(library);
            } else {
                let module = self
                    .load(&imported_uri)
                    .map_err(|error| PackageBuildError {
                        span: Some(import.span),
                        message: format!(
                            "Cannot link imported module '{}': {}",
                            import.path, error.message
                        ),
                    })?;
                imports.push(oche_hir::ModuleImport {
                    alias: import.alias,
                    module,
                });
                self.edges[id.index()].push((module, import.span));
            }
        }

        self.stack.pop();
        self.loading[id.index()] = false;
        self.modules[id.index()] = Some(oche_hir::LinkedModule { program, imports });
        Ok(id)
    }

    fn source(&mut self, uri: &str) -> std::result::Result<String, PackageBuildError> {
        if let Some(file) = self.analysis.file_by_name(uri) {
            return Ok(self
                .analysis
                .file(file)
                .expect("indexed editor file exists")
                .text
                .clone());
        }
        let path = file_uri_path(uri).ok_or_else(|| PackageBuildError {
            span: None,
            message: format!("URI '{uri}' is not a readable file URI"),
        })?;
        let source = std::fs::read_to_string(&path).map_err(|error| PackageBuildError {
            span: None,
            message: format!("Cannot read '{}': {error}", path.display()),
        })?;
        self.analysis.set(uri, &source);
        Ok(source)
    }
}

fn analysis_span(span: oche_frontend::SourceSpan) -> oche_analysis::Span {
    oche_analysis::Span {
        start: span.start,
        end: span.end,
    }
}

fn resolve_import_uri(base: &str, relative: &str) -> Option<String> {
    if relative.contains("://") {
        return Some(relative.to_string());
    }
    let scheme_end = base.find("://").map_or(0, |index| index + 3);
    let authority_end = if scheme_end == 0 {
        0
    } else {
        base[scheme_end..]
            .find('/')
            .map_or(base.len(), |index| scheme_end + index)
    };
    let prefix = &base[..authority_end];
    let path = &base[authority_end..];
    let parent_end = path.rfind('/')?;
    let joined = format!("{}/{}", &path[..parent_end], relative);
    let absolute = joined.starts_with('/');
    let mut segments = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    Some(format!(
        "{prefix}{}{}",
        if absolute { "/" } else { "" },
        segments.join("/")
    ))
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let value = u8::from_str_radix(&encoded[index + 1..index + 3], 16).ok()?;
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn synthetic_hover(typed: &oche_hir::TypedProgram, word: &str) -> Option<Value> {
    let function = typed
        .synthetic_functions
        .iter()
        .find(|function| function.name == word)
        .or_else(|| {
            typed.synthetic_functions.iter().find(|function| {
                let member_name = match function.origin {
                    oche_hir::SyntheticOrigin::JsonEncode => {
                        Some(format!("{}_toJson", function.target))
                    }
                    oche_hir::SyntheticOrigin::JsonDecode => {
                        Some(format!("{}_fromJson", function.target))
                    }
                    _ => None,
                };
                member_name.as_deref() == Some(word)
            })
        })?;
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("arg{index}: {ty}"))
        .collect::<Vec<_>>()
        .join(", ");
    let origin = match function.origin {
        oche_hir::SyntheticOrigin::JsonEncode | oche_hir::SyntheticOrigin::JsonEncodeValue => {
            format!("`comptime jsonEncode({})`", function.target)
        }
        oche_hir::SyntheticOrigin::JsonDecode | oche_hir::SyntheticOrigin::JsonDecodeValue => {
            format!("`comptime jsonDecode({})`", function.target)
        }
        oche_hir::SyntheticOrigin::Validate => {
            format!("`comptime validate({})`", function.target)
        }
    };
    Some(json!({
        "contents": {
            "kind": "markdown",
            "value": format!(
                "```oche\nfn {}({parameters}) -> {}\n```\n\nGenerated by {origin}.",
                word, function.result
            )
        }
    }))
}

fn diagnostic_at(source: &str, span: oche_analysis::Span, message: String) -> Diagnostic {
    Diagnostic {
        range: Range::new(
            offset_to_position(source, span.start),
            offset_to_position(source, span.end),
        ),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("oche".into()),
        message,
        ..Diagnostic::default()
    }
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_text = prefix.rsplit('\n').next().unwrap_or("");
    Position::new(line as u32, line_text.encode_utf16().count() as u32)
}

fn position_to_offset(source: &str, target_line: usize, target_character: usize) -> Option<usize> {
    let mut offset = 0;
    let line = source.split_inclusive('\n').nth(target_line)?;
    for previous in source.split_inclusive('\n').take(target_line) {
        offset += previous.len();
    }
    let mut utf16 = 0;
    for (index, character) in line.char_indices() {
        if utf16 >= target_character {
            return Some(offset + index);
        }
        utf16 += character.len_utf16();
    }
    (utf16 == target_character).then_some(offset + line.len())
}

fn word_at(source: &str, offset: usize) -> Option<&str> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    let is_word = |character: char| character.is_alphanumeric() || character == '_';
    let start = source[..offset]
        .char_indices()
        .rev()
        .find(|(_, character)| !is_word(*character))
        .map_or(0, |(index, character)| index + character.len_utf8());
    let end = source[offset..]
        .char_indices()
        .find(|(_, character)| !is_word(*character))
        .map_or(source.len(), |(index, _)| offset + index);
    (start < end).then_some(&source[start..end])
}

fn associated_synthetic_name(source: &str, offset: usize, method: &str) -> Option<String> {
    let word = word_at(source, offset)?;
    let word_start = word.as_ptr() as usize - source.as_ptr() as usize;
    let dot = word_start.checked_sub(1)?;
    (source.as_bytes().get(dot) == Some(&b'.')).then_some(())?;
    let owner_offset = dot.checked_sub(1)?;
    let owner = word_at(source, owner_offset)?;
    owner
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
        .then(|| format!("{owner}_{method}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: Value) -> Vec<u8> {
        let body = serde_json::to_vec(&value).unwrap();
        format!("Content-Length: {}\r\n\r\n", body.len())
            .bytes()
            .chain(body)
            .collect()
    }

    fn output_messages(mut output: &[u8]) -> Vec<Value> {
        let mut messages = Vec::new();
        while !output.is_empty() {
            let header_end = output
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .unwrap();
            let header = std::str::from_utf8(&output[..header_end]).unwrap();
            let length = header
                .strip_prefix("Content-Length: ")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let body_start = header_end + 4;
            let body_end = body_start + length;
            messages.push(serde_json::from_slice(&output[body_start..body_end]).unwrap());
            output = &output[body_end..];
        }
        messages
    }

    #[test]
    fn initializes_and_publishes_parse_diagnostics() {
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///bad.oche","text":"fn bad() -> Int { missing }","version":1}}
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("textDocumentSync"));
        assert!(text.contains("publishDiagnostics"));
        assert!(text.contains("Unknown variable"));
    }

    #[test]
    fn publishes_multiple_recovered_parser_diagnostics() {
        let source = "fn first( -> Int { 1 }\nfn second( -> Int { 2 }";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///recovery.oche","text":source,"version":1}}
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert_eq!(text.matches("Expected identifier").count(), 2);
    }

    #[test]
    fn publishes_comptime_diagnostics_and_serves_hover() {
        let source = "struct User { age: Int }\ncomptime validate(User, age: non_empty)";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///schema.oche","text":source,"version":1}}
        })));
        input.extend(frame(json!({
            "jsonrpc":"2.0","id":2,"method":"textDocument/hover",
            "params":{"textDocument":{"uri":"file:///schema.oche"},"position":{"line":0,"character":20}}
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("cannot be applied"));
        assert!(text.contains("Signed 64-bit integer"));
    }

    #[test]
    fn publishes_hir_type_diagnostics_and_hovers_comptime_symbols() {
        let bad = "fn wrong() -> Int { \"text\" }";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///bad.oche","text":bad,"version":1}}
        })));
        let generated = "struct User { name: String }\ncomptime jsonDecode(User)\nfn decode(source: String) -> Result<User, String> { User.fromJson(source) }";
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///generated.oche","text":generated,"version":1}}
        })));
        input.extend(frame(json!({
            "jsonrpc":"2.0","id":2,"method":"textDocument/hover",
            "params":{"textDocument":{"uri":"file:///generated.oche"},"position":{"line":2,"character":60}}
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("expected Int, received String"));
        assert!(text.contains("\"start\":{\"character\":20,\"line\":0}"));
        assert!(text.contains("\"end\":{\"character\":26,\"line\":0}"));
        assert!(text.contains("fn User_fromJson(arg0: String) -> Result<User, String>"));
        assert!(text.contains("Generated by `comptime jsonDecode(User)`"));
    }

    #[test]
    fn links_open_dart_definitions_and_reanalyzes_dependents() {
        let definition = "dart \"package:tools/tools.dart\" { fn greet(name: String) -> String }";
        let main = "import \"tools.d.oche\" as tools\nfn greet() -> String { tools.greet() }";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///workspace/tools.d.oche","text":definition,"version":1}}
        })));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///workspace/main.oche","text":main,"version":1}}
        })));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didChange",
            "params":{
                "textDocument":{"uri":"file:///workspace/tools.d.oche","version":2},
                "contentChanges":[{"text":"dart \"package:tools/tools.dart\" { fn greet() -> String }"}]
            }
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let main_diagnostics = output_messages(&output)
            .into_iter()
            .filter(|message| {
                message.pointer("/method").and_then(Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                    && message.pointer("/params/uri").and_then(Value::as_str)
                        == Some("file:///workspace/main.oche")
            })
            .collect::<Vec<_>>();
        assert_eq!(main_diagnostics.len(), 2);
        assert!(
            main_diagnostics[0]
                .to_string()
                .contains("expects 1 argument(s), received 0")
        );
        assert_eq!(
            main_diagnostics[1]
                .pointer("/params/diagnostics")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn links_transitive_open_oche_modules_and_reanalyzes_dependents() {
        let math = "pub fn double(value: Int) -> Int { value + value }";
        let service = "import \"math.oche\" as math\npub fn answer() -> Int { math.double() }";
        let main = "import \"service.oche\" as service\nfn answer() -> Int { service.answer() }";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        for (uri, text) in [
            ("file:///workspace/math.oche", math),
            ("file:///workspace/service.oche", service),
            ("file:///workspace/main.oche", main),
        ] {
            input.extend(frame(json!({
                "jsonrpc":"2.0","method":"textDocument/didOpen",
                "params":{"textDocument":{"uri":uri,"text":text,"version":1}}
            })));
        }
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didChange",
            "params":{
                "textDocument":{"uri":"file:///workspace/math.oche","version":2},
                "contentChanges":[{"text":"pub fn double() -> Int { 42 }"}]
            }
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));

        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let messages = output_messages(&output);
        let service_diagnostics = messages
            .iter()
            .filter(|message| {
                message.pointer("/method").and_then(Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                    && message.pointer("/params/uri").and_then(Value::as_str)
                        == Some("file:///workspace/service.oche")
            })
            .collect::<Vec<_>>();
        assert!(service_diagnostics.iter().any(|message| {
            message
                .to_string()
                .contains("math.double expects 1 argument(s), received 0")
        }));
        assert_eq!(
            service_diagnostics
                .last()
                .unwrap()
                .pointer("/params/diagnostics")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            0
        );
        let main_diagnostics = messages
            .iter()
            .filter(|message| {
                message.pointer("/method").and_then(Value::as_str)
                    == Some("textDocument/publishDiagnostics")
                    && message.pointer("/params/uri").and_then(Value::as_str)
                        == Some("file:///workspace/main.oche")
            })
            .collect::<Vec<_>>();
        assert!(
            main_diagnostics
                .iter()
                .any(|message| message.to_string().contains("has a type error"))
        );
        assert_eq!(
            main_diagnostics
                .last()
                .unwrap()
                .pointer("/params/diagnostics")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn resolves_relative_import_uris_lexically() {
        assert_eq!(
            resolve_import_uri("file:///workspace/src/main.oche", "../tools/tools.d.oche"),
            Some("file:///workspace/tools/tools.d.oche".into())
        );
        assert_eq!(
            file_uri_path("file:///workspace/my%20tools.d.oche"),
            Some(PathBuf::from("/workspace/my tools.d.oche"))
        );
    }

    #[test]
    fn advertises_and_serves_document_formatting() {
        let source = "fn add(a:Int,b:Int)->Int{a+b}";
        let mut input = frame(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}));
        input.extend(frame(json!({
            "jsonrpc":"2.0","method":"textDocument/didOpen",
            "params":{"textDocument":{"uri":"file:///format.oche","text":source,"version":1}}
        })));
        input.extend(frame(json!({
            "jsonrpc":"2.0","id":2,"method":"textDocument/formatting",
            "params":{"textDocument":{"uri":"file:///format.oche"},"options":{"tabSize":2,"insertSpaces":true}}
        })));
        input.extend(frame(
            json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}),
        ));
        input.extend(frame(
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ));
        let mut output = Vec::new();
        run(input.as_slice(), &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("documentFormattingProvider"));
        assert!(text.contains("fn add(a: Int, b: Int) -> Int { a + b }"));
    }
}
