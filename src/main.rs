use {
  arrayvec::ArrayString,
  cursive_core::style::{
    BaseColor::*, Color::*, PaletteColor::*,
  },
  cursive::{
    event::Event,
    theme::Theme,
    views::{
      OnEventView, Panel, TextContent, TextView,
    },
    Cursive, CursiveExt, 
  },
  nvml_wrapper::{
    device::Device, enum_wrappers::device::TemperatureSensor, error::NvmlError, Nvml
  },
  regex::Regex,
  std::{
    error::Error,
    ffi::OsStr,
    fs::read_to_string,
    path::Path,
    time::Instant,
  },
  sudo,
};

fn main() -> Result<(), Box<dyn Error>> {
  sudo::escalate_if_needed()?;
  let nvml = init_nvml_so()?;
  let mut curve = FanCurveUwU::new(); // Still hard coded, but we are getting there.
  curve.add(10,  0)?;
  curve.add(20, 30)?;
  curve.add(30, 60)?;
  curve.add(36, 70)?;
  curve.add(40, 80)?;
  curve.add(52, 90)?;
  curve.add(58,100)?;
  let mut fan_service = FanService {
    nvml, card_idx: None, card_name: ArrayString::new(),
    curve, instant: Instant::now(), first_time: FirstTime(true),
    text: "".to_owned(),
  };
  let name = { // Once we get the card name, we want to reuse it elsewhere.
    // If card_idx is None, as it is before we get here,
    // running fan_service.device()? picks the nVidia card
    // we're going to use and fills in fan_service.card_name
    // and .card_idx to the name and index of the chosen card.
    let _ = fan_service.device()?;
    fan_service.card_name.as_str().to_owned()
  };
  let mut siv = Cursive::new();
  let content = TextContent::new("  Temp: ??, Fan Speed: ???  ");
  let content_c = content.clone();
  siv.set_user_data(fan_service);
  siv.with_theme(|theme: &mut Theme| { // One day, this could be customized.
    theme.palette[Background] = Dark(Black);
    theme.palette[Shadow] = Rgb(30, 0, 0);
    theme.palette[View] = Rgb(15, 25, 65);
    theme.palette[Primary] = Rgb(0, 200, 0);
    theme.palette[TitlePrimary] = Rgb(0, 100, 0);
  });
  siv.add_layer(Panel::new(
    // I struggled for 2 nights after work trying to get a game like event loop running without
    // using a whole bloody game engine. I just wanted to check the fans every 10 secs and display
    // the temp and fan speed. I started with a global callback on Event::Refresh but it did not
    // seem to be executing the code. Finding OnEventView(content).on_event(Event::Refresh, ...)
    // was the first step. I had to put everything I might need into a single struct in order to
    // do what must be done from within the closure used by on_event(). I started off trying to
    // get the Device from Nvml and put it in the struct, but the Borrow Checker said no. I think
    // it's because Device has &Nvml in its struct, which means it's pointing back to the original
    // Nvml struct from whence it came.  So I had to reason with myself... I was trying to not have
    // to call nvml.device(idx) every time I wanted access to the device. I wanted to just hang onto
    // my Device that required me to "carefully" unwrap a Result to get. I was being ridiculous.
    // I'm checking the temp every 10 seconds and getting the Device by index does not take some
    // absurd amount of time. Going from nothing to having the temp known and the fan speed set
    // according to that temp feels pretty instantaneous regardless of how many question marks and
    // unwraps seem to be in the way. Those are just my promises to the compiler that everything
    // is going to be fine. trust_me!(sus_code)
    OnEventView::new(
      TextView::new_with_content(content.clone())
    ).on_event(Event::Refresh, move |s| {
      s.with_user_data(|fs: &mut FanService| {
        if fs.first_time.0 { // We don't want to wait 10 secs for our first service
          fs.first_time.0 = false;
          fs.service_service().unwrap();
          return;
        }
        if fs.instant.elapsed().as_secs() >= 10 {
          fs.service_service().unwrap();
          fs.instant = Instant::now();
        }
      });
      let txt: String = s.user_data::<FanService>().unwrap().text.clone();
      if txt.len() > 0 { // This is probably not necessary, but neither was using Cursive
        content_c.set_content(&txt);
      } else {
        content_c.set_content("  Temp: ??C, Fan Speed: ???%  ");
      }
    })
  ).title(name));
  siv.add_global_callback('q', |s| s.quit());
  siv.set_fps(10);
  siv.set_autorefresh(true);
  siv.run();
  Ok(())
}

