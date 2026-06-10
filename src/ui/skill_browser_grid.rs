use bevy::{prelude::*, render::view::RenderLayers};

use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    player::skills::{get_disabled_skills, ActiveSkill},
    ui::{
        heirloom_browser_grid::{grid_backdrop_size, GRID_CELL, GRID_COLS, GRID_ICON},
        interactions::Interactable,
        inventory_ui::UIState,
    },
};

/// Marks entities belonging to the dev skill picker beside the inventory dev buttons.
#[derive(Component)]
pub struct DevSkillPickerGridLayer;

#[derive(Component, Clone)]
pub struct DevSkillPickerIcon {
    pub active_skill: ActiveSkill,
}

pub fn despawn_dev_skill_picker_grid_layers(
    commands: &mut Commands,
    layers: &Query<Entity, With<DevSkillPickerGridLayer>>,
) {
    for e in layers.iter() {
        commands.entity(e).despawn_recursive();
    }
}

/// Every assignable active skill for the dev picker, sorted for grid display.
pub fn sorted_dev_skill_grid_entries() -> Vec<ActiveSkill> {
    let mut skills: Vec<ActiveSkill> = ActiveSkill::iter()
        .filter(|skill| {
            !get_disabled_skills().contains(skill)
                && *skill != ActiveSkill::LaserBeam
                && !skill.is_movement_skill()
        })
        .collect();
    skills.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    skills
}

/// Spawns the active-skill icon grid used by the dev skill picker.
pub fn spawn_skill_grid_overlay(
    commands: &mut Commands,
    graphics: &Graphics,
    center: Vec2,
    inner_w: f32,
    inner_h: f32,
    entries: Vec<ActiveSkill>,
    parent: Option<Entity>,
) {
    if entries.is_empty() {
        return;
    }

    let (backdrop_w, backdrop_h, grid_w) = grid_backdrop_size(entries.len(), inner_w, inner_h);

    let backdrop = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(35. / 255., 70. / 255., 70. / 255., 1.),
                    custom_size: Some(Vec2::new(backdrop_w, backdrop_h)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(center.x, center.y, 98.5)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            DevSkillPickerGridLayer,
            UIState::Inventory,
            Name::new("Skill grid backdrop"),
        ))
        .id();
    if let Some(parent_entity) = parent {
        commands.entity(parent_entity).add_child(backdrop);
    }

    let start_x = center.x - grid_w * 0.5 + GRID_CELL * 0.5;
    let start_y = center.y + backdrop_h * 0.5 - GRID_CELL * 0.65;

    for (i, active_skill) in entries.into_iter().enumerate() {
        let col = i % GRID_COLS;
        let row = i / GRID_COLS;
        let x = start_x + col as f32 * GRID_CELL;
        let y = start_y - row as f32 * GRID_CELL;

        let icon = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_active_skill_icon(active_skill),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(GRID_ICON, GRID_ICON)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(x, y, 99.5)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                Interactable::default(),
                DevSkillPickerIcon { active_skill },
                DevSkillPickerGridLayer,
                UIState::Inventory,
                Name::new("Skill grid icon"),
            ))
            .id();
        if let Some(parent_entity) = parent {
            commands.entity(parent_entity).add_child(icon);
        }
    }
}
