use std::sync::Arc;

use hyper::server::conn::Http;
use tokio::{net::TcpListener, sync::mpsc};
use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{codegen::CompressionEncoding, transport::Server, Request, Response, Status};
use torsen::torsen_api::{
    torsen_api_server::{TorsenApi, TorsenApiServer},
    HeartbeatReq, HeartbeatRsp,
};
use tower_http::ServiceBuilderExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Default)]
struct TorsenServer {}

#[tonic::async_trait]
impl TorsenApi for TorsenServer {
    type HeartbeatStream = ReceiverStream<Result<HeartbeatRsp, Status>>;
    async fn heartbeat(
        &self,
        request: Request<HeartbeatReq>,
    ) -> Result<Response<Self::HeartbeatStream>, Status> {
        println!("request: {:?}", request);
        let (tx, rx) = mpsc::channel(10);
        let response = HeartbeatRsp::default();
        tx.send(Ok(response)).await.unwrap();
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry().with(fmt::layer()).init();
    let certs = {
        let cert_file = std::fs::File::open("tls/torsen-ca.crt")?;
        let mut cert_buf = std::io::BufReader::new(&cert_file);
        rustls_pemfile::certs(&mut cert_buf)?
            .into_iter()
            .map(Certificate)
            .collect()
    };
    let key = {
        let key_file = std::fs::File::open("tls/torsen-ca.key")?;
        let mut key_buf = std::io::BufReader::new(&key_file);
        rustls_pemfile::pkcs8_private_keys(&mut key_buf)?
            .into_iter()
            .map(PrivateKey)
            .next()
            .unwrap()
    };
    let mut tls = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    tls.alpn_protocols = vec![b"h2".to_vec()];
    let server = TorsenServer::default();
    let service = TorsenApiServer::new(server)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);

    let svc = Server::builder().add_service(service).into_service();
    let mut http = Http::new();
    http.http2_only(true);
    let listener = TcpListener::bind("[::1]:50051").await?;
    let tls_acceptor = TlsAcceptor::from(Arc::new(tls));

    loop {
        let (conn, addr) = match listener.accept().await {
            Ok(incoming) => incoming,
            Err(e) => {
                eprint!("Error accepting connection: {}", e);
                continue;
            }
        };
        let http = http.clone();
        let tls_acceptor = tls_acceptor.clone();
        let svc = svc.clone();
        tokio::spawn(async move {
            let mut certificates = Vec::new();
            let conn = tls_acceptor
                .accept_with(conn, |info| {
                    if let Some(certs) = info.peer_certificates() {
                        for cert in certs {
                            certificates.push(cert.clone());
                        }
                    }
                })
                .await
                .unwrap();
            let svc = tower::ServiceBuilder::new()
                .add_extension(Arc::new(ConnInfo { addr, certificates }))
                .service(svc);
            http.serve_connection(conn, svc).await.unwrap();
        });
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct ConnInfo {
    addr: std::net::SocketAddr,
    certificates: Vec<Certificate>,
}
