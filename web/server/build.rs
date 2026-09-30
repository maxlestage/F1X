//! Compile le frontend Yew (`../frontend`) en WebAssembly puis génère le glue JS avec
//! wasm-bindgen. Les fichiers produits sont embarqués dans le binaire du serveur :
//! un seul `cargo build` suffit (Heroku, Docker, local) — ni trunk ni npm.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let frontend = manifest_dir.join("../frontend");
    println!("cargo:rerun-if-changed=../frontend/src");
    println!("cargo:rerun-if-changed=../frontend/Cargo.toml");
    println!("cargo:rerun-if-changed=../Cargo.lock");
    println!("cargo:rerun-if-changed=../protocol/src");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let target_dir = out_dir.join("wasm-target");
    let pkg_dir = out_dir.join("pkg");

    // La cible wasm peut manquer (ex. buildpack Heroku) : on l'installe si rustup est là.
    let _ = Command::new("rustup")
        .args(["target", "add", TARGET])
        .status();

    let mut cmd = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    cmd.current_dir(&frontend)
        .args([
            "build",
            "--release",
            "--locked",
            "-p",
            "f1x-frontend",
            "--target",
            TARGET,
        ])
        .arg("--target-dir")
        .arg(&target_dir)
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "z");
    // Ne pas transmettre la configuration du build hôte au build wasm.
    for (key, _) in env::vars() {
        let keep = key == "CARGO_HOME"
            || key.starts_with("CARGO_HTTP_")
            || key.starts_with("CARGO_NET_")
            || key.starts_with("CARGO_REGISTRIES_")
            || key == "CARGO_PROFILE_RELEASE_OPT_LEVEL";
        if (key.starts_with("CARGO_") && !keep)
            || matches!(
                key.as_str(),
                "RUSTC" | "RUSTDOC" | "RUSTFLAGS" | "RUSTC_WRAPPER" | "RUSTC_WORKSPACE_WRAPPER"
            )
        {
            cmd.env_remove(&key);
        }
    }
    let status = cmd
        .status()
        .expect("impossible de lancer cargo pour le frontend");
    assert!(status.success(), "la compilation du frontend Yew a échoué");

    let wasm = target_dir
        .join(TARGET)
        .join("release")
        .join("f1x-frontend.wasm");
    wasm_bindgen_cli_support::Bindgen::new()
        .input_path(&wasm)
        .web(true)
        .expect("mode web de wasm-bindgen")
        .out_name("f1x_frontend")
        .typescript(false)
        .generate(&pkg_dir)
        .expect("wasm-bindgen a échoué");
}
