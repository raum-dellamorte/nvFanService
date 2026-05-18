use {
  crate::{
    nvfs_config::NvfsConfig,
    FanService,
    FanServiceArcMutex,
  },
  bytes::Bytes,
  futures::{SinkExt, StreamExt},
  nix::unistd::{chown, Gid, Group,},
  serde::{
    Deserialize,
    Serialize,
  },
  std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{ Arc, Mutex, },
    time::Instant,
  },
  tokio::{
    net::UnixListener,
    time::{ interval, Duration, },
  },
  tokio_util::codec::{Framed, LengthDelimitedCodec,},
};

pub async fn daemon(conf: NvfsConfig, socket_path: &str) -> anyhow::Result<()> {
  let curve = conf.fan_curve().try_into()?;
  let mut fan_service = FanService::new(curve)?;
  let card_name = {
    let _ = fan_service.device()?;
    fan_service.card_name.as_str().to_owned()
  };
  fan_service.service_service()?; 
  let fan_service = Arc::new(Mutex::new(fan_service));
  let fan_service_killer = fan_service.clone();
  let fan_service_tokio = fan_service.clone();
  // timed_service_service checks the temp and sets the fans according to the curve
  timed_service_service(fan_service_tokio);
  let mut run_server = true;
  if Path::new(socket_path).exists() {
    std::fs::remove_file(socket_path)?;
  }
  let listener = UnixListener::bind(socket_path)?;
  let wheel = Group::from_name("wheel")?
    .ok_or_else(|| anyhow::anyhow!("group 'wheel' not found"))?;
  chown(socket_path, None, Some(Gid::from_raw(wheel.gid.as_raw())))?;
  std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o660))?;
  'server: loop {
    if run_server {
      let fsc = fan_service.clone();
      timed_service_service(fsc);
    }
    let (stream, _addr) = listener.accept().await?;
    
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
    while let Some(frame) = framed.next().await {
      let bytes = frame?;
      let mut req: Envelope<Request> = serde_json::from_slice(&bytes)?;
      let body = match req.body {
        Request::GetCardName => {
          Response::CardName(card_name.to_owned())
        }
        Request::GetTempSpeed => {
          let ts = fan_service.current_tempspeed();
          Response::TempSpeed { temp: ts.0, speed: ts.1 }
        }
        Request::GetFanCurve => {
          Response::FanCurve(Some(fan_service.curve_copy()))
        }
        Request::SetFanCurve(ref mut new_curve) => {
          if let Err(e) = fan_service.update_curve(new_curve.take().unwrap()) {
            Response::Error(format!("Failed to set fan curve: {}", e))
          } else {
            Response::Ok
          }
        }
        Request::UpdateTempNodeWithNewSpeed { temp, speed } => {
          if let Err(e) = fan_service.update_temp_node_with_new_speed(temp, speed) {
            Response::Error(format!("Failed to update temperature node: {}", e))
          } else {
            Response::Ok
          }
        }
        Request::PauseServer => {
          run_server = false;
          service_service_happy_end(fan_service.clone());
          Response::Ok
        }
        Request::ResumeServer => {
          run_server = true;
          Response::Ok
        }
        Request::KillServer => {
          service_service_happy_end(fan_service.clone());
          Response::ServerShutdown
        }
      };
      let kill_server = match req.body {
        Request::KillServer => true,
        _ => false,
      };
      let response = Envelope { id: req.id, body: body};
      framed.send(Bytes::from(serde_json::to_vec(&response)?)).await?;
      if kill_server {
        break 'server;
      }
    }
  }
  service_service_happy_end(fan_service_killer);
  return anyhow::Ok(());
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum Request {
  GetCardName,
  GetTempSpeed,
  GetFanCurve,
  SetFanCurve(Option<Vec<(i32, u32)>>),
  UpdateTempNodeWithNewSpeed{temp: i32, speed: u32},
  PauseServer,
  ResumeServer,
  KillServer,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum Response {
  Ok,
  ServerShutdown,
  Error(String),
  CardName(String),
  TempSpeed{
    temp: i32,
    speed: u32,
  },
  FanCurve(Option<Vec<(i32,u32)>>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub id: u64,
    pub body: T,
}

fn timed_service_service(fs: Arc<Mutex<FanService>>) {
  tokio::spawn(async move {
    let mut instant = Instant::now();
    let mut ticker = interval(Duration::from_millis(100));
    let mut pause = false;
    loop {
      if pause {
        service_service_happy_end(fs.clone());
      }
      ticker.tick().await;
      let mut fsc = fs.lock().unwrap();
      pause = fsc.pause;
      if fsc.kill_switch {
        break;
      }
      if pause { continue; }
      if instant.elapsed() >= Duration::from_millis(500) {
        if let Err(e) = fsc.service_service() {
          error!("Error in service service: {}", e);
        }
        instant = Instant::now();
      }
    }
  });
}

fn service_service_happy_end(fs: Arc<Mutex<FanService>>) {
  if let Ok(fs) = fs.lock().as_mut() {
    let fan_count: u32 = fs.device()
      .expect("Failed to get num_fans from Device while attempting to return control to hardware fancurve.")
      .num_fans()
      .expect("Failed to get number of fans from device while attempting to return control to hardware fancurve.");
    for idx in 0..fan_count {
      fs.device()
        .expect("Failed to get Device while attempting to return control to hardware fancurve.")
        .set_default_fan_speed(idx)
          .expect("Failed to set default fan speed while closing.");
    }
  }
}
