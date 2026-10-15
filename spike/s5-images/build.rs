//! Compiles the spike viewer UI.

fn main() -> Result<(), slint_build::CompileError> {
    slint_build::compile("ui/viewer.slint")
}
