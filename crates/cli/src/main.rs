use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use ariadne::{Color, Label, Report, ReportKind, Source};
use snafu::{ResultExt, Snafu};

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("Could not {action} at {}: {source}", path.display()))]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("Could not run {}: {source}", program.display()))]
    Command {
        program: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("{action} returned invalid UTF-8: {source}"))]
    Utf8 {
        action: &'static str,
        source: std::string::FromUtf8Error,
    },
    #[snafu(display("Cannot discover Dart; set OCHE_DART_SDK or install Dart with mise"))]
    DartNotFound,
    #[snafu(display("Dart executable has no SDK parent directory: {}", path.display()))]
    MissingSdkParent { path: PathBuf },
    #[snafu(display("Not a Dart SDK with embedding headers: {}", path.display()))]
    InvalidSdk { path: PathBuf },
    #[snafu(display("Missing Dart embedding header: {}", path.display()))]
    MissingHeader { path: PathBuf },
    #[snafu(display("{path}:{line}:{column}: {source}", path = path.display()))]
    Frontend {
        path: PathBuf,
        line: usize,
        column: usize,
        source: oche_frontend::Error,
    },
    #[snafu(display("{path}:{line}:{column}: {source}", path = path.display()))]
    Typecheck {
        path: PathBuf,
        line: usize,
        column: usize,
        source: Box<oche_hir::Error>,
    },
    #[snafu(display("{}: code generation failed: {source}", path.display()))]
    Codegen {
        path: PathBuf,
        source: Box<oche_codegen::Error>,
    },
    #[snafu(display("{path}:{line}:{column}: {source}", path = path.display()))]
    Comptime {
        path: PathBuf,
        line: usize,
        column: usize,
        source: Box<oche_comptime::Error>,
    },
    #[snafu(display("{path}:{line}:{column}: Dart VM comptime evaluation failed:\n{details}", path = path.display()))]
    ComptimeVm {
        path: PathBuf,
        line: usize,
        column: usize,
        details: String,
    },
    #[snafu(display("System clock is before the Unix epoch: {source}"))]
    Clock { source: std::time::SystemTimeError },
    #[snafu(display("Could not allocate a unique temporary directory"))]
    TempDirectoryExhausted,
    #[snafu(display("Dart {kind} compilation failed:\n{details}"))]
    Compilation { kind: &'static str, details: String },
    #[snafu(display("dart --version exited unsuccessfully"))]
    DartVersionFailed,
    #[snafu(display("Language server failed: {source}"))]
    Lsp { source: oche_lsp::Error },
    #[snafu(display("Could not generate Oche definitions: {source}"))]
    Interop { source: oche_interop::Error },
    #[snafu(display("The following files require formatting:\n{files}"))]
    FormattingRequired { files: String },
    #[snafu(display("Imported definition file {} must contain exactly one Dart library declaration and no executable Oche declarations", path.display()))]
    InvalidDefinition { path: PathBuf },
    #[snafu(display("Circular Oche module import: {chain}"))]
    CircularImport { chain: String },
}

/// Oche is a Dart-family language for agent runtimes.
#[derive(Debug, usage_rs::Cli)]
#[usage(
    bin = "oche",
    version = env!("CARGO_PKG_VERSION"),
    arg_required_else_help,
    completion
)]
struct Cli {
    #[usage(subcommand)]
    command: OcheCommand,
}

#[derive(Debug, usage_rs::Subcommands)]
enum OcheCommand {
    /// Run the Oche language server over standard input and output.
    Lsp,
    /// Print Dart SDK and embedding-header information.
    #[usage(name = "sdk-info")]
    SdkInfo(SdkInfoArgs),
    /// Generate an Oche definition file from a Dart library.
    #[usage(name = "bindgen-dart")]
    BindgenDart(BindgenDartArgs),
    /// Format Oche source files.
    Fmt(FormatArgs),
    /// Type-check an Oche source file.
    Check(SourceArgs),
    /// Print the parsed Oche syntax tree.
    Ast(SourceArgs),
    /// Emit generated Dart source.
    #[usage(name = "emit-dart")]
    EmitDart(SourceArgs),
    /// Compile an Oche program to a Dart Kernel component.
    Build(BuildArgs),
    /// Compile an Oche library to an unlinked Dart Kernel component.
    #[usage(name = "build-module")]
    BuildModule(RequiredOutputArgs),
    /// Run public Oche functions through a persistent JIT RPC process.
    Serve(SourceArgs),
    /// Compile an Oche program to a native executable.
    #[usage(name = "build-exe")]
    BuildExe(BuildArgs),
    /// Compile an Oche library to a JavaScript module.
    #[usage(name = "build-js")]
    BuildJs(BuildJsArgs),
    /// Compile and invoke an Oche function on the Dart VM.
    Run(RunArgs),
    /// Run Oche test functions on the Dart VM.
    Test(TestArgs),
}

