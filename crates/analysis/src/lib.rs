//! Shared source database and immutable compiler analysis snapshots.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Revision(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn at(source: &str, offset: usize) -> Self {
        let start = offset.min(source.len());
        let end = source
            .get(start..)
            .and_then(|tail| tail.chars().next())
            .map_or(start, |character| start + character.len_utf8());
        Self { start, end }
    }

    fn from_source(source: &str, span: oche_frontend::SourceSpan) -> Self {
        let start = span.start.min(source.len());
        let end = span.end.clamp(start, source.len());
        if start == end {
            Self::at(source, start)
        } else {
            Self { start, end }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Syntax,
    Type,
    Comptime,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub file: FileId,
    pub span: Span,
    pub stage: Stage,
    pub cause: Cause,
}

#[derive(Debug)]
pub enum Cause {
    Frontend(oche_frontend::Error),
    Type(oche_hir::Error),
    Comptime(oche_comptime::Error),
}

impl std::fmt::Display for Cause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frontend(error) => error.fmt(formatter),
            Self::Type(error) => error.fmt(formatter),
            Self::Comptime(error) => error.fmt(formatter),
        }
    }
}

#[derive(Debug)]
pub struct ParsedSnapshot {
    pub file: FileId,
    pub revision: Revision,
    pub program: Option<oche_frontend::Program>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct AnalysisSnapshot {
    pub file: FileId,
    pub revision: Revision,
    pub program: Option<oche_frontend::Program>,
    pub typed: Option<oche_hir::TypedProgram>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct SourceFile {
    pub id: FileId,
    pub name: String,
    pub revision: Revision,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Database {
    sources: Vec<SourceFile>,
    files_by_name: HashMap<String, FileId>,
    next_revision: u64,
}

impl Database {
    pub fn set(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        let name = name.into();
        let text = text.into();
        self.next_revision += 1;
        let revision = Revision(self.next_revision);
        if let Some(id) = self.files_by_name.get(&name).copied() {
            let source = &mut self.sources[id.0 as usize];
            source.text = text;
            source.revision = revision;
            return id;
        }
        let id = FileId(self.sources.len() as u32);
        self.sources.push(SourceFile {
            id,
            name: name.clone(),
            revision,
            text,
        });
        self.files_by_name.insert(name, id);
        id
    }

    pub fn remove(&mut self, name: &str) -> Option<FileId> {
        self.files_by_name.remove(name)
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.sources.get(id.0 as usize)
    }

    pub fn file_by_name(&self, name: &str) -> Option<FileId> {
        self.files_by_name.get(name).copied()
    }

    pub fn parse(&self, file: FileId) -> Option<ParsedSnapshot> {
        let source = self.file(file)?;
        let output = oche_frontend::parse_recovering(&source.text);
        let diagnostics = output
            .diagnostics
            .into_iter()
            .map(|error| Diagnostic {
                file,
                span: Span::at(&source.text, error.offset()),
                stage: Stage::Syntax,
                cause: Cause::Frontend(error),
            })
            .collect();
        Some(ParsedSnapshot {
            file,
            revision: source.revision,
            program: output.program,
            diagnostics,
        })
    }

    pub fn analyze(&self, file: FileId) -> Option<AnalysisSnapshot> {
        let parsed = self.parse(file)?;
        if !parsed.diagnostics.is_empty() {
            return Some(AnalysisSnapshot {
                file,
                revision: parsed.revision,
                program: parsed.program,
                typed: None,
                diagnostics: parsed.diagnostics,
            });
        }
        self.analyze_program(
            file,
            parsed.program.expect("valid syntax produces a program"),
        )
    }

    pub fn analyze_program(
        &self,
        file: FileId,
        program: oche_frontend::Program,
    ) -> Option<AnalysisSnapshot> {
        let source = self.file(file)?;
        let mut diagnostics = Vec::new();
        let imports_linked = program.imports.iter().all(|import| {
            matches!(
                import.path.as_str(),
                "std/json" | "std/json.oche" | "std/isolate" | "std/isolate.oche"
            ) || program
                .dart_libraries
                .iter()
                .any(|library| library.namespace == import.alias)
        });
        let typed = if imports_linked {
            if let Err(error) = oche_frontend::validate_linked(&program) {
                diagnostics.push(Diagnostic {
                    file,
                    span: Span::at(&source.text, error.offset()),
                    stage: Stage::Type,
                    cause: Cause::Frontend(error),
                });
                None
            } else {
                match oche_hir::lower_located(&program) {
                    Ok(typed) => Some(typed),
                    Err(error) => {
                        let span = error.span.map_or_else(
                            || {
                                Span::at(
                                    &source.text,
                                    program
                                        .functions
                                        .get(error.function.index)
                                        .map_or(0, |function| function.span.start),
                                )
                            },
                            |span| Span::from_source(&source.text, span),
                        );
                        diagnostics.push(Diagnostic {
                            file,
                            span,
                            stage: Stage::Type,
                            cause: Cause::Type(*error.source),
                        });
                        None
                    }
                }
            }
        } else {
            None
        };
        if let Err(error) = oche_comptime::validate(&program) {
            diagnostics.push(Diagnostic {
                file,
                span: Span::at(&source.text, error.offset()),
                stage: Stage::Comptime,
                cause: Cause::Comptime(error),
            });
        }
        Some(AnalysisSnapshot {
            file,
            revision: source.revision,
            program: Some(program),
            typed,
            diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_file_ids_stable_and_revisions_monotonic() {
        let mut database = Database::default();
        let first = database.set("main.oche", "fn value() -> Int { 1 }");
        let initial_revision = database.file(first).unwrap().revision;
        let second = database.set("main.oche", "fn value() -> Int { 2 }");
        assert_eq!(first, second);
        assert_ne!(database.file(second).unwrap().revision, initial_revision);
    }

    #[test]
    fn snapshots_accumulate_syntax_diagnostics_and_share_semantics() {
        let mut database = Database::default();
        let broken = database.set(
            "broken.oche",
            "fn first( -> Int { 1 }\nfn second( -> Int { 2 }",
        );
        let parsed = database.parse(broken).unwrap();
        assert_eq!(parsed.diagnostics.len(), 2);

        let typed = database.set("typed.oche", "fn wrong() -> Int { \"text\" }");
        let snapshot = database.analyze(typed).unwrap();
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(snapshot.diagnostics[0].stage, Stage::Type);
    }

    #[test]
    fn reports_value_comptime_boundary_errors_without_executing_the_vm() {
        let mut database = Database::default();
        let source = "fn runtime() -> Int { 1 } fn value() -> Int { comptime runtime() }";
        let file = database.set("invalid-comptime.oche", source);
        let snapshot = database.analyze(file).unwrap();
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(snapshot.diagnostics[0].stage, Stage::Comptime);
        assert!(
            snapshot.diagnostics[0]
                .cause
                .to_string()
                .contains("must be a synchronous const fn")
        );
    }

    #[test]
    fn locates_typed_match_diagnostics_at_the_relevant_pattern() {
        let mut database = Database::default();
        let source = "fn choose(value: Bool) -> Int {
  match value {
    true => 1,
    true => 2,
    false => 0,
  }
}";
        let file = database.set("redundant.oche", source);
        let snapshot = database.analyze(file).unwrap();
        assert_eq!(snapshot.diagnostics.len(), 1);
        let expected = source.match_indices("true").nth(1).unwrap().0;
        assert_eq!(
            snapshot.diagnostics[0].span,
            Span {
                start: expected,
                end: expected + "true".len(),
            }
        );
        assert!(matches!(
            snapshot.diagnostics[0].cause,
            Cause::Type(oche_hir::Error::RedundantMatchArm)
        ));

        let source = "fn choose(value: Bool) -> Int { match value { true => 1 } }";
        let file = database.set("incomplete.oche", source);
        let snapshot = database.analyze(file).unwrap();
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert_eq!(
            snapshot.diagnostics[0].span,
            Span::at(source, source.find("match").unwrap())
        );
        assert!(matches!(
            snapshot.diagnostics[0].cause,
            Cause::Type(oche_hir::Error::NonExhaustiveMatch { .. })
        ));

        let source = "fn choose(value: Option<Bool>) -> Int {
  match value { Some(1) => 1, None => 0 }
}";
        let file = database.set("nested.oche", source);
        let snapshot = database.analyze(file).unwrap();
        let expected = source.find('1').unwrap();
        assert_eq!(
            snapshot.diagnostics[0].span,
            Span {
                start: expected,
                end: expected + 1,
            }
        );
        assert!(matches!(
            snapshot.diagnostics[0].cause,
            Cause::Type(oche_hir::Error::TypeMismatch { .. })
        ));
    }

    #[test]
    fn locates_general_typed_expression_diagnostics() {
        let mut database = Database::default();
        for (name, source, needle) in [
            (
                "operand.oche",
                "fn bad(value: Int) -> Int { value + true }",
                "true",
            ),
            (
                "method.oche",
                "fn bad(value: Int) -> Int { value.len() }",
                "value.len()",
            ),
            ("await.oche", "async fn bad() -> Int { 1.await }", "1.await"),
        ] {
            let file = database.set(name, source);
            let snapshot = database.analyze(file).unwrap();
            assert_eq!(snapshot.diagnostics.len(), 1, "{source}");
            assert_eq!(
                snapshot.diagnostics[0].span,
                Span {
                    start: source.find(needle).unwrap(),
                    end: source.find(needle).unwrap() + needle.len(),
                },
                "{source}"
            );
            assert_eq!(snapshot.diagnostics[0].stage, Stage::Type);
        }
    }

    #[test]
    fn validates_calls_after_definition_linking() {
        let source = "import \"tools.d.oche\" as tools\nfn greet() -> String { tools.greet() }";
        let definition = oche_frontend::parse(
            "dart \"package:tools/tools.dart\" { fn greet(name: String) -> String }",
        )
        .unwrap();
        let mut program = oche_frontend::parse(source).unwrap();
        program
            .dart_libraries
            .push(definition.into_dart_definition("tools".into()).unwrap());
        let mut database = Database::default();
        let file = database.set("main.oche", source);
        let snapshot = database.analyze_program(file, program).unwrap();
        assert!(snapshot.typed.is_none());
        assert_eq!(snapshot.diagnostics.len(), 1);
        assert!(matches!(
            snapshot.diagnostics[0].cause,
            Cause::Frontend(oche_frontend::Error::WrongArgumentCount {
                expected: 1,
                actual: 0,
                ..
            })
        ));
    }
}
