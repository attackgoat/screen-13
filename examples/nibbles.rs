// Nibbles is a game for one player. There is only one level and the game only ends once your score
// overflows an unsigned platform-wide integer on your machine. You play as Sammy the snake on an
// 80x50 map bounded by a wall made of snake poison. To level up you must steer Sammy towards the
// green colored food. Use the arrow keys to control Sammy and have fun!

use {
    rand::random,
    screen_13::{
        camera::Orthographic,
        color::qb_color,
        gpu::{Command, Font},
        input::Key,
        math::{vec3, Coord},
        pak::Pak,
        prelude::*,
    },
    std::{
        env::current_exe,
        io::{Read, Seek},
        time::{Duration, Instant},
    },
};

const THINK_TIME: Duration = Duration::from_millis(120);
const SCREEN_SIZE: Extent = Extent::new(320, 200);

fn main() -> ! {
    let engine = Engine::new(Program::new("nibbles"));
    let mut pak = Pak::open(
        current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("nibbles.pak"),
    )
    .expect("ERROR: You must first pack the game content into a file by running the following command: `cargo run examples/content/nibbles.txt`");

    let nibbles = Box::new(Nibbles::load(engine.gpu(), &mut pak));

    engine.run(nibbles);
}

enum Direction {
    Down,
    Left,
    Right,
    Up,
}

struct Nibbles {
    direction: Direction,
    font: Font,
    food: Coord,
    last_update: Instant,
    sammy: Vec<Coord>,
    score: usize,
    view: Orthographic,
}

impl Nibbles {
    fn load<R: Read + Seek>(gpu: &Gpu, mut pak: &mut Pak<R>) -> Self {
        // We only need a font
        let font = gpu.load_font(&mut pak, "small_10px");

        // Setup an orthographic camera to provide a 2D view
        let eye = vec3(SCREEN_SIZE.x as f32 * 0.5, SCREEN_SIZE.y as f32 * 0.5, -1.0);
        let target = vec3(eye.x(), eye.y(), 0.0);
        let view = Orthographic::new(
            eye,
            target,
            0.0,
            SCREEN_SIZE.x as f32,
            SCREEN_SIZE.y as f32,
            0.0,
            -1.0,
            1.0,
        );

        let mut game = Self {
            direction: Direction::Right,
            font,
            food: Default::default(),
            last_update: Instant::now(),
            sammy: Default::default(),
            score: Default::default(),
            view,
        };

        // Setup the initial game state
        game.reset();

        game
    }

    fn detect_collision(&mut self) {
        let sammy = *self.sammy.last().unwrap();

        // Look for walls...
        if sammy.x < 1 || sammy.x > 79 || sammy.y < 1 || sammy.y > 49 {
            return self.reset();
        }

        // Make sure sammy doesn't get eaten
        for idx in 0..self.sammy.len() - 1 {
            if sammy == self.sammy[idx] {
                self.reset()
            }
        }
    }

    fn detect_food(&mut self) {
        let sammy_mouth = self.sammy.last().unwrap();
        if *sammy_mouth == self.food {
            self.move_food();
            self.score += 1;
        } else {
            // TODO: Looks like a VecDequeue should be used here for `sammy`?
            self.sammy.remove(0);
        }
    }

    /// Picks a new spot to place the 'food' sammy so desperately seeks. Makes no attempt to place the food
    /// on a non-sammy position.
    fn move_food(&mut self) {
        let x = random::<f32>() * 78.0;
        let y = random::<f32>() * 48.0;
        self.food = Coord::new(x as _, y as _);
    }

    fn move_sammy(&mut self, input: &Input) {
        // Update Sammy's direction of movement
        if input.keys.is_key_down(Key::Down) {
            self.direction = Direction::Down;
        } else if input.keys.is_key_down(Key::Left) {
            self.direction = Direction::Left;
        } else if input.keys.is_key_down(Key::Right) {
            self.direction = Direction::Right;
        } else if input.keys.is_key_down(Key::Up) {
            self.direction = Direction::Up;
        }

        // Figure out where Sammy is moving next (this may be a wall or food!)
        let last = self.sammy.last().unwrap();
        let next = match self.direction {
            Direction::Down => Coord::new(last.x, last.y + 1),
            Direction::Left => Coord::new(last.x - 1, last.y),
            Direction::Right => Coord::new(last.x + 1, last.y),
            Direction::Up => Coord::new(last.x, last.y - 1),
        };

        self.sammy.push(next);
    }

