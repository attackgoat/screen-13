mod default_program_icon {
    include!(concat!(env!("OUT_DIR"), "/default_program_icon.rs"));
}

use {
    super::program_root,
    serde::{Deserialize, Serialize},
    std::{
        convert::{AsRef, TryFrom},
        io::Error,
        path::PathBuf,
    },
    winit::window::{BadIcon, Icon as WinitIcon},
};

const DEFAULT_AUTHOR: &str = "screen-13";
const DEFAULT_NAME: &str = "default";

/// A small picture which represents the program, to be used by the operating system in different
/// ways on each platform. Pixels are RGBA formatted.
#[derive(Debug, Deserialize, Serialize)]
pub struct Icon<'a> {
    /// Icon height in pixels.
    pub height: u32,

    /// Array of RGBA-formatted pixel data.
    ///
    /// The length is four times the number of pixels.
    pub pixels: &'a [u8],

    /// Icon width in pixels.
    pub width: u32,
}

impl Icon<'static> {
    /// A fantastic icon chosen based on the order in which it was generated. 🗿
    pub const DEFAULT: Self = Self {
        height: default_program_icon::HEIGHT,
        pixels: &default_program_icon::PIXELS,
        width: default_program_icon::WIDTH,
    };
}

impl Default for Icon<'static> {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl<'a> TryFrom<&'a Icon<'_>> for WinitIcon {
    type Error = BadIcon;

    fn try_from(val: &'a Icon) -> Result<Self, Self::Error> {
        Self::from_rgba(val.pixels.to_owned(), val.width, val.height)
    }
}

/// Program is the required information to start an event loop, and therefore an `Engine`.
///
/// Remarks: The fullscreen/windowed setting this program describes may not be what Screen 13
/// chooses at runtime if there is a previously written configuration file present.
#[derive(Debug, Deserialize, Serialize)]
pub struct Program<'a, 'b> {
    /// Program author, or company.
    pub author: &'static str,

    /// Whether the program uses a full-screen video mode or not.
    pub fullscreen: bool,

    /// Program window icon, if set.
    pub icon: Option<Icon<'b>>,

    /// Program name, or title.
    pub name: &'static str,

    /// Whether the program window is resizable or not, while in window mode.
    ///
    /// Has no effect while in fullscreen mode.
    pub resizable: bool,

    /// Program window title.
    ///
    /// This is what is shown to the user.
    pub title: &'a str,
}

impl Program<'static, 'static> {
    /// A default program description, with a fullscreen setting.
    ///
    /// This is most useful for small examples and demos. Real programs should
    /// fill in all the info manually.
    pub const FULLSCREEN: Program<'static, 'static> = Program {
        author: DEFAULT_AUTHOR,
        fullscreen: true,
        icon: Some(Icon::DEFAULT),
        name: DEFAULT_NAME,
        resizable: true,
        title: DEFAULT_NAME,
    };

    /// A default program description, with a window mode setting.
    ///
    /// This is most useful for small examples and demos. Real programs should
    /// fill in all the info manually.
    pub const WINDOW: Program<'static, 'static> = Program {
        author: DEFAULT_AUTHOR,
        fullscreen: false,
        icon: Some(Icon::DEFAULT),
        name: DEFAULT_NAME,
        resizable: true,
        title: DEFAULT_NAME,
    };
}

impl Program<'_, '_> {
    /// Creates a new Program description.
    ///
    /// By default the program will be fullscreen; use the builder functions to change this and
    /// make other important choices.
    ///
    /// Remarks: Programs running in windowed mode automatically select an appropriately sized
    /// and placed window. Current logic provides a window centered on the primary display at HD
    /// resolution.
    ///
    /// Remarks: When in debug mode the default is windowed mode.
    pub const fn new(name: &'static str, author: &'static str) -> Self {
        #[cfg(not(debug_assertions))]
        let fullscreen = true;

        #[cfg(debug_assertions)]
        let fullscreen = false;

        Self {
            author,
            fullscreen,
            icon: None,
            name,
            resizable: true,
            title: name,
        }
    }

    const fn new_default() -> Self {
        Self::new(DEFAULT_NAME, DEFAULT_AUTHOR)
    }

    /// Sets whether the program starts as fullscreen of in window mode.
    pub const fn with_fullscreen(self) -> Self {
        self.with_fullscreen_is(true)
    }

    /// Sets whether the program starts as fullscreen of in window mode.
    pub const fn with_fullscreen_is(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    /// Sets the program name and program author (also known as publisher). These values are used
    /// for multiple purposes, including locating configuration files.
    pub const fn with_name_author(mut self, name: &'static str, author: &'static str) -> Self {
        self.author = author;
        self.name = name;
        self
    }

    /// Sets whether the window is resizable or not.
    pub const fn with_resizable(self) -> Self {
        self.with_resizable_is(true)
    }

    /// Sets whether the window is resizable or not.
    pub const fn with_resizable_is(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Sets whether the program starts in window mode instead of as fullscreen.
    pub const fn with_window(self) -> Self {
        self.with_window_is(true)
    }

    /// Sets whether the program starts in window mode instead of as fullscreen.
    pub const fn with_window_is(self, window: bool) -> Self {
        self.with_fullscreen_is(!window)
    }

    /// Clears the previously set window icon.
    pub fn without_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    /// Gets the filesystem root for this program. The returned path is a good place to store
    /// program configuration and data on a per-user basis.
    pub fn root(&self) -> Result<PathBuf, Error> {
        program_root(self)
    }
}

impl<'a> Program<'a, '_> {
    /// Sets the window title, separately from the program name which is used internally to cache
    /// configuration changes.
    pub const fn with_title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }
}

impl<'b> Program<'_, 'b> {
    /// Sets the window icon. The icon must be an rgba formatted pixel array, and must be square.
    pub fn with_icon(self, icon: Icon<'b>) -> Self {
        self.with_icon_is(Some(icon))
    }

    /// Sets the window icon. The icon must be an rgba formatted pixel array, and must be square.
    pub fn with_icon_is(mut self, icon: Option<Icon<'b>>) -> Self {
        self.icon = icon;
        self
    }
}

impl<'a, 'b> AsRef<Program<'a, 'b>> for Program<'a, 'b> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl Default for Program<'_, '_> {
    fn default() -> Self {
        Self::new_default()
    }
}
