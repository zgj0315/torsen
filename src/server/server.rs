use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use torsen::torsen::{
    torsen_server::{Torsen, TorsenServer},
    HeartbeatReq, HeartbeatRsp,
};

#[derive(Debug, Default)]
struct TorsenService {}

#[tonic::async_trait]
impl Torsen for TorsenService {
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
    let addr = "[::1]:50051".parse()?;
    let service = TorsenService::default();
    Server::builder()
        .add_service(TorsenServer::new(service))
        .serve(addr)
        .await?;
    Ok(())
}
