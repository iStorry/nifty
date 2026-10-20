//! Links a compiled program against the real runtime library and runs it.
//!
//! The library is the one `cargo build` leaves in the target folder, so build the
//! workspace before running this test.

use std::mem::{offset_of, size_of};
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

use cranelift_codegen::ir::{InstBuilder, types};
use nifty_codegen::{Blob, ObjectFile, RUNTIME_LIBRARY, Target, link};
use nifty_rt::{ABI_VERSION, ProgramDesc, SpanDesc, StateDesc, symbols};

const FILE: &str = "main.nf";
const LINE: u32 = 3;
const COLUMN: u32 = 7;

/// The runtime library built alongside this test: tests run from `<target>/<profile>/deps`.
fn runtime_library() -> PathBuf {
    let test = env::current_exe().expect("a test knows where it is");
    let profile = test
        .parent()
        .and_then(|deps| deps.parent())
        .expect("tests run from the target folder's deps folder");
    let library = profile.join(RUNTIME_LIBRARY);
    assert!(
        library.is_file(),
        "{} is missing; run `cargo build --workspace` before this test",
        library.display()
    );
    library
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("nf-codegen-test-{name}"));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("remove stale scratch dir");
    }
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// A program with one `int` state whose starting value divides by zero, at `main.nf:3:7`.
fn failing_program() -> Vec<u8> {
    let mut object = ObjectFile::new("link-test").expect("Cranelift supports this system");
    let pointer = object.pointer_type();

    let fail = object
        .import_function(
            symbols::FAIL_DIVIDE_BY_ZERO,
            &object.signature(&[pointer, types::I32], &[]),
        )
        .expect("import the failure call");
    let initialiser = object
        .define_function(
            object.signature(&[pointer, pointer], &[types::I8]),
            |body| {
                let context = body.params()[0];
                let span = body.builder.ins().iconst(types::I32, 0);
                body.call(fail, &[context, span]);
                let failed = body.builder.ins().iconst(types::I8, 0);
                body.builder.ins().return_(&[failed]);
            },
        )
        .expect("compile the state's initialiser");
    let int_type = object
        .import_data(symbols::TYPE_INT)
        .expect("import the int type");

    let mut state = Blob::zeroed(size_of::<StateDesc>());
    state.put_pointer(offset_of!(StateDesc, ty), Target::Data(int_type));
    state.put_pointer(offset_of!(StateDesc, init), Target::Function(initialiser));
    let states = object.define_data(state).expect("define the states");

    let file = object
        .define_data(Blob::bytes(FILE.as_bytes()))
        .expect("define the file name");
    let mut span = Blob::zeroed(size_of::<SpanDesc>());
    span.put_slice(offset_of!(SpanDesc, file), Target::Data(file), FILE.len());
    span.put_u32(offset_of!(SpanDesc, line), LINE);
    span.put_u32(offset_of!(SpanDesc, column), COLUMN);
    let spans = object.define_data(span).expect("define the spans");

    let mut program = Blob::zeroed(size_of::<ProgramDesc>());
    program.put_u32(offset_of!(ProgramDesc, abi_version), ABI_VERSION);
    program.put_slice(offset_of!(ProgramDesc, states), Target::Data(states), 1);
    program.put_slice(offset_of!(ProgramDesc, spans), Target::Data(spans), 1);
    let program = object.define_data(program).expect("define the program");

    object.define_main(program).expect("compile main");
    object.finish().expect("write the object")
}

#[test]
fn a_linked_program_runs_against_the_runtime() {
    let dir = scratch_dir("link");
    let object = dir.join("app.o");
    fs::write(&object, failing_program()).expect("write the object file");
    let executable = dir.join(format!("app{}", env::consts::EXE_SUFFIX));

    link(&object, &runtime_library(), &executable).unwrap_or_else(|error| panic!("{error}"));

    let output = Command::new(&executable).output().expect("run the app");
    assert_eq!(
        output.status.code(),
        Some(1),
        "the app stops with its error"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        format!("{FILE}:{LINE}:{COLUMN}: can't divide by zero")
    );
    fs::remove_dir_all(&dir).expect("remove scratch dir");
}
