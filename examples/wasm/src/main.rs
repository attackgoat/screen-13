#![deny(warnings)]

use {
    hyper::{
        header::{HeaderValue, CONTENT_TYPE},
        service::{make_service_fn, service_fn},
        {Body, Error, Method, Request, Response, Result, Server, StatusCode},
    },
    tokio::fs::File,
    tokio_util::codec::{BytesCodec, FramedRead},
};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let addr = "127.0.0.1:1337".parse().unwrap();
    let make_service = make_service_fn(|_| async { Ok::<_, Error>(service_fn(response_examples)) });
    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    server.await.unwrap();
}

async fn response_examples(req: Request<Body>) -> Result<Response<Body>> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") | (&Method::GET, "/index.html") => {
            send_file("res/index.html", "text/html").await
        }
        (&Method::GET, "/wasm.js") => {
            send_file(
                "target/wasm32-unknown-unknown/debug/wasm.js",
                "text/javascript",
            )
            .await
        }
        (&Method::GET, "/wasm_bg.js") => {
            send_file(
                "target/wasm32-unknown-unknown/debug/wasm_bg.js",
                "text/javascript",
            )
            .await
        }
        (&Method::GET, "/wasm_bg.wasm") => {
            send_file(
                "target/wasm32-unknown-unknown/debug/wasm_bg.wasm",
                "application/wasm",
            )
            .await
        }
        _ => Ok(not_found()),
    }
}

/// HTTP status code 404
fn not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Not Found".into())
        .unwrap()
}

async fn send_file(filename: &'static str, content_type: &'static str) -> Result<Response<Body>> {
    if let Ok(file) = File::open(filename).await {
        let stream = FramedRead::new(file, BytesCodec::new());
        let body = Body::wrap_stream(stream);
        let mut response = Response::new(body);
        response
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));

        return Ok(response);
    }

    Ok(not_found())
}
