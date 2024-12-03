use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tonic::Request;
use torsen::torsen_api::{
    rpc_fn_req, rpc_fn_rsp, torsen_api_client::TorsenApiClient, HeartbeatReq, ReqFn002, RpcFnReq,
    RpcFnRsp,
};
use tracing_subscriber::filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_max_level(filter::LevelFilter::INFO)
        .init();
    let mut client = TorsenApiClient::connect("http://[::1]:50051").await?;
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
    let rsp = client.rpc_fn_c2s(rpc_fn_req).await?;
    log::info!("rpc fn 001 rsp: {:?}", rsp);
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let req_fn_002 = ReqFn002 {
        name: "this is name content".to_string(),
        age: 18,
    };
    let rpc_fn_req = RpcFnReq {
        req: Some(rpc_fn_req::Req::ReqFn002(req_fn_002)),
    };
    let rsp = client.rpc_fn_c2s(rpc_fn_req).await?;
    log::info!("rpc fn 002 rsp: {:?}", rsp);

    // rpc_fn_s2c

    // 执行server下发的消息，并返回结果给server
    let (tx, mut rx) = mpsc::channel(10);
    let outbound = async_stream::stream! {
        while let Some(rpc_fn_req) = rx.recv().await {
           log::info!("rpc_fn_req: {:?}", rpc_fn_req);
           log::info!("do some work and send rsp to server");
           tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
           let rsp_fn_001 = "this is rsp fn 001 from client";
           let rpc_fn_rsp = RpcFnRsp {
               rsp: Some(rpc_fn_rsp::Rsp::RspFn001(rsp_fn_001.to_string())),
           };
           yield rpc_fn_rsp;
        }
    };
    let req_stream = Request::new(outbound);

    // 接收server下发的信息
    let rsp = client.rpc_fn_s2c(req_stream).await?;
    let mut rsp_stream = rsp.into_inner();
    while let Some(received) = rsp_stream.next().await {
        match received {
            Ok(rpc_fn_req) => {
                tx.send(rpc_fn_req).await?;
            }
            Err(e) => {
                log::error!("err: {}", e);
            }
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
    Ok(())
}
