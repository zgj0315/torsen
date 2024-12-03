use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{codegen::CompressionEncoding, transport::Server, Request, Response, Status};
use torsen::torsen_api::rpc_fn_req::Req;
use torsen::torsen_api::{
    rpc_fn_rsp,
    torsen_api_server::{TorsenApi, TorsenApiServer},
    HeartbeatReq, HeartbeatRsp, RpcFnReq, RpcFnRsp, RspFn002,
};
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

    async fn rpc_fn_c2s(&self, request: Request<RpcFnReq>) -> Result<Response<RpcFnRsp>, Status> {
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

    let server = TorsenServer::default();
    let service = TorsenApiServer::new(server)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);

    let addr = "[::1]:50051".parse().unwrap();
    log::info!("Torsen Server is running...");
    Server::builder().add_service(service).serve(addr).await?;
    Ok(())
}
