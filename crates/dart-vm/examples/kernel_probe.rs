use dart_vm_binding::{Api, Scope};
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

fn run(paths: &[String]) -> std::result::Result<String, String> {
    // Called synchronously through a non-leaf FFI call in an ordinary JIT VM.
    let api = unsafe { Api::from_process() }.map_err(|e| e.to_string())?;
    let mut isolate = unsafe { api.attach_current() }.map_err(|e| e.to_string())?;
    let group = isolate.group_id();
    let scope = isolate.scope().map_err(|e| e.to_string())?;
    exercise(&scope, paths).map_err(|e| e.to_string()).map(|(p50, p95)| {
        format!(r#"{{"ok":true,"versions":{},"group_id":{},"load_register_p50_ms":{},"load_register_p95_ms":{},"negative_checks":3}}"#,
            paths.len(), group, p50, p95)
    })
}

fn exercise(scope: &Scope<'_>, paths: &[String]) -> dart_vm_binding::Result<(f64, f64)> {
    let host = scope.root_library()?;
    let tools = scope.field(host, "tools")?;
    // Genuine error paths: empty input, invalid Kernel, and type mismatch.
    assert!(scope.load_kernel(Vec::new()).is_err());
    assert!(scope.load_kernel(vec![0; 16]).is_err());
    assert!(
        scope
            .integer_value(scope.string("not an integer")?)
            .is_err()
    );
    let mut timings = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let bytes = std::fs::read(path).expect("fixture Kernel must exist");
        let begin = Instant::now();
        let library = scope.load_kernel(bytes)?;
        scope.finalize_loading()?;
        scope.invoke(library, "register", &[tools])?;
        timings.push(begin.elapsed().as_secs_f64() * 1000.0);
        let result = scope.invoke(host, "callTool", &[scope.integer(20)?, scope.integer(22)?])?;
        assert_eq!(scope.integer_value(result)?, 42 + index as i64);
    }
    // Confirm all versions still have live closures, not just the final alias.
    for index in 0..paths.len() {
        let result = scope.invoke(
            host,
            "callVersion",
            &[
                scope.integer(index as i64 + 1)?,
                scope.integer(20)?,
                scope.integer(22)?,
            ],
        )?;
        assert_eq!(scope.integer_value(result)?, 42 + index as i64);
    }
    timings.sort_by(f64::total_cmp);
    Ok((
        timings[timings.len() / 2],
        timings[(timings.len() * 95).div_ceil(100) - 1],
    ))
}

/// # Safety
/// Must be called on an entered Dart JIT isolate through a non-leaf FFI call.
/// `paths` has `count` readable NUL-terminated UTF-8 paths. Keep this library
/// loaded for the group's entire lifetime, including external-data finalizers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_kernel_probe(
    paths: *const *const c_char,
    count: isize,
) -> *mut c_char {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if paths.is_null() || count <= 0 {
            return Err("No Kernel paths provided".to_string());
        }
        let paths = unsafe { std::slice::from_raw_parts(paths, count as usize) };
        let paths = paths
            .iter()
            .map(|path| {
                if path.is_null() {
                    return Err("Null path".to_string());
                }
                unsafe { CStr::from_ptr(*path) }
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|e| e.to_string())
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        run(&paths)
    }));
    let report = match outcome {
        Ok(Ok(report)) => report,
        Ok(Err(error)) => format!("ERROR: {error}"),
        Err(_) => "ERROR: Rust probe panicked".into(),
    };
    // The matching free function must release this allocation.
    CString::new(report.replace('\0', "\\0"))
        .expect("NUL removed")
        .into_raw()
}

/// # Safety
/// `report` is NULL or a pointer returned by dart_kernel_probe, freed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_kernel_probe_free(report: *mut c_char) {
    if !report.is_null() {
        unsafe { drop(CString::from_raw(report)) };
    }
}
