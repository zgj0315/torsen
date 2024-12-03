use hyper::server::conn::http2::Builder;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::service::TowerToHyperService;
use std::path::Path;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::mpsc};
use tokio_rustls::{
    rustls::{
        pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer},
        ServerConfig,
    },
    TlsAcceptor,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{codegen::CompressionEncoding, transport::Server, Request, Response, Status};
use torsen::torsen_api::rpc_fn_req::Req;
use torsen::torsen_api::{
    rpc_fn_rsp,
    torsen_api_server::{TorsenApi, TorsenApiServer},
    HeartbeatReq, HeartbeatRsp, RpcFnReq, RpcFnRsp, RspFn002,
};
use tower::ServiceExt;
use tower_http::ServiceBuilderExt;
use tracing_subscriber::filter;

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
            for i in 0..2 {
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

    async fn rpc_fn(&self, request: Request<RpcFnReq>) -> Result<Response<RpcFnRsp>, Status> {
        match request.into_inner().req {
            None => {
                return Ok(Response::new(RpcFnRsp { rsp: None }));
            }
            Some(req) => match req {
                Req::ReqFn001(req_fn_001) => {
                    let rsp_fn_001 = format!("get req: {}", req_fn_001);
                    let rpc_fn_rsp = RpcFnRsp {
                        rsp: Some(rpc_fn_rsp::Rsp::RspFn001(rsp_fn_001)),
                    };
                    return Ok(Response::new(rpc_fn_rsp));
                }
                Req::ReqFn002(req_fn_002) => {
                    let msg = format!("get req: {}, {}", req_fn_002.name, req_fn_002.age);
                    let rpc_fn_rsp = RpcFnRsp {
                        rsp: Some(rpc_fn_rsp::Rsp::RspFn002(RspFn002 { msg })),
                    };
                    return Ok(Response::new(rpc_fn_rsp));
                }
            },
        }
    }
}

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

    let certs = {
        let cert_file = std::fs::File::open(tls_path.join("torsen-ca.crt"))?;
        let mut cert_buf = std::io::BufReader::new(&cert_file);
        CertificateDer::pem_reader_iter(&mut cert_buf).collect::<Result<Vec<_>, _>>()?
    };
    let key = {
        let key_file = std::fs::File::open(tls_path.join("torsen-ca.key"))?;
        let mut key_buf = std::io::BufReader::new(&key_file);
        PrivateKeyDer::from_pem_reader(&mut key_buf)?
    };
    let mut tls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;
    tls.alpn_protocols = vec![b"h2".to_vec()];
    let server = TorsenServer::default();
    let service = TorsenApiServer::new(server)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);
    let svc = Server::builder().add_service(service).into_service();

    let http = Builder::new(TokioExecutor::new());

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
            match http
                .serve_connection(
                    TokioIo::new(conn),
                    TowerToHyperService::new(
                        svc.map_request(|req: http::Request<_>| req.map(Body::new)),
                    ),
                )
                .await
            {
                Ok(r) => {
                    log::info!("connection: {:?}", r);
                }
                Err(e) => {
                    log::error!("http server error: {:?}", e)
                }
            }
        });
    }
}

#[derive(Debug)]
struct ConnInfo {
    addr: std::net::SocketAddr,
    certificates: Vec<CertificateDer<'static>>,
}
