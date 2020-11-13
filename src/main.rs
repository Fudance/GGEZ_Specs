use ggez::event::{self, KeyCode, KeyMods};
use ggez::*;
use specs::*;
use specs_derive::*;
use std::env;
use std::path;
use std::sync::Arc;

const DESIRED_FPS: u32 = 60;

// COMPONENTS
// using VecStorage as a sensible default
#[derive(Component, Debug, PartialEq)]
#[storage(VecStorage)]
struct Position {
    position: nalgebra::Point2<f32>,
}

#[derive(Component, Copy, Clone, Debug, PartialEq)]
#[storage(VecStorage)]
struct CollisionBox {
    origin: nalgebra::Point2<f32>,
    height: f32,
    width: f32,
}

#[derive(Component, Debug, PartialEq)]
#[storage(VecStorage)]
struct Image {
    // images can be shared across multiple entities (as we do here)
    // specs needs to use components across threads and with a lifetime
    // longer than 'static. To make this work we need to use an Arc to
    // reference count across threads
    image: Arc<graphics::Image>,
}

// This is a tag to say something is player controllable
// we use null storage as we're only using this as a marker component
// see the specs book for more information:
// (https://slide-rs.github.io/specs/11_advanced_component.html)
// I had to derive Default to make this work
#[derive(Component, Default)]
#[storage(NullStorage)]
struct ControllableTag;

// SYSTEMS

// the update position system will update entities with the ControllableTag marker
// to keep things simple we won't bother with velocity and the delta time
// When we move the player, we also need to update their collision component
struct MovementSystem;
struct CollisionSystem;

impl<'a> System<'a> for MovementSystem {
    type SystemData = (
        Read<'a, Direction>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, CollisionBox>,
        ReadStorage<'a, ControllableTag>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (dir, mut pos, mut coll_box, controlled) = data;

        for (pos, coll_box, _) in (&mut pos, &mut coll_box, &controlled).join() {
            if dir.up {
                pos.position.y = pos.position.y - 10.0;
            }
            if dir.down {
                pos.position.y = pos.position.y + 10.0;
            }
            if dir.left {
                pos.position.x = pos.position.x - 10.0;
            }
            if dir.right {
                pos.position.x = pos.position.x + 10.0;
            }

            // if an entity has an updated position, we also need to update it's
            // collision box.
            coll_box.origin.x = pos.position.x;
            coll_box.origin.y = pos.position.y;
        }
    }
}

impl<'a> System<'a> for CollisionSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        ReadStorage<'a, CollisionBox>,
        ReadStorage<'a, ControllableTag>,
    );

    fn run(&mut self, data: Self::SystemData) {
        //println!("Running the collision system");
        let (pos, coll_box, controlled_storage) = data;

        // First find the player collision boxes, we don't assume a single player
        for (player_box, _) in (&coll_box, &controlled_storage).join() {
            // Now check all entities with a collision box that aren't player controlled
            for (_, coll_box, _) in (&pos, &coll_box, !&controlled_storage).join() {
                if player_box.origin.x < coll_box.origin.x + coll_box.width
                    && player_box.origin.x + player_box.width > coll_box.origin.x
                    && player_box.origin.y < coll_box.origin.y + coll_box.height
                    && player_box.origin.y + player_box.height > coll_box.origin.y
                {
                    println!("Collision detected");
                }
            }
        }
    }
}

// INTERNAL STRUCTS
// Direction is passed into the MovementSystem system via a resource
// we'll use a struct instead of an enum to capture multiple keys pressed at once
// this is still not great, but it'll do for example purposes
#[derive(Clone, Copy, Default)]
struct Direction {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Direction {
    fn new() -> Self {
        Direction {
            up: false,
            down: false,
            left: false,
            right: false,
        }
    }
}

struct MainState {
    dt: std::time::Duration,
    specs_world: World,
    player_input: Direction,
    movement_system: MovementSystem,
    collision_system: CollisionSystem,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let ship_image = graphics::Image::new(ctx, "/ship.PNG")?;
        let ship_height = ship_image.height() as f32;
        let ship_width = ship_image.width() as f32;
        let ship = Arc::new(ship_image);

        let dt = std::time::Duration::new(0, 0);

        // create a new world
        let mut world = World::new();
        world.register::<Position>();
        world.register::<CollisionBox>();
        world.register::<Image>();
        world.register::<ControllableTag>();

