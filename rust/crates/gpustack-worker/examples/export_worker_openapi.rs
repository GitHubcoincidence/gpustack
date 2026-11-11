//! Print the canonical managed Worker OpenAPI document to standard output.

fn main() {
    let document = gpustack_worker::worker_api::openapi_document();
    let json = document
        .to_pretty_json()
        .expect("Worker OpenAPI document must serialize");
    println!("{json}");
}