    fn reset(&mut self) {
        self.direction = Direction::Right;
        self.sammy.clear();
        self.score = 0;

        // Put a medium-sized snake in the center of the arena
        for x in -3..=3 {
            self.sammy.push(Coord::new(x + 39, 24));
        }

        // Set an initial food position
        self.move_food();
    }
}

impl Screen for Nibbles {
    fn render(&self, gpu: &Gpu) -> Render {
        let mut frame = gpu.render(
            #[cfg(debug_assertions)]
            "nibbles",
            SCREEN_SIZE,
        );

        // Blue background
        frame.clear(qb_color(1));

        // Player name on the left-top of the screen
        frame.text(
            #[cfg(debug_assertions)]
            "player name",
            &self.font,
            "Player: Sammy",
            Coord::new(2, 10),
            qb_color(15),
        );

        // Score at the right-top of the screen
        let score = format!("Score: {}", self.score);
        let score_size = self.font.measure(&score);
        frame.text(
            #[cfg(debug_assertions)]
            "score",
            &self.font,
            &score,
            Coord::new(SCREEN_SIZE.x as i32 - score_size.x as i32 - 1, 10),
            qb_color(15),
        );

        // Drawing commands for the arena (the deadly walls of death)...
        let arena_color = qb_color(4);
        let top_left = vec3(2.5, 12.5, 0.0);
        let top_right = vec3(SCREEN_SIZE.x as f32 - 2.5, 12.5, 0.0);
        let bottom_left = vec3(2.5, SCREEN_SIZE.y as f32 - 2.5, 0.0);
        let bottom_right = vec3(SCREEN_SIZE.x as f32 - 2.5, SCREEN_SIZE.y as f32 - 2.5, 0.0);
        let mut cmds = vec![
            Command::line(top_left, arena_color, top_right, arena_color, 4.0),
            Command::line(top_right, arena_color, bottom_right, arena_color, 4.0),
            Command::line(bottom_right, arena_color, bottom_left, arena_color, 4.0),
            Command::line(bottom_left, arena_color, top_left, arena_color, 4.0),
        ];

        // Drawing commands for sammy (the snake)...
        let sammy_color = qb_color(14);
        for seg in &self.sammy {
            let start = vec3(seg.x as f32 * 4.0, seg.y as f32 * 4.0, 0.0);
            let end = vec3(start.x() + 4.0, start.y() + 4.0, 0.0);
            cmds.push(Command::line(start, sammy_color, end, sammy_color, 4.0));
        }

        // Drawing commands for the food...
        let food_color = qb_color(10);
        {
            let start = vec3(self.food.x as f32 * 4.0, self.food.y as f32 * 4.0, 0.0);
            let end = vec3(start.x() + 4.0, start.y() + 4.0, 0.0);
            cmds.push(Command::line(start, food_color, end, food_color, 4.0));
        }

        // Send the Arena, Sammy, and Food as one batch. Lines will be drawn in the correct
        // z-order given their 3D coordinates however in this case all lines have a Z of 0,
        // so the order of submission is important - Food will be drawn last on top.
        frame.draw(
            #[cfg(debug_assertions)]
            "arena and sammy",
            &self.view,
            &mut cmds,
        );

        // Present the completed frame to the screen
        frame
    }

    fn update(mut self: Box<Self>, _: &Gpu, input: &Input) -> DynScreen {
        // Note: This way of handling time stepping is beyond horrible, please do not
        // copy this pattern! I did this to write less code not win hearts...
        let elapsed = Instant::now() - self.last_update;
        if elapsed > THINK_TIME {
            self.last_update = Instant::now();

            // Handle the game update logic
            self.move_sammy(input);
            self.detect_collision();
            self.detect_food();
        }

        // Return `self` which makes sure we keep drawing/updating this screen only
        self
    }
}
