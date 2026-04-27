use glam::DVec2;

#[derive(Debug, Clone)]
pub struct Vertex {
    pub p: DVec2,
}

#[derive(Debug, Clone)]
pub struct Sector {
    pub floor_height: f64,
    pub ceiling_height: f64,
    pub floor_texture: String,
    pub ceiling_texture: String,
    pub light_level: i16,
    pub special: i16,
    pub tag: i16,
}

#[derive(Debug, Clone)]
pub struct SideDef {
    pub texture_offset: f64,
    pub row_offset: f64,
    pub top_texture: String,
    pub bottom_texture: String,
    pub mid_texture: String,
    pub sector: usize,
}

#[derive(Debug, Clone)]
pub struct LineDef {
    pub v1: usize,
    pub v2: usize,
    pub flags: u16,
    pub special: u16,
    pub tag: i16,
    pub sidedef: [Option<usize>; 2],
    pub sectors: [Option<usize>; 2], // front, back
}

#[derive(Debug, Clone)]
pub struct Seg {
    pub v1: usize,
    pub v2: usize,
    pub angle: u16,
    pub linedef: Option<usize>,
    pub side: u8,
    pub offset: u16,
}

#[derive(Debug, Clone)]
pub struct SubSector {
    pub num_segs: u16,
    pub first_seg: usize,
    pub sector: usize,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub x: i16,
    pub y: i16,
    pub dx: i16,
    pub dy: i16,
    pub bbox: [[i16; 4]; 2],
    pub children: [u16; 2],
}

