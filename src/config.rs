use {
    crate::math::Extent,
    app_dirs::{get_app_root, AppDataType, AppDirsError, AppInfo},
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

pub fn get_game_root(game: &'static str) -> Result<PathBuf, IoError> {
    // Converts the app_dirs crate AppDirsError to a regular IO Error
    match get_app_root(
        AppDataType::UserConfig,
        &AppInfo {
            name: game,
            author: "Attack Goat",
        },
    ) {
        Err(err) => Err(match err {
            AppDirsError::Io(err) => err,
            AppDirsError::InvalidAppInfo => IoError::from(ErrorKind::InvalidInput),
            AppDirsError::NotSupported => IoError::from(ErrorKind::InvalidData),
        }),
        Ok(res) => Ok(res),
    }
}

fn get_config_path(game: &'static str) -> Result<PathBuf, IoError> {
    let game_root = get_game_root(game)?;

    Ok(game_root.join(CONFIG_FILENAME))
}

pub struct Config {
    data: Data,
    game: &'static str,
}

#[derive(Default, Deserialize, Serialize)]
struct Data {
    fullscreen: Option<bool>,
    render_buf_len: Option<usize>,
    swapchain_len: Option<usize>,
    window_dimensions: Option<(usize, usize)>,
}

impl Config {
    pub fn read(game: &'static str) -> Result<Self, IoError> {
        let config_path = get_config_path(game)?;
        Ok(if config_path.exists() {
            let config_file = read_to_string(&*config_path).unwrap_or_else(|_| {
                #[cfg(debug_assertions)]
                warn!("Engine config file read error, creating a new one");

                "".to_owned()
            });
            Self {
                data: from_str(&config_file).unwrap_or_default(),
                game,
            }
        } else {
            #[cfg(debug_assertions)]
            info!("Engine config file not found, creating a new one");
            let mut res = Self {
                data: Data::default(),
                game,
            };
            let dims = res.window_dimensions();
            res.data.fullscreen = Some(res.fullscreen());
            res.data.render_buf_len = Some(res.render_buf_len());
            res.data.swapchain_len = Some(res.swapchain_len());
            res.data.window_dimensions = Some((dims.x as _, dims.y as _));
            res.write()?;
            res
        })
    }

    /// The default value is windowed mode (false).
    pub fn fullscreen(&self) -> bool {
        self.data.fullscreen.unwrap_or_default()
    }

    /// Value will be in the range of [1,8]. The default value is 3.
    pub fn render_buf_len(&self) -> usize {
        self.data.render_buf_len.unwrap_or(3).max(1).min(8)
    }

    /// Value will be in the range of [1,3]. The default value is 2.
    pub fn swapchain_len(&self) -> usize {
        self.data.swapchain_len.unwrap_or(2).max(1).min(3)
    }

    /// The default value is 1920x1080 (HD)
    pub fn window_dimensions(&self) -> Extent {
        let dims = self.data.window_dimensions.unwrap_or((1920, 1080));
        Extent::new(dims.0 as _, dims.1 as _)
    }

    pub fn write(&self) -> Result<(), IoError> {
        let game_root = get_game_root(self.game)?;

        if !game_root.exists() {
            create_dir_all(&*game_root)?;
        }

        let config_path = get_config_path(self.game)?;
        let mut config_file = File::create(&*config_path)?;

        let toml = to_string_pretty(&self.data);
        if toml.is_err() {
            return Err(IoError::from(ErrorKind::Other));
        }
        let toml = toml.unwrap();

        config_file.write_all(toml.as_bytes())?;

        Ok(())
    }
}
