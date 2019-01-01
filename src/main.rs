//! Basic hello world example.

extern crate ggez;
extern crate rand;
extern crate rodio;

use std::env;
use std::path;
use std::sync::mpsc::Sender;

use rand::prelude::*;

use ggez::conf;
use ggez::conf::{WindowMode, WindowSetup};
use ggez::event;
use ggez::event::Keycode;
use ggez::event::Mod;
use ggez::graphics;
use ggez::graphics::Point2;
use ggez::graphics::Vector2;
use ggez::timer;
use ggez::{Context, ContextBuilder, GameResult};

use ggez::nalgebra as na;

mod waves;
use waves::notes;
use waves::{make_waves, sine_wave, DynamicWave, WaveCommand, WaveUpdate};

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;
const DESIRED_FPS: u32 = 60;
const X_PLAYER_MAX_SPEED: f32 = 300.0;
const EAR_DIST: f32 = 250.0;
const RELOAD_TIME: f32 = 0.50;
const VISIBLE_TIME: f32 = 0.10;
const TARGET_AMPLITUDE: f32 = 0.20;

#[derive(Debug)]
struct InputState {
    left: bool,
    right: bool,
    shoot: bool,
    jump: bool,
}

#[derive(Debug)]
enum ActorType {
    Player,
    Enemy,
}

#[derive(Debug)]
struct Actor {
    tag: ActorType,
    pos: Point2,
    width: u32,
    height: u32,
    velocity: Vector2,
    tone: f32,
    sound_id: Option<u64>,
}

fn create_player(swave: &Sender<WaveCommand>) -> Actor {
    let sound_id = random();
    swave
        .send(WaveCommand::Replace(
            sound_id,
            DynamicWave::new(880.0, 0.0, sine_wave),
        ))
        .unwrap();
    Actor {
        tag: ActorType::Player,
        pos: Point2::new(0.0, SCREEN_HEIGHT as f32 - 50.0),
        velocity: na::zero(),
        width: 32,
        height: 32,
        tone: notes::A4,
        sound_id: Some(sound_id),
    }
}

fn create_enemy(swave: &Sender<WaveCommand>, note: f32) -> Actor {
    let sound_id = random();

    //Main tone
    swave
        .send(WaveCommand::Replace(
            sound_id,
            DynamicWave::new(440.0, 0.0, sine_wave),
        ))
        .unwrap();

    Actor {
        tag: ActorType::Enemy,
        pos: Point2::new(
            (random::<u32>() % SCREEN_WIDTH) as f32,
            (random::<u32>() % SCREEN_HEIGHT) as f32,
        ),
        velocity: Vector2::new(
            random::<u32>() as f32 % 100.0,
            random::<u32>() as f32 % 50.0,
        ),
        width: 64,
        height: 64,
        tone: note,
        sound_id: Some(sound_id),
    }
}

fn destroy_enemy(enemy: Actor, swave: &Sender<WaveCommand>) {
    if let Some(sound_id) = enemy.sound_id {
        swave.send(WaveCommand::Delete(sound_id)).unwrap();
    }
}

impl Actor {
    fn draw(
        &self,
        assets: &mut Assets,
        ctx: &mut Context,
        _world_coords: (u32, u32),
    ) -> GameResult<()> {
        //let (sh, sw) = world_coords;
        let image = assets.actor_image(self);
        let drawparams = graphics::DrawParam {
            dest: self.pos,
            rotation: 0.0,
            offset: graphics::Point2::new(0.0, 0.0),
            ..Default::default()
        };
        graphics::draw_ex(ctx, image, drawparams)
    }

    fn center(&self) -> Point2 {
        Point2::new(
            self.pos.x + self.width as f32 / 2.0,
            self.pos.y + self.height as f32 / 2.0,
        )
    }
}

