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

    // Empreintes de contenu : les URL des fichiers changent dès que leur contenu change,
    // donc un navigateur ne peut jamais mélanger un ancien glue JS avec un nouveau wasm.
    let read = |p: PathBuf| {
        std::fs::read(&p).unwrap_or_else(|e| panic!("lecture de {}: {e}", p.display()))
    };
    let mut app = read(pkg_dir.join("f1x_frontend.js"));
    app.extend(read(pkg_dir.join("f1x_frontend_bg.wasm")));
    println!("cargo:rustc-env=F1X_APP_HASH={}", fnv1a(&app));
    println!("cargo:rerun-if-changed=static/app.css");
    println!(
        "cargo:rustc-env=F1X_CSS_HASH={}",
        fnv1a(&read(manifest_dir.join("static/app.css")))
    );
}

/// Hachage FNV-1a 64 bits (suffisant pour invalider un cache), en hexadécimal.
fn fnv1a(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}
