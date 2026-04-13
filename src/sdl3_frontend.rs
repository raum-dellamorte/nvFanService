#![allow(unused_braces)]
use {
  crate::{
    client::FanServiceClient,
  },
  image::{
    load_from_memory_with_format,
    ImageFormat,
  },
  sdl3::{
    event::Event,
    keyboard::Keycode,
    pixels::Color,
    rect::Rect,
    surface::Surface,
  },
  sdl3_sys::pixels::SDL_PixelFormat,
  std::time::{
    Duration,
    Instant,
  },
};

// Colors
const BG : Color = Color::RGBA(0, 0, 0, 255);
const PRIMARY : Color = Color::RGBA(0, 200, 0, 255);
const SHADOW_TXT : Color = Color::RGBA(150, 0, 150, 255);
const SLIDER_BG : Color = Color::RGBA(15, 25, 65, 255);
const SLIDER_HIGHLIGHT : Color = Color::RGBA(25, 35, 75, 255);
const SLIDER_TRACK : Color = Color::RGBA(30, 10, 10, 255);
const SLIDER_KNOB : Color = Color::RGBA(0, 100, 60, 255);
const SHADOW_KNOB : Color = Color::RGBA(20, 80, 60, 255);
const SLIDER_TEMP_LABEL : Color = Color::RGBA(0, 60, 120, 255);