#[derive(Debug, usage_rs::Args)]
struct SdkInfoArgs {
    /// Dart SDK directory; otherwise use OCHE_DART_SDK or mise.
    #[usage(value_name = "SDK_DIRECTORY", value_hint = usage_rs::ValueHint::DirPath)]
    sdk_directory: Option<PathBuf>,
}

#[derive(Debug, usage_rs::Args)]
struct BindgenDartArgs {
    /// Dart source library to inspect.
    #[usage(value_name = "INPUT", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
    /// Generated Oche definition file.
    #[usage(short = 'o', long, value_name = "OUTPUT", value_hint = usage_rs::ValueHint::FilePath)]
    output: PathBuf,
}

#[derive(Debug, usage_rs::Args)]
struct FormatArgs {
    /// Report files requiring formatting without changing them.
    #[usage(long)]
    check: bool,
    /// Oche source files to format.
    #[usage(arg, required, value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    files: Vec<PathBuf>,
}

#[derive(Debug, usage_rs::Args)]
struct SourceArgs {
    /// Oche source file.
    #[usage(value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
}

#[derive(Debug, usage_rs::Args)]
struct BuildArgs {
    /// Oche source file.
    #[usage(value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
    /// Output path; defaults to the source path with the target extension.
    #[usage(short = 'o', long, value_name = "OUTPUT", value_hint = usage_rs::ValueHint::FilePath)]
    output: Option<PathBuf>,
}

#[derive(Debug, usage_rs::Args)]
struct RequiredOutputArgs {
    /// Oche source file.
    #[usage(value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
    /// Output Dart Kernel component.
    #[usage(short = 'o', long, value_name = "OUTPUT", value_hint = usage_rs::ValueHint::FilePath)]
    output: PathBuf,
}

#[derive(Debug, usage_rs::Args)]
struct BuildJsArgs {
    /// Oche source file.
    #[usage(value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
    /// Embed compiled Dart JavaScript in the generated ES module.
    #[usage(long)]
    single: bool,
    /// Output ES module; defaults to the source path with an .mjs extension.
    #[usage(short = 'o', long, value_name = "OUTPUT", value_hint = usage_rs::ValueHint::FilePath)]
    output: Option<PathBuf>,
}

#[derive(Debug, usage_rs::Args)]
struct RunArgs {
    /// Oche source file.
    #[usage(value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    input: PathBuf,
    /// Function to invoke.
    #[usage(value_name = "FUNCTION")]
    function: String,
    /// Arguments passed to the Oche function.
    #[usage(trailing_var_arg, allow_hyphen_values, value_name = "ARG")]
    arguments: Vec<String>,
}

#[derive(Debug, usage_rs::Args)]
struct TestArgs {
    /// Oche source files containing tests.
    #[usage(arg, required, value_name = "FILE", value_hint = usage_rs::ValueHint::FilePath)]
    files: Vec<PathBuf>,
}

fn discover_sdk(explicit: Option<&Path>) -> Result<PathBuf> {
    let sdk = if let Some(path) = explicit {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("OCHE_DART_SDK") {
        PathBuf::from(path)
    } else {
        let program = PathBuf::from("mise");
        let output = Command::new(&program)
            .args(["which", "dart"])
            .output()
            .context(CommandSnafu { program })?;
        if !output.status.success() {
            return DartNotFoundSnafu.fail();
        }
        let executable = PathBuf::from(
            String::from_utf8(output.stdout)
                .context(Utf8Snafu {
                    action: "mise which dart",
                })?
                .trim(),
        );
        let executable = executable.canonicalize().context(IoSnafu {
            action: "canonicalize Dart executable",
            path: executable,
        })?;
        executable
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| Error::MissingSdkParent {
                path: executable.clone(),
            })?
            .to_path_buf()
    };
    let unresolved_sdk = sdk.clone();
    let sdk = sdk.canonicalize().context(IoSnafu {
        action: "canonicalize Dart SDK",
        path: unresolved_sdk,
    })?;
    if !sdk.join("include/dart_api.h").is_file() {
        return InvalidSdkSnafu { path: sdk }.fail();
    }
    Ok(sdk)
}

