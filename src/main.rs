use std::{cmp::Ordering, marker::PhantomData};

use bevy::{prelude::*, sprite::collide_aabb::collide, window::PrimaryWindow};

#[derive(Component, Default)]
struct Player;

#[derive(Component)]
struct Solid;

#[derive(Component)]
struct Ball;

#[derive(Component, Default)]
struct Movement {
    velocity: Vec2,
    velocity_remainder: Vec2,
    on_ground: bool,
}

#[derive(Component, Default)]
struct Racket;

#[derive(Component, Default)]
struct Size(Vec2);

#[derive(Component)]
struct Bounces(i8);

#[derive(Component)]
struct CollidesWithPlayer;

#[derive(Component)]
struct CollidesWithBall;

#[derive(Component, Default)]
struct Jump {
    var_jump_timer: f32,
    var_jump_speed: f32,
}

#[derive(Event)]
struct SolidCollisionEvent<T: Component> {
    collider: Entity,
    collided_x: bool,
    collided_y: bool,
    marker: PhantomData<T>,
}

// Process physics 60 ticks per second
const TIME_STEP: f32 = 1.0 / 60.0;
const VAR_JUMP_TIME: f32 = 0.2;
const JUMP_SPEED: f32 = -105.;
const MAX_RUN: f32 = 90.;
const RUN_ACCEL: f32 = 1000.;
const AIR_MULT: f32 = 0.65;
const PLAYER_MAX_FALL_SPEED: f32 = 160.;
const BALL_MAX_FALL_SPEED: f32 = 240.;
const HALF_GRAV_THRESHOLD: f32 = 40.;
const PLAYER_MASS: f32 = 900.;
const BALL_MASS: f32 = 1500.;
const MAX_BALL_BOUNCES: i8 = 1;
const GROUND_TILE_SIZE: f32 = 16.;
const PLAYER_SIZE: f32 = 32.;
const RACKET_SIZE: f32 = 16.;
const BALL_SIZE: f32 = 16.;

fn approach(val: f32, target: f32, max_move: f32) -> f32 {
    if val > target {
        target.max(val - max_move)
    } else {
        target.min(val + max_move)
    }
}

fn run_velocity_x(movement: &Movement, direction: f32) -> f32 {
    let mult = if movement.on_ground { 1. } else { AIR_MULT };
    approach(
        movement.velocity.x,
        MAX_RUN * direction,
        RUN_ACCEL * mult * TIME_STEP,
    )
}

fn player_movement_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<
        (
            Entity,
            &mut Movement,
            &mut Transform,
            &mut Jump,
            &mut AnimationIndices,
        ),
        With<Player>,
    >,
    mut commands: Commands
) {
    for (entity, mut movement, mut transform, mut jump, mut animation_indices) in &mut query {
        let is_jump_key_down = keyboard_input.pressed(KeyCode::Up);
        let is_left_key_down = keyboard_input.pressed(KeyCode::Left);
        let is_right_key_down = keyboard_input.pressed(KeyCode::Right);

        // apply gravity
        let abs_vel_y = movement.velocity.y.abs();
        let mult: f32 = if abs_vel_y < HALF_GRAV_THRESHOLD && is_jump_key_down {
            0.5
        } else {
            1.0
        };

        movement.velocity.y = approach(
            movement.velocity.y,
            PLAYER_MAX_FALL_SPEED,
            PLAYER_MASS * mult * TIME_STEP,
        );

        if jump.var_jump_timer > 0.0 {
            if is_jump_key_down {
                movement.velocity.y = jump.var_jump_speed.min(movement.velocity.y);
                jump.var_jump_timer -= TIME_STEP;
            } else {
                jump.var_jump_timer = 0.0;
            }
        }

        let mut is_running = false;
        if is_left_key_down {
            movement.velocity.x = run_velocity_x(movement.as_ref(), -1.);
            is_running = true;
            transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
        } else if is_right_key_down {
            movement.velocity.x = run_velocity_x(movement.as_ref(), 1.);
            is_running = true;
            transform.rotation = Quat::default();
        } else {
            movement.velocity.x = run_velocity_x(movement.as_ref(), 0.);
        }

        if !movement.on_ground {
            jump_animation(&mut animation_indices);
        } else if is_running {
            run_animation(&mut animation_indices);
        } else {
            idle_animation(&mut animation_indices);
        }

        let is_jump_just_pressed: bool = keyboard_input.just_pressed(KeyCode::Up);
        if is_jump_just_pressed && movement.on_ground {
            // init jump
            movement.velocity.y -= JUMP_SPEED;
            jump.var_jump_timer = VAR_JUMP_TIME;
            jump.var_jump_speed = JUMP_SPEED;
        }

        let is_space_just_pressed = keyboard_input.just_pressed(KeyCode::Space);
        if is_space_just_pressed {
            commands.entity(entity)
                .insert(Racket);
        }

        let is_space_just_released = keyboard_input.just_released(KeyCode::Space);
        if is_space_just_released {
            commands.entity(entity)
                .remove::<Racket>();
        }
    }
}

