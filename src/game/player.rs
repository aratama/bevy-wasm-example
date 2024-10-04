use crate::game::constant::*;
use crate::game::ldtk_util::*;
use crate::game::serialize::*;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_aseprite_ultra::prelude::*;
use bevy_ecs_ldtk::prelude::*;
use bevy_rapier2d::prelude::*;
use std::f32::consts::PI;

#[derive(Component)]
pub struct Player;

pub fn setup_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player_data: &PlayerData,
) {
    commands.spawn((
        Player,
        AsepriteAnimationBundle {
            aseprite: asset_server.load("player.aseprite"),
            transform: Transform::from_xyz(player_data.x, player_data.y, 1.0),
            animation: Animation::default().with_tag("idle").with_speed(0.2),
            sprite: Sprite {
                // flip_x: true,
                // ここもanchorは効かないことに注意。Aseprite側のpivotで設定
                // anchor: bevy::sprite::Anchor::Custom(Vec2::new(0.0, 1.0)),
                ..default()
            },

            ..default()
        },
        KinematicCharacterController::default(),
        RigidBody::KinematicPositionBased,
        Collider::ball(5.0),
        GravityScale(0.0),
        LockedAxes::ROTATION_LOCKED,
    ));
}

pub fn update_player(
    keys: Res<ButtonInput<KeyCode>>,
    mut player_query: Query<
        (
            &mut Transform,
            &mut KinematicCharacterController,
            &GlobalTransform,
            &mut Sprite,
        ),
        (With<Player>, Without<Camera2d>),
    >,
    mut camera_query: Query<
        (&Camera, &mut Transform, &GlobalTransform),
        (With<Camera2d>, Without<Player>),
    >,

    ldtk_projects: Query<&Handle<LdtkProject>>,
    ldtk_project_assets: Res<Assets<LdtkProject>>,
    mut level_selection: ResMut<LevelSelection>,
    q_window: Query<&Window, With<PrimaryWindow>>,
) {
    let speed = 2.0;

    let velocity = Vec2::new(
        to_s(&keys, KeyCode::KeyD) - to_s(&keys, KeyCode::KeyA),
        to_s(&keys, KeyCode::KeyW) - to_s(&keys, KeyCode::KeyS),
    )
    .normalize_or_zero()
        * speed;

    let (mut player, mut controller, _, mut player_sprite) = player_query.single_mut();

    // 本棚などのエンティティが設置されているレイヤーは z が 3 に設定されているらしく、
    // y を z に変換する同一の式を設定しても、さらに 3 を加算してようやく z が合致するらしい
    // https://trouv.github.io/bevy_ecs_ldtk/v0.10.0/explanation/anatomy-of-the-world.html
    player.translation.z = 3.0 + (-player.translation.y * Z_ORDER_SCALE);
    controller.translation = Some(velocity);

    // println!(
    //     "player y:{} z:{}",
    //     player.translation.y, player.translation.z
    // );

    if let Ok((camera, mut camera_transform, camera_global_transform)) =
        camera_query.get_single_mut()
    {
        camera_transform.translation.x +=
            (player.translation.x - camera_transform.translation.x) * 0.1;
        camera_transform.translation.y +=
            (player.translation.y - camera_transform.translation.y) * 0.1;

        // プレイヤーの向き
        if let Ok(window) = q_window.get_single() {
            let cursor = window
                .cursor_position()
                .and_then(|cursor| camera.viewport_to_world(camera_global_transform, cursor))
                .map(|ray| ray.origin.truncate())
                .unwrap_or(Vec2::ZERO);

            let ray = cursor - player.translation.truncate();
            let angle = ray.y.atan2(ray.x);
            // println!("angle: {}", angle);
            if angle < -PI * 0.5 || PI * 0.5 < angle {
                player_sprite.flip_x = true;
            } else {
                player_sprite.flip_x = false;
            }

            // レベルの追従
            // https://trouv.github.io/bevy_ecs_ldtk/v0.10.0/how-to-guides/make-level-selection-follow-player.html
            // ただし上記の方法では読み込み済みのレベルを利用して現在のレベルを判定しているため、
            // 歩いて移動する場合は機能しますが、プレイヤーがワープしたり、セーブした位置から再開する場合は機能しません。
            // このため、常にアセット全体を検索して現在のレベルを判定しています
            let player_position = Vec2::new(player.translation.x, player.translation.y);
            if let Ok(project_handle) = ldtk_projects.get_single() {
                if let Some(ldtk_project) = ldtk_project_assets.get(project_handle) {
                    let found = find_level(ldtk_project, player_position);
                    if let Some(level_iid) = found {
                        *level_selection = LevelSelection::Iid(level_iid.clone());
                    }
                }
            }
        }
    }
}

fn to_s(keys: &Res<ButtonInput<KeyCode>>, code: bevy::input::keyboard::KeyCode) -> f32 {
    return if keys.pressed(code) { 1.0 } else { 0.0 };
}
