#![allow(unused_braces)]
use {
  arrayvec::ArrayString,
  // cursive::{
  //   event::Event,
  //   theme::Theme,
  //   views::{
  //     HideableView, LinearLayout, NamedView, OnEventView,
  //     Panel, TextContent, TextView,
  //   },
  //   Cursive,
  //   CursiveExt,
  //   // XY,
  // },
  // cursive_core::style::{
  //   BaseColor::*, Color::*, PaletteColor::*,
  // },
  nvml_wrapper::{
    device::Device, enum_wrappers::device::TemperatureSensor, error::NvmlError, Nvml,
  },
  regex::Regex,
  std::{
    error::Error,
    ffi::OsStr,
    fs::read_to_string,
    path::Path,
    sync::{
      Arc, Mutex,
    },
    time::{
      Duration,
      Instant,
    },
  },
  sdl3::{
    rect::Rect,
    // render::Texture,
    pixels::Color,
    event::Event,
    keyboard::Keycode,
  },
  crate::{
    // cursive_custom::FanCurveUnitView,
    elevate::elevate_if_needed,
  },
};

#[macro_use]
extern crate log;

// mod cursive_custom;
mod elevate;

// Settings
const DRY_RUN:bool = false; // Change me to a command line parameter like `--dry-run`
// Colors
const BG : Color = Color::RGBA(0, 0, 0, 255);
const PRIMARY : Color = Color::RGBA(0, 200, 0, 255);
const SLIDER_BG : Color = Color::RGBA(15, 25, 65, 255);
const SLIDER_TRACK : Color = Color::RGBA(30, 10, 10, 255);
const SLIDER_KNOB : Color = Color::RGBA(0, 100, 60, 255);
const SLIDER_TEMP_LABEL : Color = Color::RGBA(0, 60, 120, 255);