fn ball_movement_system(mut query: Query<&mut Movement, With<Ball>>) {
    let mut movement = query.get_single_mut().unwrap();
    if !movement.on_ground {
        movement.velocity.y = approach(
            movement.velocity.y,
            BALL_MAX_FALL_SPEED,
            BALL_MASS * TIME_STEP,
        );
    }
}

fn run_animation(animation_indices: &mut AnimationIndices) {
    animation_indices.first = 18;
    animation_indices.last = 21;
}

fn idle_animation(animation_indices: &mut AnimationIndices) {
    animation_indices.first = 15;
    animation_indices.last = 15;
}

fn jump_animation(animation_indices: &mut AnimationIndices) {
    animation_indices.first = 17;
    animation_indices.last = 17;
}

fn sign(number: i32) -> i32 {
    match number.cmp(&0) {
        Ordering::Less => -1,
        Ordering::Greater => 1,
        Ordering::Equal => 0,
    }
}

fn collision_system<T: Component>(
    solid_query: Query<&Transform, With<Solid>>,
    mut entity_query: Query<
        (Entity, &mut Movement, &mut Transform, &Size),
        (With<T>, Without<Solid>),
    >,
    mut collision_events: EventWriter<SolidCollisionEvent<T>>,
) {
    let (entity, mut entity_movement, mut entity_transform, entity_size) =
        entity_query.single_mut();
    let velocity_delta = entity_movement.velocity * TIME_STEP;
    entity_movement.velocity_remainder += velocity_delta;

    let mut move_x = entity_movement.velocity_remainder.x.round() as i32;
    let mut collided_x = false;
    if move_x != 0 {
        entity_movement.velocity_remainder.x -= move_x as f32;
        let move_sign = sign(move_x);

        while move_x != 0 && !collided_x {
            let new_kin_pos = entity_transform.translation + Vec3::new(move_sign as f32, 0.0, 0.0);

            for solid_transform in &solid_query {
                let collision = collide(
                    solid_transform.translation,
                    solid_transform.scale.truncate(),
                    new_kin_pos,
                    entity_size.0,
                );

                if collision.is_some() {
                    collided_x = true;
                    break;
                }
            }
            if !collided_x {
                entity_transform.translation.x += move_sign as f32;
                move_x -= move_sign;
            }
        }
    }

    let mut move_y = entity_movement.velocity_remainder.y.round() as i32;
    let mut collided_y = false;
    if move_y != 0 {
        entity_movement.velocity_remainder.y -= move_y as f32;
        let move_sign = sign(move_y);

        while move_y != 0 && !collided_y {
            for solid_transform in &solid_query {
                // Make it so we can use + sign here instead, right?
                let new_kin_pos =
                    entity_transform.translation - Vec3::new(0.0, move_sign as f32, 0.0);
                let collision = collide(
                    solid_transform.translation,
                    solid_transform.scale.truncate(),
                    new_kin_pos,
                    entity_size.0,
                );

                if collision.is_some() {
                    collided_y = true;
                    break;
                }
            }
            if !collided_y {
                entity_transform.translation.y -= move_sign as f32;
                move_y -= move_sign;
            }
        }

        entity_movement.on_ground = collided_y;
    }

    if collided_x || collided_y {
        collision_events.send(SolidCollisionEvent::<T> {
            collider: entity,
            collided_x,
            collided_y,
            marker: default(),
        });
    }
}

fn player_collision_response_system(
    mut query: Query<&mut Movement>,
    mut events: EventReader<SolidCollisionEvent<Player>>,
) {
    for event in events.iter() {
        let mut movement = query.get_mut(event.collider).unwrap();
        if event.collided_x {
            movement.velocity.x = 0.0;
        }
        if event.collided_y {
            movement.velocity.y = 0.0;
        }
    }
}

fn ball_collision_response_system(
    mut query: Query<(&mut Movement, &mut Bounces)>,
    mut events: EventReader<SolidCollisionEvent<Ball>>,
) {
    for event in events.iter() {
        let (mut movement, mut bounces) = query.get_mut(event.collider).unwrap();
        if event.collided_x {
            movement.velocity.x *= -1.5;
        }
        if event.collided_y {
            if bounces.0 >= MAX_BALL_BOUNCES {
                movement.velocity.y = 0.0;
                movement.on_ground = true;
                bounces.0 = 0;
            } else {
                movement.velocity.y *= -1.5;
                bounces.0 += 1;
            }
        }
    }
}

#[derive(Component)]
struct AnimationIndices {
    first: usize,
    last: usize,
}

// What is Deref, DerefMut?
#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

