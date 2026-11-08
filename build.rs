// Bettet unter Windows das Programmsymbol und die Versionsinfo in die .exe ein.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "ChatGameOverlay")
            .set("FileDescription", "ChatGameOverlay – Twitch-Chat über dem Spiel")
            .set("CompanyName", "Areadox");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Symbol konnte nicht eingebettet werden: {e}");
        }
    }
}
