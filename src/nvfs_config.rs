#![allow(dead_code)]
use {
  crate::{
    elevate::running_unpriviledged,
    FanCurveUwU,
  },
  anyhow::anyhow,
  kdl::{
    KdlDocument,
    // KdlEntry,
    KdlError,
    KdlValue,
  },
  nom::{
    branch::alt,
    character::complete::{
      char,
      i32 as nom_i32,
      u32 as nom_u32,
    },
    combinator::{
      all_consuming,
      verify,
    },
    sequence::{
      preceded, terminated,
    },
    IResult,
    Parser,
  },
  platform_dirs::AppDirs,
  std::{
    path::PathBuf,
    sync::{Arc, Mutex,},
  }
};

const CONFIG_KDL: &str =
r#"// nvFanService settings
fan_curve {
  :20C      @0%
  :30C     @50%
  :40C     @70%
  :48C     @90%
  :52C    @100%
} 
"#;

// Create or acquire $HOME/.config/nvfanservice/nvfanservice.kdl
#[allow(clippy::needless_return)] // 'return' statements make the intention more obvious.
pub fn get_or_create_config(is_root: bool) -> Result<NvfsConfig, anyhow::Error> {
  let app_name = Some("nvFanService");
  let config_file = "config.kdl";
  let mut path: PathBuf = if is_root {
    "/etc/nvFanService".into()
  } else {
    if let Some(app_dirs) = AppDirs::new(app_name, true) {
      app_dirs.config_dir
    } else {
      let error = "Failed to get home directory. Cannot check for or create config file.".to_owned();
      log::info!("{}", error);
      return Err(anyhow!(error));
    }
  };
  if let Err(e) = std::fs::create_dir_all(&path) {
    let error = format!("Failed to create config dir: {}\nError: {}", path.display(), e);
    // log::error!("{}", error);
    return Err(anyhow!(error));
  }
  path.push(config_file);
  match std::fs::exists(&path) {
    Err(e)    => {
      let error = format!("Failed to check existence of config file: {}\nError: {}", path.display(), e);
      // log::error!("{}", error);
      return Err(anyhow!(error));
    }
    Ok(false) => {
      // The file does not exist, so we create it with the default config
      if let Err(e) = std::fs::write(&path, CONFIG_KDL) {
        let error = format!("Failed to write default config file: {}\nError: {}", path.display(), e);
        // log::error!("{}", error);
        return Err(anyhow!(error));
      };
      get_or_create_config(is_root)
    }
    Ok(true)  => {
      // The file exists, now we need to validate it
      match std::fs::read_to_string(&path) { 
        Err(e) => {
          let error = format!("File exists but failed to read: {}\nError: {}", path.display(), e);
          // log::error!("{}", error);
          return Err(anyhow!(error));
        }
        Ok(conf) => {
          return validate_config(conf);
        }
      }
    }
  }
}

#[allow(clippy::needless_return)]
fn validate_config(conf: String) -> Result<NvfsConfig, anyhow::Error> {
  let conf = &conf;
  let doc: Result<KdlDocument, KdlError> = conf.parse();
  match doc {
    Err(e) => {
      let error = format!("Failed to parse KDL:\n{}\n\nError: {}", conf, e);
      log::error!("{}", error);
      Err(anyhow!(error))
    }
    Ok(conf) => {
      Ok(conf.into())
    }
  }
}

pub fn save_curve_to_config_kdl(curve: Arc<Mutex<FanCurveUwU>>) -> Result<(), anyhow::Error> {
  let curve: Vec<(i32, u32)> = {
    let c = curve.lock().unwrap();
    let cc = &*c;
    cc.into()
  };
  let mut new_conf = Vec::new();
  new_conf.push("fan_curve {".to_string());
  for (temp, speed) in curve {
    new_conf.push(format!("  :{}C     @{}%", temp, speed));
  }
  new_conf.push("}".to_string());
  let is_root = !running_unpriviledged();
  let new_file = new_conf.join("\n");
  let app_name = Some("nvFanService");
  let config_file = "config.kdl";
  let mut path: PathBuf = if is_root {
    "/etc/nvFanService".into()
  } else {
    if let Some(app_dirs) = AppDirs::new(app_name, true) {
      app_dirs.config_dir
    } else {
      let error = "Failed to get home directory. Cannot check for or create config file.".to_owned();
      log::info!("{}", error);
      return Err(anyhow!(error));
    }
  };
  if let Err(e) = std::fs::create_dir_all(&path) {
    let error = format!("Failed to create config dir: {}\nError: {}", path.display(), e);
    // log::error!("{}", error);
    return Err(anyhow!(error));
  }
  path.push(config_file);
  match std::fs::exists(&path) {
    Err(e)    => {
      let error = format!("Failed to check existence of config file: {}\nError: {}", path.display(), e);
      // log::error!("{}", error);
      return Err(anyhow!(error));
    }
    Ok(false) => {
      // The file does not exist, oddly, so we're free to write it
      if let Err(e) = std::fs::write(&path, new_file) {
        let error = format!("Error saving new config.kdl: Failed on write: {}\nError: {}", path.display(), e);
        return Err(anyhow!(error));
      };
      Ok(())
    }
    Ok(true)  => {
      // The file exists, delete and write new file
      if let Err(e) = std::fs::remove_file(&path) {
        let error = format!("Failed to delete existing config file: {}\nError: {}", path.display(), e);
        return Err(anyhow!(error));
      };
      if let Err(e) = std::fs::write(&path, new_file) {
        let error = format!("Error saving new config.kdl after deleting the old file: Failed on write: {}\nError: {}", path.display(), e);
        return Err(anyhow!(error));
      };
      Ok(())
    }
  }
}