struct Assets {
    player_image: graphics::Image,
    enemy_image: graphics::Image,
    ray_image: graphics::Image,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_image = graphics::Image::new(ctx, "/player.png")?;
        let enemy_image = graphics::Image::new(ctx, "/rock64.png")?;
        let ray_image = graphics::Image::new(ctx, "/ray.png")?;

        Ok(Assets {
            player_image,
            enemy_image,
            ray_image,
        })
    }

    fn actor_image(&mut self, actor: &Actor) -> &mut graphics::Image {
        match actor.tag {
            ActorType::Player => &mut self.player_image,
            ActorType::Enemy => &mut self.enemy_image,
        }
    }
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            left: false,
            right: false,
            shoot: false,
            jump: false,
        }
    }
}

struct Gun {
    time_to_reload: f32,
    visible: bool,
}

impl Default for Gun {
    fn default() -> Gun {
        Gun {
            time_to_reload: 0.0,
            visible: false,
        }
    }
}

impl Gun {
    fn draw(&self, player: &Actor, assets: &mut Assets, ctx: &mut Context) -> GameResult<()> {
        if self.visible {
            let image = &assets.ray_image;
            let drawparams = graphics::DrawParam {
                dest: player.pos
                    + Vector2::new(
                        player.width as f32 / 2.0 - image.width() as f32 / 2.0,
                        -1000.0,
                    ),
                rotation: 0.0,
                offset: graphics::Point2::new(0.0, 0.0),
                ..Default::default()
            };
            graphics::draw_ex(ctx, image, drawparams)
        } else {
            Ok(())
        }
    }
}

struct Level {
    notes: Vec<f32>,
}

impl Default for Level {
    fn default() -> Level {
        use notes::*;

        let mut vie_en_rose = vec![
            //Hold me close and hold me fast
            C5, B4, A4, G4, E4, C5, B4, //This magic spell you cast
            A4, G4, E4, C4, B4, A4, //This is la vie en rose
            G4, E4, C4, C4, B4, A4, G4, //When you kiss me heaven sighs
            C5, B4, A4, G4, E4, C5, B4, //And though I close my eyes
            A4, G4, E4, C4, B4, A4, //I see la vie en rose
            G4, E4, C4, C4, B4, A4, G4, //When you press me to your heart
            C5, B4, A4, G4, E4, C5, B4, //I'm in a world apart
            A4, G4, E4, C4, B4, A4, //A world where roses bloom
            G4, E4, C4, C5, C5, C5, //And when you speak angels sing from above
            D5, D5, C5, D5, D5, C5, D5, D5, C5, G4,
            //Everyday words seem to turn into love songs
            D5, D5, C5, D5, D5, C5, D5, D5, C5, E5, D5, //Give your heart and soul to me
            C5, B4, A4, G4, E4, C5, B4, //And life will always be
            A4, G4, E4, C4, B4, A4, //La vie en rose
            G4, A4, B4, C5,
        ];
        vie_en_rose.reverse();

        Level { notes: vie_en_rose }
    }
}

// First we make a structure to contain the game's state
struct MainState {
    text: graphics::Text,
    assets: Assets,
    frames: usize,
    input: InputState,
    player: Actor,
    enemies: Vec<Actor>,
    swave: Sender<WaveCommand>,
    gun: Gun,
    levels: Vec<Level>,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        // The ttf file will be in your resources directory. Later, we
        // will mount that directory so we can omit it in the path here.
        let font = graphics::Font::new(ctx, "/DejaVuSerif.ttf", 48)?;
        let text = graphics::Text::new(ctx, "Hello world!", &font)?;

        let swave = make_waves();

        let mut s = MainState {
            text,
            frames: 0,
            input: InputState::default(),
            player: create_player(&swave),
            enemies: Vec::new(),
            assets: Assets::new(ctx)?,
            swave,
            gun: Gun::default(),
            levels: vec![Level::default()],
        };
        Ok(s)
    }
}

