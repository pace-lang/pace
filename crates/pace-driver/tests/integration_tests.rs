use pace_driver::CompilerSession;

use std::path::Path;

#[test]
fn test_basic_arithmetic_typecheck() {
    let session = CompilerSession::new();
    let src = "
    func main() {
        let x: Int = 5 + 10;
        let y: Float = 3.14 * 2.0;
        let z: Bool = x > 10;
    }";
    assert!(session.check_source(src).is_ok(), "Basic arithmetic failed to typecheck");
}

#[test]
fn test_type_error_detection() {
    let session = CompilerSession::new();
    let src = "
    func main() {
        let x: Int = \"hello\";
    }";
    assert!(session.check_source(src).is_err(), "Expected type error for string to int assignment");
}

#[test]
fn test_run_simple_program() {
    let session = CompilerSession::new();
    let src = "
    func main() {
        let x: Int = 42;
    }";
    // release = false (none optimization)
    assert!(session.run_source(src, false).is_ok(), "Failed to JIT run simple program");
}

#[test]
fn test_run_examples_suite() {
    let session = CompilerSession::new();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let main_pace_path = Path::new(manifest_dir)
        .join("../../examples/pace-project/src/main.pace")
        .canonicalize()
        .expect("Failed to find main.pace in examples");
        
    let main_path_str = main_pace_path.to_str().unwrap();
    
    // Parse, typecheck, compile, and JIT run the whole examples suite
    let result = session.run_file(main_path_str, false);
    assert!(result.is_ok(), "Failed to run the examples suite: {:?}", result.err());
}
