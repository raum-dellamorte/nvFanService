#![allow(unused_braces,dead_code)] //,unused_imports,unused_variables,unused_mut)]
use {
  crate::{
    client::FanServiceClient,
    cursive_frontend::nvfs_cursive_frontend,
    daemon::daemon,
    elevate::running_unpriviledged,
    nvfs_config::NvfsConfig,
    sdl3_frontend::nvfs_sdl3_frontend,
  },
  anyhow::{anyhow, Result,},
  arrayvec::ArrayString,
  nvml_wrapper::{
    device::Device,
    enum_wrappers::device::TemperatureSensor,
    error::NvmlError,
    Nvml,
  },
  regex::Regex,
  std::{
    error::Error,
    ffi::OsStr,
    fs::read_to_string,
    // io::Write,
    path::Path,
    sync::{ Arc, Mutex, },
  },
};

#[macro_use]
extern crate log;

mod cursive_frontend;
mod client;
mod daemon;
mod elevate;
mod nvfs_config;
mod sdl3_frontend;

// Settings
const DRY_RUN:bool = false; // Change me to a command line parameter like `--dry-run`

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
  let as_root = !running_unpriviledged();
  let conf: NvfsConfig = if let Ok(conf) = nvfs_config::get_or_create_config(as_root) {
    conf
  } else {
    return Err(anyhow!("Failed to get or create config"));
  };
  let socket_path = "/run/nvFanService.sock";
  if as_root {
    daemon(conf, socket_path).await
  } else {
    client(conf, socket_path).await
  }
}

async fn client(conf: NvfsConfig, socket_path: &str) -> anyhow::Result<()> {
  let client = FanServiceClient::new(conf, socket_path).await?;
  let args: Vec<String> = std::env::args().collect();
  if args.len() == 2 && &args[1] == "cli" {
    nvfs_cursive_frontend(client).await
  } else {
    nvfs_sdl3_frontend(client).await
  }
}

struct FanService {
  nvml: Nvml,
  card_idx: Option<u32>,
  card_name: ArrayString<32>,
  curve: Arc<Mutex<FanCurveUwU>>,
  current_temp_speed: TempSpeed,
  pause: bool,
  kill_switch: bool,
}
impl FanService {
  fn new(curve: FanCurveUwU) -> Result<FanService, anyhow::Error> {
    let nvml = init_nvml_so()?;
    let curve = Arc::new(Mutex::new(curve));
    Ok(FanService {
      nvml, card_idx: None, card_name: ArrayString::new(),
      curve: curve.clone(), current_temp_speed: TempSpeed::default(),
      pause: false, kill_switch: false,
    })
  }
  
  // fn set_card_id(&mut self, idx: u32) { self.card_idx = Some(idx); }
  
  fn service_service(&mut self) -> Result<(), anyhow::Error> {
    // if the 1st GPU is not the one we want to control, can we TemperatureSensor::Gpu + 1 ???
    let gpu_idx = if self.device().is_ok() {
      let Ok(gpu_idx) = TemperatureSensor::try_from(self.card_idx.unwrap()) else {
        return Err(anyhow!("Failed to convert device index to TemperatureSensor enum in service_fans()"));
      };
      gpu_idx.clone()
    } else { return Err(anyhow!("service_service: Failed to get device."));};
    
    let (fan_count, temp) = if let Ok(device) = &self.device() {
      let Ok(fan_count) = device.num_fans() else {
        return Err(anyhow!("Failed to get num_fans from device in service_fans()"));
      };
      let Ok(temp) = device.temperature(gpu_idx) else {
        return Err(anyhow!("Failed to get temperature reading from device in service_fans()"));
      };
      // let gpu_idx = gpu_idx.cl
      (fan_count, temp)
    } else { return Err(anyhow!("This should be unreachable"));};
    
    #[allow(unused_mut)]
    if let (Ok(curve), Ok(mut device)) = (self.curve.clone().lock(), self.device()) {
      let n: usize = curve.points.len();
      for ts in (0..n).rev() {
        if let Ok(temp_speed) = curve.points[ts].clone().lock() 
        && temp as i32 >= temp_speed.temp() {
          let speed: u32;
          if let Some(Ok(next_ts)) = 
            if ts + 1 >= n { None } else {
              Some(curve.points[ts + 1].lock())
            }
          {
            let temp_now = temp as f32;
            let atemp = temp_speed.temp() as f32;
            let btemp = next_ts.temp() as f32;
            let aspeed = temp_speed.speed() as f32;
            let bspeed = next_ts.speed() as f32;
            let temp_range = btemp - atemp;
            let temp_diff_pct = (temp_now - atemp) / temp_range;
            speed = (aspeed + (bspeed - aspeed) * temp_diff_pct) as u32;
          } else {
            speed = temp_speed.speed();
          }
          for idx in 0..fan_count {
            if device.fan_speed(idx)? != speed && !DRY_RUN {
              match device.set_fan_speed(idx, speed) {
                Ok(_) => {}
                Err(e) => {
                  self.current_temp_speed = TempSpeed(temp as i32, speed);
                  return Err(anyhow!("Can't set fan speed: {}", e));
                }
              }
            }
          }
          self.current_temp_speed = TempSpeed(temp as i32, speed);
          return Ok(());
        }
      }
    }
    
    Err(anyhow!("Nothing happened, I swear!"))
  }
  
