use anyhow::Result;
use gameplay::{Actor, ActorClass};
use glam::DVec2;
use input::InputState;
use level::{ActivationKind, KeyColor, Level, SpecialOutcome};

pub struct Game {
    pub level: Level,
    pub player: Actor,
    pub actors: Vec<Actor>,
    pub input: InputState,
    pub armor: i32,
    pub ammo_bullets: i32,
    pub ammo_shells: i32,
    pub keys: PlayerKeys,
    pub completed: Option<ExitResult>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerKeys {
    pub blue: bool,
    pub yellow: bool,
    pub red: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitResult {
    pub secret: bool,
}

impl Game {
    pub fn new(level: Level) -> Self {
        let mut player_pos = DVec2::new(0.0, 0.0);
        let mut player_angle = 0.0;
        let mut actors = Vec::new();
        let mut next_id = 1;

        for thing in &level.things {
            let pos = DVec2::new(thing.x as f64, thing.y as f64);
            match thing.type_id {
                1 => {
                    // Player 1 start
                    player_pos = pos;
                    player_angle = (thing.angle as f64).to_radians();
                }
                // Common Doom monsters
                3004 | 9 | 3001 | 3002 | 58 | 3003 | 3005 | 3006 | 7 | 16 => {
                    actors.push(Actor::new(next_id, ActorClass::Monster, thing.type_id, pos));
                    next_id += 1;
                }
                type_id if item_effect(type_id).is_some() => {
                    actors.push(Actor::new(next_id, ActorClass::Item, thing.type_id, pos));
                    next_id += 1;
                }
                _ => {}
            }
        }

        if actors.is_empty() {
            let fallback_pos = player_pos + DVec2::new(192.0, 0.0);
            actors.push(Actor::new(next_id, ActorClass::Monster, 3001, fallback_pos));
        }

        let mut player = Actor::new(0, ActorClass::Player, 1, player_pos);
        player.angle = player_angle;

        Self {
            level,
            player,
            actors,
            input: InputState::new(),
            armor: 0,
            ammo_bullets: 50,
            ammo_shells: 0,
            keys: PlayerKeys::default(),
            completed: None,
        }
    }

    pub fn tick(&mut self, dt: f64) -> Result<()> {
        let old_player_pos = self.player.pos_to_dvec2();
        if self.input.use_action {
            if let Some(linedef_index) =
                self.level
                    .use_linedef_index(self.player.pos_to_dvec2(), self.player.angle, 64.0)
            {
                if self.can_activate_linedef(linedef_index) {
                    let outcome = self
                        .level
                        .activate_linedef(linedef_index, ActivationKind::Use);
                    self.apply_outcome(outcome);
                } else {
                    println!("You need a key for this door.");
                }
            }
        }
        let outcome = self.level.tick_specials(dt);
        self.apply_outcome(outcome);

        let mut player_clone = self.player.clone();
        self.player.think(
            dt,
            &self.input,
            &mut player_clone,
            &mut self.actors,
            &self.level,
        );

        let outcome = self
            .level
            .activate_crossed_lines(old_player_pos, self.player.pos_to_dvec2());
        self.apply_outcome(outcome);
        self.collect_items();

        for i in 0..self.actors.len() {
            let (left, right) = self.actors.split_at_mut(i);
            let (actor, tail) = right.split_first_mut().expect("actor index in bounds");
            let mut others = Vec::with_capacity(left.len() + tail.len());
            others.extend(left.iter().cloned());
            others.extend(tail.iter().cloned());
            actor.think(dt, &self.input, &mut self.player, &mut others, &self.level);
        }

        if self.player.health < 100 {
            println!(
                "Player HP: {} (State: {:?})",
                self.player.health, self.player.state
            );
        }

        Ok(())
    }

    fn can_activate_linedef(&self, linedef_index: usize) -> bool {
        match self.level.required_key_for_linedef(linedef_index) {
            Some(KeyColor::Blue) => self.keys.blue,
            Some(KeyColor::Yellow) => self.keys.yellow,
            Some(KeyColor::Red) => self.keys.red,
            None => true,
        }
    }

    fn apply_outcome(&mut self, outcome: SpecialOutcome) {
        if let SpecialOutcome::Exit { secret } = outcome {
            self.completed = Some(ExitResult { secret });
            println!(
                "{} exit triggered.",
                if secret { "Secret" } else { "Normal" }
            );
        }
    }

    fn collect_items(&mut self) {
        let player_pos = self.player.pos_to_dvec2();
        let player_radius = self.player.radius_f64();
        let mut picked = Vec::new();

        for (idx, actor) in self.actors.iter().enumerate() {
            if actor.class != ActorClass::Item || actor.is_dead {
                continue;
            }
            let dist_sq = (actor.pos_to_dvec2() - player_pos).length_squared();
            let radius = actor.radius_f64() + player_radius;
            if dist_sq <= radius * radius {
                picked.push((idx, actor.type_id));
            }
        }

        for (idx, type_id) in picked.into_iter().rev() {
            if self.apply_item(type_id) {
                self.actors.swap_remove(idx);
            }
        }
    }

    fn apply_item(&mut self, type_id: i16) -> bool {
        let Some(effect) = item_effect(type_id) else {
            return false;
        };
        match effect {
            ItemEffect::Health(amount, max) => {
                if self.player.health >= max {
                    return false;
                }
                self.player.health = (self.player.health + amount).min(max);
            }
            ItemEffect::Armor(amount) => {
                if self.armor >= amount {
                    return false;
                }
                self.armor = amount;
            }
            ItemEffect::ArmorBonus(amount, max) => {
                if self.armor >= max {
                    return false;
                }
                self.armor = (self.armor + amount).min(max);
            }
            ItemEffect::AmmoBullets(amount) => self.ammo_bullets += amount,
            ItemEffect::AmmoShells(amount) => self.ammo_shells += amount,
            ItemEffect::Key(color) => match color {
                KeyColor::Blue => self.keys.blue = true,
                KeyColor::Yellow => self.keys.yellow = true,
                KeyColor::Red => self.keys.red = true,
            },
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
enum ItemEffect {
    Health(i32, i32),
    Armor(i32),
    ArmorBonus(i32, i32),
    AmmoBullets(i32),
    AmmoShells(i32),
    Key(KeyColor),
}

fn item_effect(type_id: i16) -> Option<ItemEffect> {
    match type_id {
        2011 => Some(ItemEffect::Health(10, 100)),
        2012 => Some(ItemEffect::Health(25, 100)),
        2014 => Some(ItemEffect::Health(1, 200)),
        2018 => Some(ItemEffect::Armor(100)),
        2019 => Some(ItemEffect::Armor(200)),
        2015 => Some(ItemEffect::ArmorBonus(1, 200)),
        2007 => Some(ItemEffect::AmmoBullets(10)),
        2002 => Some(ItemEffect::AmmoBullets(20)),
        2008 | 2001 => Some(ItemEffect::AmmoShells(4)),
        5 | 40 => Some(ItemEffect::Key(KeyColor::Blue)),
        6 | 39 => Some(ItemEffect::Key(KeyColor::Yellow)),
        13 | 38 => Some(ItemEffect::Key(KeyColor::Red)),
        _ => None,
    }
}
