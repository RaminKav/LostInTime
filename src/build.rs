fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("windows") {
        embed_resource::compile("icon.rc");
    }
}