  fn device(&mut self) -> Result<Device<'_>, NvmlError> {
    if self.card_idx.is_some() { return self.nvml.device_by_index(self.card_idx.unwrap()) }
    let device_count = self.nvml.device_count().unwrap_or(0);
    match device_count.cmp(&1) {
      std::cmp::Ordering::Less => Err(NvmlError::NotFound),
      std::cmp::Ordering::Equal => {
        println!("Found one nVidia GPU.");
        let device = self.nvml.device_by_index(0);
        if let Ok(device) = &device {
          let name = device.name().unwrap_or("<Unable to get device name>".to_owned());
          self.card_name.push_str(&name);
          self.card_idx = Some(0);
          println!("~> {}", &name);
        }
        device
      }
      std::cmp::Ordering::Greater => {
        println!("Found {} nVidia devices.\nPlease choose one:", device_count);
        let mut devices = Vec::new();
        for i in 0..device_count {
          let device = self.nvml.device_by_index(i);
          if let Ok(device) = &device {
            let name = device.name().unwrap_or("<Unable to get device name>".to_owned());
            self.card_name.push_str(&name);
            println!("{} ~> {}", i + 1, &name);
            devices.push((i, name));
          }
        }
        // fixme: defaulting to the first so I don't have to write a user prompt right now
        println!("Picking a card not yet implemented.\nUsing first available card.");
        if devices.is_empty() { 
          Err(NvmlError::NotFound) // No Devices Found
        } else {
          self.card_idx = Some(devices[0].0);
          self.card_name.push_str(&devices[0].1);
          self.nvml.device_by_index(0) // Return first device found
        }
      }
    }
  }
}

fn init_nvml_so() -> Result<Nvml, NvmlError> {
  let init_result = Nvml::init();
  if init_result.is_ok() { return init_result }
  // We're still here? libnvidia-ml.so is in another castle
  // Attempt to locate libnvidia-ml.so.<current driver triple>
  println!("Default libnvidia-ml.so not found.");
  println!("Attempting to load libnvidia-ml.so.<current driver triple>");
  let file = Path::new("/proc/driver/nvidia/version");
  if let Ok(true) = Path::try_exists(file) {
    let drv_ver_info = read_to_string(file).unwrap();
    let re: Regex = Regex::new(r"(?m)Kernel Module +(\d+\.\d+\.\d+)").unwrap();
    let captures = re.captures(&drv_ver_info);
    if let Some(res) = captures {
      if res.len() > 1 {
        let ver = res[1].to_owned();
        println!("Found nVidia driver version: {}", ver);
        let libname = format!("libnvidia-ml.so.{}", ver);
        let libpath = format!("/usr/lib64/{}", &libname);
        println!("Initializing with {}", &libname);
        let init_result = Nvml::builder().lib_path(OsStr::new(&libpath)).init();
        if init_result.is_ok() { return init_result }
      }
    } else {
      println!("/proc/driver/nvidia/version exists but finding driver triple failed.");
      println!("Proc results:\n{}", drv_ver_info);
      println!("Regex:\n{:?}\nresults:\n{:?}", re, captures);
    }
  } else {
    println!("/proc/driver/nvidia/version not found.")
  }
  
  let libname = "libnvidia-ml.so.1".to_owned();
  let libpath = format!("/usr/lib64/{}", &libname);
  println!("Attempting to use {} as our NVML Library.", libname);
  let init_result = Nvml::builder().lib_path(OsStr::new(&libpath)).init();
  init_result
}

pub trait FanServiceArcMutex {
  fn current_tempspeed(&self) -> (i32,u32);
  fn curve(&self) -> Arc<Mutex<FanCurveUwU>>;
  fn curve_copy(&self) -> Vec<(i32, u32)>;
  fn update_curve(&self, new_curve: Vec<(i32, u32)>) -> Result<(), anyhow::Error>;
  fn update_temp_node_with_new_speed(&self, temp: i32, speed: u32) -> Result<(), anyhow::Error>;
}

