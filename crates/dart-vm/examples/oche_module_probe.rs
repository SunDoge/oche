use dart_vm_binding::Api;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn run(tool_path: &str, agent_path: &str) -> std::result::Result<String, String> {
    let api = unsafe { Api::from_process() }.map_err(|error| error.to_string())?;
    let mut isolate = unsafe { api.attach_current() }.map_err(|error| error.to_string())?;
    let group_id = isolate.group_id();
    let scope = isolate.scope().map_err(|error| error.to_string())?;
    let host = scope.root_library().map_err(|error| error.to_string())?;
    let register_tool = scope
        .field(host, "registerTool")
        .map_err(|error| error.to_string())?;
    let bytes = std::fs::read(tool_path).map_err(|error| error.to_string())?;
    let library = scope
        .load_kernel(bytes)
        .map_err(|error| error.to_string())?;
    scope
        .finalize_loading()
        .map_err(|error| error.to_string())?;
    scope
        .invoke(library, "register", &[register_tool])
        .map_err(|error| error.to_string())?;
    let left = scope.integer(20).map_err(|error| error.to_string())?;
    let right = scope.integer(22).map_err(|error| error.to_string())?;
    let result = scope
        .invoke(host, "callTool", &[left, right])
        .and_then(|value| scope.integer_value(value))
        .map_err(|error| error.to_string())?;

    let agent_bytes = std::fs::read(agent_path).map_err(|error| error.to_string())?;
    let agent = scope
        .load_kernel(agent_bytes)
        .map_err(|error| error.to_string())?;
    scope
        .finalize_loading()
        .map_err(|error| error.to_string())?;
    scope
        .invoke(agent, "register", &[register_tool])
        .map_err(|error| error.to_string())?;
    let abi_version = scope
        .invoke(
            host,
            "registeredTool",
            &[scope
                .string("abi_version")
                .map_err(|error| error.to_string())?],
        )
        .map_err(|error| error.to_string())?;
    let agent_abi = scope
        .invoke_closure(abi_version, &[])
        .and_then(|value| scope.integer_value(value))
        .map_err(|error| error.to_string())?;
    let start_function = scope
        .invoke(
            host,
            "registeredTool",
            &[scope.string("start").map_err(|error| error.to_string())?],
        )
        .map_err(|error| error.to_string())?;
    let start = scope
        .invoke_closure(start_function, &[])
        .and_then(|value| scope.string_value(value))
        .map_err(|error| error.to_string())?;
    expect_effect(&start, 1)?;
    let model_completed = scope
        .invoke(
            host,
            "registeredTool",
            &[scope
                .string("model_completed")
                .map_err(|error| error.to_string())?],
        )
        .map_err(|error| error.to_string())?;
    let after_model = scope
        .invoke_closure(
            model_completed,
            &[
                scope.string(&start).map_err(|error| error.to_string())?,
                scope.integer(0).map_err(|error| error.to_string())?,
                scope.boolean(false).map_err(|error| error.to_string())?,
            ],
        )
        .and_then(|value| scope.string_value(value))
        .map_err(|error| error.to_string())?;
    expect_effect(&after_model, 3)?;
    let queues_completed = scope
        .invoke(
            host,
            "registeredTool",
            &[scope
                .string("queues_completed")
                .map_err(|error| error.to_string())?],
        )
        .map_err(|error| error.to_string())?;
    let done = scope
        .invoke_closure(
            queues_completed,
            &[
                scope
                    .string(&after_model)
                    .map_err(|error| error.to_string())?,
                scope.integer(0).map_err(|error| error.to_string())?,
                scope.integer(0).map_err(|error| error.to_string())?,
                scope.boolean(false).map_err(|error| error.to_string())?,
                scope.boolean(false).map_err(|error| error.to_string())?,
            ],
        )
        .and_then(|value| scope.string_value(value))
        .map_err(|error| error.to_string())?;
    expect_effect(&done, 4)?;

    let float = scope.double(3.75).map_err(|error| error.to_string())?;
    if scope
        .double_value(float)
        .map_err(|error| error.to_string())?
        != 3.75
    {
        return Err("Float round trip changed its value".into());
    }
    let boolean = scope.boolean(true).map_err(|error| error.to_string())?;
    if !scope
        .boolean_value(boolean)
        .map_err(|error| error.to_string())?
    {
        return Err("Bool round trip changed its value".into());
    }
    let string = scope.string("agent ✓").map_err(|error| error.to_string())?;
    if scope
        .string_value(string)
        .map_err(|error| error.to_string())?
        != "agent ✓"
    {
        return Err("String round trip changed its value".into());
    }
    Ok(format!(
        r#"{{"ok":true,"result":{result},"group_id":{group_id},"agent_abi":{agent_abi},"agent_effects":[1,3,4],"primitive_roundtrips":3}}"#
    ))
}

fn expect_effect(encoded: &str, expected: i64) -> std::result::Result<(), String> {
    let state: serde_json::Value =
        serde_json::from_str(encoded).map_err(|error| error.to_string())?;
    let effect = state
        .get("effect")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| "Agent state has no integer effect".to_string())?;
    if effect != expected {
        return Err(format!(
            "Expected agent effect {expected}, received {effect}"
        ));
    }
    Ok(())
}

/// # Safety
/// Must be called through non-leaf FFI on a thread with an entered Dart isolate.
/// Both arguments point to readable NUL-terminated UTF-8 Kernel paths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_oche_module_probe(
    tool_path: *const c_char,
    agent_path: *const c_char,
) -> *mut c_char {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if tool_path.is_null() || agent_path.is_null() {
            return Err("Null Kernel path".to_string());
        }
        let tool_path = unsafe { CStr::from_ptr(tool_path) }
            .to_str()
            .map_err(|error| error.to_string())?;
        let agent_path = unsafe { CStr::from_ptr(agent_path) }
            .to_str()
            .map_err(|error| error.to_string())?;
        run(tool_path, agent_path)
    }));
    let report = match outcome {
        Ok(Ok(report)) => report,
        Ok(Err(error)) => format!("ERROR: {error}"),
        Err(_) => "ERROR: Rust probe panicked".into(),
    };
    CString::new(report.replace('\0', "\\0"))
        .expect("NUL removed")
        .into_raw()
}

/// # Safety
/// `report` is NULL or a pointer returned by `dart_oche_module_probe`, once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dart_oche_module_probe_free(report: *mut c_char) {
    if !report.is_null() {
        unsafe { drop(CString::from_raw(report)) };
    }
}
