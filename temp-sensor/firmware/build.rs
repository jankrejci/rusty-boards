/// Build script that doubles as the linker error handler.
///
/// The `--error-handling-script` linker flag points to the build script's own
/// compiled binary via `current_exe()`. This must be computed at build time,
/// which is why it lives here instead of in `.cargo/config.toml`. When the
/// linker encounters an undefined symbol, it re-invokes this binary with the
/// error details, allowing us to print helpful diagnostic messages.
fn main() {
    linker_be_nice();
}

fn linker_be_nice() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 {
        let kind = &args[1];
        let what = &args[2];

        match kind.as_str() {
            "undefined-symbol" => match what.as_str() {
                "_defmt_timestamp" => {
                    eprintln!();
                    eprintln!(
                        "defmt not found - make sure defmt.x is added as a linker script and you have included use defmt_rtt as _;"
                    );
                    eprintln!();
                }
                "_stack_start" => {
                    eprintln!();
                    eprintln!("Is the linker script linkall.x missing?");
                    eprintln!();
                }
                _ => (),
            },
            _ => {
                std::process::exit(1);
            }
        }

        std::process::exit(0);
    }

    println!(
        "cargo:rustc-link-arg=--error-handling-script={}",
        std::env::current_exe()
            .expect("failed to get build script path")
            .display()
    );
}