#[derive(Debug, Clone)]
pub struct Thing {
    pub x: i16,
    pub y: i16,
    pub angle: i16,
    pub type_id: i16,
    pub flags: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorKind {
    Open,
    Raise,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyColor {
    Blue,
    Yellow,
    Red,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialOutcome {
    None,
    Exit { secret: bool },
}

#[derive(Debug, Clone)]
pub struct ActiveDoor {
    pub sector: usize,
    pub kind: DoorKind,
    pub direction: i8,
    pub speed: f64,
    pub top_height: f64,
    pub bottom_height: f64,
    pub wait_time: f64,
    pub countdown: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorKind {
    Move,
    Lift,
}

#[derive(Debug, Clone)]
pub struct ActiveFloor {
    pub sector: usize,
    pub kind: FloorKind,
    pub direction: i8,
    pub speed: f64,
    pub top_height: f64,
    pub bottom_height: f64,
    pub wait_time: f64,
    pub countdown: f64,
}

#[derive(Debug, Clone)]
pub struct OpeningInfo {
    pub front_sector: usize,
    pub back_sector: Option<usize>,
    pub front_floor: f64,
    pub front_ceiling: f64,
    pub opening_bottom: f64,
    pub opening_top: f64,
    pub solid: bool,
    pub masked_middle: bool,
}

#[derive(Debug)]
pub struct Level {
    pub vertices: Vec<Vertex>,
    pub sectors: Vec<Sector>,
    pub sidedefs: Vec<SideDef>,
    pub linedefs: Vec<LineDef>,
    pub segs: Vec<Seg>,
    pub subsectors: Vec<SubSector>,
    pub nodes: Vec<Node>,
    pub things: Vec<Thing>,
    pub active_doors: Vec<ActiveDoor>,
    pub active_floors: Vec<ActiveFloor>,
}

impl Level {
    pub fn tick_specials(&mut self, dt: f64) -> SpecialOutcome {
        let mut finished = Vec::new();
        for (idx, door) in self.active_doors.iter_mut().enumerate() {
            let sector = &mut self.sectors[door.sector];
            match door.direction {
                0 => {
                    door.countdown -= dt;
                    if door.countdown <= 0.0 && door.kind == DoorKind::Raise {
                        door.direction = -1;
                    }
                }
                1 => {
                    sector.ceiling_height =
                        (sector.ceiling_height + door.speed * dt).min(door.top_height);
                    if (sector.ceiling_height - door.top_height).abs() < 0.001 {
                        match door.kind {
                            DoorKind::Open => finished.push(idx),
                            DoorKind::Raise => {
                                door.direction = 0;
                                door.countdown = door.wait_time;
                            }
                            DoorKind::Close => {}
                        }
                    }
                }
                -1 => {
                    sector.ceiling_height =
                        (sector.ceiling_height - door.speed * dt).max(door.bottom_height);
                    if (sector.ceiling_height - door.bottom_height).abs() < 0.001 {
                        match door.kind {
                            DoorKind::Close | DoorKind::Raise => finished.push(idx),
                            DoorKind::Open => {}
                        }
                    }
                }
                _ => {}
            }
        }

        for idx in finished.into_iter().rev() {
            self.active_doors.swap_remove(idx);
        }

        let mut finished = Vec::new();
        for (idx, floor) in self.active_floors.iter_mut().enumerate() {
            let sector = &mut self.sectors[floor.sector];
            match floor.direction {
                0 => {
                    floor.countdown -= dt;
                    if floor.countdown <= 0.0 && floor.kind == FloorKind::Lift {
                        floor.direction = 1;
                    }
                }
                1 => {
                    sector.floor_height =
                        (sector.floor_height + floor.speed * dt).min(floor.top_height);
                    if (sector.floor_height - floor.top_height).abs() < 0.001 {
                        finished.push(idx);
                    }
                }
                -1 => {
                    sector.floor_height =
                        (sector.floor_height - floor.speed * dt).max(floor.bottom_height);
                    if (sector.floor_height - floor.bottom_height).abs() < 0.001 {
                        if floor.kind == FloorKind::Lift {
                            floor.direction = 0;
                            floor.countdown = floor.wait_time;
                        } else {
                            finished.push(idx);
                        }
                    }
                }
                _ => {}
            }
        }

        for idx in finished.into_iter().rev() {
            self.active_floors.swap_remove(idx);
        }

        SpecialOutcome::None
    }

    pub fn activate_use_line(&mut self, origin: DVec2, angle: f64, range: f64) -> bool {
        let Some(linedef_index) = self.use_linedef_index(origin, angle, range) else {
            return false;
        };
        let before_doors = self.active_doors.len();
        let before_floors = self.active_floors.len();
        let before_special = self.linedefs[linedef_index].special;
        let outcome = self.activate_linedef(linedef_index, ActivationKind::Use);
        outcome != SpecialOutcome::None
            || self.active_doors.len() != before_doors
            || self.active_floors.len() != before_floors
            || self.linedefs[linedef_index].special != before_special
    }

    pub fn use_linedef_index(&self, origin: DVec2, angle: f64, range: f64) -> Option<usize> {
        let dir = DVec2::new(angle.cos(), angle.sin());
        let end = origin + dir * range;
        self.segment_intersections(origin, end)
            .into_iter()
            .find(|(t, _)| *t > 0.0001)
            .map(|(_, linedef_index)| linedef_index)
    }

    pub fn activate_use_line_outcome(
        &mut self,
        origin: DVec2,
        angle: f64,
        range: f64,
    ) -> SpecialOutcome {
        self.use_linedef_index(origin, angle, range)
            .map(|linedef_index| self.activate_linedef(linedef_index, ActivationKind::Use))
            .unwrap_or(SpecialOutcome::None)
    }

    pub fn activate_crossed_lines(&mut self, start: DVec2, end: DVec2) -> SpecialOutcome {
        let mut outcome = SpecialOutcome::None;
        let crossed: Vec<usize> = self
            .segment_intersections(start, end)
            .into_iter()
            .filter_map(|(t, linedef_index)| (t > 0.0001 && t < 0.9999).then_some(linedef_index))
            .collect();

        for linedef_index in crossed {
            let next = self.activate_linedef(linedef_index, ActivationKind::Cross);
            if next != SpecialOutcome::None {
                outcome = next;
            }
        }
        outcome
    }

    pub fn activate_linedef(
        &mut self,
        linedef_index: usize,
        activation: ActivationKind,
    ) -> SpecialOutcome {
        let Some(action) = LineAction::from_linedef(self.linedefs[linedef_index].special) else {
            return SpecialOutcome::None;
        };
        if action.activation != activation {
            return SpecialOutcome::None;
        }

        let activated = if self.linedefs[linedef_index].tag == 0 {
            self.activate_target(linedef_index, action)
        } else {
            let tag = self.linedefs[linedef_index].tag;
            let sector_indices: Vec<usize> = self
                .sectors
                .iter()
                .enumerate()
                .filter_map(|(idx, sector)| (sector.tag == tag).then_some(idx))
                .collect();
            sector_indices.into_iter().fold(false, |activated, idx| {
                self.start_sector_action(idx, action) || activated
            })
        };

        if activated && !action.repeatable {
            self.linedefs[linedef_index].special = 0;
        }
        if activated {
            action.outcome
        } else {
            SpecialOutcome::None
        }
    }

    pub fn required_key_for_linedef(&self, linedef_index: usize) -> Option<KeyColor> {
        match self.linedefs.get(linedef_index)?.special {
            26 | 32 => Some(KeyColor::Blue),
            27 | 34 => Some(KeyColor::Yellow),
            28 | 33 => Some(KeyColor::Red),
            _ => None,
        }
    }

    fn manual_door_sector(&self, linedef_index: usize) -> Option<usize> {
        self.linedefs
            .get(linedef_index)
            .and_then(|linedef| linedef.sectors[1])
    }

    fn activate_target(&mut self, linedef_index: usize, action: LineAction) -> bool {
        match action.effect {
            LineEffect::Door { .. } => self
                .manual_door_sector(linedef_index)
                .map(|sector| self.start_sector_action(sector, action))
                .unwrap_or(false),
            LineEffect::Floor { .. } => self
                .linedefs
                .get(linedef_index)
                .and_then(|linedef| linedef.sectors[0])
                .map(|sector| self.start_sector_action(sector, action))
                .unwrap_or(false),
            LineEffect::Exit => true,
        }
    }

    fn start_sector_action(&mut self, sector: usize, action: LineAction) -> bool {
        match action.effect {
            LineEffect::Door {
                kind,
                speed,
                wait_time,
            } => self.start_door(sector, kind, speed, wait_time),
            LineEffect::Floor {
                kind,
                direction,
                speed,
                wait_time,
            } => self.start_floor(sector, kind, direction, speed, wait_time),
            LineEffect::Exit => true,
        }
    }

    fn start_door(&mut self, sector: usize, kind: DoorKind, speed: f64, wait_time: f64) -> bool {
        if self.active_doors.iter().any(|door| door.sector == sector) {
            return false;
        }
        let current_ceiling = self.sectors[sector].ceiling_height;
        let bottom_height = self.sectors[sector].floor_height;
        let top_height = match kind {
            DoorKind::Close => {
                self.lowest_neighbor_ceiling(sector)
                    .unwrap_or(current_ceiling)
                    - 4.0
            }
            DoorKind::Open | DoorKind::Raise => {
                self.lowest_neighbor_ceiling(sector)
                    .unwrap_or(current_ceiling)
                    - 4.0
            }
        }
        .max(bottom_height);

        let direction = match kind {
            DoorKind::Close => -1,
            DoorKind::Open | DoorKind::Raise => {
                if top_height <= current_ceiling + 0.001 {
                    return false;
                }
                1
            }
        };

        self.active_doors.push(ActiveDoor {
            sector,
            kind,
            direction,
            speed,
            top_height,
            bottom_height,
            wait_time,
            countdown: wait_time,
        });
        true
    }

    fn start_floor(
        &mut self,
        sector: usize,
        kind: FloorKind,
        direction: i8,
        speed: f64,
        wait_time: f64,
    ) -> bool {
        if self
            .active_floors
            .iter()
            .any(|floor| floor.sector == sector)
        {
            return false;
        }
        let current_floor = self.sectors[sector].floor_height;
        let (bottom_height, top_height, direction) = match kind {
            FloorKind::Lift => {
                let bottom = self.lowest_neighbor_floor(sector).unwrap_or(current_floor);
                (bottom, current_floor, -1)
            }
            FloorKind::Move if direction > 0 => {
                let top = self
                    .next_higher_neighbor_floor(sector)
                    .unwrap_or(current_floor);
                if top <= current_floor + 0.001 {
                    return false;
                }
                (current_floor, top, 1)
            }
            FloorKind::Move => {
                let bottom = self.lowest_neighbor_floor(sector).unwrap_or(current_floor);
                if bottom >= current_floor - 0.001 {
                    return false;
                }
                (bottom, current_floor, -1)
            }
        };

        self.active_floors.push(ActiveFloor {
            sector,
            kind,
            direction,
            speed,
            top_height,
            bottom_height,
            wait_time,
            countdown: wait_time,
        });
        true
    }

    fn lowest_neighbor_ceiling(&self, sector: usize) -> Option<f64> {
        self.linedefs
            .iter()
            .filter_map(|line| match line.sectors {
                [Some(a), Some(b)] if a == sector => Some(self.sectors[b].ceiling_height),
                [Some(a), Some(b)] if b == sector => Some(self.sectors[a].ceiling_height),
                _ => None,
            })
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    fn neighbor_floors(&self, sector: usize) -> impl Iterator<Item = f64> + '_ {
        self.linedefs
            .iter()
            .filter_map(move |line| match line.sectors {
                [Some(a), Some(b)] if a == sector => Some(self.sectors[b].floor_height),
                [Some(a), Some(b)] if b == sector => Some(self.sectors[a].floor_height),
                _ => None,
            })
    }

    fn lowest_neighbor_floor(&self, sector: usize) -> Option<f64> {
        self.neighbor_floors(sector)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    fn next_higher_neighbor_floor(&self, sector: usize) -> Option<f64> {
        let current = self.sectors[sector].floor_height;
        self.neighbor_floors(sector)
            .filter(|height| *height > current + 0.001)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn point_on_node_side(&self, point: DVec2, node_index: usize) -> usize {
        let node = &self.nodes[node_index];
        let node_origin = DVec2::new(node.x as f64, node.y as f64);
        let node_dir = DVec2::new(node.dx as f64, node.dy as f64);
        let rel = point - node_origin;
        let cross = node_dir.x * rel.y - node_dir.y * rel.x;
        if cross <= 0.0 {
            0
        } else {
            1
        }
    }

    pub fn find_subsector(&self, point: DVec2) -> Option<usize> {
        if self.subsectors.is_empty() {
            return None;
        }
        if self.nodes.is_empty() {
            return Some(0);
        }

        let mut node_index = self.nodes.len() - 1;
        loop {
            let side = self.point_on_node_side(point, node_index);
            let child = self.nodes[node_index].children[side];
            if (child & 0x8000) != 0 {
                return Some((child & 0x7fff) as usize);
            }
            node_index = child as usize;
        }
    }

    pub fn find_sector(&self, point: DVec2) -> Option<usize> {
        self.find_subsector(point)
            .and_then(|subsector_index| self.subsectors.get(subsector_index))
            .map(|subsector| subsector.sector)
    }

    pub fn opening_for_seg(&self, seg: &Seg) -> Option<OpeningInfo> {
        let linedef_index = seg.linedef?;
        self.opening_for_linedef(linedef_index, seg.side as usize)
    }

    pub fn opening_for_linedef(
        &self,
        linedef_index: usize,
        side_index: usize,
    ) -> Option<OpeningInfo> {
        let linedef = self.linedefs.get(linedef_index)?;
        let front_sector_idx = linedef.sectors.get(side_index).copied().flatten()?;
        let front_sector = self.sectors.get(front_sector_idx)?;
        let back_sector_idx = linedef.sectors.get(1 - side_index).copied().flatten();
        let back_sector = back_sector_idx.and_then(|idx| self.sectors.get(idx));
        let sidedef_idx = linedef.sidedef.get(side_index).copied().flatten()?;
        let sidedef = self.sidedefs.get(sidedef_idx)?;

        let opening_bottom = back_sector
            .map(|back| front_sector.floor_height.max(back.floor_height))
            .unwrap_or(front_sector.floor_height);
        let opening_top = back_sector
            .map(|back| front_sector.ceiling_height.min(back.ceiling_height))
            .unwrap_or(front_sector.floor_height);
        let solid = match back_sector {
            Some(_) => opening_top <= opening_bottom,
            None => true,
        };

        Some(OpeningInfo {
            front_sector: front_sector_idx,
            back_sector: back_sector_idx,
            front_floor: front_sector.floor_height,
            front_ceiling: front_sector.ceiling_height,
            opening_bottom,
            opening_top,
            solid,
            masked_middle: !solid && sidedef.mid_texture != "-",
        })
    }

    pub fn opening_contains_height(&self, opening: &OpeningInfo, z: f64) -> bool {
        !opening.solid && z > opening.opening_bottom && z < opening.opening_top
    }

    pub fn linedef_side_for_point(&self, point: DVec2, linedef_index: usize) -> Option<usize> {
        let linedef = self.linedefs.get(linedef_index)?;
        let start = self.vertices.get(linedef.v1)?.p;
        let end = self.vertices.get(linedef.v2)?.p;
        let rel = point - start;
        let dir = end - start;
        let cross = dir.x * rel.y - dir.y * rel.x;
        Some(if cross <= 0.0 { 0 } else { 1 })
    }

    pub fn opening_for_point_on_linedef(
        &self,
        linedef_index: usize,
        point: DVec2,
    ) -> Option<OpeningInfo> {
        let side_index = self.linedef_side_for_point(point, linedef_index)?;
        self.opening_for_linedef(linedef_index, side_index)
    }

    pub fn actor_can_traverse_opening(
        &self,
        opening: &OpeningInfo,
        actor_height: f64,
        step_height: f64,
    ) -> bool {
        if opening.solid {
            return false;
        }
        if opening.opening_bottom > opening.front_floor + step_height {
            return false;
        }
        let destination_floor = opening
            .back_sector
            .and_then(|idx| self.sectors.get(idx))
            .map(|sector| sector.floor_height)
            .unwrap_or(opening.opening_bottom);
        destination_floor + actor_height < opening.opening_top
    }

    pub fn movement_trace_clear(
        &self,
        start: DVec2,
        end: DVec2,
        actor_height: f64,
        step_height: f64,
    ) -> bool {
        if (end - start).length_squared() < 0.0001 {
            return true;
        }

        for (t, linedef_index) in self.segment_intersections(start, end) {
            if t <= 0.0001 || t >= 0.9999 {
                continue;
            }
            let before = start + (end - start) * (t - 0.0001).max(0.0);
            let Some(opening) = self.opening_for_point_on_linedef(linedef_index, before) else {
                return false;
            };
            if !self.actor_can_traverse_opening(&opening, actor_height, step_height) {
                return false;
            }
        }

        true
    }

    pub fn line_of_sight_clear(&self, start: DVec2, start_z: f64, end: DVec2, end_z: f64) -> bool {
        if (end - start).length_squared() < 0.0001 {
            return true;
        }

        for (t, linedef_index) in self.segment_intersections(start, end) {
            if t <= 0.0001 || t >= 0.9999 {
                continue;
            }
            let before = start + (end - start) * (t - 0.0001).max(0.0);
            let z = start_z + (end_z - start_z) * t;
            let Some(opening) = self.opening_for_point_on_linedef(linedef_index, before) else {
                return false;
            };
            if !self.opening_contains_height(&opening, z) {
                return false;
            }
        }

        true
    }

    pub fn segment_intersections(&self, start: DVec2, end: DVec2) -> Vec<(f64, usize)> {
        let mut hits = Vec::new();
        for (linedef_index, linedef) in self.linedefs.iter().enumerate() {
            let a = self.vertices[linedef.v1].p;
            let b = self.vertices[linedef.v2].p;
            if let Some(t) = segment_segment_intersection_param(start, end, a, b) {
                hits.push((t, linedef_index));
            }
        }
        hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        hits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationKind {
    Use,
    Cross,
    Shoot,
}

#[derive(Debug, Clone, Copy)]
enum LineEffect {
    Door {
        kind: DoorKind,
        speed: f64,
        wait_time: f64,
    },
    Floor {
        kind: FloorKind,
        direction: i8,
        speed: f64,
        wait_time: f64,
    },
    Exit,
}

#[derive(Debug, Clone, Copy)]
struct LineAction {
    activation: ActivationKind,
    effect: LineEffect,
    repeatable: bool,
    outcome: SpecialOutcome,
}

impl LineAction {
    fn from_linedef(special: u16) -> Option<Self> {
        const TICRATE: f64 = 35.0;
        let normal = 2.0 * TICRATE;
        let blazing = 8.0 * TICRATE;
        let wait = 150.0 / TICRATE;
        let slow_floor = TICRATE;
        let fast_floor = 4.0 * TICRATE;

        let (activation, effect, repeatable, outcome) = match special {
            1 | 26 | 27 | 28 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Raise,
                    speed: normal,
                    wait_time: wait,
                },
                true,
                SpecialOutcome::None,
            ),
            31 | 32 | 33 | 34 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Open,
                    speed: normal,
                    wait_time: 0.0,
                },
                false,
                SpecialOutcome::None,
            ),
            2 => (
                ActivationKind::Cross,
                LineEffect::Door {
                    kind: DoorKind::Open,
                    speed: normal,
                    wait_time: 0.0,
                },
                false,
                SpecialOutcome::None,
            ),
            63 | 90 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Raise,
                    speed: normal,
                    wait_time: wait,
                },
                true,
                SpecialOutcome::None,
            ),
            61 | 86 | 103 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Open,
                    speed: normal,
                    wait_time: 0.0,
                },
                special == 61 || special == 86,
                SpecialOutcome::None,
            ),
            117 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Raise,
                    speed: blazing,
                    wait_time: wait,
                },
                true,
                SpecialOutcome::None,
            ),
            118 => (
                ActivationKind::Use,
                LineEffect::Door {
                    kind: DoorKind::Open,
                    speed: blazing,
                    wait_time: 0.0,
                },
                false,
                SpecialOutcome::None,
            ),
            10 | 88 => (
                if special == 10 {
                    ActivationKind::Cross
                } else {
                    ActivationKind::Use
                },
                LineEffect::Floor {
                    kind: FloorKind::Lift,
                    direction: -1,
                    speed: fast_floor,
                    wait_time: 3.0,
                },
                special == 88,
                SpecialOutcome::None,
            ),
            21 | 62 => (
                ActivationKind::Use,
                LineEffect::Floor {
                    kind: FloorKind::Lift,
                    direction: -1,
                    speed: fast_floor,
                    wait_time: 3.0,
                },
                special == 62,
                SpecialOutcome::None,
            ),
            18 | 22 | 64 | 91 | 101 => (
                if matches!(special, 22) {
                    ActivationKind::Cross
                } else {
                    ActivationKind::Use
                },
                LineEffect::Floor {
                    kind: FloorKind::Move,
                    direction: 1,
                    speed: slow_floor,
                    wait_time: 0.0,
                },
                matches!(special, 64 | 91),
                SpecialOutcome::None,
            ),
            19 | 23 | 36 | 38 | 70 | 71 | 82 | 83 => (
                if matches!(special, 19 | 36 | 38) {
                    ActivationKind::Cross
                } else {
                    ActivationKind::Use
                },
                LineEffect::Floor {
                    kind: FloorKind::Move,
                    direction: -1,
                    speed: if matches!(special, 36 | 70 | 71) {
                        fast_floor
                    } else {
                        slow_floor
                    },
                    wait_time: 0.0,
                },
                matches!(special, 70 | 82 | 83),
                SpecialOutcome::None,
            ),
            11 | 51 => (
                ActivationKind::Use,
                LineEffect::Exit,
                false,
                SpecialOutcome::Exit {
                    secret: special == 51,
                },
            ),
            52 | 124 => (
                ActivationKind::Cross,
                LineEffect::Exit,
                false,
                SpecialOutcome::Exit {
                    secret: special == 124,
                },
            ),
            _ => return None,
        };

        Some(Self {
            activation,
            effect,
            repeatable,
            outcome,
        })
    }
}

