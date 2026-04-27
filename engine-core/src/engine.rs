use anyhow::Result;
use std::time::Instant;

pub trait Engine {
    fn run(&mut self) -> Result<()>;
}

pub struct GameLoop {
    pub last_tick: Instant,
    pub tick_rate: f64, // ticks per second
    pub accumulator: f64,
}

impl GameLoop {
    pub fn new(tick_rate: f64) -> Self {
        Self {
            last_tick: Instant::now(),
            tick_rate,
            accumulator: 0.0,
        }
    }

    pub fn update<F>(&mut self, mut tick_fn: F) -> Result<()>
    where
        F: FnMut(f64) -> Result<()>,
    {
        let now = Instant::now();
        let delta = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        self.accumulator += delta;

        let tick_duration = 1.0 / self.tick_rate;
        while self.accumulator >= tick_duration {
            tick_fn(tick_duration)?;
            self.accumulator -= tick_duration;
        }
        Ok(())
    }
}
