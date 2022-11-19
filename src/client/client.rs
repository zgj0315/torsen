use torsen::torsen_api::{torsen_api_client::TorsenApiClient, HeartbeatReq};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = TorsenApiClient::connect("http://[::1]:50051").await?;
    let request = HeartbeatReq::default();
    let response = client.heartbeat(request).await?;
    println!("response: {:?}", response);
    Ok(())
}