        // create our 2 spaceship Entities
        // intially we'll not add all the components while we figure out what we
        // need
        world
            .create_entity()
            .with(Position {
                position: nalgebra::Point2::new(75.0, 100.0),
            })
            .with(CollisionBox {
                origin: nalgebra::Point2::new(75.0, 100.0),
                height: ship_height,
                width: ship_width,
            })
            .with(Image {
                image: ship.clone(),
            })
            .with(ControllableTag)
            .build();

        // The static ship does not require the ControllableTag
        world
            .create_entity()
            .with(Position {
                position: nalgebra::Point2::new(275.0, 100.0),
            })
            .with(CollisionBox {
                origin: nalgebra::Point2::new(275.0, 100.0),
                height: ship_height,
                width: ship_width,
            })
            .with(Image {
                image: ship.clone(),
            })
            .build();

        // Create 2 structs to manage player input
        // One belongs to MainState and is kept up to date by the ggez event handling
        // The other belongs to the specs world and tracks the MainState struct
        let player_input = Direction::new();
        let player_input_world = Direction::new();

        // register the player controller with the world
        // add_resource is deprecated TODO - PR to update the book?
        world.insert(player_input_world);

        let update_pos = MovementSystem;
        let coll_system = CollisionSystem;

        let ms = MainState {
            dt: dt,
            specs_world: world,
            player_input: player_input,
            movement_system: update_pos,
            collision_system: coll_system,
        };

        Ok(ms)
    }
}

impl ggez::event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        while timer::check_update_time(ctx, DESIRED_FPS) {
            self.dt = timer::delta(ctx);

            //println!("dt = {}ns", self.dt.subsec_nanos());
            //println!("fps = {}", timer::fps(ctx));

            // run our update systems here
            self.movement_system.run_now(&self.specs_world);
            self.collision_system.run_now(&self.specs_world);

            self.specs_world.maintain();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, graphics::BLACK);

        // Get the components we need from the world for drawing
        let positions = self.specs_world.read_storage::<Position>();
        let images = self.specs_world.read_storage::<Image>();

        // this is our rendering "system"
        for (p, i) in (&positions, &images).join() {
            graphics::draw(
                ctx,
                &*i.image,
                graphics::DrawParam::default().dest(p.position),
            )
            .unwrap_or_else(|err| println!("draw error {:?}", err));
        }

        graphics::present(ctx)?;

        timer::yield_now();
        Ok(())
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        keycode: KeyCode,
        _keymod: KeyMods,
        repeat: bool,
    ) {
        if !repeat {
            // we don't multiple registrations of a keypress
            match keycode {
                KeyCode::Up => {
                    self.player_input.up = true;
                }
                KeyCode::Down => {
                    self.player_input.down = true;
                }
                KeyCode::Left => {
                    self.player_input.left = true;
                }
                KeyCode::Right => {
                    self.player_input.right = true;
                }
                _ => (),
            }
            // Update the world-owned player_input struct to match the current
            // state of the MainState owned struct
            let mut input_state = self.specs_world.write_resource::<Direction>();
            *input_state = self.player_input;
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
        match keycode {
            KeyCode::Up => {
                self.player_input.up = false;
            }
            KeyCode::Down => {
                self.player_input.down = false;
            }
            KeyCode::Left => {
                self.player_input.left = false;
            }
            KeyCode::Right => {
                self.player_input.right = false;
            }
            _ => (),
        }

        // track the MainState input in the Direction resource in the specs world
        let mut input_state = self.specs_world.write_resource::<Direction>();
        *input_state = self.player_input;
    }
}

fn main() {
    let resource_dir = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        path
    } else {
        path::PathBuf::from("./resources")
    };
    println!("Resource dir: {:?}", resource_dir);

    // create a context to start the main loop
    let mut c = conf::Conf::new();

    let win_setup = conf::WindowSetup {
        title: "GGEZ and specs test".to_owned(),
        samples: conf::NumSamples::Zero,
        vsync: true,
        icon: "".to_owned(),
        srgb: true,
    };

    c.window_setup = win_setup;

    let (ref mut ctx, ref mut event_loop) = ContextBuilder::new("ggez/specs", "Fudance")
        .conf(c)
        .add_resource_path(resource_dir)
        .build()
        .unwrap();

    let state = &mut MainState::new(ctx).unwrap();

    // start the main loop with the context and state
    event::run(ctx, event_loop, state).unwrap();
}
