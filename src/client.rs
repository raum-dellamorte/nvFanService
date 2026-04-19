use {
  crate::{
    // cursive_frontend::nvfs_cursive_frontend,
    nvfs_config::NvfsConfig,
    daemon::{Envelope, Request, Response,},
    FanCurveUwU,
  },
  anyhow::{
    // anyhow,
    Context,
    Result,
  },
  bytes::Bytes,
  futures::{SinkExt, StreamExt},
  std::sync::{ Arc, Mutex,},
  tokio::net::UnixStream,
  tokio_util::codec::{Framed, LengthDelimitedCodec,},
};

pub struct FanServiceClient {
  pub conf: NvfsConfig,
  socket_path: String,
  connection: Framed<UnixStream, LengthDelimitedCodec>,
  next_id: u64,
  curve: Arc<Mutex<FanCurveUwU>>,
}
impl FanServiceClient {
  pub async fn new(conf: NvfsConfig, socket_path: &str) -> Result<Self, anyhow::Error> {
    let stream = UnixStream::connect(socket_path)
      .await
      .with_context(|| format!("Failed to connect to socket at {}", socket_path))?;
    let socket_path = socket_path.into();
    let connection = Framed::new(stream, LengthDelimitedCodec::new());
    let curve = conf.fan_curve().try_into()?;
    let curve = Arc::new(Mutex::new(curve));
    Ok(Self { conf, socket_path, connection, next_id: 1, curve })
  }
  async fn send_request(&mut self, body: Request) -> Result<Response, anyhow::Error> {
    let id = self.next_id;
    self.next_id += 1;
    let request = Envelope { id, body };
    let payload = serde_json::to_vec(&request).context("Failed to serialize request")?;
    self.connection
      .send(Bytes::from(payload))
      .await
      .context("Failed to send request to the daemon")?;
    let frame = self.connection
      .next()
      .await
      .ok_or_else(|| anyhow::anyhow!("The daemon seems to have closed the connection"))?
      .context("Failed to read response frame from the daemon")?;
    let response: Envelope<Response> = serde_json::from_slice(&frame)
      .context("Failed to decode the response from the daemon")?;
    Ok(response.body)
  }
  pub async fn request_card_name(&mut self) -> Result<String, anyhow::Error> {
    let response = self.send_request(Request::GetCardName).await?;
    match response {
      Response::CardName(name) => Ok(name),
      e => Err(anyhow::anyhow!("Unexpected Response: {:?}", e)),
    }
  }
  pub async fn request_tempspeed(&mut self) -> Result<(i32, u32), anyhow::Error> {
    let response = self.send_request(Request::GetTempSpeed).await?;
    match response {
      Response::TempSpeed{temp, speed} => Ok((temp, speed)),
      e => Err(anyhow::anyhow!("Unexpected Response: {:?}", e)),
    }
  }
  pub async fn request_curve(&mut self) -> Result<Vec<(i32, u32)>, anyhow::Error> {
    let response = self.send_request(Request::GetFanCurve).await?;
    match response {
      Response::FanCurve(curve) if curve.is_some() => Ok(curve.unwrap().into()),
      e => Err(anyhow::anyhow!("Unexpected Response: {:?}", e)),
    }
  }
  pub async fn set_curve(&mut self, curve: Vec<(i32, u32)>) -> Result<(), anyhow::Error> {
    let response = self.send_request(Request::SetFanCurve(Some(curve))).await?;
    match response {
      Response::Ok => Ok(()),
      e => Err(anyhow::anyhow!("Unexpected Response: {:?}", e)),
    }
  }
  pub async fn update_temp_node_with_new_speed(&mut self, temp: i32, speed: u32) -> Result<(), anyhow::Error> {
    let response = self.send_request(Request::UpdateTempNodeWithNewSpeed{temp, speed}).await?;
    match response {
      Response::Ok => Ok(()),
      e => Err(anyhow::anyhow!("Unexpected Response: {:?}", e)),
    }
  }
  pub fn user_curve(&self) -> Arc<Mutex<FanCurveUwU>> {
    self.curve.clone()
  }
}
