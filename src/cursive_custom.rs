#![allow(unused_braces, dead_code)]
use {
  cursive_core::{
    direction::{Direction, Orientation},
    event::{Callback, Event, EventResult, Key, MouseButton, MouseEvent},
    style::PaletteStyle,
    view::{CannotFocus, View},
    Cursive, Printer, Vec2, With,
  },
};
use std::sync::Arc;

type FanCurveUnitCallback = dyn Fn(&mut Cursive, i32, u32) + Send + Sync;

const MAX_FAN_SPEED: usize = 100;

/// A vertical slider with a labled slide.
///
/// # Examples
///
/// ```
/// use cursive_core::views::{Dialog, SliderView};
///
/// let slider_view = SliderView::horizontal(10)
///     .on_change(|s, n| {
///         if n == 5 {
///             s.add_layer(Dialog::info("5! Pick 5!"));
///         }
///     })
///     .on_enter(|s, n| match n {
///         5 => s.add_layer(Dialog::info("You did it!")),
///         n => s.add_layer(Dialog::info(format!("Why {}? Why not 5?", n))),
///     });
/// ```
pub struct FanCurveUnitView {
    on_change: Option<Arc<FanCurveUnitCallback>>,
    on_enter: Option<Arc<FanCurveUnitCallback>>,
    height: usize,
    fan_speed: u32,
    temp: i32,
    min_temp: i32,
    max_temp: i32,
    dragging: bool,
}

impl FanCurveUnitView {
  /// Creates a new `FanCurveUnitView` with the given height, min, max and starting temperatures, and starting fan speed.
  ///
  /// The view is 4 wide by `height` deep.
  /// The slider represents the fan speed by percentage, 
  /// from 0 to 100 inclusive, of hardware max fan speed.
  /// If the supplied height is 10, 
  /// The actual range of values for this slider is `[0, max_value - 1]`.
  pub fn new(temp: i32, fan_speed: u32) -> Self {
    Self {
      on_change: None,
      on_enter: None,
      height: 10,
      fan_speed,
      temp,
      min_temp: 5,
      max_temp: 95,
      dragging: false,
    }
  }
  
  /// Sets the height of the slider. No callback.
  pub fn set_height(&mut self, height: usize) {
    self.height = height;
  }
  /// Chainable method to set the height.
  #[must_use]
  pub fn with_height(self, height: usize) -> Self {
    self.with(|s| {
      s.set_height(height);
    })
  }
  /// Gets the current height.
  pub fn get_height(&self) -> usize {
    self.height
  }
  
  /// Chainable method to set the curve point's min and max temperature values.
  ///
  /// Chainable variant.
  #[must_use]
  pub fn with_temp_min_max(self, min: i32, max: i32) -> Self {
    self.with(|s| {
      s.min_temp = min;
      s.max_temp = max;
    })
  }
  /// Gets the curve point's current max temperature value.
  pub fn get_max_temp(&self) -> i32 {
    self.max_temp
  }
  /// Gets the curve point's current min temperature value.
  pub fn get_min_temp(&self) -> i32 {
    self.min_temp
  }
  
  /// Chainable method to set the curve point's temperature and fan speed values.
  #[must_use]
  pub fn with_temp_speed(self, temp: i32, speed: u32) -> Self {
    self.with(|s| {
      s.temp = temp;
      s.fan_speed = speed;
    })
  }
  /// Gets the curve point's current temperature value.
  pub fn get_temp(&self) -> i32 {
    self.temp
  }
  /// Gets the curve point's current fan speed value.
  pub fn get_speed(&self) -> u32 {
    self.fan_speed
  }
  
  /// Sets a callback to be called when the slider is moved.
  #[cursive_core::callback_helpers]
  pub fn set_on_change<F>(&mut self, callback: F)
  where
      F: Fn(&mut Cursive, i32, u32) + 'static + Send + Sync,
  {
      self.on_change = Some(Arc::new(callback));
  }
  /// Chainable method to set a callback to be called when the slider is moved.
  #[must_use]
  pub fn on_change<F>(self, callback: F) -> Self
  where
      F: Fn(&mut Cursive, i32, u32) + 'static + Send + Sync,
  {
      self.with(|s| s.set_on_change(callback))
  }
  
