use hyper_util::rt::TokioIo;
use std::fs;
use std::path::Path;
#[cfg(unix)]
use tokio::net::UnixStream;
use tonic::transport::{Endpoint, Uri};
use torsen::torsen_api::{
    rpc_fn_req, torsen_api_client::TorsenApiClient, HeartbeatReq, ReqFn002, RpcFnReq,
};
use tower::service_fn;
use tracing_subscriber::filter;

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_max_level(filter::LevelFilter::INFO)
        .init();

    let channel = Endpoint::try_from("http://[::]:50051")?
        .connect_with_connector(service_fn(|_: Uri| async {
            let root_path = Path::new(file!())
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            let uds_path = root_path.join("uds");
            fs::create_dir_all(&uds_path)?;
            let rpc_fn_path = uds_path.join("rpc_fn");
            // Connect to a Uds socket
            Ok::<_, std::io::Error>(TokioIo::new(UnixStream::connect(rpc_fn_path).await?))
        }))
        .await?;

    let mut client = TorsenApiClient::new(channel);

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

#[cfg(not(unix))]
fn main() {
    panic!("The `uds` example only works on unix systems!");
}