// Then we implement the `ggez:event::EventHandler` trait on it, which
// requires callbacks for updating and drawing the game state each frame.
//
// The `EventHandler` trait also contains callbacks for event handling
// that you can override if you wish, but the defaults are fine.
impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        while timer::check_update_time(ctx, DESIRED_FPS) {
            let seconds = 1.0 / (DESIRED_FPS as f32);

            if self.enemies.len() < 1 && self.levels.len() > 0 {
                if let Some(note) = self.levels[0].notes.pop() {
                    self.enemies.push(create_enemy(&self.swave, note));
                }
            }

            handle_player_input(&mut self.player, &self.input, seconds);

            apply_motion(&mut self.player, seconds);
            apply_walls(&mut self.player, false);

            //if (self.frames % 100) == 0 {
            for e in &mut self.enemies {
                apply_motion(e, seconds);
                apply_walls(e, true);
                update_enemy_sound(&self.player, e, &self.swave);
            }
            // }

            update_player_sound(&self.player, &self.enemies, &self.swave);

            handle_shoot(self, seconds);
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);

        // // Drawables are drawn from their top-left corner.
        // let dest_point = graphics::Point2::new(10.0, 10.0);
        // graphics::draw(ctx, &self.text, dest_point, 0.0)?;

        self.player.draw(&mut self.assets, ctx, (0, 0))?;

        for e in &mut self.enemies {
            e.draw(&mut self.assets, ctx, (0, 0))?;
        }

        self.gun.draw(&self.player, &mut self.assets, ctx)?;

        self.frames += 1;
        if (self.frames % 100) == 0 {
            println!("FPS: {}", ggez::timer::get_fps(ctx));
        }

        graphics::present(ctx);

        Ok(())
    }

    // Handle key events.  These just map keyboard events
    // and alter our input state appropriately.
    fn key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        match keycode {
            Keycode::Left => {
                self.input.left = true;
            }
            Keycode::Right => {
                self.input.right = true;
            }
            Keycode::Up => {
                self.input.jump = true;
            }
            Keycode::Space => {
                self.input.shoot = true;
            }
            Keycode::P => {
                let img = graphics::screenshot(ctx).expect("Could not take screenshot");
                img.encode(ctx, graphics::ImageFormat::Png, "/screenshot.png")
                    .expect("Could not save screenshot");
            }
            Keycode::Escape => ctx.quit().unwrap(),
            _ => (), // Do nothing
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        match keycode {
            Keycode::Left => self.input.left = false,
            Keycode::Right => self.input.right = false,
            Keycode::Space => self.input.shoot = false,
            Keycode::Up => {
                self.input.jump = false;
            }
            _ => (), // Do nothing
        }
    }
}

fn handle_player_input(player: &mut Actor, input: &InputState, dseconds: f32) {
    let cont = 0.0 + if input.left { -1.0 } else { 0.0 } + if input.right { 1.0 } else { 0.0 };

    player.pos.x += cont * X_PLAYER_MAX_SPEED * dseconds;
}

fn update_enemy_sound(player: &Actor, enemy: &Actor, swave: &Sender<WaveCommand>) {
    let _dist = (player.pos - enemy.pos).norm_squared();
    //let amp = 1.0 - (player.pos.y - enemy.pos.y).abs() / SCREEN_HEIGHT as f32;

    let freq = enemy.tone;

    let mut leftamp =
        1.0 - (((player.center().x - EAR_DIST) - enemy.pos.x).abs() / SCREEN_WIDTH as f32);
    leftamp *= leftamp;
    let mut rightamp =
        1.0 - ((player.center().x + EAR_DIST) - enemy.pos.x).abs() / SCREEN_WIDTH as f32;
    rightamp *= rightamp;

    // let sideamp = if player.center().x < enemy.pos.x {
    //     (amp, 0.0)
    // } else if player.center().x > enemy.pos.x + enemy.width as f32 {
    //     (0.0, amp)
    // }
    // else {
    //     (amp, amp)
    // };

    //println!("{:?} {:?}", freq, amp);

    swave
        .send(WaveCommand::Update(
            enemy.sound_id.unwrap(),
            WaveUpdate {
                freq,
                amp: (leftamp, rightamp),
            },
        ))
        .unwrap();
}

