use {
    super::root,
    crate::math::Extent,
    serde::{Deserialize, Serialize},
    std::{
        fs::{create_dir_all, read_to_string, File},
        io::{Error as IoError, ErrorKind, Write},
        path::PathBuf,
    },
    toml::{from_str, to_string_pretty},
};

/// The name of the config while while in debug mode
#[cfg(debug_assertions)]
const CONFIG_FILENAME: &str = "engine-debug.toml";

/// The name of the config while while in release mode
#[cfg(not(debug_assertions))]
const CONFIG_FILENAME: &str = "engine.toml";

fn config_path(name: &'static str, author: &'static str) -> Result<PathBuf, IoError> {
    let program_root = root(name, author)?;

    Ok(program_root.join(CONFIG_FILENAME))
}

pub struct Config {
    data: Data,
    program_author: &'static str,
    program_name: &'static str,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Data {
    fullscreen: Option<bool>,
    swapchain_len: Option<u32>,
    window_dimensions: Option<(usize, usize)>,
}

impl Config {
    pub fn read(program_name: &'static str, program_author: &'static str) -> Result<Self, IoError> {
        let config_path = config_path(program_name, program_author)?;
        Ok(if config_path.exists() {
            #[cfg(debug_assertions)]
            debug!("Loaded config {}", config_path.display());

            let config_file = read_to_string(&*config_path).unwrap_or_else(|_| {
                #[cfg(debug_assertions)]
                warn!("Engine config file read error, creating a new one");

                "".to_owned()
            });
            let config: Schema = from_str(&config_file).unwrap_or_default();

            Self {
                data: config.data,
                program_author,
                program_name,
            }
        } else {
            #[cfg(debug_assertions)]
            info!("Engine config file not found, creating a new one");

            let mut res = Self {
                data: Data::default(),
                program_author,
                program_name,
            };
            res.data.fullscreen = None;
            res.data.swapchain_len = Some(res.swapchain_len());
            res.data.window_dimensions = None;
            res.write()?;

            res
        })
    }

    /// The default value is windowed mode (false).
    pub fn fullscreen(&self) -> Option<bool> {
        self.data.fullscreen
    }

    /// Value will be in the range of [1,3]. The default value is 3.
    pub fn swapchain_len(&self) -> u32 {
        self.data.swapchain_len.unwrap_or(3).max(1).min(3)
    }

    /// The dimensions of the window if set.
    pub fn window_dimensions(&self) -> Option<Extent> {
        self.data
            .window_dimensions
            .map(|dims| Extent::new(dims.0 as _, dims.1 as _))
    }

    pub fn write(&self) -> Result<(), IoError> {
        let program_root = root(self.program_name, self.program_author)?;

        if !program_root.exists() {
            create_dir_all(&*program_root)?;
        }

        let config_path = config_path(self.program_name, self.program_author)?;
        let mut config_file = File::create(&*config_path)?;

        let toml = to_string_pretty(&Schema {
            data: self.data.clone(),
        });
        if toml.is_err() {
            return Err(IoError::from(ErrorKind::Other));
        }
        let toml = toml.unwrap();

        config_file.write_all(toml.as_bytes())?;

        Ok(())
    }
}

#[derive(Default, Deserialize, Serialize)]
struct Schema {
    #[serde(rename = "screen-13")]
    data: Data,
}