impl FanServiceArcMutex for Arc<Mutex<FanService>> {
  fn current_tempspeed(&self) -> (i32,u32) {
    if let Ok(fs) = self.lock() {
      fs.current_temp_speed.into()
    } else {
      (0,0)
    }
  }
  fn curve(&self) -> Arc<Mutex<FanCurveUwU>> {
    self.lock().unwrap().curve.clone()
  }
  fn curve_copy(&self) -> Vec<(i32, u32)> {
    let fs = self.lock().unwrap();
    let cc = fs.curve.lock().unwrap();
    cc.points.iter().map(|p| {
      let p = p.lock().unwrap();
      (p.0, p.1)
    }).collect()
  }
  fn update_curve(&self, new_curve: Vec<(i32, u32)>) -> Result<(), anyhow::Error> {
    let curve_clone = self.curve().clone();
    let mut curve_lock = curve_clone.lock().unwrap();
    let curve = &mut *curve_lock;
    let new_curve = new_curve.try_into()?;
    *curve = new_curve;
    Ok(())
  }
  fn update_temp_node_with_new_speed(&self, temp: i32, speed: u32) -> Result<(), anyhow::Error> {
    let fscc = self.curve();
    let curve_lock = fscc.lock().unwrap();
    let curve = & *curve_lock;
    for pnt in curve.points.iter() {
      let mut p = pnt.lock().unwrap();
      if temp != p.0 { continue; }
      p.1 = speed;
      return Ok(());
    }
    Err(anyhow::anyhow!("Temp {}C not found in fan curve UwU", temp))
  }
}

#[derive(Debug, Clone, Copy)]
struct TempSpeed(i32,u32);
impl TempSpeed {
  fn temp(&self) -> i32 { self.0 }
  fn speed(&self) -> u32 { self.1 }
  #[allow(dead_code)]
  fn update_temp(&mut self, temp: i32) { self.0 = temp; }
  #[allow(dead_code)]
  fn update_speed(&mut self, speed: u32) { self.1 = speed; }
}
impl Default for TempSpeed {
  fn default() -> Self { TempSpeed(5,0) }
}
impl From<TempSpeed> for (i32,u32) {
  fn from(value: TempSpeed) -> Self {
    (value.0, value.1)
  }
}
impl From<(i32,u32)> for TempSpeed {
  fn from(value: (i32,u32)) -> Self {
    TempSpeed(value.0, value.1)
  }
}
struct TempSpeedArcMutex(Arc<Mutex<TempSpeed>>);
impl TryFrom<(i32,u32)> for TempSpeedArcMutex {
  type Error = anyhow::Error;
  fn try_from(value: (i32,u32)) -> Result<Self, Self::Error> {
    if !(5..=95).contains(&value.0) {
      Err(anyhow!("Temperature must be between 5C and 95C"))
    } else if !(0..=100).contains(&value.1) {
      Err(anyhow!("Fan speed must be between 0% and 100%"))
    } else {
      Ok(TempSpeedArcMutex(Arc::new(Mutex::new(TempSpeed(value.0, value.1)))))
    }
  }
}

pub struct FanCurveUwU { // For the theme. I'm sorry.
  points: Vec<Arc<Mutex<TempSpeed>>>,
}
#[allow(dead_code)]
impl FanCurveUwU {
  fn new() -> Self { Self{ points: Vec::new() } }
  fn add(&mut self, temp: i32, speed: u32) -> Result<(), Box<dyn Error>> {
    let ts: TempSpeedArcMutex = (temp,speed).try_into()?;
    if self.points.is_empty() { self.points.push(ts.0); return Ok(()) }
    for i in 0..self.points.len() {
      if let Ok(temp_speed) = self.points[i].clone().lock().as_mut() {
        if temp > temp_speed.temp() { continue }
        if temp_speed.temp() == temp {
          temp_speed.update_speed(speed); return Ok(())
        }
        if temp < temp_speed.temp() {
          self.points.insert(i, ts.0);
          return Ok(());
        }
      }
    }
    self.points.push(ts.0);
    Ok(())
  }
}
impl From<&FanCurveUwU> for Vec<(i32,u32)> {
  fn from(curve: &FanCurveUwU) -> Self {
    curve.points.iter().map(|ts| {
      let ts = ts.lock().unwrap();
      (ts.temp(), ts.speed())
    }).collect()
  }
}
impl TryFrom<Vec<(i32,u32)>> for FanCurveUwU {
  type Error = anyhow::Error;
  fn try_from(list: Vec<(i32,u32)>) -> Result<Self, Self::Error> {
    let mut points = Vec::new();
    for tmpspd in list {
      let ts: TempSpeedArcMutex = tmpspd.try_into()?;
      points.push(ts.0);
    }
    Ok( Self { points } )
  }
}