struct FanService {
  nvml: Nvml,
  card_idx: Option<u32>,
  card_name: ArrayString<32>,
  curve: FanCurveUwU,
  instant: Instant,
  first_time: FirstTime,
  text: String,
}
impl FanService {
  // fn set_card_id(&mut self, idx: u32) { self.card_idx = Some(idx); }
  
  fn service_service(&mut self) -> Result<(), Box<dyn Error>> {
    // if the 1st GPU is not the one we want to control, can we TemperatureSensor::Gpu + 1 ???
    let Ok(fan_count) = self.device()?.num_fans() else { return Err("Failed to get num_fans from device in service_fans()")? };
    let Ok(gpu_idx) = TemperatureSensor::try_from(self.card_idx.unwrap()) else { return Err("Failed to convert device index to TemperatureSensor enum in service_fans()")? };
    let Ok(temp) = self.device()?.temperature(gpu_idx) else { return Err("Failed to get temperature reading from device in service_fans()")? };
    let n: usize = self.curve.points.len();
    for ts in 0..n {
      if temp >= self.curve.points[ts].temp() {
        for idx in 0..fan_count {
          if self.device()?.fan_speed(idx)? != self.curve.points[ts].speed() {
            let spd: u32 = self.curve.points[ts].speed();
            self.device()?.set_fan_speed(idx, spd)?;
          }
        }
        self.text = format!("  Temp: {}C, Fan Speed: {:>3}%  ", temp, self.curve.points[ts].speed());
        return Ok(());
      }
    }
    Err("Nothing happened, I swear!")?
  }
  
  fn device(&mut self) -> Result<Device, NvmlError> {
    if self.card_idx.is_some() { return self.nvml.device_by_index(self.card_idx.unwrap()) }
    let device_count = self.nvml.device_count().unwrap_or(0);
    if device_count == 0 { return Err(NvmlError::NotFound) }
    if device_count == 1 {
      println!("Found one nVidia GPU.");
      let device = self.nvml.device_by_index(0);
      if device.is_ok() {
        let name = device.as_ref().unwrap().name().unwrap_or("<Unable to get device name>".to_owned());
        self.card_name.push_str(&name);
        self.card_idx = Some(0);
        println!("~> {}", &name);
      }
      return device;
    } else if device_count > 1 {
      println!("Found {} nVidia devices.\nPlease choose one:", device_count);
      let mut devices = Vec::new();
      for i in 0..device_count {
        let device = self.nvml.device_by_index(i);
        if device.is_ok() {
          let name = device.as_ref().unwrap().name().unwrap_or("<Unable to get device name>".to_owned());
          self.card_name.push_str(&name);
          println!("{} ~> {}", i + 1, &name);
          devices.push((i, name));
        }
      }
      // fixme: defaulting to the first so I don't have to write a user prompt right now
      println!("Picking a card not yet implemented.\nUsing first available card.");
      if devices.len() > 0 {
        self.card_idx = Some(devices[0].0);
        self.card_name.push_str(&devices[0].1);
        return self.nvml.device_by_index(0);
      }
      return Err(NvmlError::NotFound);
    }
    return Err(NvmlError::NotFound);
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
  if let Ok(true) = Path::try_exists(&file) {
    let drv_ver_info = read_to_string(file).unwrap();
    let re: Regex = Regex::new(r"(?m)Kernel Module +(\d+\.\d+\.\d+)").unwrap();
    let captures = re.captures(&drv_ver_info);
    if let Some(res) = captures {
      if res.len() > 1 {
        let ver = res[1].to_owned();
        println!("Found nVidia driver version: {}", ver);
        let libname = format!("libnvidia-ml.so.{}", ver);
        println!("Initializing with {}", &libname);
        let init_result = Nvml::builder().lib_path(OsStr::new(&libname)).init();
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
  println!("Attempting to use {} as our NVML Library.", libname);
  let init_result = Nvml::builder().lib_path(OsStr::new(&libname)).init();
  return init_result
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

#[derive(Clone, Copy)]
struct FirstTime(bool);
