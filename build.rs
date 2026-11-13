fn main() {
    println!("cargo:rerun-if-changed=assets/zoomer.rc");
    println!("cargo:rerun-if-changed=assets/windows-app.manifest");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Err(error) =
            embed_resource::compile("assets/zoomer.rc", embed_resource::NONE).manifest_required()
    {
        panic!("Windows SDK resource compiler is required: {error:?}");
    }
}
