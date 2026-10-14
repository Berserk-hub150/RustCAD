//! Dependency-free CAD document, geometry, history and exchange layer.
use std::{f64::consts::PI, fmt::Write};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn dist(self, b: Self) -> f64 {
        (self.x - b.x).hypot(self.y - b.y)
    }
    pub fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y)
    }
    pub fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y)
    }
    pub fn mul(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }
    pub fn rotate(self, origin: Self, r: f64) -> Self {
        let p = self.sub(origin);
        let (s, c) = r.sin_cos();
        Self::new(p.x * c - p.y * s, p.x * s + p.y * c).add(origin)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line(Point, Point),
    Polyline {
        points: Vec<Point>,
        closed: bool,
    },
    Circle {
        center: Point,
        radius: f64,
    },
    Arc {
        center: Point,
        radius: f64,
        start: f64,
        end: f64,
    },
    Text {
        at: Point,
        text: String,
        height: f64,
    },
    Dimension {
        a: Point,
        b: Point,
        offset: f64,
    },
    Box3D {
        min: [f64; 3],
        max: [f64; 3],
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub id: u64,
    pub layer: usize,
    pub shape: Shape,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub color: [u8; 3],
}
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub entities: Vec<Entity>,
    pub layers: Vec<Layer>,
    pub next_id: u64,
    pub units: String,
}
impl Default for Drawing {
    fn default() -> Self {
        Self {
            entities: vec![],
            layers: vec![Layer {
                name: "0".into(),
                visible: true,
                locked: false,
                color: [203, 225, 240],
            }],
            next_id: 1,
            units: "mm".into(),
        }
    }
}
fn valid_point(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
impl Shape {
    pub fn validate(&self) -> bool {
        match self {
            Self::Line(a, b) => valid_point(*a) && valid_point(*b) && a.dist(*b) > 1e-10,
            Self::Dimension { a, b, offset } => {
                valid_point(*a) && valid_point(*b) && a.dist(*b) > 1e-10 && offset.is_finite()
            }
            Self::Polyline { points, .. } => {
                points.len() >= 2 && points.iter().all(|p| valid_point(*p))
            }
            Self::Circle { center, radius } => {
                valid_point(*center) && radius.is_finite() && *radius > 0.
            }
            Self::Arc {
                center,
                radius,
                start,
                end,
            } => {
                valid_point(*center)
                    && radius.is_finite()
                    && *radius > 0.
                    && start.is_finite()
                    && end.is_finite()
            }
            Self::Text { at, height, .. } => valid_point(*at) && height.is_finite() && *height > 0.,
            Self::Box3D { min, max } => {
                min.iter().chain(max).all(|x| x.is_finite()) && (0..3).all(|i| max[i] > min[i])
            }
        }
    }
    pub fn segments(&self) -> Vec<(Point, Point)> {
        match self {
            Self::Line(a, b) => vec![(*a, *b)],
            Self::Polyline { points, closed } => {
                let mut v = points.windows(2).map(|p| (p[0], p[1])).collect::<Vec<_>>();
                if *closed && points.len() > 2 {
                    v.push((*points.last().unwrap(), points[0]))
                }
                v
            }
            Self::Circle { center, radius } => curve_segments(*center, *radius, 0., 2. * PI, 96),
            Self::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let span = (*end - *start).rem_euclid(360.);
                curve_segments(
                    *center,
                    *radius,
                    start.to_radians(),
                    span.to_radians(),
                    ((span / 4.).ceil() as usize).max(2),
                )
            }
            Self::Dimension { a, b, offset } => {
                let d = b.sub(*a);
                let n = Point::new(-d.y, d.x).mul(*offset / a.dist(*b));
                vec![(*a, a.add(n)), (*b, b.add(n)), (a.add(n), b.add(n))]
            }
            Self::Box3D { min, max } => {
                let ps = [
                    Point::new(min[0], min[1]),
                    Point::new(max[0], min[1]),
                    Point::new(max[0], max[1]),
                    Point::new(min[0], max[1]),
                ];
                (0..4).map(|i| (ps[i], ps[(i + 1) % 4])).collect()
            }
            Self::Text { .. } => vec![],
        }
    }
    pub fn anchors(&self) -> Vec<(Point, &'static str)> {
        match self {
            Self::Line(a, b) => vec![(*a, "END"), (*b, "END"), (a.add(*b).mul(0.5), "MID")],
            Self::Polyline { points, .. } => points
                .iter()
                .map(|p| (*p, "END"))
                .chain(points.windows(2).map(|p| (p[0].add(p[1]).mul(0.5), "MID")))
                .collect(),
            Self::Circle { center, radius } | Self::Arc { center, radius, .. } => {
                let mut p = vec![(*center, "CEN")];
                for i in 0..4 {
                    let t = i as f64 * PI / 2.;
                    if let Self::Arc { start, end, .. } = self {
                        if (t.to_degrees() - *start).rem_euclid(360.)
                            > (*end - *start).rem_euclid(360.) + 1e-9
                        {
                            continue;
                        }
                    }
                    p.push((
                        center.add(Point::new(t.cos(), t.sin()).mul(*radius)),
                        "QUAD",
                    ))
                }
                if let Self::Arc { start, end, .. } = self {
                    for a in [start, end] {
                        p.push((
                            center.add(
                                Point::new(a.to_radians().cos(), a.to_radians().sin()).mul(*radius),
                            ),
                            "END",
                        ))
                    }
                }
                p
            }
            Self::Text { at, .. } => vec![(*at, "INS")],
            Self::Dimension { a, b, .. } => vec![(*a, "END"), (*b, "END")],
            Self::Box3D { min, max } => vec![
                (Point::new(min[0], min[1]), "END"),
                (Point::new(max[0], max[1]), "END"),
            ],
        }
    }
    pub fn distance(&self, p: Point) -> f64 {
        if let Self::Circle { center, radius } = self {
            return (center.dist(p) - radius).abs();
        }
        if let Self::Text { at, .. } = self {
            return at.dist(p);
        }
        self.segments()
            .iter()
            .map(|(a, b)| segment_distance(p, *a, *b))
            .fold(f64::INFINITY, f64::min)
    }
    pub fn map_points(&mut self, f: impl Fn(Point) -> Point, scale: f64, angle: f64) {
        match self {
            Self::Line(a, b) | Self::Dimension { a, b, .. } => {
                *a = f(*a);
                *b = f(*b)
            }
            Self::Polyline { points, .. } => points.iter_mut().for_each(|p| *p = f(*p)),
            Self::Circle { center, radius } => {
                *center = f(*center);
                *radius *= scale.abs()
            }
            Self::Arc {
                center,
                radius,
                start,
                end,
            } => {
                *center = f(*center);
                *radius *= scale.abs();
                *start += angle;
                *end += angle
            }
            Self::Text { at, height, .. } => {
                *at = f(*at);
                *height *= scale.abs()
            }
            Self::Box3D { min, max } => {
                let corners = [
                    Point::new(min[0], min[1]),
                    Point::new(max[0], min[1]),
                    Point::new(max[0], max[1]),
                    Point::new(min[0], max[1]),
                ]
                .map(f);
                min[0] = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
                min[1] = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
                max[0] = corners
                    .iter()
                    .map(|p| p.x)
                    .fold(f64::NEG_INFINITY, f64::max);
                max[1] = corners
                    .iter()
                    .map(|p| p.y)
                    .fold(f64::NEG_INFINITY, f64::max);
                min[2] *= scale.abs();
                max[2] *= scale.abs()
            }
        }
        if let Self::Dimension { offset, .. } = self {
            *offset *= scale.abs()
        }
    }
}
fn curve_segments(c: Point, r: f64, start: f64, span: f64, n: usize) -> Vec<(Point, Point)> {
    (0..n)
        .map(|i| {
            let point = |j: usize| {
                let t = start + span * j as f64 / n as f64;
                c.add(Point::new(t.cos(), t.sin()).mul(r))
            };
            (point(i), point(i + 1))
        })
        .collect()
}
pub fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let d = b.sub(a);
    let len = d.x * d.x + d.y * d.y;
    if len < 1e-20 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len;
    p.dist(a.add(d.mul(t.clamp(0., 1.))))
}
pub fn intersection(a: Point, b: Point, c: Point, d: Point, finite: bool) -> Option<Point> {
    let r = b.sub(a);
    let s = d.sub(c);
    let cross = |u: Point, v: Point| u.x * v.y - u.y * v.x;
    let den = cross(r, s);
    if den.abs() < 1e-10 {
        return None;
    }
    let t = cross(c.sub(a), s) / den;
    let u = cross(c.sub(a), r) / den;
    if finite && (!(0. - 1e-9..=1. + 1e-9).contains(&t) || !(0. - 1e-9..=1. + 1e-9).contains(&u)) {
        return None;
    }
    Some(a.add(r.mul(t)))
}
impl Drawing {
    pub fn add(&mut self, layer: usize, shape: Shape) -> Result<u64, String> {
        if layer >= self.layers.len() {
            return Err("Unknown layer".into());
        }
        if self.layers[layer].locked {
            return Err("Layer is locked".into());
        }
        if !shape.validate() {
            return Err("Invalid or degenerate geometry".into());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.entities.push(Entity { id, layer, shape });
        Ok(id)
    }
    pub fn selectable(&self, e: &Entity) -> bool {
        self.layers
            .get(e.layer)
            .is_some_and(|l| l.visible && !l.locked)
    }
    pub fn hit(&self, p: Point, tol: f64) -> Option<u64> {
        self.entities
            .iter()
            .filter(|e| self.selectable(e))
            .map(|e| (e.id, e.shape.distance(p)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|a| a.0)
    }
    pub fn snap(&self, p: Point, tol: f64, grid: Option<f64>) -> (Point, String) {
        let mut best = (p, String::new(), tol);
        for e in self
            .entities
            .iter()
            .filter(|e| self.layers[e.layer].visible)
        {
            for (q, label) in e.shape.anchors() {
                let d = p.dist(q);
                if d < best.2 {
                    best = (q, label.into(), d)
                }
            }
        }
        let segs = self
            .entities
            .iter()
            .filter(|e| self.layers[e.layer].visible)
            .filter(|e| matches!(e.shape, Shape::Line(..) | Shape::Polyline { .. }))
            .flat_map(|e| e.shape.segments())
            .take(200)
            .collect::<Vec<_>>();
        for i in 0..segs.len() {
            for j in i + 1..segs.len() {
                if let Some(q) = intersection(segs[i].0, segs[i].1, segs[j].0, segs[j].1, true) {
                    let d = p.dist(q);
                    if d < best.2 {
                        best = (q, "INT".into(), d)
                    }
                }
            }
        }
        if !best.1.is_empty() {
            return (best.0, best.1);
        }
        if let Some(g) = grid.filter(|g| g.is_finite() && *g > 0.) {
            return (
                Point::new((p.x / g).round() * g, (p.y / g).round() * g),
                "GRID".into(),
            );
        }
        (p, String::new())
    }
    pub fn bounds(&self) -> Option<(Point, Point)> {
        let ps = self
            .entities
            .iter()
            .flat_map(|e| {
                e.shape
                    .anchors()
                    .into_iter()
                    .map(|a| a.0)
                    .chain(e.shape.segments().into_iter().flat_map(|(a, b)| [a, b]))
            })
            .collect::<Vec<_>>();
        if ps.is_empty() {
            return None;
        }
        Some((
            Point::new(
                ps.iter().map(|p| p.x).fold(f64::INFINITY, f64::min),
                ps.iter().map(|p| p.y).fold(f64::INFINITY, f64::min),
            ),
            Point::new(
                ps.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max),
                ps.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max),
            ),
        ))
    }
    pub fn edit(&mut self, ids: &[u64], mut f: impl FnMut(&mut Shape)) -> usize {
        let layers = &self.layers;
        let mut n = 0;
        for e in &mut self.entities {
            if ids.contains(&e.id) && !layers[e.layer].locked {
                f(&mut e.shape);
                n += 1
            }
        }
        n
    }
    pub fn erase(&mut self, ids: &[u64]) {
        let layers = &self.layers;
        self.entities
            .retain(|e| !ids.contains(&e.id) || layers[e.layer].locked);
    }
    pub fn offset(&mut self, id: u64, amount: f64) -> Result<u64, String> {
        if !amount.is_finite() {
            return Err("Offset must be finite".into());
        }
        let e = self
            .entities
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or("Select an entity")?;
        let shape = match e.shape {
            Shape::Line(a, b) => {
                let d = b.sub(a);
                let n = Point::new(-d.y, d.x).mul(amount / a.dist(b));
                Shape::Line(a.add(n), b.add(n))
            }
            Shape::Circle { center, radius } => Shape::Circle {
                center,
                radius: radius + amount,
            },
            Shape::Arc {
                center,
                radius,
                start,
                end,
            } => Shape::Arc {
                center,
                radius: radius + amount,
                start,
                end,
            },
            _ => return Err("OFFSET supports lines, circles and arcs".into()),
        };
        self.add(e.layer, shape)
    }
    pub fn trim_line(&mut self, id: u64, boundary: u64, keep: Point) -> Result<(), String> {
        let entity = self
            .entities
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or("Missing line")?;
        if self.layers[entity.layer].locked {
            return Err("Layer is locked".into());
        }
        let cutter = self
            .entities
            .iter()
            .find(|e| e.id == boundary)
            .ok_or("Missing boundary")?;
        if let Shape::Line(a, b) = entity.shape {
            let cut = cutter
                .shape
                .segments()
                .iter()
                .filter_map(|(c, d)| intersection(a, b, *c, *d, true))
                .min_by(|p, q| p.dist(keep).total_cmp(&q.dist(keep)))
                .ok_or("No intersection")?;
            let new = if keep.dist(a) <= keep.dist(b) {
                Shape::Line(a, cut)
            } else {
                Shape::Line(cut, b)
            };
            if !new.validate() {
                return Err("Trim would erase the line".into());
            }
            self.entities.iter_mut().find(|e| e.id == id).unwrap().shape = new;
            Ok(())
        } else {
            Err("TRIM supports a line target and line/polyline boundaries".into())
        }
    }
    pub fn sample() -> Self {
        let mut d = Self::default();
        d.layers.push(Layer {
            name: "Dimensions".into(),
            visible: true,
            locked: false,
            color: [95, 230, 174],
        });
        d.add(
            0,
            Shape::Polyline {
                points: vec![
                    Point::new(0., 0.),
                    Point::new(120., 0.),
                    Point::new(120., 80.),
                    Point::new(0., 80.),
                ],
                closed: true,
            },
        )
        .unwrap();
        for (x, y) in [(15., 15.), (105., 15.), (105., 65.), (15., 65.)] {
            d.add(
                0,
                Shape::Circle {
                    center: Point::new(x, y),
                    radius: 5.,
                },
            )
            .unwrap();
        }
        d.add(
            1,
            Shape::Dimension {
                a: Point::new(0., 0.),
                b: Point::new(120., 0.),
                offset: -15.,
            },
        )
        .unwrap();
        d.add(
            1,
            Shape::Dimension {
                a: Point::new(120., 0.),
                b: Point::new(120., 80.),
                offset: -15.,
            },
        )
        .unwrap();
        d.add(
            1,
            Shape::Text {
                at: Point::new(0., 105.),
                text: "MOUNTING PLATE / 120 x 80 mm".into(),
                height: 4.,
            },
        )
        .unwrap();
        d
    }
}
#[derive(Default)]
pub struct History {
    undo: Vec<Drawing>,
    redo: Vec<Drawing>,
}
impl History {
    pub fn checkpoint(&mut self, d: &Drawing) {
        self.undo.push(d.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear()
    }
    pub fn undo(&mut self, d: &mut Drawing) -> bool {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(d.clone());
            *d = prev;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, d: &mut Drawing) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(d.clone());
            *d = next;
            true
        } else {
            false
        }
    }
}
fn escape(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}
fn unescape(s: &str) -> String {
    s.replace("%09", "\t")
        .replace("%0A", "\n")
        .replace("%0D", "\r")
        .replace("%25", "%")
}
pub fn save_project(d: &Drawing) -> String {
    let mut s = format!("RUSTCAD\t1\t{}\n", escape(&d.units));
    for l in &d.layers {
        writeln!(
            s,
            "LAYER\t{}\t{}\t{}\t{}\t{}\t{}",
            escape(&l.name),
            l.visible as u8,
            l.locked as u8,
            l.color[0],
            l.color[1],
            l.color[2]
        )
        .unwrap()
    }
    for e in &d.entities {
        let p = |p: Point| format!("{}\t{}", p.x, p.y);
        let data = match &e.shape {
            Shape::Line(a, b) => format!("LINE\t{}\t{}", p(*a), p(*b)),
            Shape::Circle { center, radius } => format!("CIRCLE\t{}\t{}", p(*center), radius),
            Shape::Arc {
                center,
                radius,
                start,
                end,
            } => format!("ARC\t{}\t{}\t{}\t{}", p(*center), radius, start, end),
            Shape::Text { at, text, height } => {
                format!("TEXT\t{}\t{}\t{}", p(*at), height, escape(text))
            }
            Shape::Dimension { a, b, offset } => format!("DIM\t{}\t{}\t{}", p(*a), p(*b), offset),
            Shape::Polyline { points, closed } => format!(
                "POLY\t{}\t{}",
                *closed as u8,
                points.iter().map(|q| p(*q)).collect::<Vec<_>>().join("\t")
            ),
            Shape::Box3D { min, max } => format!(
                "BOX\t{}\t{}\t{}\t{}\t{}\t{}",
                min[0], min[1], min[2], max[0], max[1], max[2]
            ),
        };
        writeln!(s, "ENTITY\t{}\t{}\t{}", e.id, e.layer, data).unwrap()
    }
    s
}
pub fn load_project(s: &str) -> Result<Drawing, String> {
    let mut lines = s.lines();
    let header = lines
        .next()
        .ok_or("Empty document")?
        .split('\t')
        .collect::<Vec<_>>();
    if header.len() != 3 || header[0] != "RUSTCAD" || header[1] != "1" {
        return Err("Unsupported project format".into());
    }
    let mut d = Drawing {
        units: unescape(header[2]),
        layers: vec![],
        ..Drawing::default()
    };
    for (i, line) in lines.enumerate() {
        let v = line.split('\t').collect::<Vec<_>>();
        let f = |n: usize| -> Result<f64, String> {
            v.get(n)
                .ok_or("Missing field")?
                .parse::<f64>()
                .map_err(|_| format!("Invalid number on line {}", i + 2))
        };
        let p = |n| -> Result<Point, String> { Ok(Point::new(f(n)?, f(n + 1)?)) };
        match v.first().copied() {
            Some("LAYER") if v.len() == 7 => d.layers.push(Layer {
                name: unescape(v[1]),
                visible: v[2] == "1",
                locked: v[3] == "1",
                color: [
                    v[4].parse().map_err(|_| "Invalid color")?,
                    v[5].parse().map_err(|_| "Invalid color")?,
                    v[6].parse().map_err(|_| "Invalid color")?,
                ],
            }),
            Some("ENTITY") => {
                let id = v
                    .get(1)
                    .ok_or("Missing ID")?
                    .parse::<u64>()
                    .map_err(|_| "Invalid ID")?;
                let layer = v
                    .get(2)
                    .ok_or("Missing layer")?
                    .parse::<usize>()
                    .map_err(|_| "Invalid layer")?;
                let shape = match v.get(3).copied() {
                    Some("LINE") => Shape::Line(p(4)?, p(6)?),
                    Some("CIRCLE") => Shape::Circle {
                        center: p(4)?,
                        radius: f(6)?,
                    },
                    Some("ARC") => Shape::Arc {
                        center: p(4)?,
                        radius: f(6)?,
                        start: f(7)?,
                        end: f(8)?,
                    },
                    Some("TEXT") => Shape::Text {
                        at: p(4)?,
                        height: f(6)?,
                        text: unescape(v.get(7).ok_or("Missing text")?),
                    },
                    Some("DIM") => Shape::Dimension {
                        a: p(4)?,
                        b: p(6)?,
                        offset: f(8)?,
                    },
                    Some("BOX") => Shape::Box3D {
                        min: [f(4)?, f(5)?, f(6)?],
                        max: [f(7)?, f(8)?, f(9)?],
                    },
                    Some("POLY") if v.len() >= 9 && (v.len() - 5) % 2 == 0 => Shape::Polyline {
                        closed: v[4] == "1",
                        points: (5..v.len())
                            .step_by(2)
                            .map(p)
                            .collect::<Result<Vec<_>, _>>()?,
                    },
                    _ => return Err(format!("Unsupported entity on line {}", i + 2)),
                };
                if layer >= d.layers.len()
                    || !shape.validate()
                    || id == 0
                    || d.entities.iter().any(|e| e.id == id)
                {
                    return Err(format!("Invalid entity on line {}", i + 2));
                }
                d.next_id = d.next_id.max(id + 1);
                d.entities.push(Entity { id, layer, shape });
            }
            _ => return Err(format!("Unknown record on line {}", i + 2)),
        }
    }
    if d.layers.is_empty() {
        return Err("Document has no layers".into());
    }
    Ok(d)
}

