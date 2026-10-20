//! Links a compiled program against the real runtime library and runs it.
//!
//! The library is the one `cargo build` leaves in the target folder, so build the
//! workspace before running this test.

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

use nifty_codegen::{RUNTIME_LIBRARY, compile_app, link};

mod support;

use support::{FILE, check, locate};

/// A program whose state can't start: dividing by zero, at line 3, column 15.
const SOURCE: &str = "fn zero() -> int { return 0 }

state count = 1 / zero()

window(\"Never\") {}
";
const LINE: u32 = 3;
const COLUMN: u32 = 15;

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

fn failing_program() -> Vec<u8> {
    compile_app("link-test", &check(SOURCE), &|span| locate(SOURCE, span))
        .unwrap_or_else(|error| panic!("the program compiles: {error}"))
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
