use hyper::{client::HttpConnector, Uri};
use std::path::Path;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tonic::codegen::CompressionEncoding;
use torsen::torsen_api::{
    rpc_fn_req, torsen_api_client::TorsenApiClient, HeartbeatReq, ReqFn002, RpcFnReq,
};
use tracing_subscriber::filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_max_level(filter::LevelFilter::INFO)
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

    let agent_id = "agent_id_001";
    let agent_type = "agent_type_001";
    let request = HeartbeatReq {
        agent_id: agent_id.to_string(),
        agent_type: agent_type.to_string(),
    };
    let mut stream = client.heartbeat(request).await?.into_inner();
    while let Some(feature) = stream.message().await? {
        log::info!("heartbeat rsp: {:?}", feature);
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let req_fn_001 = "this is req fn 001 content";
    let rpc_fn_req = RpcFnReq {
        req: Some(rpc_fn_req::Req::ReqFn001(req_fn_001.into())),
    };
    let rsp = client.rpc_fn(rpc_fn_req).await?;
    log::info!("rpc fn 001 rsp: {:?}", rsp);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let req_fn_002 = ReqFn002 {
        name: "this is name content".to_string(),
        age: 18,
    };
    let rpc_fn_req = RpcFnReq {
        req: Some(rpc_fn_req::Req::ReqFn002(req_fn_002.into())),
    };
    let rsp = client.rpc_fn(rpc_fn_req).await?;
    log::info!("rpc fn 002 rsp: {:?}", rsp);
    Ok(())
}
