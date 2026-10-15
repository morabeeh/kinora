//! Compiles the spike's `.slint` file.

fn main() -> Result<(), slint_build::CompileError> {
    slint_build::compile("ui/bench.slint")
}
