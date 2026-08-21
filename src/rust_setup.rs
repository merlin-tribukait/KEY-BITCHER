use anyhow::Result;

fn probe(binary: &str) -> Option<String> {
    which::which(binary)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

pub fn check_rust_env() -> Result<()> {
    println!("Checking Rust toolchain...");

    let rustc = probe("rustc");
    let cargo = probe("cargo");

    match (rustc, cargo) {
        (Some(rustc), Some(cargo)) => {
            println!("Rust appears installed:");
            println!("  rustc at {}", rustc);
            println!("  cargo at {}", cargo);
        }
        _ => {
            println!("Rust is not fully installed.");
            println!("On Windows (PowerShell), run:");
            println!("  winget install Rustlang.Rustup");
            println!("Then restart the shell and run:");
            println!("  rustup default stable");
        }
    }

    for (bin, hint) in [
        ("clippy-driver", "rustup component add clippy"),
        ("rustfmt", "rustup component add rustfmt"),
        ("rust-analyzer", "rustup component add rust-analyzer"),
        ("cargo-watch", "cargo install cargo-watch"),
        ("taplo", "cargo install taplo-cli --locked"),
    ] {
        match probe(bin) {
            Some(p) => println!("  [ok] {} ({})", bin, p),
            None => println!("  [missing] {} -> {}", bin, hint),
        }
    }

    Ok(())
}
