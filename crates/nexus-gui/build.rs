fn main() {
    println!("cargo:rerun-if-changed=../../assets/nexuslab.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/nexuslab.ico")
            .set("ProductName", "NexusLab")
            .set(
                "FileDescription",
                "NexusLab — Hardware & Numerical Workbench",
            )
            .set("CompanyName", "Norbert John")
            .set(
                "LegalCopyright",
                "Copyright © 2026 Norbert John and NexusLab contributors",
            )
            .set("OriginalFilename", "nexuslab.exe")
            .compile()
            .expect("Windows SDK resource compiler is required to embed the NexusLab icon");
    }
}
