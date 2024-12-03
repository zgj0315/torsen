use hyper::{client::HttpConnector, Uri};
use std::path::Path;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tonic::codegen::CompressionEncoding;
use torsen::torsen_api::{torsen_api_client::TorsenApiClient, HeartbeatReq};
use tracing_subscriber::filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_max_level(filter::LevelFilter::TRACE)
        .init();
    let root_path = Path::new(file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let tls_path = root_path.join("tls");
    let cert_file = std::fs::File::open(tls_path.join("sub-ca.crt"))?;
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
    for i in 0..3 {
        let agent_id = format!("agent_id_{}", i);
        let agent_type = format!("agent_type_{}", i);
        let request = HeartbeatReq {
            agent_id,
            agent_type,
        };
        let mut stream = client.heartbeat(request).await?.into_inner();
        while let Some(feature) = stream.message().await? {
            log::info!("{:?}", feature);
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    Ok(())
}
