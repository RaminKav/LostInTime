use bevy::{
    prelude::*,
    render::camera::{Camera2d, RenderTarget},
};

pub struct MousePlugin;

#[derive(Deref, Debug)]
pub struct MousePosition(pub Vec2);

impl Plugin for MousePlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(CoreStage::PreUpdate, mouse_position)
            .insert_resource(MousePosition(Vec2::default()));
    }
}

//Thanks Cheatbook! https://bevy-cheatbook.github.io/cookbook/cursor2world.html
fn mouse_position(
    // need to get window dimensions
    wnds: Res<Windows>,
    mut mouse_position: ResMut<MousePosition>,
    // query to get camera transform
    q_camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    // get the camera info and transform
    // assuming there is exactly one main camera entity, so query::single() is OK
    if let Ok((camera, camera_transform)) = q_camera.get_single() {
        // assuming camera has render target window (instead of Image)
        let window_id = match camera.target {
            RenderTarget::Window(win_id) => win_id,
            _ => panic!("expecting camera to have rendertarget window"),
        };

        // get the window that the camera is displaying to
        let wnd = wnds.get(window_id).unwrap();

        // check if the cursor is inside the window and get its position
        if let Some(screen_pos) = wnd.cursor_position() {
            // get the size of the window
            let window_size = Vec2::new(wnd.width() as f32, wnd.height() as f32);

            // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
            let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;

            // matrix for undoing the projection and camera transform
            let ndc_to_world =
                camera_transform.compute_matrix() * camera.projection_matrix.inverse();

            // use it to convert ndc to world-space coordinates
            let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));

            // reduce it to a 2D value
            let world_pos: Vec2 = world_pos.truncate();

            mouse_position.0 = world_pos;
        }
    }
}
