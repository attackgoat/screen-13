// Gorilla is a game for two players. Your mission is to hit your opponent with the exploding
// banana by varying the angle and power of your throw, taking into account wind speed,
// gravity, and the city skyline. I didn't have gorilla or banana graphics so we have bunnies
// and bullets instead, sorry folks.

#![allow(warnings)]

use {
    screen_13::{
        gpu::{Bitmap, Font},
        pak::Pak,
        prelude::*,
    },
    std::io::{Read, Seek},
};

const SCREEN_SIZE: Extent = Extent::new(640, 400);

fn main() -> ! {
    loop {}
}

enum BuildingType {
    Brick,
    Cement,
}

struct Building {
    height: usize,
    ty: BuildingType,
}

struct City {
    atlas: Bitmap,
    buildings: [Building; 12],
}

impl City {
    fn load<R: Read + Seek>(gpu: &Gpu, pak: &Pak<R>) -> Self {
        //Self {}
        todo!()
    }
}

struct Intro {
    city: City,
}

impl Screen for Intro {
    fn render(&self, gpu: &Gpu, _: Extent) -> Render {
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            "intro",
            SCREEN_SIZE,
        );

        frame
    }

    fn update(mut self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        self
    }
}

struct Gorilla {
    font: Font,
}

impl Screen for Gorilla {
    fn render(&self, gpu: &Gpu, _: Extent) -> Render {
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            "gorilla render",
            SCREEN_SIZE,
        );

        frame
    }

    fn update(mut self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        self
    }
}