fn animate_player_sprite_system(
    time: Res<Time>,
    mut query: Query<(
        &AnimationIndices,
        &mut AnimationTimer,
        &mut TextureAtlasSprite,
    )>,
) {
    for (indices, mut timer, mut sprite) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            sprite.index = if sprite.index == indices.last
                || sprite.index < indices.first
                || sprite.index > indices.last
            {
                indices.first
            } else {
                sprite.index + 1
            };
        }
    }
}

fn object_debug_system(
    mut gizmos: Gizmos,
    solid_query: Query<&Transform, (With<Solid>, Without<Player>)>,
    player_query: Query<(&Transform, &Size, Option<&Racket>), With<Player>>,
    ball_query: Query<(&Transform, &Size), With<Ball>>,
) {
    let (player_transform, player_size, racket) = player_query.single();
    gizmos.rect_2d(
        player_transform.translation.truncate(),
        0.0,
        player_size.0,
        Color::GREEN,
    );
    if let Some(_racket) = racket {
        gizmos.rect_2d(
            player_transform.translation.truncate() + Vec2::new(16., 0.),
            0.0,
            Vec2::new(RACKET_SIZE, RACKET_SIZE),
            Color::DARK_GREEN,
        );
    }
    let (ball_transform, ball_size) = ball_query.single();
    gizmos.rect_2d(
        ball_transform.translation.truncate(),
        0.0,
        ball_size.0,
        Color::BLUE,
    );
    for solid in &solid_query {
        gizmos.rect_2d(
            solid.translation.truncate(),
            0.0,
            solid.scale.truncate(),
            Color::RED,
        );
    }
}

fn setup_system(
    mut commands: Commands,
    query: Query<&Window, With<PrimaryWindow>>,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    let Ok(window) = query.get_single() else {
        return;
    };

    commands.spawn(Camera2dBundle::default());
    // player
    let player_texture_handle = asset_server.load("player_atlas.png");
    let player_texture_atlas = TextureAtlas::from_grid(
        player_texture_handle,
        Vec2::new(8.0, 8.0),
        16,
        3,
        None,
        None,
    );
    let player_texture_atlas_handle = texture_atlases.add(player_texture_atlas);
    let animation_indices = AnimationIndices {
        first: 18,
        last: 21,
    };

    commands.spawn((
        SpriteSheetBundle {
            transform: Transform::from_scale(Vec3::splat(4.0)),
            texture_atlas: player_texture_atlas_handle,
            sprite: TextureAtlasSprite::new(animation_indices.first),
            ..default()
        },
        animation_indices,
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
        Player,
        Size(Vec2::new(PLAYER_SIZE, PLAYER_SIZE)),
        Movement { ..default() },
        Jump { ..default() },
    ));
    // ground
    let left_edge = (window.width() / 2.0) * -1.0;
    let bottom_edge = (window.height() / 2.0) * -1.0;

    commands.spawn((
        Solid,
        Transform {
            translation: Vec3::new(0.0, bottom_edge + (GROUND_TILE_SIZE / 2.0), 1.0),
            scale: Vec3::new(window.width(), GROUND_TILE_SIZE, 1.0),
            ..default()
        },
    ));

    // ground tiles
    let num_ground_tiles = (window.width() / GROUND_TILE_SIZE).ceil() as u32;
    let ground_tile_texture = asset_server.load("TennisCourtTile.png");

    for i in 0..num_ground_tiles {
        commands.spawn(SpriteBundle {
            transform: Transform {
                translation: Vec3::new(
                    left_edge + (i as f32 * GROUND_TILE_SIZE) + (GROUND_TILE_SIZE / 2.0),
                    bottom_edge + (GROUND_TILE_SIZE / 2.0),
                    0.0,
                ),
                ..default()
            },
            texture: ground_tile_texture.clone(),
            ..default()
        });
    }

    // ball
    let ball_texture = asset_server.load("ball.png");
    commands.spawn((
        Ball,
        SpriteBundle {
            transform: Transform {
                translation: Vec3::new(64.0, 0.0, 0.0),
                scale: Vec3::splat(2.0),
                ..default()
            },
            texture: ball_texture,
            ..default()
        },
        Size(Vec2::new(BALL_SIZE, BALL_SIZE)),
        Bounces(0),
        Movement { ..default() },
    ));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_event::<SolidCollisionEvent<Player>>()
        .add_event::<SolidCollisionEvent<Ball>>()
        .add_systems(Startup, setup_system)
        .add_systems(
            FixedUpdate,
            (
                player_movement_system,
                apply_deferred,
                collision_system::<Player>.after(player_movement_system),
                player_collision_response_system.after(collision_system::<Player>),
                animate_player_sprite_system.after(player_movement_system),
                ball_movement_system,
                collision_system::<Ball>.after(ball_movement_system),
                ball_collision_response_system.after(collision_system::<Ball>),
            ),
        )
        .add_systems(PostUpdate, object_debug_system)
        .insert_resource(FixedTime::new_from_secs(TIME_STEP))
        .run();
}
