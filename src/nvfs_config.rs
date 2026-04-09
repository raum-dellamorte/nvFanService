#![allow(dead_code)]
use {
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
  std::path::PathBuf,
};

const CONFIG_KDL: &str =
r#"// nvFanService settings
fan_curve {
  :10C      @0%
  :20C     @30%
  :30C     @60%
  :36C     @70%
  :40C     @80%
  :46C     @90%
  :50C    @100%
} 
"#;

// Create or acquire $HOME/.config/nvfanservice/nvfanservice.kdl
#[allow(clippy::needless_return)] // 'return' statements make the intention more obvious.
pub fn get_or_create_config(is_root: bool) -> Result<NvfsConfig, String> {
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
      return Err(error)
    }
  };
  if let Err(e) = std::fs::create_dir_all(&path) {
    let error = format!("Failed to create config dir: {}\nError: {}", path.display(), e);
    // log::error!("{}", error);
    return Err(error);
  }
  path.push(config_file);
  match std::fs::exists(&path) {
    Err(e)    => {
      let error = format!("Failed to check existence of config file: {}\nError: {}", path.display(), e);
      // log::error!("{}", error);
      return Err(error);
    }
    Ok(false) => {
      // The file does not exist, so we create it with the default config
      if let Err(e) = std::fs::write(&path, CONFIG_KDL) {
        let error = format!("Failed to write default config file: {}\nError: {}", path.display(), e);
        // log::error!("{}", error);
        return Err(error);
      };
      get_or_create_config(is_root)
    }
    Ok(true)  => {
      // The file exists, now we need to validate it
      match std::fs::read_to_string(&path) { 
        Err(e) => {
          let error = format!("File exists but failed to read: {}\nError: {}", path.display(), e);
          // log::error!("{}", error);
          return Err(error);
        }
        Ok(conf) => {
          return validate_config(conf);
        }
      }
    }
  }
}

#[allow(clippy::needless_return)]
fn validate_config(conf: String) -> Result<NvfsConfig, String> {
  let conf = &conf;
  let doc: Result<KdlDocument, KdlError> = conf.parse();
  match doc {
    Err(e) => {
      let error = format!("Failed to parse KDL:\n{}\n\nError: {}", conf, e);
      log::error!("{}", error);
      Err(error)
    }
    Ok(conf) => {
      conf.try_into()
    }
  }
}

#[derive(Debug)]
pub struct NvfsConfig {
  fan_curve: Vec<(i32, u32)>,
}
impl NvfsConfig {
  pub fn fan_curve(&self) -> Vec<(i32, u32)> { self.fan_curve.to_owned() }
}
impl TryFrom<KdlDocument> for NvfsConfig {
  type Error = String;
  fn try_from(conf: KdlDocument) -> Result<Self, Self::Error> {
    let fan_curve: Vec<(i32, u32)> = match conf.fan_curve() {
      Err(e) => { return Err(e); }
      Ok(val) => { val }
    };
    Ok(Self { fan_curve })
  }
}

pub trait NvfsValues {
  fn fan_curve(&self) -> Result<Vec<(i32, u32)>, String>;
}

#[allow(clippy::needless_return)]
impl NvfsValues for KdlDocument {
  fn fan_curve(&self) -> Result<Vec<(i32, u32)>, String> {
    if let Some(node) = self.get("fan_curve") {
      if let Some(children) = node.children() {
        let mut fan_curve = Vec::new();
        for child in children.nodes() {
          let temp = match parse_temp(child.name().value()) {
            Ok((&_, temp)) => { temp }
            Err(e) => { return Err(format!("Invalid temperature value: {}", e)); }
          };
          let pct = if let Some(entry) = child.entry(0) {
            if let KdlValue::String(pct) = entry.value() {
              if let Ok((_, pct)) = parse_pct(pct) {
                pct
              } else { return Err(format!("Temperature {}C has invalid Fan Percent value", temp)); }
            } else { return Err(format!("Temperature {}C: Failed to read percentage value as a string", temp)); }
          } else { return Err(format!("Temperature {}C has no Fan Percent value", temp)); };
          fan_curve.push((temp, pct));
        }
        fan_curve.sort_by_key(|&(temp, _)| temp);
        return Ok(fan_curve);
      } else { return Err("'fan_curve' node has no fields".to_owned()); }
    } else { return Err("'fan_curve' node not present".to_owned()); }
  }
}

fn parse_temp(input: &str) -> IResult<&str, i32> {
  all_consuming(preceded(char(':'), terminated(verify(nom_i32, |&x| x >= 5 && x <= 95), alt((char('C'), char('c')))))).parse(input)
}

fn parse_pct(input: &str) -> IResult<&str, u32> {
  all_consuming(preceded(char('@'), terminated(verify(nom_u32, |&x| x <= 100), char('%')))).parse(input)
}