fn dart_executable(sdk: &Path) -> PathBuf {
    sdk.join("bin")
        .join(if cfg!(windows) { "dart.exe" } else { "dart" })
}

fn parse_file(path: &Path) -> Result<(String, oche_hir::LinkedPackage)> {
    let mut loader = PackageLoader::default();
    let root = loader.load(path)?;
    let modules = loader
        .modules
        .into_iter()
        .map(|module| module.expect("successful package loading fills every module slot"))
        .collect::<Vec<_>>();
    let package = oche_hir::LinkedPackage { root, modules };

    if let Err(error) = oche_hir::lower_package_located(&package) {
        let module = error.function.module.index();
        let source = &loader.sources[module];
        let span = error.span.map_or_else(
            || {
                package.modules[module]
                    .program
                    .functions
                    .get(error.function.index)
                    .map_or(0, |function| function.span.start)
            },
            |span| span.start,
        );
        let (line, column) = line_column(source, span);
        return Err(Error::Typecheck {
            path: loader.paths[module].clone(),
            line,
            column,
            source: error.source,
        });
    }
    for (index, module) in package.modules.iter().enumerate() {
        if let Err(error) = oche_comptime::evaluate(&module.program) {
            return Err(comptime_error(
                &loader.paths[index],
                &loader.sources[index],
                error,
            ));
        }
    }
    Ok((loader.sources[root.index()].clone(), package))
}

