use tokio::sync::mpsc;
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{codegen::CompressionEncoding, transport::Server, Request, Response, Status};
use torsen::torsen_api::{
    torsen_api_server::{TorsenApi, TorsenApiServer},
    HeartbeatReq, HeartbeatRsp,
};

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

    let addr = "[::1]:50051".parse()?;
    let server = TorsenServer::default();
    let service = TorsenApiServer::new(server)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);
    Server::builder().add_service(service).serve(addr).await?;
    Ok(())
}
