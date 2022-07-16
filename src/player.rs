use crate::assets::Graphics;
use crate::item::{Breakable, Tool, WorldObject};
use crate::mouse::MousePosition;
use crate::GameState;
use bevy::prelude::*;
use bevy::sprite::Anchor;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(GameState::Main).with_system(Self::spawn_player.after("graphics")),
        )
        .add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_system(Self::player_break_block)
                .with_system(Self::player_movement),
        );
    }
}

#[derive(Component, Default)]
pub struct Hands {
    pub tool: Option<Tool>,
}

#[derive(Component)]
pub struct Player {
    speed: f32,
    arm_length: f32,
}

impl PlayerPlugin {
    fn player_break_block(
        mut commands: Commands,
        mouse_input: Res<Input<MouseButton>>,
        graphics: Res<Graphics>,
        mouse_position: Res<MousePosition>,
        mut player_query: Query<(&Transform, &Player, &Hands)>,
        mut breakable_query: Query<(Entity, &Transform, &WorldObject, &Breakable)>,
    ) {
        if !mouse_input.just_pressed(MouseButton::Left) {
            return;
        }
        println!("{:?}", mouse_position);
        let (player_transform, player, hands) = player_query.single_mut();
        if let Some((ent, transform, world_object, breakable)) = breakable_query
            .iter_mut()
            .filter_map(|(ent, transform, world_object, breakable)| {
                let distance = transform
                    .translation
                    .truncate()
                    .distance(player_transform.translation.truncate());
                if player.arm_length > distance && check_mouse_collides(transform, &mouse_position)
                {
                    println!("{:?}", transform.translation);
                    Some((ent, transform, world_object, breakable))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .pop()
        {
            commands.entity(ent).despawn_recursive();
            if let Some(new_object) = breakable.turnsInto {
                //Become what you always were meant to be
                //println!("Pickupable found its new life as a {:?}", new_object);
                new_object.spawn(&mut commands, &graphics, transform.translation.truncate());
            }
        }
    }

    fn player_movement(
        keyboard: Res<Input<KeyCode>>,
        time: Res<Time>,
        mut player_query: Query<(&mut Transform, &Player)>,
    ) {
        let (mut player_transform, player) = player_query.single_mut();

        if keyboard.pressed(KeyCode::A) {
            player_transform.translation.x -= player.speed * time.delta_seconds();
        }
        if keyboard.pressed(KeyCode::D) {
            player_transform.translation.x += player.speed * time.delta_seconds();
        }
        if keyboard.pressed(KeyCode::W) {
            player_transform.translation.y += player.speed * time.delta_seconds();
        }
        if keyboard.pressed(KeyCode::S) {
            player_transform.translation.y -= player.speed * time.delta_seconds();
        }
        println!("{:?}", player_transform.translation)
    }

    fn spawn_player(mut commands: Commands, graphics: Res<Graphics>) {
        let mut sprite = TextureAtlasSprite::new(graphics.player_index);
        sprite.custom_size = Some(Vec2::splat(1.));
        sprite.anchor = Anchor::Custom(Vec2::new(0.0, 0.5 - 30.0 / 32.0));
        commands
            .spawn_bundle(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.clone(),
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, 700.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Player {
                speed: 3.0,
                arm_length: 2.0,
            })
            .insert(Hands { tool: None })
            .insert(Name::new("Player"));
    }
}

fn check_mouse_collides(transform: &Transform, mouse_position: &Res<MousePosition>) -> bool {
    //TODO: add custom anchor support, rn its center anchor
    let mouse_x = mouse_position[0];
    let mouse_y = mouse_position[1];

    let t_x = transform.translation[0];
    let t_y = transform.translation[1];

    return t_x - 0.25 < mouse_x
        && mouse_x < t_x + 0.25
        && t_y - 0.25 < mouse_y
        && mouse_y < t_y + 0.25;
}