#[derive(Debug)]
pub struct NvfsConfig {
  fan_curve: Vec<(i32, u32)>,
  pub font: Option<PathBuf>,
  errors: Vec<anyhow::Error>,
}
impl NvfsConfig {
  pub fn fan_curve(&self) -> Vec<(i32, u32)> { self.fan_curve.to_owned() }
}
impl From<KdlDocument> for NvfsConfig {
  fn from(conf: KdlDocument) -> Self {
    let mut errors = Vec::new();
    let (fan_curve, error) = conf.fan_curve();
    if error.is_err() {
      errors.push(error.err().unwrap());
    }
    let font_res = conf.font();
    let mut font = None;
    if font_res.is_ok() {
      font = font_res.unwrap();
    } else {
      errors.push(font_res.err().unwrap());
    }
    Self { fan_curve, font, errors }
  }
}

pub trait NvfsValues {
  fn fan_curve(&self) -> (Vec<(i32, u32)>, anyhow::Result<()>);
  fn font(&self) -> anyhow::Result<Option<PathBuf>>;
}

#[allow(clippy::needless_return)]
impl NvfsValues for KdlDocument {
  fn fan_curve(&self) -> (Vec<(i32, u32)>, anyhow::Result<()>) {
    let default: Vec<(i32, u32)> = vec![
      (20, 0),
      (30, 50),
      (40, 70),
      (48, 90),
      (52, 100),
    ];
    if let Some(node) = self.get("fan_curve") {
      if let Some(children) = node.children() {
        let mut fan_curve = Vec::new();
        for child in children.nodes() {
          let temp: i32 = match parse_temp(child.name().value()) {
            Ok((&_, temp)) => { temp }
            Err(e) => { return (default, Err(anyhow!("Invalid temperature value: {}", e))); }
          };
          let pct: u32 = if let Some(entry) = child.entry(0) {
            if let KdlValue::String(pct) = entry.value() {
              if let Ok((_, pct)) = parse_pct(pct) {
                pct
              } else { return (default, Err(anyhow!("Temperature {}C has invalid Fan Percent value", temp))); }
            } else { return (default, Err(anyhow!("Temperature {}C: Failed to read percentage value as a string", temp))); }
          } else { return (default, Err(anyhow!("Temperature {}C has no Fan Percent value", temp))); };
          fan_curve.push((temp, pct));
        }
        fan_curve.sort_by_key(|&(temp, _)| temp);
        return (fan_curve, Ok(()));
      } else { return (default, Err(anyhow!("'fan_curve' node has no fields"))); }
    } else { return (default, Err(anyhow!("'fan_curve' node not present"))); }
  }
  fn font(&self) -> anyhow::Result<Option<PathBuf>> {
    if let Some(node) = self.get("font") {
      if let Some(entry) = node.entry(0) {
        if let KdlValue::String(font) = entry.value() {
          let font = PathBuf::from(font);
          if font.exists() {
            Ok(Some(font))
          } else { Err(anyhow!("font file does not exist at the given path: {}", font.display())) }
        } else { Err(anyhow!("font node exists but has no valid font path")) }
      } else { Err(anyhow!("font node exists but has no entries")) }
    } else { Ok(None) }
  }
}

fn parse_temp(input: &str) -> IResult<&str, i32> {
  all_consuming(preceded(char(':'), terminated(verify(nom_i32, |&x| x >= 5 && x <= 95), alt((char('C'), char('c')))))).parse(input)
}

fn parse_pct(input: &str) -> IResult<&str, u32> {
  all_consuming(preceded(char('@'), terminated(verify(nom_u32, |&x| x <= 100), char('%')))).parse(input)
}
