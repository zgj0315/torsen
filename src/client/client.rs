use tonic::{codegen::CompressionEncoding, transport::Channel};
use torsen::torsen_api::{torsen_api_client::TorsenApiClient, HeartbeatReq};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = Channel::builder("http://[::1]:50051".parse().unwrap())
        .connect()
        .await
        .unwrap();
    let mut client = TorsenApiClient::new(channel)
        .send_compressed(CompressionEncoding::Gzip)
        .accept_compressed(CompressionEncoding::Gzip);
    let request = HeartbeatReq::default();
    let response = client.heartbeat(request).await?;
    println!("response: {:?}", response);
    Ok(())
}
