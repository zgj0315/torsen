use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{
    codegen::CompressionEncoding, transport::Server, Request, Response, Status, Streaming,
};
use torsen::torsen_api::rpc_fn_req::Req;
use torsen::torsen_api::{
    rpc_fn_req, rpc_fn_rsp,
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

    type RpcFnS2cStream = ReceiverStream<Result<RpcFnReq, Status>>;

    async fn rpc_fn_s2c(
        &self,
        request: Request<Streaming<RpcFnRsp>>,
    ) -> Result<Response<Self::RpcFnS2cStream>, Status> {
        // 建立连接，进入函数，开始下发指令
        let (tx, rx) = mpsc::channel(10);
        let req_fn_001 = "this is req fn 001 content from server";
        let rpc_fn_req = RpcFnReq {
            req: Some(rpc_fn_req::Req::ReqFn001(req_fn_001.into())),
        };
        tx.send(Ok(rpc_fn_req)).await.unwrap();

        // 启动接收返回的协程
        let mut in_stream = request.into_inner();
        tokio::spawn(async move {
            while let Some(result) = in_stream.next().await {
                match result {
                    Ok(rpc_fn_rsp) => {
                        log::info!("get rsp from client, rpc_fn_rsp: {:?}", rpc_fn_rsp);
                    }
                    Err(e) => {
                        log::error!("err: {}", e);
                    }
                }
            }
        });
        log::info!("ready to receive rsp and send cmd to client");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        Ok(Response::new(ReceiverStream::new(rx)))
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
