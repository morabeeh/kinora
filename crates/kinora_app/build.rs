//! Build script: compiles the `.slint` UI files into Rust code at build time
//! (similar to how XAML is compiled into generated C# partial classes).

fn main() -> Result<(), slint_build::CompileError> {
    slint_build::compile("ui/app_window.slint")
}