fn evaluate_value_comptime(
    path: &Path,
    source: &str,
    program: &mut oche_frontend::Program,
) -> Result<()> {
    let requests =
        oche_comptime::vm_requests(program).map_err(|error| comptime_error(path, source, error))?;
    if requests.is_empty() {
        return Ok(());
    }

    let generated = oche_codegen::emit_dart_comptime(program).map_err(|source| Error::Codegen {
        path: path.to_path_buf(),
        source: Box::new(source),
    })?;
    let temporary = TemporaryDirectory::create()?;
    let dart_source = temporary.0.join("comptime.dart");
    let kernel = temporary.0.join("comptime.dill");
    write_generated(&dart_source, generated)?;
    let sdk = discover_sdk(None)?;
    let dart = dart_executable(&sdk);
    compile_kernel(&dart, &dart_source, &kernel, true)?;

    let mut values = HashMap::new();
    for request in requests {
        let output = Command::new(&dart)
            .arg(&kernel)
            .arg(request.id.to_string())
            .output()
            .context(CommandSnafu {
                program: dart.clone(),
            })?;
        let (line, column) = line_column(source, request.offset);
        if !output.status.success() {
            return Err(Error::ComptimeVm {
                path: path.to_path_buf(),
                line,
                column,
                details: format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }
        let decoded: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|error| Error::ComptimeVm {
                path: path.to_path_buf(),
                line,
                column,
                details: format!("invalid evaluator protocol response: {error}"),
            })?;
        let value = decode_comptime_value(&decoded, &request.result, program).ok_or_else(|| {
            Error::ComptimeVm {
                path: path.to_path_buf(),
                line,
                column,
                details: format!(
                    "evaluator returned {decoded} for declared type {}",
                    request.result
                ),
            }
        })?;
        values.insert(request.id, value);
    }
    oche_comptime::apply_vm_values(program, &values)
        .map_err(|error| comptime_error(path, source, error))
}

fn decode_comptime_value(
    decoded: &serde_json::Value,
    ty: &oche_frontend::Type,
    program: &oche_frontend::Program,
) -> Option<oche_comptime::Value> {
    let oche_frontend::Type::Named { name, arguments } = ty;
    match (name.as_str(), arguments.as_slice()) {
        ("Int", []) => decoded.as_i64().map(oche_comptime::Value::Int),
        ("Float", []) => decoded.as_f64().map(oche_comptime::Value::Float),
        ("Bool", []) => decoded.as_bool().map(oche_comptime::Value::Bool),
        ("String", []) => decoded
            .as_str()
            .map(|value| oche_comptime::Value::String(value.into())),
        ("Type", []) => decode_comptime_type(decoded).map(oche_comptime::Value::Type),
        ("List", [element]) => decoded
            .as_array()?
            .iter()
            .map(|value| decode_comptime_value(value, element, program))
            .collect::<Option<Vec<_>>>()
            .map(oche_comptime::Value::List),
        (_, []) => {
            let structure = program
                .structs
                .iter()
                .find(|structure| structure.name == *name)?;
            let object = decoded.as_object()?;
            let fields = structure
                .fields
                .iter()
                .map(|field| {
                    Some((
                        field.name.clone(),
                        decode_comptime_value(object.get(&field.name)?, &field.ty, program)?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(oche_comptime::Value::Struct {
                name: name.clone(),
                fields,
            })
        }
        _ => None,
    }
}

fn decode_comptime_type(decoded: &serde_json::Value) -> Option<oche_frontend::Type> {
    let object = decoded.as_object()?;
    let name = object.get("name")?.as_str()?.to_owned();
    let arguments = object
        .get("arguments")?
        .as_array()?
        .iter()
        .map(decode_comptime_type)
        .collect::<Option<Vec<_>>>()?;
    Some(oche_frontend::Type::Named { name, arguments })
}

#[derive(Default)]
struct PackageLoader {
    ids: HashMap<PathBuf, oche_hir::ModuleId>,
    paths: Vec<PathBuf>,
    sources: Vec<String>,
    modules: Vec<Option<oche_hir::LinkedModule>>,
    loading: Vec<bool>,
    stack: Vec<oche_hir::ModuleId>,
}

impl PackageLoader {
    fn load(&mut self, unresolved: &Path) -> Result<oche_hir::ModuleId> {
        let path = unresolved.canonicalize().context(IoSnafu {
            action: "resolve Oche source",
            path: unresolved.to_path_buf(),
        })?;
        if let Some(id) = self.ids.get(&path).copied() {
            if self.loading[id.index()] {
                let start = self
                    .stack
                    .iter()
                    .position(|candidate| *candidate == id)
                    .unwrap_or(0);
                let chain = self.stack[start..]
                    .iter()
                    .map(|module| self.paths[module.index()].display().to_string())
                    .chain(std::iter::once(path.display().to_string()))
                    .collect::<Vec<_>>()
                    .join(" -> ");
                return CircularImportSnafu { chain }.fail();
            }
            return Ok(id);
        }

        let source = std::fs::read_to_string(&path).context(IoSnafu {
            action: "read source",
            path: path.clone(),
        })?;
        let mut output = oche_frontend::parse_recovering(&source);
        if !output.diagnostics.is_empty() {
            return Err(frontend_error(&path, &source, output.diagnostics.remove(0)));
        }
        let mut program = output
            .program
            .expect("valid syntax always produces a program");
        evaluate_value_comptime(&path, &source, &mut program)?;
        let id = oche_hir::ModuleId::new(self.modules.len());
        self.ids.insert(path.clone(), id);
        self.paths.push(path.clone());
        self.sources.push(source);
        self.modules.push(None);
        self.loading.push(true);
        self.stack.push(id);

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut imports = Vec::new();
        for import in program.imports.clone() {
            if matches!(
                import.path.as_str(),
                "std/json" | "std/json.oche" | "std/isolate" | "std/isolate.oche"
            ) {
                continue;
            }
            let imported_path = parent.join(&import.path);
            if import.path.ends_with(".d.oche") {
                let definition_source =
                    std::fs::read_to_string(&imported_path).context(IoSnafu {
                        action: "read imported definition",
                        path: imported_path.clone(),
                    })?;
                let definition = oche_frontend::parse(&definition_source)
                    .map_err(|error| frontend_error(&imported_path, &definition_source, error))?;
                let Some(library) = definition.into_dart_definition(import.alias) else {
                    return InvalidDefinitionSnafu {
                        path: imported_path,
                    }
                    .fail();
                };
                program.dart_libraries.push(library);
            } else {
                let module = self.load(&imported_path)?;
                imports.push(oche_hir::ModuleImport {
                    alias: import.alias,
                    module,
                });
            }
        }

        self.stack.pop();
        self.loading[id.index()] = false;
        self.modules[id.index()] = Some(oche_hir::LinkedModule { program, imports });
        Ok(id)
    }
}

fn frontend_error(path: &Path, source: &str, error: oche_frontend::Error) -> Error {
    let (line, column) = line_column(source, error.offset());
    Error::Frontend {
        path: path.to_path_buf(),
        line,
        column,
        source: error,
    }
}

fn file_uri(path: &Path) -> Result<String> {
    let canonical = path.canonicalize().context(IoSnafu {
        action: "canonicalize Dart source",
        path: path.to_path_buf(),
    })?;
    let mut uri = String::from("file://");
    for byte in canonical.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(uri, "%{byte:02X}").unwrap();
        }
    }
    Ok(uri)
}

fn comptime_error(path: &Path, source: &str, error: oche_comptime::Error) -> Error {
    let (line, column) = line_column(source, error.offset());
    Error::Comptime {
        path: path.to_path_buf(),
        line,
        column,
        source: Box::new(error),
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, column)
}

fn compile_kernel(dart: &Path, source: &Path, output: &Path, link_platform: bool) -> Result<()> {
    let mut command = Command::new(dart);
    command
        .args(["compile", "kernel"])
        .arg(source)
        .arg("-o")
        .arg(output)
        .args(["--no-embed-sources", "--verbosity=error"]);
    if !link_platform {
        command.arg("--no-link-platform");
    }
    let result = command.output().context(CommandSnafu {
        program: dart.to_path_buf(),
    })?;
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        return CompilationSnafu {
            kind: "Kernel",
            details: format!("{stdout}{stderr}"),
        }
        .fail();
    }
    Ok(())
}

fn compile_executable(dart: &Path, source: &Path, output: &Path) -> Result<()> {
    let result = Command::new(dart)
        .args(["compile", "exe"])
        .arg(source)
        .arg("-o")
        .arg(output)
        .arg("--verbosity=error")
        .output()
        .context(CommandSnafu {
            program: dart.to_path_buf(),
        })?;
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        return CompilationSnafu {
            kind: "AOT",
            details: format!("{stdout}{stderr}"),
        }
        .fail();
    }
    Ok(())
}

fn compile_javascript(dart: &Path, source: &Path, output: &Path) -> Result<()> {
    let result = Command::new(dart)
        .args(["compile", "js", "-O2"])
        .arg(source)
        .arg("-o")
        .arg(output)
        .output()
        .context(CommandSnafu {
            program: dart.to_path_buf(),
        })?;
    if !result.status.success() {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        return CompilationSnafu {
            kind: "JavaScript",
            details: format!("{stdout}{stderr}"),
        }
        .fail();
    }
    Ok(())
}

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn create() -> Result<Self> {
        let base = env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context(ClockSnafu)?
            .as_nanos();
        for attempt in 0..100 {
            let path = base.join(format!("oche-{}-{nonce}-{attempt}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(Error::Io {
                        action: "create temporary directory",
                        path,
                        source,
                    });
                }
            }
        }
        TempDirectoryExhaustedSnafu.fail()
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn generated_source(path: &Path) -> Result<String> {
    let (_, package) = parse_file(path)?;
    oche_codegen::emit_dart_package(&package).map_err(|source| Error::Codegen {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn write_generated(path: &Path, contents: String) -> Result<()> {
    std::fs::write(path, contents).context(IoSnafu {
        action: "write generated Dart source",
        path: path.to_path_buf(),
    })
}

fn inspect_source(input: &Path, print_ast: bool) -> Result<()> {
    let (_, package) = parse_file(input)?;
    let program = &package.module(package.root).program;
    if print_ast {
        println!("{program:#?}");
    } else {
        println!("Checked {} function(s)", program.functions.len());
        for function in &program.functions {
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.ty))
                .collect::<Vec<_>>()
                .join(", ");
            println!("fn {}({parameters}) -> {}", function.name, function.result);
        }
    }
    Ok(())
}

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        OcheCommand::Lsp => {
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            oche_lsp::run(stdin.lock(), stdout.lock()).context(LspSnafu)?;
        }
        OcheCommand::SdkInfo(arguments) => {
            let sdk = discover_sdk(arguments.sdk_directory.as_deref())?;
            let dart = dart_executable(&sdk);
            let output = Command::new(&dart)
                .arg("--version")
                .output()
                .context(CommandSnafu { program: dart })?;
            if !output.status.success() {
                return DartVersionFailedSnafu.fail();
            }
            print!(
                "{}",
                String::from_utf8(output.stdout).context(Utf8Snafu {
                    action: "dart --version stdout",
                })?
            );
            print!(
                "{}",
                String::from_utf8(output.stderr).context(Utf8Snafu {
                    action: "dart --version stderr",
                })?
            );
            println!("SDK: {}", sdk.display());
            for header in [
                "dart_api.h",
                "dart_api_dl.h",
                "dart_native_api.h",
                "dart_tools_api.h",
                "dart_version.h",
            ] {
                let path = sdk.join("include").join(header);
                if !path.is_file() {
                    return MissingHeaderSnafu { path }.fail();
                }
                println!("{}", path.display());
            }
        }
        OcheCommand::BindgenDart(arguments) => {
            let input = &arguments.input;
            let output = &arguments.output;
            let source = std::fs::read_to_string(input).context(IoSnafu {
                action: "read Dart source",
                path: input.to_path_buf(),
            })?;
            let report =
                oche_interop::generate(&source, &file_uri(input)?).context(InteropSnafu)?;
            std::fs::write(output, report.definitions).context(IoSnafu {
                action: "write Oche definitions",
                path: output.to_path_buf(),
            })?;
            println!(
                "Generated {} function definition(s) in {} ({} skipped)",
                report.exported.len(),
                output.display(),
                report.skipped
            );
        }
        OcheCommand::Fmt(arguments) => {
            let mut changed = Vec::new();
            for path in &arguments.files {
                let source = std::fs::read_to_string(path).context(IoSnafu {
                    action: "read source for formatting",
                    path: path.to_path_buf(),
                })?;
                let formatted = oche_formatter::format(&source, 100)
                    .map_err(|error| frontend_error(path, &source, error))?;
                if source != formatted {
                    changed.push(path.display().to_string());
                    if !arguments.check {
                        std::fs::write(path, formatted).context(IoSnafu {
                            action: "write formatted source",
                            path: path.to_path_buf(),
                        })?;
                    }
                }
            }
            if arguments.check && !changed.is_empty() {
                return FormattingRequiredSnafu {
                    files: changed.join("\n"),
                }
                .fail();
            }
            if !arguments.check {
                println!("Formatted {} file(s)", changed.len());
            }
        }
        OcheCommand::Check(arguments) => inspect_source(&arguments.input, false)?,
        OcheCommand::Ast(arguments) => inspect_source(&arguments.input, true)?,
        OcheCommand::EmitDart(arguments) => {
            print!("{}", generated_source(&arguments.input)?);
        }
        OcheCommand::Build(arguments) => {
            let input = &arguments.input;
            let output = arguments
                .output
                .unwrap_or_else(|| input.with_extension("dill"));
            let generated = generated_source(input)?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("program.dart");
            write_generated(&dart_source, generated)?;
            let sdk = discover_sdk(None)?;
            compile_kernel(&dart_executable(&sdk), &dart_source, &output, true)?;
            println!("Built {}", output.display());
        }
        OcheCommand::BuildModule(arguments) => {
            let input = &arguments.input;
            let output = &arguments.output;
            let (_, package) = parse_file(input)?;
            let generated = oche_codegen::emit_dart_module_package(&package).map_err(|source| {
                Error::Codegen {
                    path: input.to_path_buf(),
                    source: Box::new(source),
                }
            })?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("module.dart");
            write_generated(&dart_source, generated)?;
            let sdk = discover_sdk(None)?;
            compile_kernel(&dart_executable(&sdk), &dart_source, output, false)?;
            println!("Built module {}", output.display());
        }
        OcheCommand::Serve(arguments) => {
            let input = &arguments.input;
            let (_, package) = parse_file(input)?;
            let generated =
                oche_codegen::emit_dart_jit_rpc_package(&package).map_err(|source| {
                    Error::Codegen {
                        path: input.to_path_buf(),
                        source: Box::new(source),
                    }
                })?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("server.dart");
            let kernel = temporary.0.join("server.dill");
            write_generated(&dart_source, generated)?;
            let sdk = discover_sdk(None)?;
            let dart = dart_executable(&sdk);
            compile_kernel(&dart, &dart_source, &kernel, true)?;
            let status = Command::new(&dart)
                .arg(kernel)
                .status()
                .context(CommandSnafu { program: dart })?;
            return Ok(ExitCode::from(status.code().unwrap_or(1) as u8));
        }
        OcheCommand::BuildExe(arguments) => {
            let input = &arguments.input;
            let output = arguments.output.unwrap_or_else(|| {
                if cfg!(windows) {
                    input.with_extension("exe")
                } else {
                    input.with_extension("")
                }
            });
            let generated = generated_source(input)?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("program.dart");
            write_generated(&dart_source, generated)?;
            let sdk = discover_sdk(None)?;
            compile_executable(&dart_executable(&sdk), &dart_source, &output)?;
            println!("Built {}", output.display());
        }
        OcheCommand::BuildJs(arguments) => {
            let input = &arguments.input;
            let single = arguments.single;
            let output = arguments
                .output
                .unwrap_or_else(|| input.with_extension("mjs"));
            let (_, package) = parse_file(input)?;
            let generated =
                oche_codegen::emit_dart_js_module_package(&package).map_err(|source| {
                    Error::Codegen {
                        path: input.to_path_buf(),
                        source: Box::new(source),
                    }
                })?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("module.dart");
            write_generated(&dart_source, generated)?;
            let javascript = if single {
                temporary.0.join("module.dart.js")
            } else {
                let stem = output
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("module");
                output.with_file_name(format!("{stem}.dart.js"))
            };
            let sdk = discover_sdk(None)?;
            compile_javascript(&dart_executable(&sdk), &dart_source, &javascript)?;
            if single {
                let compiled = std::fs::read_to_string(&javascript).context(IoSnafu {
                    action: "read compiled JavaScript",
                    path: javascript.clone(),
                })?;
                let compiled = compiled
                    .lines()
                    .filter(|line| !line.starts_with("//# sourceMappingURL="))
                    .collect::<Vec<_>>()
                    .join("\n");
                let module = format!(
                    "if (globalThis.self === undefined) globalThis.self = globalThis;\n{compiled}\n\nconst ocheModule = globalThis.__ocheExports;\nif (ocheModule === undefined) throw new Error('Oche module did not register its exports');\nexport default ocheModule;\n"
                );
                std::fs::write(&output, module).context(IoSnafu {
                    action: "write single-file JavaScript module",
                    path: output.clone(),
                })?;
            } else {
                let asset = javascript
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("module.dart.js")
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"");
                let wrapper = format!(
                    "if (globalThis.self === undefined) globalThis.self = globalThis;\nawait import('./{asset}');\n\nconst ocheModule = globalThis.__ocheExports;\nif (ocheModule === undefined) throw new Error('Oche module did not register its exports');\nexport default ocheModule;\n"
                );
                std::fs::write(&output, wrapper).context(IoSnafu {
                    action: "write JavaScript module wrapper",
                    path: output.clone(),
                })?;
            }
            println!("Built JavaScript module {}", output.display());
        }
        OcheCommand::Run(arguments) => {
            let input = &arguments.input;
            let generated = generated_source(input)?;
            let temporary = TemporaryDirectory::create()?;
            let dart_source = temporary.0.join("program.dart");
            let kernel = temporary.0.join("program.dill");
            write_generated(&dart_source, generated)?;
            let sdk = discover_sdk(None)?;
            let dart = dart_executable(&sdk);
            compile_kernel(&dart, &dart_source, &kernel, true)?;
            let status = Command::new(&dart)
                .arg(kernel)
                .arg(arguments.function)
                .args(arguments.arguments)
                .status()
                .context(CommandSnafu { program: dart })?;
            return Ok(ExitCode::from(status.code().unwrap_or(1) as u8));
        }
        OcheCommand::Test(arguments) => {
            let sdk = discover_sdk(None)?;
            let dart = dart_executable(&sdk);
            let mut failed = false;
            for input in &arguments.files {
                let (_, package) = parse_file(input)?;
                let generated =
                    oche_codegen::emit_dart_tests_package(&package).map_err(|source| {
                        Error::Codegen {
                            path: input.to_path_buf(),
                            source: Box::new(source),
                        }
                    })?;
                let temporary = TemporaryDirectory::create()?;
                let dart_source = temporary.0.join("tests.dart");
                write_generated(&dart_source, generated)?;
                if arguments.files.len() > 1 {
                    println!("{}", input.display());
                }
                let status =
                    Command::new(&dart)
                        .arg(&dart_source)
                        .status()
                        .context(CommandSnafu {
                            program: dart.clone(),
                        })?;
                failed |= !status.success();
            }
            return Ok(if failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            });
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            if !render_source_error(&error) {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn render_source_error(error: &Error) -> bool {
    let (path, line, column, message) = match error {
        Error::Frontend {
            path,
            line,
            column,
            source,
        } => (path, *line, *column, source.to_string()),
        Error::Typecheck {
            path,
            line,
            column,
            source,
        } => (path, *line, *column, source.to_string()),
        Error::Comptime {
            path,
            line,
            column,
            source,
        } => (path, *line, *column, source.to_string()),
        Error::ComptimeVm {
            path,
            line,
            column,
            details,
        } => (
            path,
            *line,
            *column,
            format!("Dart VM comptime evaluation failed: {details}"),
        ),
        _ => return false,
    };
    let Ok(source) = std::fs::read_to_string(path) else {
        return false;
    };
    let offset = line_column_offset(&source, line, column);
    let end = source
        .get(offset..)
        .and_then(|tail| tail.chars().next())
        .map_or(offset, |character| offset + character.len_utf8());
    let file = path.to_string_lossy();
    Report::build(ReportKind::Error, (file.as_ref(), offset..end))
        .with_message(message.clone())
        .with_label(
            Label::new((file.as_ref(), offset..end))
                .with_color(Color::Red)
                .with_message(message),
        )
        .finish()
        .eprint((file.as_ref(), Source::from(source)))
        .is_ok()
}

fn line_column_offset(source: &str, line: usize, column: usize) -> usize {
    let line_start = source
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let line_text = source
        .get(line_start..)
        .map(|tail| tail.split_once('\n').map_or(tail, |(line, _)| line))
        .unwrap_or("");
    line_start
        + line_text
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(line_text.len(), |(offset, _)| offset)
}

#[cfg(test)]
mod cli_tests {
    use super::{Cli, OcheCommand};
    use std::path::Path;
    use usage_rs::test as harness;

    #[test]
    fn parses_build_flags_without_positional_indexing() {
        let argv = harness::argv([
            "build-js",
            "--single",
            "examples/js_module.oche",
            "--output",
            "core.mjs",
        ]);
        let cli = harness::parse(Cli::spec(), &argv.words(), Cli::parse_from).unwrap();
        let OcheCommand::BuildJs(arguments) = cli.command else {
            panic!("expected build-js command");
        };
        assert!(arguments.single);
        assert_eq!(arguments.input, Path::new("examples/js_module.oche"));
        assert_eq!(arguments.output.as_deref(), Some(Path::new("core.mjs")));
    }

    #[test]
    fn preserves_hyphenated_function_arguments() {
        let argv = harness::argv(["run", "examples/arithmetic.oche", "add", "-1", "2"]);
        let cli = harness::parse(Cli::spec(), &argv.words(), Cli::parse_from).unwrap();
        let OcheCommand::Run(arguments) = cli.command else {
            panic!("expected run command");
        };
        assert_eq!(arguments.function, "add");
        assert_eq!(arguments.arguments, ["-1", "2"]);
    }

    #[test]
    fn requires_format_input() {
        let argv = harness::argv(["fmt", "--check"]);
        let error = harness::parse(Cli::spec(), &argv.words(), Cli::parse_from).unwrap_err();
        assert!(error.contains("required arguments"), "{error}");
        assert!(error.contains("<FILE>"), "{error}");
    }

    #[test]
    fn completes_subcommands_from_the_compiled_spec() {
        assert_eq!(
            harness::candidates(Cli::spec(), "oche build-"),
            ["build-exe", "build-js", "build-module"]
        );
    }
}
