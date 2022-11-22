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
use tracing::Level;

#[derive(Debug, Default)]
struct TorsenServer {}

#[tonic::async_trait]
impl TorsenApi for TorsenServer {
    type HeartbeatStream = ReceiverStream<Result<HeartbeatRsp, Status>>;
    async fn heartbeat(
        &self,
        request: Request<HeartbeatReq>,
    ) -> Result<Response<Self::HeartbeatStream>, Status> {
        let (tx, rx) = mpsc::channel(10);
        let heartbeat_req = request.into_inner();
        log::info!("heartbeat_req: {:?}", heartbeat_req);
        let cmd_content = format!(
            "agent_id: {}, agent_type: {}",
            heartbeat_req.agent_id.to_owned(),
            heartbeat_req.agent_type.to_owned()
        );
        tokio::spawn(async move {
            for i in 0..10 {
                let response = HeartbeatRsp {
                    cmd_type: i % 2,
                    cmd_content: cmd_content.clone(),
                };
                log::info!("send response: {:?}", response);
                tx.send(Ok(response)).await.unwrap();
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let file_appender = tracing_appender::rolling::daily("log", "server.log");
    // let (non_blocking, _guart) = tracing_appender::non_blocking(file_appender);
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        // .with_writer(non_blocking)
        .with_writer(std::io::stdout)
        .with_ansi(false)
        .event_format(format)
        .init();

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
    log::info!("Torsen Server is running...");
    loop {
        let (conn, addr) = match listener.accept().await {
            Ok(incoming) => incoming,
            Err(e) => {
                log::error!("Error accepting connection: {}", e);
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