fn update_player_sound(player: &Actor, enemies: &Vec<Actor>, swave: &Sender<WaveCommand>) {
    let command = if let Some(e) = &enemies
        .iter()
        .find(|e| e.pos.x < player.center().x && e.pos.x + e.width as f32 > player.center().x)
    {
        WaveUpdate {
            freq: e.tone + 6.0,
            amp: (TARGET_AMPLITUDE, TARGET_AMPLITUDE),
        }
    } else {
        WaveUpdate {
            freq: 0.0,
            amp: (0.0, 0.0),
        }
    };

    swave
        .send(WaveCommand::Update(player.sound_id.unwrap(), command))
        .unwrap();
}

fn apply_motion(a: &mut Actor, dseconds: f32) {
    a.pos += a.velocity.map(|i| i * dseconds);
}

fn apply_walls(a: &mut Actor, gutter: bool) {
    let bottom = if gutter { 50.0 } else { 0.0 };

    if a.pos.x + a.width as f32 > SCREEN_WIDTH as f32 {
        a.pos.x = (SCREEN_WIDTH - a.width) as f32;
        a.velocity.x = -a.velocity.x
    }
    if a.pos.x < 0.0 {
        a.pos.x = 0.0;
        a.velocity.x = -a.velocity.x
    }
    if a.pos.y + a.height as f32 > SCREEN_HEIGHT as f32 - bottom {
        a.pos.y = (SCREEN_HEIGHT - bottom as u32 - a.height) as f32;
        a.velocity.y = -a.velocity.y
    }
    if a.pos.y < 0.0 {
        a.pos.y = 0.0;
        a.velocity.y = -a.velocity.y
    }
}

fn handle_shoot(state: &mut MainState, dseconds: f32) {
    state.gun.time_to_reload = f32::max(0.0, state.gun.time_to_reload - dseconds);

    if state.input.shoot && state.gun.time_to_reload <= 0.0 {
        let idx = (0..state.enemies.len()).into_iter().find(|&i| {
            let e = &state.enemies[i];
            if e.pos.x < state.player.center().x
                && e.pos.x + e.width as f32 > state.player.center().x
            {
                true
            } else {
                false
            }
        });

        match idx {
            Some(i) => destroy_enemy(state.enemies.remove(i), &state.swave),
            None => (),
        };

        state.gun.time_to_reload = RELOAD_TIME;
    }

    state.gun.visible = state.gun.time_to_reload > RELOAD_TIME - VISIBLE_TIME;
}

// Now our main function, which does three things:
//
// * First, create a new `ggez::conf::Conf`
// object which contains configuration info on things such
// as screen resolution and window title.
// * Second, create a `ggez::game::Game` object which will
// do the work of creating our MainState and running our game.
// * Then, just call `game.run()` which runs the `Game` mainloop.
pub fn main() {
    //let c = conf::Conf::new();
    let ctx = &mut ContextBuilder::new("helloworld", "ggez")
        .window_setup(WindowSetup::default().title("Tonal"))
        .build()
        .unwrap();

    graphics::set_mode(
        ctx,
        WindowMode::default().dimensions(SCREEN_WIDTH, SCREEN_HEIGHT),
    )
    .unwrap();

    //let ctx = &mut Context::load_from_conf("helloworld", "ggez", c).unwrap();

    // We add the CARGO_MANIFEST_DIR/resources to the filesystem's path
    // so that ggez will look in our cargo project directory for files.
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        ctx.filesystem.mount(&path, true);
    }

    let state = &mut MainState::new(ctx).expect("Could not create MainState");
    if let Err(e) = event::run(ctx, state) {
        println!("Error encountered: {}", e);
    } else {
        println!("Game exited cleanly.");
    }
}