  /// Sets a callback to be called when the `<Enter>` key is pressed.
  #[cursive_core::callback_helpers]
  pub fn set_on_enter<F>(&mut self, callback: F)
  where
      F: Fn(&mut Cursive, i32, u32) + 'static + Send + Sync,
  {
      self.on_enter = Some(Arc::new(callback));
  }
  /// Chainable method to set a callback to be called when the `<Enter>` key is pressed.
  #[must_use]
  pub fn on_enter<F>(self, callback: F) -> Self
  where
      F: Fn(&mut Cursive, i32, u32) + 'static + Send + Sync,
  {
      self.with(|s| s.set_on_enter(callback))
  }
  
  fn call_on_change(&self) -> EventResult {
    EventResult::Consumed(self.on_change.clone().map(|cb| {
      let slider_temp = self.temp;
      let slider_speed = self.fan_speed;
      Callback::from_fn(move |s| {
        cb(s, slider_temp, slider_speed);
      })
    }))
  }
  
  fn slide_plus(&mut self) -> EventResult {
      if self.fan_speed + 1 <= MAX_FAN_SPEED as u32 {
          self.fan_speed += 1;
          self.call_on_change()
      } else {
          EventResult::Ignored
      }
  }
  fn slide_minus(&mut self) -> EventResult {
      if self.fan_speed > 0 {
          self.fan_speed -= 1;
          self.call_on_change()
      } else {
          EventResult::Ignored
      }
  }
  
  fn req_size(&self) -> Vec2 {
      (4, self.height + 1).into()
  }
}

impl View for FanCurveUnitView {
  fn draw(&self, printer: &Printer) {
    printer.print_vline((0, 0), self.height, "  | ");
    let style = if printer.focused {
      PaletteStyle::Highlight
    } else {
      PaletteStyle::HighlightInactive
    };
    let slider_txt = format!("{: >3}", self.fan_speed);
    let temperature_txt = format!("{: >2}C", self.temp);
    let slider_height = ::std::cmp::min(self.height - 1, self.height.saturating_sub((self.fan_speed as usize + 9) / self.height));
    printer.with_style(style, |printer| {
      printer.print((1, slider_height), &slider_txt);
      printer.print((1, self.height), &temperature_txt);
    });
  }
  
  fn required_size(&mut self, _: Vec2) -> Vec2 {
      self.req_size()
  }
  
  fn on_event(&mut self, event: Event) -> EventResult {
    match event {
      Event::Key(Key::Up) => self.slide_plus(),
      Event::Key(Key::Down) => self.slide_minus(),
      Event::Key(Key::Enter) if self.on_enter.is_some() => {
        let slider_temp = self.temp;
        let slider_speed = self.fan_speed;
        let cb = self.on_enter.clone().unwrap();
        EventResult::with_cb(move |s| {
          cb(s, slider_temp, slider_speed);
        })
      }
      Event::Mouse {
        event: MouseEvent::Hold(MouseButton::Left),
        position,
        offset,
      } if self.dragging => {
        let position = position.saturating_sub(offset);
        let position = Orientation::Vertical.get(&position);
        let position = MAX_FAN_SPEED - (position * self.height);
        let position = ::std::cmp::min(position, MAX_FAN_SPEED) as u32;
        self.fan_speed = position;
        self.call_on_change()
      }
      Event::Mouse {
        event: MouseEvent::Press(MouseButton::Left),
        position,
        offset,
      } if position.fits_in_rect(offset, self.req_size()) => {
        if let Some(position) = position.checked_sub(offset) {
          self.dragging = true;
          self.fan_speed = Orientation::Vertical.get(&position) as u32;
        }
        self.call_on_change()
      }
      Event::Mouse {
        event: MouseEvent::Release(MouseButton::Left),
        ..
      } => {
        self.dragging = false;
        EventResult::Ignored
      }
      _ => EventResult::Ignored,
    }
  }
  
  fn take_focus(&mut self, _: Direction) -> Result<EventResult, CannotFocus> {
    Ok(EventResult::Consumed(None))
  }
}

// TODO: Rename the view itself as Slider to match the config?
// #[cursive_core::blueprint(FanCurveUnitView::new(height, max_value))]
// struct Blueprint {
//     height: u32,
//     max_value: u32,

//     on_change: Option<_>,
//     on_enter: Option<_>,
// }