fn main() -> Result<(), Box<dyn Error>> {
  elevate_if_needed()?;
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
  let mut fan_service = FanService {
    nvml, card_idx: None, card_name: ArrayString::new(),
    curve: curve.clone(), instant: Instant::now(), first_time: FirstTime(true),
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
  // let content = TextContent::new("  Temp: ??C, Fan Speed: ???%  ");
  
  if let Ok(session) = ::std::env::var("XDG_SESSION_TYPE") {
    if session == "wayland" {
      ::std::env::set_var("SDL_VIDEODRIVER", "wayland");
    }
  }
  let sdl_context = sdl3::init().unwrap();
  let ttf_context = sdl3::ttf::init().unwrap();
  let fira = ttf_context.load_font("/usr/share/fonts/TTF/FiraCodeNerdFontMono-Regular.ttf", 26.0)
    .expect("Couldn't load FiraCodeNerdFontMono Regular TTF");
  let video_subsystem = sdl_context.video().unwrap();
  let window = video_subsystem
    .window("nvFanService UwU", 500, 400)
    .position_centered()
    .resizable()
    .build()
    .unwrap();
  let mut canvas = window.into_canvas();
  let texture_creator = canvas.texture_creator();
  let mut event_pump = sdl_context.event_pump().unwrap();
  
  canvas.set_draw_color(Color::RGB(0, 255, 255));
  canvas.clear();
  canvas.present();
  let mut i = 0; // For controlling when timed_service_service is run.
  timed_service_service(&mut fan_service);
  let txt_gfx_card = fira.render(&name).blended(PRIMARY).unwrap();
  let txt_gfx_card_half_w: i32 = (txt_gfx_card.width() / 2) as i32;
  let txt_region_gfx_card = Rect::new(
    0,0,txt_gfx_card.width(),txt_gfx_card.height()
  );
  let mut txt_speed_and_temp = fira.render(&fan_service.text).blended(PRIMARY).unwrap();
  let txt_speed_and_temp_half_w: i32 = (txt_speed_and_temp.width() / 2) as i32;
  let txt_region_speed_and_temp = Rect::new(
    0,0,txt_speed_and_temp.width(),txt_speed_and_temp.height()
  );
  'running: loop {
    // Check Events!
    for event in event_pump.poll_iter() {
      match event {
        Event::Quit {..} |
        Event::KeyDown { keycode: Some(Keycode::Escape), .. }
        | Event::KeyDown { keycode: Some(Keycode::Q), .. } => {
          break 'running
        },
        _ => {}
      }
    }
    // Update!
    i = (i + 1) % 10;
    if i == 0 {
      timed_service_service(&mut fan_service);
      txt_speed_and_temp = fira.render(&fan_service.text).blended(PRIMARY).unwrap();
    }
    let tex_speed_and_temp = txt_speed_and_temp.as_texture(&texture_creator).unwrap();
    let tex_gfx_card = txt_gfx_card.as_texture(&texture_creator).unwrap();
    let canvas_rect = canvas.viewport();
    let win_half_w: i32 = (canvas_rect.width() / 2) as i32;
    let x_gfx_card = win_half_w - txt_gfx_card_half_w;
    let x_speed_and_temp: i32 = win_half_w - txt_speed_and_temp_half_w;
    let draw_pos_gfx_card = Rect::new(
      x_gfx_card,20,txt_gfx_card.width(),txt_gfx_card.height()
    );
    let draw_pos_speed_and_temp = Rect::new(
      x_speed_and_temp,40 + txt_gfx_card.height() as i32,
      txt_speed_and_temp.width(),txt_speed_and_temp.height()
    );
    let header_height = 60 + txt_gfx_card.height() + txt_speed_and_temp.height();
    let draw_pos_slider_box = Rect::new(
      10, header_height as i32,
      canvas_rect.width() - 20, canvas_rect.height() - header_height - 10,
    );
    let mut sliders = Vec::new();
    {
      if let Ok(curve) = fan_service.curve.clone().lock() {
        // for each point in the curve ake Rects to draw sliders
        let count = curve.points.len() as u32;
        let w = draw_pos_slider_box.width() - 10;
        let x_offset = draw_pos_slider_box.x() + 5 + (w / count / 2) as i32;
        for i in 0..count {
          let slider_track = Rect::new(
            x_offset + (i * w / count) as i32 - 4_i32,
            draw_pos_slider_box.y() + 5,
            8, draw_pos_slider_box.height() - 40,
          );
          let slider_label = Rect::new(
            x_offset + (i * w / count) as i32 - 30_i32,
            draw_pos_slider_box.y() + (draw_pos_slider_box.height() - 35) as i32,
            60, 30,
          );
          if let Ok(ts) = curve.points[i as usize].lock() {
            let speed = ts.speed();
            let temp = ts.temp();
            let spd_txt = format!("{:>3}%", speed);
            let tmp_txt = format!("{:>3}C", temp);
            let x = slider_track.x() - 26;
            let y = slider_track.y() + (
              (slider_track.height() as f32 - 30.0) / 100.0 * (100.0 - speed as f32)
            ) as i32;
            let slider_knob = Rect::new(x, y, 60, 30);
            let tex_spd = fira.render(&spd_txt).blended(PRIMARY)
              .unwrap()
              .as_texture(&texture_creator)
              .unwrap();
            let tex_temp = fira.render(&tmp_txt).blended(PRIMARY)
              .unwrap()
              .as_texture(&texture_creator)
              .unwrap();
            sliders.push((slider_track, slider_knob, slider_label, tex_temp, tex_spd));
          }
        }
      }
      0
    };
    // Draw!
    canvas.set_draw_color(BG);
    canvas.clear();
    canvas.copy(&tex_gfx_card, txt_region_gfx_card, draw_pos_gfx_card).unwrap();
    canvas.copy(&tex_speed_and_temp, txt_region_speed_and_temp, draw_pos_speed_and_temp).unwrap();
    canvas.set_draw_color(SLIDER_BG);
    canvas.fill_rect(draw_pos_slider_box).unwrap();
    canvas.set_draw_color(SLIDER_TRACK);
    for (slider_track, _, _, _, _) in &sliders {
      canvas.fill_rect(*slider_track).unwrap();
    }
    canvas.set_draw_color(SLIDER_TEMP_LABEL);
    for (_, _, slider_label, tex_tmp, _) in &sliders {
      canvas.fill_rect(*slider_label).unwrap();
      canvas.copy(tex_tmp, None, *slider_label).unwrap();
    }
    canvas.set_draw_color(SLIDER_KNOB);
    for (_, slider_knob, _, _, tex_spd) in &sliders {
      canvas.fill_rect(*slider_knob).unwrap();
      canvas.copy(tex_spd, None, *slider_knob).unwrap();
    }
    // Present result!
    canvas.present();
    ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
  }
  end_service_service(&mut fan_service);
  Ok(())
}

