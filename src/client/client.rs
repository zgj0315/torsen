use hyper::{client::HttpConnector, Uri};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tonic::codegen::CompressionEncoding;
use torsen::torsen_api::{torsen_api_client::TorsenApiClient, HeartbeatReq};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry().with(fmt::layer()).init();
    let cert_file = std::fs::File::open("tls/sub-ca.crt")?;
    let mut cert_buf = std::io::BufReader::new(&cert_file);
    let mut roots = RootCertStore::empty();
    let certs = rustls_pemfile::certs(&mut cert_buf)?;
    roots.add_parsable_certificates(&certs);
    let tls = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();

    let mut http = HttpConnector::new();
    http.enforce_http(false);

    let connector = tower::ServiceBuilder::new()
        .layer_fn(move |s| {
            let tls = tls.clone();
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config(tls)
                .https_or_http()
                .enable_http2()
                .wrap_connector(s)
        })
        .map_request(|_| Uri::from_static("https://[::1]:50051"))
        .service(http);

    let client = hyper::Client::builder().build(connector);
    let uri = Uri::from_static("https://example.com");
    let mut client = TorsenApiClient::with_origin(client, uri)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);

    let request = HeartbeatReq::default();
    let response = client.heartbeat(request).await?;
    log::info!("response: {:?}", response);
    Ok(())
}
