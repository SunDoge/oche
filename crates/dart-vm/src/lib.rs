//! Minimal bindings for a currently entered Dart JIT isolate.
//!
//! The raw C signatures are generated from the pinned Dart SDK by the workspace
//! xtask. The ownership, scope, thread, error, and symbol-resolution layer is
//! hand-written here.
//!
//! No VM initialization, native I/O resolver, or standalone embedding is provided
//! yet. The Linux resolver uses symbols exported by the running `dartvm`.
//! Scoped handles cannot outlive their API scope or move between Rust threads.
//! Kernel allocations are transferred to VM-managed external typed data, with a
//! native finalizer. Keep this Rust library loaded until the isolate group exits.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr;
use std::rc::Rc;

use snafu::Snafu;

#[allow(
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    reason = "names mirror the Dart C embedding API"
)]
mod generated {
    include!("generated/dart_api.rs");
}

macro_rules! resolved {
    ($function:expr) => {
        $function.expect("resolved Dart VM function pointer")
    };
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Missing VM symbol: {name}"))]
    MissingVmSymbol { name: &'static str },
    #[snafu(display("No isolate entered on this thread"))]
    NoCurrentIsolate,
    #[snafu(display("Current isolate changed"))]
    CurrentIsolateChanged,
    #[snafu(display("VM returned a null C handle"))]
    NullHandle,
    #[snafu(display("Dart VM API error: {message}"))]
    DartApi { message: String },
    #[snafu(display("Handle belongs to another API scope"))]
    WrongScope,
    #[snafu(display("String contains a NUL byte"))]
    StringContainsNul,
    #[snafu(display("Too many invocation arguments"))]
    TooManyArguments,
    #[snafu(display("Kernel buffer is empty"))]
    EmptyKernel,
    #[snafu(display("Kernel buffer is too large"))]
    KernelTooLarge,
}

/// Opaque C ABI definitions. These types are unsafe to use without VM context.
pub mod raw {
    use super::*;
    pub type Handle = generated::Dart_Handle;
    pub type Isolate = generated::Dart_Isolate;
    pub type CurrentIsolate = generated::Oche_Dart_CurrentIsolate;
    pub type GroupId = generated::Oche_Dart_CurrentIsolateGroupId;
    pub type ScopeFn = generated::Oche_Dart_EnterScope;
    pub type IsError = generated::Oche_Dart_IsError;
    pub type GetError = generated::Oche_Dart_GetError;
    pub type RootLibrary = generated::Oche_Dart_RootLibrary;
    pub type NewString = generated::Oche_Dart_NewStringFromCString;
    pub type StringValue = generated::Oche_Dart_StringToCString;
    pub type NewInteger = generated::Oche_Dart_NewInteger;
    pub type GetField = generated::Oche_Dart_GetField;
    pub type Invoke = generated::Oche_Dart_Invoke;
    pub type InvokeClosure = generated::Oche_Dart_InvokeClosure;
    pub type IntegerValue = generated::Oche_Dart_IntegerToInt64;
    pub type NewDouble = generated::Oche_Dart_NewDouble;
    pub type DoubleValue = generated::Oche_Dart_DoubleValue;
    pub type NewBoolean = generated::Oche_Dart_NewBoolean;
    pub type BooleanValue = generated::Oche_Dart_BooleanValue;
    pub type ExternalData = generated::Oche_Dart_NewExternalTypedDataWithFinalizer;
    pub type LoadLibrary = generated::Oche_Dart_LoadLibrary;
    pub type LoadLibraryFromKernel = generated::Oche_Dart_LoadLibraryFromKernel;
    pub type FinalizeLoading = generated::Oche_Dart_FinalizeLoading;
}

/// Resolved entry points. Never unload the underlying VM while this exists.
pub struct Api {
    current_isolate: raw::CurrentIsolate,
    group_id: raw::GroupId,
    enter_scope: raw::ScopeFn,
    exit_scope: raw::ScopeFn,
    is_error: raw::IsError,
    get_error: raw::GetError,
    root_library: raw::RootLibrary,
    new_string: raw::NewString,
    string_value: raw::StringValue,
    new_integer: raw::NewInteger,
    get_field: raw::GetField,
    invoke: raw::Invoke,
    invoke_closure: raw::InvokeClosure,
    integer_value: raw::IntegerValue,
    new_double: raw::NewDouble,
    double_value: raw::DoubleValue,
    new_boolean: raw::NewBoolean,
    boolean_value: raw::BooleanValue,
    external_data: raw::ExternalData,
    load_library: raw::LoadLibrary,
    finalize_loading: raw::FinalizeLoading,
}

impl Api {
    /// Resolve the embedding API from a caller-owned VM library or process.
    ///
    /// # Safety
    /// Returned pointers must have the exact C signatures in `raw`, from a
    /// compatible JIT VM (validated here against Dart 3.13.5). Its code must
    /// remain loaded for all calls. Resolver results cannot be data pointers.
    pub unsafe fn from_resolver(mut resolve: impl FnMut(&CStr) -> *mut c_void) -> Result<Self> {
        macro_rules! symbol {
            ($name:literal, $type:ty) => {{
                let name = cstr(concat!($name, "\0"));
                let pointer = resolve(name);
                if pointer.is_null() {
                    return Err(Error::MissingVmSymbol { name: $name });
                }
                // Safety follows from the resolver contract above.
                unsafe { std::mem::transmute::<*mut c_void, $type>(pointer) }
            }};
        }
        Ok(Self {
            current_isolate: symbol!("Dart_CurrentIsolate", raw::CurrentIsolate),
            group_id: symbol!("Dart_CurrentIsolateGroupId", raw::GroupId),
            enter_scope: symbol!("Dart_EnterScope", raw::ScopeFn),
            exit_scope: symbol!("Dart_ExitScope", raw::ScopeFn),
            is_error: symbol!("Dart_IsError", raw::IsError),
            get_error: symbol!("Dart_GetError", raw::GetError),
            root_library: symbol!("Dart_RootLibrary", raw::RootLibrary),
            new_string: symbol!("Dart_NewStringFromCString", raw::NewString),
            string_value: symbol!("Dart_StringToCString", raw::StringValue),
            new_integer: symbol!("Dart_NewInteger", raw::NewInteger),
            get_field: symbol!("Dart_GetField", raw::GetField),
            invoke: symbol!("Dart_Invoke", raw::Invoke),
            invoke_closure: symbol!("Dart_InvokeClosure", raw::InvokeClosure),
            integer_value: symbol!("Dart_IntegerToInt64", raw::IntegerValue),
            new_double: symbol!("Dart_NewDouble", raw::NewDouble),
            double_value: symbol!("Dart_DoubleValue", raw::DoubleValue),
            new_boolean: symbol!("Dart_NewBoolean", raw::NewBoolean),
            boolean_value: symbol!("Dart_BooleanValue", raw::BooleanValue),
            external_data: symbol!("Dart_NewExternalTypedDataWithFinalizer", raw::ExternalData),
            load_library: symbol!("Dart_LoadLibrary", raw::LoadLibrary),
            finalize_loading: symbol!("Dart_FinalizeLoading", raw::FinalizeLoading),
        })
    }

    /// Look up symbols in an ordinary running Linux Dart VM executable.
    ///
    /// # Safety
    /// The process must export the compatible Dart JIT API described in
    /// `from_resolver`. This does not initialize or load a VM executable.
    #[cfg(target_os = "linux")]
    pub unsafe fn from_process() -> Result<Self> {
        #[link(name = "dl")]
        unsafe extern "C" {
            fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        }
        unsafe { Self::from_resolver(|name| dlsym(ptr::null_mut(), name.as_ptr())) }
    }

    /// Attach to the isolate already entered on this OS thread.
    ///
    /// # Safety
    /// Call on a VM-entered native thread, with callbacks permitted (e.g. a
    /// non-leaf Dart FFI call). The isolate must remain entered for this value's
    /// lifetime. Do not create another attachment or exit/shutdown it meanwhile.
    pub unsafe fn attach_current(&self) -> Result<AttachedIsolate<'_>> {
        let isolate = unsafe { (resolved!(self.current_isolate))() };
        if isolate.is_null() {
            return Err(Error::NoCurrentIsolate);
        }
        Ok(AttachedIsolate {
            api: self,
            isolate,
            group_id: unsafe { (resolved!(self.group_id))() },
            _thread_bound: PhantomData,
        })
    }
}

