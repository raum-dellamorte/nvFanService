#![allow(unused_braces)]
use {
  crate::{
    cursive_frontend::nvfs_cursive_frontend,
    elevate::{
      elevate_if_needed,
      running_unpriviledged,
    },
    sdl3_frontend::nvfs_sdl3_frontend,
  },
  arrayvec::ArrayString,
  nvml_wrapper::{
    device::Device,
    enum_wrappers::device::TemperatureSensor,
    error::NvmlError,
    Nvml,
  },
  regex::Regex,
  std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::read_to_string,
    io::Write,
    path::Path,
    sync::{ Arc, Mutex, },
    time::Instant,
  },
};

#[macro_use]
extern crate log;

// mod cursive_custom;
mod cursive_frontend;
mod elevate;
mod sdl3_frontend;

// Settings
const DRY_RUN:bool = false; // Change me to a command line parameter like `--dry-run`

fn main() -> Result<(), Box<dyn Error>> {
  if running_unpriviledged() {
    // setup/check user config
    // todo!("Implement user configuration setup")
  }
  elevate_if_needed()?;
  let mut fan_service = FanService::new()?;
  let card_name = { 
    // Once we get the card name, we want to reuse it elsewhere.
    // If card_idx is None, as it is before we get here,
    // running fan_service.device()? picks the nVidia card
    // we're going to use and fills in fan_service.card_name
    // and .card_idx to the name and index of the chosen card.
    let _ = fan_service.device()?;
    fan_service.card_name.as_str().to_owned()
  };
  let fan_service = Arc::new(Mutex::new(fan_service));
  timed_service_service(fan_service.clone());
  let args: Vec<String> = env::args().collect();
  let res = if args.len() == 2 && &args[1] == "cli" {
    nvfs_cursive_frontend(&card_name, fan_service.clone())
  } else {
    nvfs_sdl3_frontend(&card_name, fan_service.clone())
  };
  end_service_service(fan_service);
  res
}

fn timed_service_service(fs: Arc<Mutex<FanService>>) {
  if let Ok(fs) = fs.lock().as_mut() {
    if fs.first_time.0 { // We don't want to wait 10 secs for our first service
      fs.first_time.0 = false;
      if let Err(_e) = fs.service_service() {}; // ignore the error, most likely we are running unprivileged
      return;
    }
    if fs.instant.elapsed().as_secs() >= 10 {
      if let Err(_e) = fs.service_service() {}; // ignore the error, most likely we are running unprivileged
      fs.instant = Instant::now();
    }
  }
}

fn end_service_service(fs: Arc<Mutex<FanService>>) {
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

struct FanService {
  nvml: Nvml,
  card_idx: Option<u32>,
  card_name: ArrayString<32>,
  curve: Arc<Mutex<FanCurveUwU>>,
  instant: Instant,
  first_time: FirstTime,
  text: String,
}
impl FanService {
  fn new() -> Result<FanService, Box<dyn Error>> {
    let nvml = init_nvml_so()?;
    let mut curve = FanCurveUwU::new();
    curve.add(10,  0)?;
    curve.add(20, 30)?;
    curve.add(30, 60)?;
    curve.add(36, 70)?;
    curve.add(40, 80)?;
    curve.add(52, 90)?;
    curve.add(58,100)?;
    let curve = Arc::new(Mutex::new(curve));
    Ok(FanService {
      nvml, card_idx: None, card_name: ArrayString::new(),
      curve: curve.clone(), instant: Instant::now(), first_time: FirstTime(true),
      text: "".to_owned(),
    })
  }
  
  // fn set_card_id(&mut self, idx: u32) { self.card_idx = Some(idx); }
  
  fn service_service(&mut self) -> Result<(), Box<dyn Error>> {
    // if the 1st GPU is not the one we want to control, can we TemperatureSensor::Gpu + 1 ???
    let gpu_idx = if self.device().is_ok() { 
      let Ok(gpu_idx) = TemperatureSensor::try_from(self.card_idx.unwrap()) else {
        return Err("Failed to convert device index to TemperatureSensor enum in service_fans()")?
      };
      gpu_idx.clone()
    } else { return Err("service_service: Failed to get device.")? };
    
    let (fan_count, temp) = if let Ok(device) = &self.device() {
      let Ok(fan_count) = device.num_fans() else {
        return Err("Failed to get num_fans from device in service_fans()")?
      };
      let Ok(temp) = device.temperature(gpu_idx) else {
        return Err("Failed to get temperature reading from device in service_fans()")?
      };
      // let gpu_idx = gpu_idx.cl
      (fan_count, temp)
    } else { return Err("This should be unreachable")? };
    
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
                  self.text = format!("  Temp: {:>2}C, Fan Speed: {:>3}%  ", temp, temp_speed.speed());
                  return Err(format!("Can't set fan speed: {}", e))?
                }
              }
            }
          }
          self.text = format!("  Temp: {:>2}C, Fan Speed: {:>3}%  ", temp, speed);
          return Ok(());
        }
      }
    }
    
    Err("Nothing happened, I swear!")?
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

pub trait FanServiceArcMutex {
  fn text(&self) -> String;
  fn curve(&self) -> Arc<Mutex<FanCurveUwU>>;
}

impl FanServiceArcMutex for Arc<Mutex<FanService>> {
  fn text(&self) -> String {
    if let Ok(fs) = self.lock() {
      fs.text.to_owned()
    } else {
      "FanServiceArcMutex.text() Fail".to_owned()
    }
  }
  fn curve(&self) -> Arc<Mutex<FanCurveUwU>> {
    self.lock().unwrap().curve.clone()
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

struct TempSpeed(i32,u32);
impl TempSpeed {
  fn temp(&self) -> i32 { self.0 }
  fn speed(&self) -> u32 { self.1 }
  #[allow(dead_code)]
  fn update_temp(&mut self, temp: i32) { self.0 = temp; }
  #[allow(dead_code)]
  fn update_speed(&mut self, speed: u32) { self.1 = speed; }
}
impl TryFrom<(i32,u32)> for TempSpeed {
  type Error = &'static str;
  fn try_from(value: (i32,u32)) -> Result<Self, Self::Error> {
    if !(5..=95).contains(&value.0) {
      Err("Temperature must be between 5C and 95C")
    } else if !(0..=100).contains(&value.1) {
      Err("Fan speed must be between 0% and 100%")
    } else {
      Ok(Self(value.0, value.1))
    }
  }
}

pub struct FanCurveUwU { // For the theme. I'm sorry.
  points: Vec<Arc<Mutex<TempSpeed>>>,
}
impl FanCurveUwU {
  fn new() -> Self { Self{ points: Vec::new() } }
  fn add(&mut self, temp: i32, speed: u32) -> Result<(), Box<dyn Error>> {
    let ts: TempSpeed = (temp,speed).try_into()?;
    let ts = Arc::new(Mutex::new(ts));
    if self.points.is_empty() { self.points.push(ts); return Ok(()) }
    for i in 0..self.points.len() {
      if let Ok(temp_speed) = self.points[i].clone().lock().as_mut() {
        if temp > temp_speed.temp() { continue }
        if temp_speed.temp() == temp {
          temp_speed.update_speed(speed); return Ok(())
        }
        if temp < temp_speed.temp() {
          if i + 1 == self.points.len() {
            self.points.push(ts); return Ok(())
          } else {
            self.points.insert(i, ts); return Ok(())
          }
        }
      }
    }
    self.points.push(ts);
    Ok(())
  }
}

#[derive(Clone, Copy)]
struct FirstTime(bool);