fn cross(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn segment_segment_intersection_param(start: DVec2, end: DVec2, a: DVec2, b: DVec2) -> Option<f64> {
    let dir = end - start;
    let seg = b - a;
    let denom = cross(dir, seg);
    if denom.abs() < 1e-6 {
        return None;
    }
    let rel = a - start;
    let t = cross(rel, seg) / denom;
    let u = cross(rel, dir) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(t)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_level() -> Level {
        Level {
            vertices: vec![
                Vertex {
                    p: DVec2::new(32.0, -32.0),
                },
                Vertex {
                    p: DVec2::new(32.0, 32.0),
                },
            ],
            sectors: vec![
                Sector {
                    floor_height: 0.0,
                    ceiling_height: 128.0,
                    floor_texture: "FLOOR0_1".to_string(),
                    ceiling_texture: "CEIL1_1".to_string(),
                    light_level: 255,
                    special: 0,
                    tag: 0,
                },
                Sector {
                    floor_height: 0.0,
                    ceiling_height: 0.0,
                    floor_texture: "FLOOR0_1".to_string(),
                    ceiling_texture: "CEIL1_1".to_string(),
                    light_level: 255,
                    special: 0,
                    tag: 7,
                },
            ],
            sidedefs: vec![
                SideDef {
                    texture_offset: 0.0,
                    row_offset: 0.0,
                    top_texture: "-".to_string(),
                    bottom_texture: "-".to_string(),
                    mid_texture: "-".to_string(),
                    sector: 0,
                },
                SideDef {
                    texture_offset: 0.0,
                    row_offset: 0.0,
                    top_texture: "-".to_string(),
                    bottom_texture: "-".to_string(),
                    mid_texture: "-".to_string(),
                    sector: 1,
                },
            ],
            linedefs: vec![LineDef {
                v1: 0,
                v2: 1,
                flags: 0,
                special: 1,
                tag: 0,
                sidedef: [Some(0), Some(1)],
                sectors: [Some(0), Some(1)],
            }],
            segs: Vec::new(),
            subsectors: Vec::new(),
            nodes: Vec::new(),
            things: Vec::new(),
            active_doors: Vec::new(),
            active_floors: Vec::new(),
        }
    }

    #[test]
    fn use_line_starts_manual_door_and_moves_ceiling() {
        let mut level = test_level();

        assert!(level.activate_use_line(DVec2::ZERO, 0.0, 64.0));
        assert_eq!(level.active_doors.len(), 1);

        level.tick_specials(1.0);
        assert_eq!(level.sectors[1].ceiling_height, 70.0);

        level.tick_specials(1.0);
        assert_eq!(level.sectors[1].ceiling_height, 124.0);
    }

    #[test]
    fn tagged_door_activates_matching_sector() {
        let mut level = test_level();
        level.linedefs[0].special = 61;
        level.linedefs[0].tag = 7;

        assert_eq!(
            level.activate_linedef(0, ActivationKind::Use),
            SpecialOutcome::None
        );
        assert_eq!(level.active_doors[0].sector, 1);
    }

    #[test]
    fn lift_lowers_then_returns_floor() {
        let mut level = test_level();
        level.sectors[1].floor_height = 64.0;
        level.sectors[1].ceiling_height = 128.0;
        level.linedefs[0].special = 62;
        level.linedefs[0].tag = 7;

        assert_eq!(
            level.activate_linedef(0, ActivationKind::Use),
            SpecialOutcome::None
        );
        assert_eq!(level.active_floors.len(), 1);

        level.tick_specials(1.0);
        assert_eq!(level.sectors[1].floor_height, 0.0);

        level.tick_specials(3.0);
        level.tick_specials(1.0);
        assert_eq!(level.sectors[1].floor_height, 64.0);
    }

    #[test]
    fn exit_special_reports_completion() {
        let mut level = test_level();
        level.linedefs[0].special = 11;
        level.linedefs[0].tag = 0;

        assert_eq!(
            level.activate_linedef(0, ActivationKind::Use),
            SpecialOutcome::Exit { secret: false }
        );
        assert_eq!(level.linedefs[0].special, 0);
    }
}