fn cstr(value: &'static str) -> &'static CStr {
    CStr::from_bytes_with_nul(value.as_bytes()).expect("internal C string")
}

/// Thread-bound attachment. This borrows the isolate, and does not shut it down.
pub struct AttachedIsolate<'api> {
    api: &'api Api,
    isolate: raw::Isolate,
    group_id: i64,
    _thread_bound: PhantomData<Rc<()>>,
}

impl AttachedIsolate<'_> {
    pub fn group_id(&self) -> i64 {
        self.group_id
    }

    pub fn scope(&mut self) -> Result<Scope<'_>> {
        if unsafe { (resolved!(self.api.current_isolate))() } != self.isolate {
            return Err(Error::CurrentIsolateChanged);
        }
        unsafe { (resolved!(self.api.enter_scope))() };
        Ok(Scope {
            api: self.api,
            _borrow: PhantomData,
            _thread_bound: PhantomData,
        })
    }
}

/// API scope: its Drop exits the scope, invalidating every local handle.
pub struct Scope<'vm> {
    api: &'vm Api,
    _borrow: PhantomData<&'vm mut ()>,
    _thread_bound: PhantomData<Rc<()>>,
}

/// Local handle, borrowed from one particular API scope.
///
/// A handle cannot be carried into a later scope:
///
/// ```compile_fail
/// use dart_vm_binding::Api;
/// fn cannot_escape(api: &Api) {
///     let mut isolate = unsafe { api.attach_current() }.unwrap();
///     let value;
///     {
///         let scope = isolate.scope().unwrap();
///         value = scope.integer(42).unwrap();
///     }
///     let scope = isolate.scope().unwrap();
///     scope.integer_value(value).unwrap();
/// }
/// ```
///
/// The scope is tied to its native thread:
///
/// ```compile_fail
/// use dart_vm_binding::Scope;
/// fn cannot_send(scope: &Scope<'_>) {
///     std::thread::scope(|threads| {
///         threads.spawn(move || { scope.integer(42).unwrap(); });
///     });
/// }
/// ```
#[derive(Clone, Copy)]
pub struct Handle<'scope> {
    raw: raw::Handle,
    owner: &'scope Scope<'scope>,
}