pub async fn nvfs_sdl3_frontend(mut fan_service: FanServiceClient) -> Result<(), anyhow::Error> {
  // SDL3 Setup
  // Prefer Wayland if available
  unsafe {
    if let Ok(session) = ::std::env::var("XDG_SESSION_TYPE") {
      if session == "wayland" {
        ::std::env::set_var("SDL_VIDEODRIVER", "wayland");
        // if let Ok(display) = ::std::env::var("WAYLAND_DISPLAY") {
        //   if display != "wayland-0" { ::std::env::set_var("WAYLAND_DISPLAY", "wayland-0"); }
        // } else {
        //   println!("WAYLAND_DISPLAY not defined")
        // }
      }
    }
    // Don't keep display awake. OLED murder is wrong.
    ::std::env::set_var("SDL_VIDEO_ALLOW_SCREENSAVER", "1");
    // revisit: Glycin was breaking things
    ::std::env::set_var("GDK_PIXBUF_USE_GLYCIN", "no");
  }
  // Initialize sdl3
  let sdl_context = sdl3::init().unwrap();
  let ttf_context = sdl3::ttf::init().unwrap();
  let fira = ttf_context.load_font("/usr/share/fonts/TTF/FiraCodeNerdFontMono-Regular.ttf", 26.0)
    .expect("Couldn't load FiraCodeNerdFontMono Regular TTF");
  // let window_icon = Surface::from_file("res/nvfanservice.png")?;
  let video_subsystem = sdl_context.video().unwrap();
  let mut window = video_subsystem
    .window("nvFanService UwU", 500, 400)
    .position_centered()
    .resizable()
    .build()
    .unwrap();
  // load window icon
  let icon_bytes = include_bytes!("../res/nvfanservice.png");
  let icon_data = load_from_memory_with_format(icon_bytes.as_ref(), ImageFormat::Png)?;
  let mut icon_vec = icon_data.into_rgb8().into_raw().to_owned();
  let icon_raw: &mut [u8] = &mut icon_vec;
  let icon_surface = Surface::from_data(
    icon_raw, 128, 128, 384, SDL_PixelFormat::RGB24.try_into()?
  )?;
  window.set_icon(icon_surface);
  // create canvas
  let mut canvas = window.into_canvas();
  let texture_creator = canvas.texture_creator();
  let mut event_pump = sdl_context.event_pump().unwrap();
  
  canvas.set_draw_color(Color::RGB(0, 255, 255));
  canvas.clear();
  canvas.present();
  let mut i = 0; // For controlling when timed_service_service is run.
  let card_name = fan_service.request_card_name().await?;
  let txt_gfx_card = fira.render(&card_name).blended(PRIMARY).unwrap();
  let txt_gfx_card_half_w: i32 = (txt_gfx_card.width() / 2) as i32;
  let txt_region_gfx_card = Rect::new(
    0,0,txt_gfx_card.width(),txt_gfx_card.height()
  );
  let mut fira_temp_and_speed = {
    let tempspeed = fan_service.request_tempspeed().await?;
    let ts_txt = format!("Temp: {:>2}C, Fan Speed: {:>3}%", tempspeed.0, tempspeed.1);
    fira.render(&ts_txt).blended(PRIMARY).unwrap()
  };
  let txt_temp_and_speed_half_w: i32 = (fira_temp_and_speed.width() / 2) as i32;
  let txt_region_temp_and_speed = Rect::new(
    0,0,fira_temp_and_speed.width(),fira_temp_and_speed.height()
  );
  let mut mouse_data = MouseData::default();
  'running: loop {
    // Check Events!
    mouse_data.is_current = false;
    for event in event_pump.poll_iter() {
      match event {
        Event::Quit {..} |
        Event::KeyDown { keycode: Some(Keycode::Escape), .. }
        | Event::KeyDown { keycode: Some(Keycode::Q), .. } => {
          break 'running
        },
        Event::MouseMotion { mousestate, x, y, xrel, yrel, ..} => {
          mouse_data.update(x, y, xrel, yrel, mousestate.left());
        },
        Event::MouseButtonDown { mouse_btn, x, y, .. } => {
          if mouse_btn as usize == 1 { mouse_data.update(x, y, 0.0, 0.0, true) };
        },
        Event::MouseButtonUp { mouse_btn, x, y, .. } => {
          if mouse_btn as usize == 1 { mouse_data.update(x, y, 0.0, 0.0, false) };
        },
        _ => {}
      }
    }
    // Update!
    i = (i + 1) % 10;
    if i == 0 {
      let tempspeed = fan_service.request_tempspeed().await?;
      let ts_txt = format!("Temp: {:>2}C, Fan Speed: {:>3}%", tempspeed.0, tempspeed.1);
      fira_temp_and_speed = fira.render(&ts_txt).blended(PRIMARY).unwrap();
    }
    let tex_temp_and_speed = fira_temp_and_speed.as_texture(&texture_creator).unwrap();
    let tex_gfx_card = txt_gfx_card.as_texture(&texture_creator).unwrap();
    let canvas_rect = canvas.viewport();
    let win_half_w: i32 = (canvas_rect.width() / 2) as i32;
    let x_gfx_card = win_half_w - txt_gfx_card_half_w;
    let x_temp_and_speed: i32 = win_half_w - txt_temp_and_speed_half_w;
    let draw_pos_gfx_card = Rect::new(
      x_gfx_card,20,txt_gfx_card.width(),txt_gfx_card.height()
    );
    let draw_pos_temp_and_speed = Rect::new(
      x_temp_and_speed,40 + txt_gfx_card.height() as i32,
      fira_temp_and_speed.width(),fira_temp_and_speed.height()
    );
    let header_height = 60 + txt_gfx_card.height() + fira_temp_and_speed.height();
    let draw_pos_slider_box = Rect::new(
      10, header_height as i32,
      canvas_rect.width() - 20, canvas_rect.height() - header_height - 10,
    );
    let mut sliders = Vec::new();
    {
      if let Ok(curve) = fan_service.user_curve().lock() {
        // for each point in the curve ake Rects to draw sliders
        let count = curve.points.len() as u32;
        let w = draw_pos_slider_box.width() - 10;
        let x_offset_base = draw_pos_slider_box.x() + 5;
        let x_offset = x_offset_base + (w / count / 2) as i32;
        for i in 0..count {
          let slider_track = Rect::new(
            x_offset + (i * w / count) as i32 - 4_i32,
            draw_pos_slider_box.y() + 5,
            8, draw_pos_slider_box.height() - 40,
          );
          let slider_hl = Rect::new(
            x_offset_base + (i * w / count) as i32,
            draw_pos_slider_box.y() + 5,
            w / count, slider_track.height(),
          );
          let highlight = mouse_data.instant.elapsed().as_secs() < 6 &&
                          mouse_data.x > slider_hl.x() as f32 &&
                          mouse_data.x < (slider_hl.x() as u32 + slider_hl.width()) as f32 &&
                          mouse_data.y > slider_hl.y() as f32 &&
                          mouse_data.y < (slider_hl.y() as u32 + slider_hl.height()) as f32;
          let slider_label = Rect::new(
            x_offset + (i * w / count) as i32 - 30_i32,
            draw_pos_slider_box.y() + (draw_pos_slider_box.height() - 35) as i32,
            60, 30,
          );
          if let Ok(ts) = curve.points[i as usize].lock().as_mut() {
            let slht = slider_track.height() as f32;
            let shadow_speed = std::cmp::min(100,(
              (slht - (mouse_data.y - slider_track.y() as f32)) * 101.0 / slht
            ) as u32);
            if mouse_data.left_released && highlight {
              if let Ok(_) = fan_service.update_temp_node_with_new_speed(ts.0, shadow_speed).await {
                ts.update_speed(shadow_speed);
              }
            }
            let speed = ts.speed();
            let temp = ts.temp();
            let spd_txt = format!("{:>3}%", speed);
            let tmp_txt = format!("{:>3}C", temp);
            let x = slider_track.x() - 26;
            let y = slider_track.y() + (
              (slht - 30.0) / 100.0 * (100.0 - speed as f32)
            ) as i32;
            let shadow_spd_txt = format!("{:>3}%", shadow_speed);
            let shadow_y = slider_track.y() + (
              (slht - 30.0) / 100.0 * (100.0 - shadow_speed as f32)
            ) as i32;
            let slider_knob = Rect::new(x, y, 60, 30);
            let shadow_knob = Rect::new(x, shadow_y, 60, 30);
            let tex_spd = fira.render(&spd_txt).blended(PRIMARY)
              .unwrap()
              .as_texture(&texture_creator)
              .unwrap();
            let tex_shadow_spd = fira.render(&shadow_spd_txt).blended(SHADOW_TXT)
              .unwrap()
              .as_texture(&texture_creator)
              .unwrap();
            let tex_temp = fira.render(&tmp_txt).blended(PRIMARY)
              .unwrap()
              .as_texture(&texture_creator)
              .unwrap();
            sliders.push((
              slider_track,
              slider_hl, highlight,
              slider_knob,
              slider_label,
              tex_temp, tex_spd,
              shadow_knob, tex_shadow_spd,
            ));
          }
        }
      }
      0
    };
    // Draw!
    canvas.set_draw_color(BG);
    canvas.clear();
    canvas.copy(&tex_gfx_card, txt_region_gfx_card, draw_pos_gfx_card).unwrap();
    canvas.copy(&tex_temp_and_speed, txt_region_temp_and_speed, draw_pos_temp_and_speed).unwrap();
    canvas.set_draw_color(SLIDER_BG);
    canvas.fill_rect(draw_pos_slider_box).unwrap();
    canvas.set_draw_color(SLIDER_HIGHLIGHT);
    for (_, highlight_rect, highlight, _, _, _, _, _, _) in &sliders {
      if *highlight { canvas.fill_rect(*highlight_rect).unwrap(); }
    }
    canvas.set_draw_color(SLIDER_TRACK);
    for (slider_track, _, _, _, _, _, _, _, _) in &sliders {
      canvas.fill_rect(*slider_track).unwrap();
    }
    canvas.set_draw_color(SLIDER_TEMP_LABEL);
    for (_, _, _, _, slider_label, tex_tmp, _, _, _) in &sliders {
      canvas.fill_rect(*slider_label).unwrap();
      canvas.copy(tex_tmp, None, *slider_label).unwrap();
    }
    canvas.set_draw_color(SLIDER_KNOB);
    for (_, _, _, slider_knob, _, _, tex_spd, _, _) in &sliders {
      canvas.fill_rect(*slider_knob).unwrap();
      canvas.copy(tex_spd, None, *slider_knob).unwrap();
    }
    canvas.set_draw_color(SHADOW_KNOB);
    for (_, _, highlight, _, _, _, _, shadow_knob, tex_shadow_spd) in &sliders {
      if *highlight {
        canvas.fill_rect(*shadow_knob).unwrap();
        canvas.copy(tex_shadow_spd, None, *shadow_knob).unwrap();
      }
    }
    // Present result!
    canvas.present();
    ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
  };
  Ok(())
}

#[derive(Clone, Copy)]
struct MouseData {
  x: f32,
  y: f32,
  dx: f32,
  dy: f32,
  instant: Instant,
  left_click: bool,
  left_released: bool,
  is_current: bool,
}
impl Default for MouseData {
  fn default() -> Self {
    Self {
      x: 0.0,
      y: 0.0,
      dx: 0.0,
      dy: 0.0,
      instant: Instant::now(),
      left_click: false,
      left_released: false,
      is_current: false,
    }
  }
}
impl MouseData {
  pub fn update(&mut self, x: f32, y: f32, dx: f32, dy: f32, left_click: bool) {
    self.left_released = self.left_click && !left_click;
    self.left_click = left_click;
    self.x = x;
    self.y = y;
    if !self.is_current {
      self.dx = dx;
      self.dy = dy;
    }
    self.instant = Instant::now();
    self.is_current = true;
  }
}
