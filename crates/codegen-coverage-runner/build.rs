fn main() {
    let output =
        std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
            .arg("--print=sysroot")
            .output()
            .expect("failed to run rustc --print=sysroot");
    let sysroot = String::from_utf8(output.stdout)
        .expect("sysroot is not utf-8")
        .trim()
        .to_owned();
    println!("cargo:rustc-link-arg=-Wl,-rpath,{sysroot}/lib");
}
