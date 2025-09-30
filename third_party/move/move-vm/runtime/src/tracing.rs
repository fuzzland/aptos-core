// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(any(debug_assertions, feature = "debugging"))]
use crate::debug::DebugContext;
use crate::{interpreter::InterpreterDebugInterface, loader::LoadedFunction, RuntimeEnvironment};
use ::{
    move_binary_format::file_format::Bytecode,
    move_vm_types::values::Locals,
    once_cell::sync::Lazy,
    std::{
        cell::RefCell,
        env,
        fs::{File, OpenOptions},
        io::Write,
        sync::Mutex,
    },
};

const MOVE_VM_TRACING_ENV_VAR_NAME: &str = "MOVE_VM_TRACE";

const MOVE_VM_STEPPING_ENV_VAR_NAME: &str = "MOVE_VM_STEP";

static FILE_PATH: Lazy<String> = Lazy::new(|| {
    env::var(MOVE_VM_TRACING_ENV_VAR_NAME).unwrap_or_else(|_| "move_vm_trace.trace".to_string())
});

pub static TRACING_ENABLED: Lazy<bool> =
    Lazy::new(|| env::var(MOVE_VM_TRACING_ENV_VAR_NAME).is_ok());

#[cfg(any(debug_assertions, feature = "debugging"))]
static DEBUGGING_ENABLED: Lazy<bool> =
    Lazy::new(|| env::var(MOVE_VM_STEPPING_ENV_VAR_NAME).is_ok());

pub static LOGGING_FILE_WRITER: Lazy<Mutex<std::io::BufWriter<File>>> = Lazy::new(|| {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&*FILE_PATH)
        .unwrap();
    Mutex::new(std::io::BufWriter::with_capacity(4096 * 1024, file))
});

// Thread-local, in-memory pc capture support.
thread_local! {
    static TL_PC_CAPTURE_ENABLED: RefCell<bool> = RefCell::new(false);
    static TL_PC_BUFFER: RefCell<Vec<u32>> = RefCell::new(Vec::new());
}

/// Begin capturing program counters for the current thread.
pub fn begin_pc_capture() {
    TL_PC_CAPTURE_ENABLED.with(|e| *e.borrow_mut() = true);
    TL_PC_BUFFER.with(|buf| buf.borrow_mut().clear());
}

/// Stop capturing and return the captured program counters for the current thread.
pub fn end_pc_capture_take() -> Vec<u32> {
    TL_PC_CAPTURE_ENABLED.with(|e| *e.borrow_mut() = false);
    TL_PC_BUFFER.with(|buf| std::mem::take(&mut *buf.borrow_mut()))
}

#[cfg(any(debug_assertions, feature = "debugging"))]
static DEBUG_CONTEXT: Lazy<Mutex<DebugContext>> = Lazy::new(|| Mutex::new(DebugContext::new()));

pub(crate) fn trace(
    function: &LoadedFunction,
    locals: &Locals,
    pc: u16,
    instr: &Bytecode,
    runtime_environment: &RuntimeEnvironment,
    interpreter: &dyn InterpreterDebugInterface,
) {
    // Always attempt to capture into thread-local buffer when enabled.
    TL_PC_CAPTURE_ENABLED.with(|enabled| {
        if *enabled.borrow() {
            TL_PC_BUFFER.with(|buf| buf.borrow_mut().push(pc as u32));
        }
    });

    if *TRACING_ENABLED {
        let writer = &mut *LOGGING_FILE_WRITER.lock().unwrap();
        writer
            .write_fmt(format_args!(
                "{},{}\n",
                function.name_as_pretty_string(),
                pc,
            ))
            .unwrap();
        writer.flush().unwrap();
    }
    #[cfg(any(debug_assertions, feature = "debugging"))]
    if *DEBUGGING_ENABLED {
        DEBUG_CONTEXT.lock().unwrap().debug_loop(
            function,
            locals,
            pc,
            instr,
            runtime_environment,
            interpreter,
        );
    }
}

#[macro_export]
macro_rules! trace {
    ($function_desc:expr, $locals:expr, $pc:expr, $instr:tt, $resolver:expr, $interp:expr) => {
        $crate::tracing::trace(&$function_desc, $locals, $pc, &$instr, $resolver, $interp)
    };
}
