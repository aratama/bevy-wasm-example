use crate::constant::MAX_WANDS;
use crate::entity::breakable::BreakableSprite;
use crate::spell::cast_spell;
use crate::wand::Wand;
use crate::{asset::GameAssets, command::GameCommand, states::GameState, world::CurrentLevel};
use bevy::prelude::*;
use bevy_light_2d::light::{PointLight2d, PointLight2dBundle};
use bevy_rapier2d::prelude::Group;
use bevy_simple_websocket::{ClientMessage, WebSocketState};
use std::f32::consts::PI;
use uuid::Uuid;

/// ライフを持ち、弾丸のダメージの対象となるエンティティを表します
#[derive(Component)]
pub struct Actor {
    pub uuid: Uuid,

    /// 次の魔法を発射できるまでのクールタイム
    pub cooltime: i32,

    pub reload_speed: i32,

    pub mana: i32,

    pub max_mana: i32,

    pub life: i32,
    pub max_life: i32,

    /// プレイヤーの位置からの相対的なポインターの位置
    pub pointer: Vec2,

    pub intensity: f32,

    pub move_state: ActorMoveState,

    pub fire_state: ActorFireState,

    /// 弾丸の発射をリモートに通知するかどうか
    /// プレイヤーキャラクターはtrue、敵キャラクターはfalseにします
    pub online: bool,

    pub group: Group,

    pub filter: Group,

    pub current_wand: usize,

    pub wands: [Option<Wand>; MAX_WANDS],
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ActorMoveState {
    Idle,
    Run,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ActorFireState {
    Idle,

    /// Actorのpointerに向かって弾丸を発射します
    Fire,
}

fn update_sprite_flip(
    actor_query: Query<&Actor>,
    mut sprite_query: Query<(&Parent, &mut Sprite), With<BreakableSprite>>,
) {
    for (parent, mut sprite) in sprite_query.iter_mut() {
        if let Ok(actor) = actor_query.get(parent.get()) {
            // プレイヤーの向き
            let angle = actor.pointer.y.atan2(actor.pointer.x);
            if angle < -PI * 0.5 || PI * 0.5 < angle {
                sprite.flip_x = true;
            } else {
                sprite.flip_x = false;
            }
        }
    }
}

fn recovery_mana(mut actor_query: Query<(&mut Actor, &Transform), Without<Camera2d>>) {
    for (mut actor, _) in actor_query.iter_mut() {
        actor.mana = (actor.mana + 1).min(actor.max_mana);
    }
}

#[derive(Component)]
pub struct ActorLight {
    owner: Entity,
}

fn update_actor_light(
    mut commands: Commands,
    mut light_query: Query<(Entity, &ActorLight, &mut PointLight2d, &mut Transform)>,
    actor_query: Query<(Entity, &Actor, &Transform), Without<ActorLight>>,
) {
    for (actor_entity, actor, transform) in actor_query.iter() {
        if light_query
            .iter()
            .find(|(_, light, _, _)| light.owner == actor_entity)
            .is_none()
        {
            // SpriteBundle に PointLight2d を追加すると、画面外に出た時に Sprite が描画されなくなり、
            // ライトも描画されず不自然になるため、別で追加する
            // https://github.com/jgayfer/bevy_light_2d/issues/26
            commands.spawn((
                ActorLight {
                    owner: actor_entity,
                },
                PointLight2dBundle {
                    transform: transform.clone(),
                    point_light: PointLight2d {
                        radius: 150.0,
                        intensity: actor.intensity,
                        falloff: 10.0,
                        ..default()
                    },
                    ..default()
                },
            ));
        }
    }

    for (light_entity, light, mut point_light, mut light_transform) in light_query.iter_mut() {
        if let Ok((_, actor, actor_transform)) = actor_query.get(light.owner) {
            point_light.intensity = actor.intensity;
            light_transform.translation.x = actor_transform.translation.x;
            light_transform.translation.y = actor_transform.translation.y;
        } else {
            commands.entity(light_entity).despawn_recursive();
        }
    }
}

/// 攻撃状態にあるアクターがスペルを詠唱します
fn fire_bullet(
    mut actor_query: Query<(&mut Actor, &mut Transform), Without<Camera2d>>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut writer: EventWriter<ClientMessage>,
    current: Res<CurrentLevel>,
    mut se_writer: EventWriter<GameCommand>,
    websocket: Res<WebSocketState>,
) {
    for (mut actor, actor_transform) in actor_query.iter_mut() {
        if actor.life <= 0 {
            return;
        }

        if let Some(wand) = &actor.wands[actor.current_wand] {
            if let Some(spell) = wand.slots[0] {
                if actor.fire_state == ActorFireState::Fire && actor.cooltime == 0 {
                    cast_spell(
                        &mut commands,
                        &assets,
                        &mut writer,
                        &current,
                        &mut se_writer,
                        &websocket,
                        &mut actor,
                        &actor_transform,
                        spell,
                    );
                } else {
                    actor.cooltime = (actor.cooltime - actor.reload_speed).max(0);
                }
            }
        }
    }
}

pub struct ActorPlugin;

impl Plugin for ActorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (update_sprite_flip, update_actor_light).run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            FixedUpdate,
            (fire_bullet, recovery_mana).run_if(in_state(GameState::InGame)),
        );
    }
}
