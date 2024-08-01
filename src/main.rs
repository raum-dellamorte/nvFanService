use {
  nvml_wrapper::{
    device::Device,
    enum_wrappers::device::TemperatureSensor,
    Nvml
  },
  regex::Regex,
  std::{
    error::Error,
    ffi::OsStr,
    fs::read_to_string,
    io,
    path::Path,
    sync::mpsc,
    sync::mpsc::{
      Receiver, TryRecvError,
    },
    thread,
    time::Instant,
  },
  sudo,
};

fn main() -> Result<(), Box<dyn Error>> {
  sudo::escalate_if_needed()?;
  let drv_ver = get_driver_version();
  let libname = if drv_ver.is_empty() {
    "libnvidia-ml.so".to_owned()
  } else {
    format!("libnvidia-ml.so.{}", drv_ver)
  };
  println!("Attempting to use {} as our NVML Library.", libname);
  let init_result = Nvml::builder().lib_path(OsStr::new(&libname)).init();
  if let Ok(nvml) = init_result {
    let mut fan_curve_uwu = FanCurveUwU::new(); // Still hard coded, but we are getting there.
    fan_curve_uwu.add(10,  0)?;
    fan_curve_uwu.add(20, 30)?;
    fan_curve_uwu.add(30, 60)?;
    fan_curve_uwu.add(36, 70)?;
    fan_curve_uwu.add(40, 80)?;
    fan_curve_uwu.add(52, 90)?;
    fan_curve_uwu.add(58,100)?;
    let fan_curve_uwu = fan_curve_uwu; // lock it down... for reasons.
    let device_count = nvml.device_count();
    if device_count.is_ok() {
      let device_count = device_count.unwrap();
      if device_count == 1 {
        println!("Found one nVidia GPU.");
        if let Ok(device) = nvml.device_by_index(0) {
          let name = device.name().unwrap_or("<Unable to get device name>".to_owned());
          println!("~> {}", &name);
          return fan_service(device, fan_curve_uwu);
        }
      } else if device_count > 1 {
        println!("Found {} nVidia devices.\nPlease choose one:", device_count);
        // List found nVidia devices
      } else {
        println!("NVML loaded but no nVidia GPUs found.");
      }
    }
  } else if let Err(e) = init_result {
    println!("Failed to load NVML.\n~> {}", e);
  }
  Ok(())
}

fn fan_service(device: Device, fan_curve_uwu: FanCurveUwU) -> Result<(), Box<dyn Error>> {
  let name = device.name()?;
  println!("Found {} fan(s) on {}.", device.num_fans()?, name);
  service_fans(&device, &fan_curve_uwu)?;
  let mut last_checked = Instant::now();
  println!("Temp: {}, Fan Speed: {}",
    device.temperature(TemperatureSensor::Gpu)?, device.fan_speed(0)?);
  let stdin_channel = spawn_stdin_channel(); // Thanks, Stack Overflow!
  'run_loop: loop {
    match stdin_channel.try_recv() { // I hate this because I don't want to have to hit enter.
      Ok(key) => {
        println!("Shutting down...");
        if key.starts_with("q") { break 'run_loop; }
      }
      Err(TryRecvError::Empty) => {}
      Err(TryRecvError::Disconnected) => {
        println!("Channel disconnected");
        break 'run_loop;
      }
    }
    if last_checked.elapsed().as_secs() >= 10 {
      last_checked = Instant::now();
      // It's been 10s, do you know what your temperature is?
      service_fans(&device, &fan_curve_uwu)?;
    }
  }
  Ok(())
}

fn service_fans(device: &Device, fan_curve_uwu: &FanCurveUwU) -> Result<(), Box<dyn Error>> {
  // if the 1st GPU is not the one we want to control, can we TemperatureSensor::Gpu + 1 ??? 
  let Ok(idx) = device.index() else { return Err("Failed to get index from device in service_fans()")? };
  let Ok(fan_count) = device.num_fans() else { return Err("Failed to get num_fans from device in service_fans()")? };
  let Ok(gpu_idx) = TemperatureSensor::try_from(idx) else { return Err("Failed to convert device index to TemperatureSensor enum in service_fans()")? };
  let Ok(temp) = device.temperature(gpu_idx) else { return Err("Failed to get temperature reading from device in service_fans()")? };
  for ts in &fan_curve_uwu.points {
    if temp >= ts.temp() {
      for idx in 0..fan_count {
        if device.fan_speed(idx)? != ts.speed() {
          device.set_fan_speed(idx, ts.speed())?;
        }
      }
      println!("Temp: {}, Fan Speed: {}", temp, ts.speed());
      break;
    }
  }
  Ok(())
}

fn get_driver_version() -> String {
  let file = Path::new("/proc/driver/nvidia/version");
  if let Ok(true) = Path::try_exists(&file) {
    let drv_ver_info = read_to_string(file).unwrap();
    let re: Regex = Regex::new(r"(?m)Kernel Module +(\d+\.\d+\.\d+)").unwrap();
    let captures = re.captures(&drv_ver_info);
    if let Some(res) = captures {
      if res.len() > 1 {
        let ver = res[1].to_owned();
        println!("Found nVidia driver version: {}", ver);
        return ver;
      }
    } else {
      println!("/proc/driver/nvidia/version exists but finding driver triple failed.");
      println!("Proc results:\n{}", drv_ver_info);
      println!("Regex:\n{:?}\nresults:\n{:?}", re, captures);
    }
  } else {
    println!("/proc/driver/nvidia/version not found.")
  }
  return String::default()
}

fn spawn_stdin_channel() -> Receiver<String> {
  // https://stackoverflow.com/questions/30012995/how-can-i-read-non-blocking-from-stdin
  let (tx, rx) = mpsc::channel::<String>();
  thread::spawn(move || loop {
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    tx.send(buffer).unwrap();
  });
  rx
}

struct TempSpeed(u32,u32);
impl TempSpeed {
  fn temp(&self) -> u32 { self.0 }
  fn speed(&self) -> u32 { self.1 }
  fn update_speed(&mut self, speed: u32) { self.1 = speed; }
}
impl TryFrom<(u32,u32)> for TempSpeed {
  type Error = &'static str;
  fn try_from(value: (u32,u32)) -> Result<Self, Self::Error> {
    if !(5..=95).contains(&value.0) {
      Err("Temperature must be between 5C and 95C")
    } else if !(0..=100).contains(&value.1) {
      Err("Fan speed must be between 0% and 100%")
    } else {
      Ok(Self(value.0, value.1))
    }
  }
}

struct FanCurveUwU { // For the theme. I'm sorry.
  points: Vec<TempSpeed>,
}
impl FanCurveUwU {
  fn new() -> Self { Self{ points: Vec::new() } }
  fn add(&mut self, temp: u32, speed: u32) -> Result<(), Box<dyn Error>> {
    let ts: TempSpeed = (temp,speed).try_into()?;
    if self.points.is_empty() { self.points.push(ts); return Ok(()) }
    for i in 0..self.points.len() {
      if self.points[i].temp() > temp { continue }
      if self.points[i].temp() < temp {
        if i + 1 == self.points.len() {
          self.points.push(ts); return Ok(())
        } else {
          self.points.insert(i, ts); return Ok(())
        }
      }
      if self.points[i].temp() == temp { self.points[i].update_speed(speed); }
    }
    self.points.push(ts);
    Ok(())
  }
}
