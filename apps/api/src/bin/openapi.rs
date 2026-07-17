fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&riichi_api::openapi_document_value()).unwrap()
    );
}
