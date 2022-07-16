use bevy::prelude::*;

use crate::{
    assets::Graphics,
    item::{Breakable, WorldObject},
    mouse::MousePosition,
    GameState,
};

pub struct ChangeTilePlugin;

impl Plugin for ChangeTilePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_update(GameState::Main).with_system(change_tile));
    }
}
fn change_tile(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut breakable: Query<(Entity, &Transform, &WorldObject, &Breakable)>,
    mouse_input: Res<Input<MouseButton>>,
    mouse_position: Res<MousePosition>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }
    println!("{:?}", mouse_position);
    if let Some((ent, transform, world_object, breakable)) = breakable
        .iter_mut()
        .filter_map(|(ent, transform, world_object, breakable)| {
            if check_mouse_collides(transform, &mouse_position) {
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
