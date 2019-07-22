# GGEZ & Specs ECS tutorial

This repository presents a simple example of wiring up GGEZ and the Specs
Entity Component System library. The code has been kept purposely short and in
a single file with lots of comments. A brief walkthrough of the code is
provided below. Examples of a simple movement systems and collisions have been
presented to help you understand how you can use the ECS pattern with GGEZ in
your own projects. 

I hope this helps bridge the gap between simple, "Hello, world" types usage of
ggez/specs together and more complex examples such as the "official" ggez game
template, which can be difficult to understand when first using these libraries
together. Note that the tutorial will mainly focus on the specs side, as that is
a little less intuitive at first, but where the libraries are glued together
will be pointed out as we go.

We'll also try to cover the 'why' as well as the 'how', as designing with an ECS
takes a little getting used to!


## Reference material 

I found the following links particularly useful when creating this example
project. I highly recommend you reading through them for more context:

[The specs book](https://slide-rs.github.io/specs/)

[The Amethyst book, concepts chapter](https://book.amethyst.rs/master/concepts/intro.html)

[The official ggez game template](https://github.com/ggez/game-template)

[SDL2 with specs example](https://github.com/sunjay/rust-simple-game-dev-tutorial)

[IOlivia Blog](https://iolivia.me/posts/entity-component-system-explained/)

[YouCodeThings Rust & Specs Roguelike](https://www.youtube.com/watch?v=1oSnLVE3YbA)

[Obsoke ARPG prototype](https://github.com/obsoke/arpg/blob/master/src/main.rs)


## Code Walkthrough

This walkthrough assumes you understand the basic concepts of what an ECS is
and how a simple GGEZ app is put together. If you're unsure I recommend
reviewing the specs book introduction and going through the [Hello ggez
tutorial](https://github.com/ggez/ggez/blob/master/docs/guides/HelloGgez.md). 
This guide is best consumed with the code open by its side.


### The Game

We're going to make a simple "game" with 2 spaceships on screen, one will be
controllable by the player and the other will be static. If the player
controller touches the static ship then a message will be printed alerting us
that a collision has taken place. This will mean we not only need simple
components for rendering, but we'll also need to handle some simple
interactions with the user (controlling the ship) and also between object
(collision detection).


### Entities, Components and Systems

Initially we need to think about what we want to achieve in our game and then
break this down into entities we require and what systems we'd want. Once we
have those in mind it is simpler to think about what components each
entity might need to enable our systems to work. The entities are simple, we
have 2 spaceships, one player controlled and the other static.

```
Entities 
    -> player controlled ship 
    -> static ship
```

The systems we should need are fairly intuitive as well. We'll need to detect
collisions between ships, control our player ship and also render the ships to
screen.

```
Systems 
    -> collision detection 
    -> movement 
    -> render 
```

Now we know what entities and systems we need, we can think about the atomic
components we might need to achieve them. Rendering is easy, we need some way
of storing the position of a entity and an image to draw, this means we need a
position component and an image. Collision detection will probably require
storing some kind of bounding shape to define the limits of our entity, in a
simple system this can be a bounding circle or square. We need some way of
flagging that an entity will be controlled by the player, so for that we will
need to use a marker component. A marker component has no associated data, it is
an empty struct and therefore we can use the Specs NullStorage type to store it
(see the Specs book for more information).

```
Components 
    -> position
    -> collision bounding shape
    -> image
    -> controllable (marker component)
```

In the code we define these components. Note that we're using specs-derive
macros to reduce the amount of boilerplate for defining a component:

``` rust
    #[derive(Component, Debug, PartialEq)]
    #[storage(VecStorage)]
    struct Position {
        position: nalgebra::Point2<f32>,
    }

    #[derive(Component, Debug, PartialEq)]
    #[storage(VecStorage)]
    struct CollisionBox {
        origin: nalgebra::Point2<f32>,
        height: f32,
        width: f32,
    }

    #[derive(Component, Debug, PartialEq)]
    #[storage(VecStorage)]
    struct Image {
        image: Arc<graphics::Image>,
    }

    #[derive(Component, Default)]
    #[storage(NullStorage)]
    struct ControllableTag;
```

Note the use of Arc in the Image component. We will want to reuse the same image
for each ship we render. Specs will complain about the lifetime of an image if
we try to use a reference. So we need to store the images on the heap and in a
way that is compatible with Specs multi-threaded nature, so we use Arc.

Now we have our components, we need to create a world to store them all in. We
store an instance of a world in the MainState and register the components in the
new function in the MainState impl block.

``` rust
    let mut world = World::new();
    world.register::<Position>();
    world.register::<CollisionBox>();
    world.register::<Image>();
    world.register::<ControllableTag>();

    let ms = MainState {
        dt: dt,
        specs_world: world,
    };

```

Now lets add our 2 entities. A player controlled ship and another static ship.
They each have the components to make our simple systems work. The only real
difference between these entities is that the first has the ControllableTag that
we use to mark this entity as one that can be controlled by the player.

``` rust
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
```

Now lets move onto the systems we'll use.

### More on Systems

Following the Specs documentation we would be tempted to try and make a
dispatcher to run our systems for us. However, when we try and do this with
GGEZ we quickly run into issues with the different threading models the 2
libraries take. This means that we'll have to run our systems manually in the
code instead of registering them with a Dispatcher in MainState.

This does have the benefit of making the Specs code fit into the GGEZ way of
doing things a bit more naturally. We will do our rendering in draw and update
entities in the update function. With a dispatcher we'd be doing all these
things from one place, either draw or, more likely, the update function.

#### Rendering

To do our rendering we join all our entities with a position and an image and
draw them inside the `draw()` function, as a rendering system:

``` rust
    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, graphics::BLACK);

        let positions = self.specs_world.read_storage::<Position>();
        let images = self.specs_world.read_storage::<Image>();

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
```

#### Player Movement

Our movement system needs to track the keys pressed by a user and make an
appropriate movement inside our system. To be able to do we'll decouple the
action of pressing/depressing a key away from the system that will be updating
the player position. 

Intially we start by handling the key presses within the key_down_event and
key_up_event handlers, the code inside these is pretty basic GGEZ stuff. Instead
of updating any co-ordinates when a key is pressed, we simple record the
currently active keys in a little struct, `Direction`. We're using a struct
instead of an enum because we want to allow multiple keys to be pressed at once
for diagonals etc. The struct needs to maintain state through different update
cycles, so we make it part of the MainState.

To enable specs to get at this data to update the player position we mirror this
direction struct and give it to the specs world as a resource. The extract below
shows where 2 Direction structs are created, one for MainState and the other is
inserted into the world as a resource so our systems can access the data.

```rust
    let player_input = Direction::new();
    let player_input_world = Direction::new();

    world.insert(player_input_world);
```

Every time a key event is processed we update the specs world instance of this
struct to ensure they both mirror each other.

```rust
    let mut input_state = self.specs_world.write_resource::<Direction>();
    *input_state = self.player_input;
```

Now that the world has an idea of what keys are being pressed, we add a system
that reads the Direction struct as a resource as well a join between all
entities with a position and the ControllableTag marker component. This should
move our ship. Note that to keep the code small the movement system is extremely
naive and only adds a number to the x/y co-ordinates of the ship. We could add a
velocity component or more physics systems if we were doing this properly.

Also, notice here that we have a CollisionBox component we need to update. That
will be explained in the next section.


```rust
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
```

To bring this all together, an instance of the UpdatePlayerPos system is put
into MainState and then run in the GGEZ update function:

```rust
    self.update_pos_system.run_now(&self.specs_world);
```

#### Collision Detection

The collision detection systems we're using is very simple. We're only
concerned with axis-aligned bounding box collisions and we're doing a brute
force comparison between all player objects and all other objects on screen- in
a real project we'd want some kind of broad-phase detection using a quad-tree
or spacial hash.

Here is how the system looks in the code:

```rust
impl<'a> System<'a> for CollisionSystem {
    type SystemData = (
        ReadStorage<'a, Position>,
        ReadStorage<'a, CollisionBox>,
        ReadStorage<'a, ControllableTag>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (pos, coll_box, controlled_storage) = data;

        for (player_box, _) in (&coll_box, &controlled_storage).join() {
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
```

This is easy stuff, we have an inner and outer loop. The outer loop is finding
all entities with a collision box and the controllable marker component, these
are player controlled entities. Notice that we're not assuming there is just
one. For each one of the player controlled entities we then run the inner loop
against it.

The inner loop finds all entities with a collision box and without the
controller component. These components must not be player controlled and
therefore we check each one of them against the player collision box. We do a
simple bounding box check and print a message to the terminal if a collision is
detected. The documentation shows how entities can be removed from the world in
systems, which is what we'd we want to do for a lot of collision types, for
example a player bullet hitting an enemy would require us to remove the bullet
and the enemy entities. This is left for an exercise for the reader :)

As previously shown, every time an entity is updated by the movement system we
also need to update it's collision component. This requires that we get all 
collidable components from storage and update it when the position of the entity
moves.