struct KernelAllocation {
    bytes: Box<[u8]>,
}

// VM finalizers may run on arbitrary threads. Only drop owned bytes; no VM calls.
unsafe extern "C" fn free_kernel(_isolate_data: *mut c_void, peer: *mut c_void) {
    if !peer.is_null() {
        unsafe { drop(Box::from_raw(peer.cast::<KernelAllocation>())) };
    }
}

impl Scope<'_> {
    fn checked(&self, raw: raw::Handle) -> Result<Handle<'_>> {
        if raw.is_null() {
            return Err(Error::NullHandle);
        }
        if unsafe { (resolved!(self.api.is_error))(raw) } {
            let message = unsafe { (resolved!(self.api.get_error))(raw) };
            let message = if message.is_null() {
                "Unknown Dart error".into()
            } else {
                unsafe { CStr::from_ptr(message) }
                    .to_string_lossy()
                    .into_owned()
            };
            return Err(Error::DartApi { message });
        }
        Ok(Handle { raw, owner: self })
    }

    fn owned(&self, handle: Handle<'_>) -> Result<raw::Handle> {
        if !ptr::eq(self, handle.owner) {
            return Err(Error::WrongScope);
        }
        Ok(handle.raw)
    }

    pub fn root_library(&self) -> Result<Handle<'_>> {
        self.checked(unsafe { (resolved!(self.api.root_library))() })
    }

    pub fn string(&self, value: &str) -> Result<Handle<'_>> {
        let value = CString::new(value).map_err(|_| Error::StringContainsNul)?;
        self.checked(unsafe { (resolved!(self.api.new_string))(value.as_ptr()) })
    }

    pub fn string_value(&self, value: Handle<'_>) -> Result<String> {
        let mut result = ptr::null();
        self.checked(unsafe {
            (resolved!(self.api.string_value))(self.owned(value)?, &mut result)
        })?;
        if result.is_null() {
            return Err(Error::NullHandle);
        }
        Ok(unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn integer(&self, value: i64) -> Result<Handle<'_>> {
        self.checked(unsafe { (resolved!(self.api.new_integer))(value) })
    }

    pub fn integer_value(&self, value: Handle<'_>) -> Result<i64> {
        let mut result = 0;
        self.checked(unsafe {
            (resolved!(self.api.integer_value))(self.owned(value)?, &mut result)
        })?;
        Ok(result)
    }

    pub fn double(&self, value: f64) -> Result<Handle<'_>> {
        self.checked(unsafe { (resolved!(self.api.new_double))(value) })
    }

    pub fn double_value(&self, value: Handle<'_>) -> Result<f64> {
        let mut result = 0.0;
        self.checked(unsafe {
            (resolved!(self.api.double_value))(self.owned(value)?, &mut result)
        })?;
        Ok(result)
    }

    pub fn boolean(&self, value: bool) -> Result<Handle<'_>> {
        self.checked(unsafe { (resolved!(self.api.new_boolean))(value) })
    }

    pub fn boolean_value(&self, value: Handle<'_>) -> Result<bool> {
        let mut result = false;
        self.checked(unsafe {
            (resolved!(self.api.boolean_value))(self.owned(value)?, &mut result)
        })?;
        Ok(result)
    }

    pub fn field(&self, target: Handle<'_>, name: &str) -> Result<Handle<'_>> {
        let name = self.string(name)?;
        self.checked(unsafe { (resolved!(self.api.get_field))(self.owned(target)?, name.raw) })
    }

    pub fn invoke(
        &self,
        target: Handle<'_>,
        name: &str,
        arguments: &[Handle<'_>],
    ) -> Result<Handle<'_>> {
        let count = c_int::try_from(arguments.len()).map_err(|_| Error::TooManyArguments)?;
        let mut args = arguments
            .iter()
            .map(|arg| self.owned(*arg))
            .collect::<Result<Vec<_>>>()?;
        let name = self.string(name)?;
        self.checked(unsafe {
            (resolved!(self.api.invoke))(self.owned(target)?, name.raw, count, args.as_mut_ptr())
        })
    }

    pub fn invoke_closure(
        &self,
        closure: Handle<'_>,
        arguments: &[Handle<'_>],
    ) -> Result<Handle<'_>> {
        let count = c_int::try_from(arguments.len()).map_err(|_| Error::TooManyArguments)?;
        let mut args = arguments
            .iter()
            .map(|argument| self.owned(*argument))
            .collect::<Result<Vec<_>>>()?;
        self.checked(unsafe {
            (resolved!(self.api.invoke_closure))(self.owned(closure)?, count, args.as_mut_ptr())
        })
    }

    /// Load a Kernel library into the current group with VM-managed byte lifetime.
    ///
    /// Uses `Dart_LoadLibrary(externalTypedData)` rather than the unmanaged raw
    /// pointer variant `Dart_LoadLibraryFromKernel`. The VM retains its blob;
    /// the finalizer releases the Rust allocation when the VM releases it.
    /// This does not promise library unloading or perform finalization itself.
    pub fn load_kernel(&self, bytes: Vec<u8>) -> Result<Handle<'_>> {
        if bytes.is_empty() {
            return Err(Error::EmptyKernel);
        }
        let length = isize::try_from(bytes.len()).map_err(|_| Error::KernelTooLarge)?;
        let mut allocation = Box::new(KernelAllocation {
            bytes: bytes.into_boxed_slice(),
        });
        let data = allocation.bytes.as_mut_ptr();
        let peer = Box::into_raw(allocation);
        let external = unsafe {
            (resolved!(self.api.external_data))(
                generated::Dart_TypedData_Type_Dart_TypedData_kUint8,
                data.cast(),
                length,
                peer.cast(),
                length,
                Some(free_kernel),
            )
        };
        let external = match self.checked(external) {
            Ok(handle) => handle,
            Err(error) => {
                // No external object/finalizer was created on API failure.
                unsafe { drop(Box::from_raw(peer)) };
                return Err(error);
            }
        };
        // Ownership has transferred, including when parsing this blob fails.
        self.checked(unsafe { (resolved!(self.api.load_library))(external.raw) })
    }

    pub fn finalize_loading(&self) -> Result<()> {
        self.checked(unsafe { (resolved!(self.api.finalize_loading))(false) })?;
        Ok(())
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        unsafe { (resolved!(self.api.exit_scope))() };
    }
}