fn pair(out: &mut String, code: i32, value: impl std::fmt::Display) {
    writeln!(out, "{}\n{}", code, value).unwrap()
}
fn dxf_line(s: &mut String, layer: &str, a: Point, b: Point) {
    pair(s, 0, "LINE");
    pair(s, 8, layer);
    pair(s, 10, a.x);
    pair(s, 20, a.y);
    pair(s, 11, b.x);
    pair(s, 21, b.y)
}
pub fn export_dxf(d: &Drawing) -> String {
    let mut s = String::new();
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "HEADER");
    pair(&mut s, 9, "$ACADVER");
    pair(&mut s, 1, "AC1009");
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "TABLES");
    pair(&mut s, 0, "TABLE");
    pair(&mut s, 2, "LAYER");
    pair(&mut s, 70, d.layers.len());
    for l in &d.layers {
        pair(&mut s, 0, "LAYER");
        pair(&mut s, 2, l.name.replace(['\n', '\r'], " "));
        pair(&mut s, 70, if l.locked { 4 } else { 0 });
        pair(&mut s, 62, if l.visible { 7 } else { -7 });
        pair(&mut s, 6, "CONTINUOUS")
    }
    pair(&mut s, 0, "ENDTAB");
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "SECTION");
    pair(&mut s, 2, "ENTITIES");
    for e in &d.entities {
        let layer = &d.layers[e.layer].name;
        match &e.shape {
            Shape::Line(a, b) => dxf_line(&mut s, layer, *a, *b),
            Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => {
                pair(
                    &mut s,
                    0,
                    if matches!(e.shape, Shape::Circle { .. }) {
                        "CIRCLE"
                    } else {
                        "ARC"
                    },
                );
                pair(&mut s, 8, layer);
                pair(&mut s, 10, center.x);
                pair(&mut s, 20, center.y);
                pair(&mut s, 40, radius);
                if let Shape::Arc { start, end, .. } = &e.shape {
                    pair(&mut s, 50, start.rem_euclid(360.));
                    pair(&mut s, 51, end.rem_euclid(360.))
                }
            }
            Shape::Polyline { points, closed } => {
                pair(&mut s, 0, "POLYLINE");
                pair(&mut s, 8, layer);
                pair(&mut s, 66, 1);
                pair(&mut s, 70, if *closed { 1 } else { 0 });
                pair(&mut s, 10, 0);
                pair(&mut s, 20, 0);
                for p in points {
                    pair(&mut s, 0, "VERTEX");
                    pair(&mut s, 8, layer);
                    pair(&mut s, 10, p.x);
                    pair(&mut s, 20, p.y)
                }
                pair(&mut s, 0, "SEQEND")
            }
            Shape::Text { at, text, height } => {
                pair(&mut s, 0, "TEXT");
                pair(&mut s, 8, layer);
                pair(&mut s, 10, at.x);
                pair(&mut s, 20, at.y);
                pair(&mut s, 40, height);
                pair(&mut s, 1, text.replace(['\n', '\r'], " "))
            }
            Shape::Dimension { a, b, offset } => {
                for (a, b) in e.shape.segments() {
                    dxf_line(&mut s, layer, a, b)
                }
                let dir = b.sub(*a);
                let mid = a
                    .add(*b)
                    .mul(0.5)
                    .add(Point::new(-dir.y, dir.x).mul(*offset / a.dist(*b)));
                pair(&mut s, 0, "TEXT");
                pair(&mut s, 8, layer);
                pair(&mut s, 10, mid.x);
                pair(&mut s, 20, mid.y);
                pair(&mut s, 40, 2.5);
                pair(&mut s, 1, format!("{:0.2}", a.dist(*b)))
            }
            Shape::Box3D { min, max } => {
                let vertices = box_vertices(*min, *max);
                for face in [
                    [0, 3, 2, 1],
                    [4, 5, 6, 7],
                    [0, 1, 5, 4],
                    [1, 2, 6, 5],
                    [2, 3, 7, 6],
                    [3, 0, 4, 7],
                ] {
                    pair(&mut s, 0, "3DFACE");
                    pair(&mut s, 8, layer);
                    for (i, j) in face.iter().enumerate() {
                        for k in 0..3 {
                            pair(&mut s, 10 + i as i32 + 10 * k as i32, vertices[*j][k])
                        }
                    }
                }
            }
        }
    }
    pair(&mut s, 0, "ENDSEC");
    pair(&mut s, 0, "EOF");
    s
}
pub struct ImportReport {
    pub drawing: Drawing,
    pub warnings: Vec<String>,
}
pub fn import_dxf(s: &str) -> Result<ImportReport, String> {
    let lines = s.lines().collect::<Vec<_>>();
    if lines.len() % 2 != 0 {
        return Err("DXF must contain code/value pairs".into());
    }
    let mut pairs = vec![];
    for c in lines.chunks(2) {
        pairs.push((
            c[0].trim()
                .parse::<i32>()
                .map_err(|_| "Invalid DXF group code")?,
            c[1].trim().to_string(),
        ))
    }
    let mut d = Drawing::default();
    let mut warnings = vec![];
    let mut section = String::new();
    let mut poly: Option<(usize, Vec<Point>, bool)> = None;
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i].0 != 0 {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1
        }
        let record = &pairs[start..i];
        let kind = record[0].1.as_str();
        let val = |code: i32| {
            record
                .iter()
                .find(|(c, _)| *c == code)
                .map(|(_, v)| v.as_str())
        };
        let number = |code: i32| -> Result<f64, String> {
            let n = val(code)
                .unwrap_or("0")
                .parse::<f64>()
                .map_err(|_| format!("Invalid {} coordinate", kind))?;
            if !n.is_finite() {
                return Err("Non-finite DXF value".into());
            }
            Ok(n)
        };
        let pt = |x, y| -> Result<Point, String> { Ok(Point::new(number(x)?, number(y)?)) };
        if kind == "SECTION" {
            section = val(2).unwrap_or("").into();
            continue;
        }
        if kind == "ENDSEC" {
            section.clear();
            continue;
        }
        if section == "TABLES" && kind == "LAYER" {
            let name = val(2).unwrap_or("0");
            let visible = number(62)? >= 0.;
            let locked = number(70)? as i32 & 4 != 0;
            if let Some(l) = d.layers.iter_mut().find(|l| l.name == name) {
                l.visible = visible;
                l.locked = locked
            } else {
                d.layers.push(Layer {
                    name: name.into(),
                    visible,
                    locked,
                    color: [203, 225, 240],
                })
            }
            continue;
        }
        if section != "ENTITIES" {
            continue;
        }
        let name = val(8).unwrap_or("0");
        let layer = if let Some(n) = d.layers.iter().position(|l| l.name == name) {
            n
        } else {
            d.layers.push(Layer {
                name: name.into(),
                visible: true,
                locked: false,
                color: [203, 225, 240],
            });
            d.layers.len() - 1
        };
        let shape = match kind {
            "LINE" => Some(Shape::Line(pt(10, 20)?, pt(11, 21)?)),
            "CIRCLE" => Some(Shape::Circle {
                center: pt(10, 20)?,
                radius: number(40)?,
            }),
            "ARC" => Some(Shape::Arc {
                center: pt(10, 20)?,
                radius: number(40)?,
                start: number(50)?,
                end: number(51)?,
            }),
            "TEXT" => Some(Shape::Text {
                at: pt(10, 20)?,
                text: val(1).unwrap_or("").into(),
                height: number(40)?.max(0.001),
            }),
            "POLYLINE" => {
                poly = Some((layer, vec![], number(70)? as i32 & 1 != 0));
                None
            }
            "VERTEX" => {
                if let Some((_, ps, _)) = &mut poly {
                    ps.push(pt(10, 20)?)
                }
                None
            }
            "SEQEND" => {
                if let Some((l, points, closed)) = poly.take() {
                    if points.len() >= 2 {
                        let id = d.next_id;
                        d.next_id += 1;
                        d.entities.push(Entity {
                            id,
                            layer: l,
                            shape: Shape::Polyline { points, closed },
                        })
                    }
                }
                None
            }
            "LWPOLYLINE" => {
                if record
                    .iter()
                    .any(|(c, v)| *c == 42 && v.parse::<f64>().map_or(true, |x| x != 0.))
                {
                    warnings.push(
                        "LWPOLYLINE bulges are not supported; imported straight segments".into(),
                    )
                }
                let mut ps = vec![];
                let mut x = None;
                for (c, v) in record {
                    if *c == 10 {
                        x = Some(v.parse::<f64>().map_err(|_| "Invalid polyline X")?)
                    }
                    if *c == 20 {
                        if let Some(x) = x.take() {
                            ps.push(Point::new(
                                x,
                                v.parse::<f64>().map_err(|_| "Invalid polyline Y")?,
                            ))
                        }
                    }
                }
                Some(Shape::Polyline {
                    points: ps,
                    closed: number(70)? as i32 & 1 != 0,
                })
            }
            _ => {
                warnings.push(format!("Skipped unsupported DXF entity: {}", kind));
                None
            }
        };
        if let Some(shape) = shape {
            if !shape.validate() {
                return Err(format!("Invalid {} geometry", kind));
            }
            let id = d.next_id;
            d.next_id += 1;
            d.entities.push(Entity { id, layer, shape })
        }
    }
    if poly.is_some() {
        return Err("Unterminated POLYLINE".into());
    }
    if !pairs.iter().any(|(c, v)| *c == 0 && v == "EOF") {
        return Err("Missing DXF EOF marker".into());
    }
    warnings.sort();
    warnings.dedup();
    Ok(ImportReport {
        drawing: d,
        warnings,
    })
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn export_svg(d: &Drawing) -> String {
    let (min, max) = d
        .bounds()
        .unwrap_or((Point::new(0., 0.), Point::new(100., 100.)));
    let margin = 10.;
    let w = (max.x - min.x).max(1.) + margin * 2.;
    let h = (max.y - min.y).max(1.) + margin * 2.;
    let mut s=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">\n<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#101b28\"/>\n",min.x-margin,-max.y-margin,w,h,min.x-margin,-max.y-margin,w,h);
    for e in &d.entities {
        let l = &d.layers[e.layer];
        if !l.visible {
            continue;
        }
        let color = format!("#{:02x}{:02x}{:02x}", l.color[0], l.color[1], l.color[2]);
        for (a, b) in e.shape.segments() {
            writeln!(
                s,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\".5\"/>",
                a.x, -a.y, b.x, -b.y, color
            )
            .unwrap()
        }
        let text = match &e.shape {
            Shape::Text { at, text, height } => Some((*at, text.clone(), *height)),
            Shape::Dimension { a, b, offset } => {
                let v = b.sub(*a);
                Some((
                    a.add(*b)
                        .mul(0.5)
                        .add(Point::new(-v.y, v.x).mul(*offset / a.dist(*b))),
                    format!("{:0.2}", a.dist(*b)),
                    3.,
                ))
            }
            _ => None,
        };
        if let Some((p, t, h)) = text {
            writeln!(s,"<text x=\"{}\" y=\"{}\" fill=\"{}\" font-size=\"{}\" font-family=\"monospace\">{}</text>",p.x,-p.y,color,h,xml(&t)).unwrap()
        }
    }
    s.push_str("</svg>\n");
    s
}
pub fn box_vertices(min: [f64; 3], max: [f64; 3]) -> [[f64; 3]; 8] {
    [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
        [min[0], min[1], max[2]],
        [max[0], min[1], max[2]],
        [max[0], max[1], max[2]],
        [min[0], max[1], max[2]],
    ]
}
pub fn export_stl(d: &Drawing) -> Result<String, String> {
    let mut s = String::from("solid rust_cad\n");
    let mut n = 0;
    for e in &d.entities {
        if let Shape::Box3D { min, max } = e.shape {
            n += 1;
            let p = box_vertices(min, max);
            for f in [
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [1, 2, 6, 5],
                [2, 3, 7, 6],
                [3, 0, 4, 7],
            ] {
                for t in [[f[0], f[1], f[2]], [f[0], f[2], f[3]]] {
                    let a = p[t[0]];
                    let b = p[t[1]];
                    let c = p[t[2]];
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let normal = [
                        u[1] * v[2] - u[2] * v[1],
                        u[2] * v[0] - u[0] * v[2],
                        u[0] * v[1] - u[1] * v[0],
                    ];
                    let len = (normal.iter().map(|x| x * x).sum::<f64>()).sqrt();
                    writeln!(
                        s,
                        "facet normal {} {} {}\nouter loop",
                        normal[0] / len,
                        normal[1] / len,
                        normal[2] / len
                    )
                    .unwrap();
                    for q in [a, b, c] {
                        writeln!(s, "vertex {} {} {}", q[0], q[1], q[2]).unwrap()
                    }
                    s.push_str("endloop\nendfacet\n")
                }
            }
        }
    }
    if n == 0 {
        return Err("No 3D boxes to export".into());
    }
    s.push_str("endsolid rust_cad\n");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_roundtrip() {
        let mut d = Drawing::sample();
        d.add(
            0,
            Shape::Text {
                at: Point::new(2., 3.),
                text: "Tab\tline\n100%".into(),
                height: 2.,
            },
        )
        .unwrap();
        d.add(
            0,
            Shape::Arc {
                center: Point::new(1., 2.),
                radius: 3.,
                start: 10.,
                end: 300.,
            },
        )
        .unwrap();
        d.add(
            0,
            Shape::Box3D {
                min: [0.; 3],
                max: [1., 2., 3.],
            },
        )
        .unwrap();
        assert_eq!(load_project(&save_project(&d)).unwrap(), d)
    }
    #[test]
    fn history_branch() {
        let mut d = Drawing::default();
        let mut h = History::default();
        h.checkpoint(&d);
        d.add(0, Shape::Line(Point::new(0., 0.), Point::new(1., 1.)))
            .unwrap();
        assert!(h.undo(&mut d));
        assert!(d.entities.is_empty());
        assert!(h.redo(&mut d));
        assert_eq!(d.entities.len(), 1);
        h.checkpoint(&d);
        assert!(!h.redo(&mut d))
    }
    #[test]
    fn dxf_interoperable_entities() {
        let mut d = Drawing::sample();
        d.add(
            0,
            Shape::Arc {
                center: Point::new(0., 0.),
                radius: 10.,
                start: 350.,
                end: 30.,
            },
        )
        .unwrap();
        let text = export_dxf(&d);
        let r = import_dxf(&text).unwrap();
        assert!(r.warnings.is_empty());
        assert_eq!(
            r.drawing
                .entities
                .iter()
                .filter(|e| matches!(e.shape, Shape::Circle { .. }))
                .count(),
            4
        );
        assert!(r
            .drawing
            .entities
            .iter()
            .any(|e| matches!(e.shape, Shape::Polyline { closed: true, .. })));
    }
    #[test]
    fn intersection_snap() {
        let mut d = Drawing::default();
        d.add(0, Shape::Line(Point::new(0., 0.), Point::new(10., 10.)))
            .unwrap();
        d.add(0, Shape::Line(Point::new(0., 10.), Point::new(10., 0.)))
            .unwrap();
        let (p, label) = d.snap(Point::new(5.1, 5.1), 0.5, None);
        assert_eq!(p, Point::new(5., 5.));
        assert!(["MID", "INT"].contains(&label.as_str()))
    }
    #[test]
    fn finite_intersections() {
        assert!(intersection(
            Point::new(0., 0.),
            Point::new(1., 0.),
            Point::new(2., -1.),
            Point::new(2., 1.),
            true
        )
        .is_none());
        assert_eq!(
            intersection(
                Point::new(0., 0.),
                Point::new(1., 0.),
                Point::new(2., -1.),
                Point::new(2., 1.),
                false
            ),
            Some(Point::new(2., 0.))
        )
    }
    #[test]
    fn reject_degenerate() {
        let mut d = Drawing::default();
        assert!(d
            .add(
                0,
                Shape::Circle {
                    center: Point::default(),
                    radius: -1.
                }
            )
            .is_err());
        assert!(d
            .add(0, Shape::Line(Point::default(), Point::default()))
            .is_err());
        assert!(d
            .add(
                0,
                Shape::Box3D {
                    min: [0.; 3],
                    max: [0.; 3]
                }
            )
            .is_err())
    }
    #[test]
    fn locked_layer() {
        let mut d = Drawing::sample();
        d.layers[0].locked = true;
        let n = d.entities.len();
        d.erase(&[1]);
        assert_eq!(d.entities.len(), n);
        assert!(d
            .add(
                0,
                Shape::Circle {
                    center: Point::default(),
                    radius: 1.
                }
            )
            .is_err())
    }
    #[test]
    fn trim_keeps_clicked_side() {
        let mut d = Drawing::default();
        let a = d
            .add(0, Shape::Line(Point::new(0., 0.), Point::new(10., 0.)))
            .unwrap();
        let b = d
            .add(0, Shape::Line(Point::new(4., -5.), Point::new(4., 5.)))
            .unwrap();
        d.trim_line(a, b, Point::new(1., 0.)).unwrap();
        assert_eq!(
            d.entities[0].shape,
            Shape::Line(Point::new(0., 0.), Point::new(4., 0.))
        )
    }
    #[test]
    fn offset_circle() {
        let mut d = Drawing::default();
        let id = d
            .add(
                0,
                Shape::Circle {
                    center: Point::default(),
                    radius: 5.,
                },
            )
            .unwrap();
        d.offset(id, 2.).unwrap();
        assert!(matches!(
            d.entities[1].shape,
            Shape::Circle { radius: 7., .. }
        ));
        assert!(d.offset(id, -8.).is_err())
    }
    #[test]
    fn svg_escapes_text() {
        let mut d = Drawing::default();
        d.add(
            0,
            Shape::Text {
                at: Point::default(),
                text: "A&B <tag>".into(),
                height: 2.,
            },
        )
        .unwrap();
        assert!(export_svg(&d).contains("A&amp;B &lt;tag&gt;"))
    }
    #[test]
    fn stl_closed_box() {
        let mut d = Drawing::default();
        d.add(
            0,
            Shape::Box3D {
                min: [0.; 3],
                max: [2., 3., 4.],
            },
        )
        .unwrap();
        assert_eq!(export_stl(&d).unwrap().matches("facet normal").count(), 12)
    }
    #[test]
    fn reject_bad_project() {
        assert!(load_project(
            "RUSTCAD\t1\tmm\nLAYER\t0\t1\t0\t255\t255\t255\nENTITY\t1\t0\tCIRCLE\tNaN\t0\t4"
        )
        .is_err());
        assert!(load_project("wrong").is_err())
    }
    #[test]
    fn dxf_unknown_is_reported() {
        let r =
            import_dxf("0\nSECTION\n2\nENTITIES\n0\nSPLINE\n8\n0\n0\nENDSEC\n0\nEOF\n").unwrap();
        assert_eq!(r.warnings.len(), 1)
    }
    #[test]
    fn arc_wrap() {
        let s = Shape::Arc {
            center: Point::default(),
            radius: 10.,
            start: 350.,
            end: 10.,
        };
        assert!(s.segments().len() < 10);
        assert!(
            s.segments().last().unwrap().1.dist(Point::new(
                10. * 10_f64.to_radians().cos(),
                10. * 10_f64.to_radians().sin()
            )) < 1e-8
        )
    }
}