fn timed_service_service(fs: &mut FanService) {
  if fs.first_time.0 { // We don't want to wait 10 secs for our first service
    fs.first_time.0 = false;
    fs.service_service().unwrap();
    return;
  }
  if fs.instant.elapsed().as_secs() >= 10 {
    fs.service_service().unwrap();
    fs.instant = Instant::now();
  }
}

fn end_service_service(fs: &mut FanService) {
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

// fn refresh_callback(siv: &mut Cursive, fan_info_text: TextContent) {
//   let (_, height) = siv.screen_size().pair();
//   if let Some(hv) = siv.find_name::<HideableView<Panel<LinearLayout>>>("SlidersHideable").as_mut() {
//     if height > 17 { // Change me to a constant
//       hv.unhide();
//     } else {
//       hv.hide();
//     }
//   }
//   siv.with_user_data(timed_service_service);
//   let txt: String = siv.user_data::<FanService>().unwrap().text.clone();
//   if txt.len() > 0 { // This is probably not necessary, but neither was using Cursive
//     fan_info_text.set_content(&txt);
//   } else {
//     //                        "  Temp: __C, Fan Speed: ___%  " // Making sure error message is the same size
//     fan_info_text.set_content("FanService Fail: Empty String.");
//   }
// }

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
  // fn set_card_id(&mut self, idx: u32) { self.card_idx = Some(idx); }
  
  fn service_service(&mut self) -> Result<(), Box<dyn Error>> {
    // if the 1st GPU is not the one we want to control, can we TemperatureSensor::Gpu + 1 ???
    let Ok(fan_count) = self.device()?.num_fans() else { return Err("Failed to get num_fans from device in service_fans()")? };
    let Ok(gpu_idx) = TemperatureSensor::try_from(self.card_idx.unwrap()) else { return Err("Failed to convert device index to TemperatureSensor enum in service_fans()")? };
    let Ok(temp) = self.device()?.temperature(gpu_idx) else { return Err("Failed to get temperature reading from device in service_fans()")? };
    #[allow(unused_mut)]
    if let (Ok(curve), Ok(mut device)) = (self.curve.clone().lock(), self.device()) {
      let n: usize = curve.points.len();
      for ts in (0..n).rev() {
        if let Ok(temp_speed) = curve.points[ts].clone().lock() {
          if temp as i32 >= temp_speed.temp() {
            for idx in 0..fan_count {
              if device.fan_speed(idx)? != temp_speed.speed() {
                let spd: u32 = temp_speed.speed();
                if !DRY_RUN { device.set_fan_speed(idx, spd)?; }
              }
            }
            self.text = format!("  Temp: {:>2}C, Fan Speed: {:>3}%  ", temp, temp_speed.speed());
            return Ok(());
          }
        }
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
  return init_result
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

struct FanCurveUwU { // For the theme. I'm sorry.
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
  // fn fan_curve_view(&self) -> Panel<LinearLayout> {
  //   let mut ll = LinearLayout::horizontal();
  //   if self.points.is_empty() { return Panel::new(LinearLayout::horizontal()) }
  //   for i in 0..self.points.len() {
  //     let temp_speed_clone = self.points[i].clone();
  //     if let Ok(temp_speed) = self.points[i].lock() {
  //       ll.add_child(
  //         FanCurveUnitView::new(temp_speed.temp(),temp_speed.speed())
  //           .on_change(move |_, slider_temp, slider_speed| {
  //             if let Ok(temp_speed) = temp_speed_clone.lock().as_mut() {
  //               if temp_speed.temp() != slider_temp {
  //                 temp_speed.update_temp(slider_temp);
  //               }
  //               if temp_speed.speed() != slider_speed {
  //                 temp_speed.update_speed(slider_speed);
  //               }
  //             }
  //           })
  //       )
  //     }
  //   }
  //   Panel::new(ll)
  // }
}

#[derive(Clone, Copy)]
struct FirstTime(bool);
