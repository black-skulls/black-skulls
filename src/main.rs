use superchain_client::{
    config,
    tokio_tungstenite::connect_async,
    tungstenite::{client::IntoClientRequest, http::header::AUTHORIZATION},
    WsClient,
};

const URL: &str = "wss://beta.superchain.app/websocket";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut req = URL.into_client_request()?;
    let config = config::Config::from_env();
    req.headers_mut().append(
        AUTHORIZATION,
        config.get_basic_authorization_value().try_into()?,
    );

    let (websocket, _) = connect_async(req).await.unwrap();
    let client = WsClient::new(websocket).await;

    let _ = client;

    Ok(())
}
