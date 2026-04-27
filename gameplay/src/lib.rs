use fixed_point::Fixed;
use glam::DVec2;
use hecs::{Entity, World};
use input::InputState;
use level::Level;

const STEP_HEIGHT: f64 = 24.0;
const PLAYER_EYE_HEIGHT: f64 = 41.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorClass {
    Player,
    Monster,
    Item,
    Projectile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorState {
    Idle,
    Chase,
    Attack,
    Pain,
    Death,
}

#[derive(Debug, Clone)]
pub struct Actor {
    pub id: usize,
    pub class: ActorClass,
    pub type_id: i16,
    pub position: [Fixed; 2],
    pub angle: f64,
    pub velocity: [Fixed; 2],
    pub health: i32,
    pub max_health: i32,
    pub radius: Fixed,
    pub height: Fixed,
    pub state: ActorState,
    pub is_dead: bool,
    pub weapon_cooldown: f64,
}

pub struct Monster;
pub struct Position(pub [Fixed; 2]);
pub struct Velocity(pub [Fixed; 2]);
pub struct Health(pub i32);
pub struct Radius(pub Fixed);
pub struct Height(pub Fixed);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterState {
    Idle,
    Chase,
    Attack,
}

pub struct MonsterAI {
    pub state: MonsterState,
    pub target: Option<Entity>,
}

pub struct WorldState {
    pub world: World,
}

impl Actor {
    pub fn update_ai_systems(world: &mut World, _dt: f64, player_pos: [Fixed; 2]) {
        for (ai, pos, _vel) in world.query_mut::<(&mut MonsterAI, &Position, &mut Velocity)>() {
            let dist_sq = (pos.0[0] - player_pos[0]) * (pos.0[0] - player_pos[0])
                + (pos.0[1] - player_pos[1]) * (pos.0[1] - player_pos[1]);

            match ai.state {
                MonsterState::Idle => {
                    if dist_sq < Fixed::from_f64(1000.0 * 1000.0) {
                        ai.state = MonsterState::Chase;
                    }
                }
                MonsterState::Chase => {
                    if dist_sq < Fixed::from_f64(32.0 * 32.0) {
                        ai.state = MonsterState::Attack;
                    }
                    // AI movement logic would go here
                }
                MonsterState::Attack => {
                    if dist_sq > Fixed::from_f64(48.0 * 48.0) {
                        ai.state = MonsterState::Chase;
                    }
                }
            }
        }
    }

    pub fn pos_to_dvec2(&self) -> DVec2 {
        DVec2::new(self.position[0].to_f64(), self.position[1].to_f64())
    }

    pub fn radius_f64(&self) -> f64 {
        self.radius.to_f64()
    }

    pub fn height_f64(&self) -> f64 {
        self.height.to_f64()
    }

    pub fn add_pos(&mut self, move_vec: DVec2) {
        let new_pos_f64 = self.pos_to_dvec2() + move_vec;
        self.position = [
            Fixed::from_f64(new_pos_f64.x),
            Fixed::from_f64(new_pos_f64.y),
        ];
    }

    pub fn take_damage(&mut self, amount: i32) {
        if self.is_dead {
            return;
        }
        self.health -= amount;
        if self.health <= 0 {
            self.health = 0;
            self.is_dead = true;
            self.state = ActorState::Death;
        } else {
            self.state = ActorState::Pain;
        }
    }

    pub fn think(
        &mut self,
        dt: f64,
        input: &InputState,
        player: &mut Actor,
        other_actors: &mut [Actor],
        level: &Level,
    ) {
        if self.is_dead {
            return;
        }
        if self.weapon_cooldown > 0.0 {
            self.weapon_cooldown -= dt;
        }

        match self.class {
            ActorClass::Player => {
                let mut move_dir = DVec2::ZERO;
                if input.forward {
                    move_dir.y += 1.0;
                }
                if input.backward {
                    move_dir.y -= 1.0;
                }
                if input.left {
                    move_dir.x -= 1.0;
                }
                if input.right {
                    move_dir.x += 1.0;
                }

                if move_dir != DVec2::ZERO {
                    move_dir = move_dir.normalize();
                    let cos_a = self.angle.cos();
                    let sin_a = self.angle.sin();
                    let rotated_x = move_dir.x * cos_a - move_dir.y * sin_a;
                    let rotated_y = move_dir.x * sin_a + move_dir.y * cos_a;
                    let move_vec = DVec2::new(rotated_x, rotated_y) * 200.0 * dt;

                    let player_dummy = self.clone(); // Self is the player here
                    self.move_with_collision(move_vec, other_actors, &player_dummy, level);
                }

                let turn_speed = 3.0;
                if input.turn_left {
                    self.angle -= turn_speed * dt;
                }
                if input.turn_right {
                    self.angle += turn_speed * dt;
                }

                let mouse_sensitivity = 0.005;
                self.angle -= input.mouse_delta_x * mouse_sensitivity;

                self.angle = self.angle.rem_euclid(2.0 * std::f64::consts::PI);

                if input.fire && self.weapon_cooldown <= 0.0 {
                    self.weapon_cooldown = 0.2;
                    self.fire_hitscan(level, other_actors);
                }
            }
            ActorClass::Monster => {
                let to_player = player.pos_to_dvec2() - self.pos_to_dvec2();
                let dist = to_player.length();

                if dist < 1000.0 && dist > 32.0 {
                    self.state = ActorState::Chase;
                    let dir = to_player.normalize();
                    self.angle = f64::atan2(dir.y, dir.x);
                    let move_vec = dir * 80.0 * dt;
                    self.move_with_collision(move_vec, other_actors, player, level);
                } else if dist <= 32.0 {
                    self.state = ActorState::Attack;
                    let can_attack = line_of_sight_between_actors(level, self, player);
                    if can_attack && self.weapon_cooldown <= 0.0 && rand_simple() < 0.15 {
                        self.weapon_cooldown = 0.6;
                        player.take_damage(1);
                    }
                } else {
                    self.state = ActorState::Idle;
                }
            }
            _ => {}
        }
    }

    fn move_with_collision(
        &mut self,
        move_vec: DVec2,
        other_actors: &[Actor],
        player: &Actor,
        level: &Level,
    ) {
        let pos = self.pos_to_dvec2();
        let new_pos_f64 = pos + move_vec;

        let path_clear =
            level.movement_trace_clear(pos, new_pos_f64, self.height_f64(), STEP_HEIGHT);
        if path_clear
            && !self.check_collision(
                [
                    Fixed::from_f64(new_pos_f64.x),
                    Fixed::from_f64(new_pos_f64.y),
                ],
                other_actors,
                player,
                level,
            )
        {
            self.add_pos(move_vec);
        } else {
            let new_pos_x_f64 = pos + DVec2::new(move_vec.x, 0.0);
            let can_move_x =
                level.movement_trace_clear(pos, new_pos_x_f64, self.height_f64(), STEP_HEIGHT)
                    && !self.check_collision(
                        [
                            Fixed::from_f64(new_pos_x_f64.x),
                            Fixed::from_f64(new_pos_x_f64.y),
                        ],
                        other_actors,
                        player,
                        level,
                    );
            if can_move_x {
                self.add_pos(DVec2::new(move_vec.x, 0.0));
            }
            let new_pos_y_f64 = pos + DVec2::new(0.0, move_vec.y);
            let can_move_y =
                level.movement_trace_clear(pos, new_pos_y_f64, self.height_f64(), STEP_HEIGHT)
                    && !self.check_collision(
                        [
                            Fixed::from_f64(new_pos_y_f64.x),
                            Fixed::from_f64(new_pos_y_f64.y),
                        ],
                        other_actors,
                        player,
                        level,
                    );
            if can_move_y {
                self.add_pos(DVec2::new(0.0, move_vec.y));
            }
        }
    }

    fn check_collision(
        &self,
        pos: [Fixed; 2],
        other_actors: &[Actor],
        player: &Actor,
        level: &Level,
    ) -> bool {
        let pos_f64 = DVec2::new(pos[0].to_f64(), pos[1].to_f64());
        // Wall collision
        for (linedef_index, linedef) in level.linedefs.iter().enumerate() {
            if blocks_actor_movement(level, linedef_index, pos_f64, self.height_f64()) {
                let v1 = level.vertices[linedef.v1].p;
                let v2 = level.vertices[linedef.v2].p;
                let closest = closest_point_on_segment(pos_f64, v1, v2);
                if (pos_f64 - closest).length_squared() < self.radius_f64() * self.radius_f64() {
                    return true;
                }
            }
        }

        // Actor collision
        if self.class != ActorClass::Player {
            let dist_sq = (pos_f64 - player.pos_to_dvec2()).length_squared();
            let combined_radius = self.radius_f64() + player.radius_f64();
            if dist_sq < combined_radius * combined_radius {
                return true;
            }
        }

        for actor in other_actors {
            if actor.id == self.id || actor.is_dead {
                continue;
            }
            let dist_sq = (pos_f64 - actor.pos_to_dvec2()).length_squared();
            let combined_radius = self.radius_f64() + actor.radius_f64();
            if dist_sq < combined_radius * combined_radius {
                return true;
            }
        }

        false
    }

    fn fire_hitscan(&self, level: &Level, actors: &mut [Actor]) {
        let dir = DVec2::new(self.angle.cos(), self.angle.sin());
        let mut closest_dist = 1000.0;
        let mut hit_idx = None;
        let wall_limit = hitscan_wall_distance(self.pos_to_dvec2(), dir, level, self.height_f64())
            .unwrap_or(closest_dist);

        for (i, actor) in actors.iter().enumerate() {
            if actor.id == self.id || actor.is_dead || actor.class != ActorClass::Monster {
                continue;
            }

            let to_actor = actor.pos_to_dvec2() - self.pos_to_dvec2();
            let projection = to_actor.dot(dir);
            if projection < 0.0 || projection > wall_limit {
                continue;
            }

            let closest_point = self.pos_to_dvec2() + dir * projection;
            let dist_sq = (closest_point - actor.pos_to_dvec2()).length_squared();

            if dist_sq < actor.radius_f64() * actor.radius_f64()
                && line_of_sight_between_points(
                    level,
                    self.pos_to_dvec2(),
                    actor.pos_to_dvec2(),
                    actor_eye_z(level, self),
                    actor_center_z(level, actor),
                )
            {
                let d = to_actor.length();
                if d < closest_dist {
                    closest_dist = d;
                    hit_idx = Some(i);
                }
            }
        }

        if let Some(idx) = hit_idx {
            actors[idx].take_damage(10);
            println!(
                "Hit actor {}! HP left: {}",
                actors[idx].id, actors[idx].health
            );
        }
    }

    pub fn new(id: usize, class: ActorClass, type_id: i16, position: DVec2) -> Self {
        let (health, radius, height) = match class {
            ActorClass::Player => (100, 16.0, 56.0),
            ActorClass::Monster => monster_stats(type_id),
            ActorClass::Item => item_stats(type_id),
            ActorClass::Projectile => (1, 8.0, 8.0),
        };

        Self {
            id,
            class,
            type_id,
            position: [Fixed::from_f64(position.x), Fixed::from_f64(position.y)],
            angle: 0.0,
            velocity: [Fixed::from_f64(0.0), Fixed::from_f64(0.0)],
            health,
            max_health: health,
            radius: Fixed::from_f64(radius),
            height: Fixed::from_f64(height),
            state: ActorState::Idle,
            is_dead: false,
            weapon_cooldown: 0.0,
        }
    }
}

fn monster_stats(type_id: i16) -> (i32, f64, f64) {
    match type_id {
        3004 => (20, 20.0, 56.0),       // Zombieman
        9 => (30, 20.0, 56.0),          // Shotgun guy
        3001 => (60, 20.0, 56.0),       // Imp
        3002 | 58 => (150, 30.0, 56.0), // Demon/Spectre
        3005 => (400, 31.0, 56.0),      // Cacodemon
        3003 => (500, 24.0, 64.0),      // Baron
        3006 => (100, 16.0, 56.0),      // Lost soul
        7 => (3000, 128.0, 100.0),      // Spider mastermind
        16 => (4000, 40.0, 110.0),      // Cyberdemon
        _ => (50, 20.0, 56.0),
    }
}

fn item_stats(type_id: i16) -> (i32, f64, f64) {
    let radius = match type_id {
        2001..=2006 => 20.0,
        _ => 16.0,
    };
    (0, radius, 16.0)
}

fn closest_point_on_segment(p: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let ab = b - a;
    let t = (p - a).dot(ab) / ab.length_squared();
    let t = t.clamp(0.0, 1.0);
    a + ab * t
}

fn hitscan_wall_distance(
    origin: DVec2,
    dir: DVec2,
    level: &Level,
    actor_height: f64,
) -> Option<f64> {
    let mut best: Option<f64> = None;
    let eye_z = actor_eye_z_for_height(level, origin, actor_height);
    for (t, linedef_index) in level.segment_intersections(origin, origin + dir * 1000.0) {
        if t <= 0.0001 {
            continue;
        }
        let before = origin + dir * (1000.0 * (t - 0.0001).max(0.0));
        if !blocks_hitscan(level, linedef_index, before, eye_z) {
            continue;
        }
        let linedef = &level.linedefs[linedef_index];
        let v1 = level.vertices[linedef.v1].p;
        let v2 = level.vertices[linedef.v2].p;
        if let Some(t) = ray_segment_intersection(origin, dir, v1, v2) {
            if t >= 0.0 {
                best = Some(best.map(|b| b.min(t)).unwrap_or(t));
            }
        }
    }
    best
}

fn ray_segment_intersection(origin: DVec2, dir: DVec2, a: DVec2, b: DVec2) -> Option<f64> {
    let seg = b - a;
    let denom = cross(dir, seg);
    if denom.abs() < 1e-6 {
        return None;
    }
    let rel = a - origin;
    let t = cross(rel, seg) / denom;
    let u = cross(rel, dir) / denom;
    if t >= 0.0 && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn blocks_actor_movement(
    level: &Level,
    linedef_index: usize,
    point: DVec2,
    actor_height: f64,
) -> bool {
    let Some(opening) = level.opening_for_point_on_linedef(linedef_index, point) else {
        return true;
    };
    !level.actor_can_traverse_opening(&opening, actor_height, STEP_HEIGHT)
}

fn blocks_hitscan(level: &Level, linedef_index: usize, point: DVec2, z: f64) -> bool {
    let Some(opening) = level.opening_for_point_on_linedef(linedef_index, point) else {
        return true;
    };
    !level.opening_contains_height(&opening, z)
}

fn actor_center_z(level: &Level, actor: &Actor) -> f64 {
    level
        .find_sector(actor.pos_to_dvec2())
        .and_then(|idx| level.sectors.get(idx))
        .map(|sector| sector.floor_height + actor.height_f64() * 0.5)
        .unwrap_or(actor.height_f64() * 0.5)
}

fn actor_eye_z(level: &Level, actor: &Actor) -> f64 {
    actor_eye_z_for_height(level, actor.pos_to_dvec2(), actor.height_f64())
}

fn actor_eye_z_for_height(level: &Level, position: DVec2, actor_height: f64) -> f64 {
    let eye_offset = if actor_height >= 56.0 {
        PLAYER_EYE_HEIGHT
    } else {
        actor_height * 0.75
    };
    level
        .find_sector(position)
        .and_then(|idx| level.sectors.get(idx))
        .map(|sector| sector.floor_height + eye_offset)
        .unwrap_or(eye_offset)
}

fn line_of_sight_between_points(
    level: &Level,
    from: DVec2,
    to: DVec2,
    from_z: f64,
    to_z: f64,
) -> bool {
    level.line_of_sight_clear(from, from_z, to, to_z)
}

fn line_of_sight_between_actors(level: &Level, from: &Actor, to: &Actor) -> bool {
    line_of_sight_between_points(
        level,
        from.pos_to_dvec2(),
        to.pos_to_dvec2(),
        actor_eye_z(level, from),
        actor_center_z(level, to),
    )
}

fn rand_simple() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (n % 1000) as f64 / 1000.0
}
