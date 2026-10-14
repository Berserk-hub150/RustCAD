#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use rust_cad_core::*;
use std::{f64::consts::PI, fs};
#[derive(Clone, Copy, PartialEq, Debug)]
enum Tool {
    Select,
    Line,
    Polyline,
    Rectangle,
    Circle,
    Arc,
    Text,
    Dimension,
    Measure,
    Pan,
    Box3D,
}
impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Line => "Line",
            Self::Polyline => "Polyline",
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::Arc => "Arc",
            Self::Text => "Text",
            Self::Dimension => "Dimension",
            Self::Measure => "Measure",
            Self::Pan => "Pan",
            Self::Box3D => "3D box",
        }
    }
    fn prompt(self, n: usize) -> &'static str {
        match (self, n) {
            (Self::Select, _) => "Click to select; Shift adds; Delete erases",
            (Self::Line, 0) | (Self::Polyline, 0) => "First point",
            (Self::Line, _) => "Next point; Escape finishes",
            (Self::Polyline, _) => "Next vertex; Enter finishes; CLOSE closes",
            (Self::Circle, 0) => "Center point",
            (Self::Circle, _) => "Radius point",
            (Self::Rectangle, 0) | (Self::Box3D, 0) => "First corner",
            (Self::Rectangle, _) | (Self::Box3D, _) => "Opposite corner",
            (Self::Arc, 0) => "Arc center",
            (Self::Arc, 1) => "Arc start point",
            (Self::Arc, _) => "End angle (counterclockwise)",
            (Self::Text, _) => "Text insertion point",
            (Self::Dimension, 0) | (Self::Measure, 0) => "First measurement point",
            (Self::Dimension, _) | (Self::Measure, _) => "Second measurement point",
            (Self::Pan, _) => "Drag to pan; wheel zooms; F fits",
        }
    }
}
struct Cad {
    d: Drawing,
    h: History,
    tool: Tool,
    points: Vec<Point>,
    selection: Vec<u64>,
    layer: usize,
    center: Point,
    zoom: f64,
    pointer: Point,
    snap_label: String,
    grid: bool,
    grid_snap: bool,
    osnap: bool,
    ortho: bool,
    spacing: f64,
    command: String,
    log: Vec<String>,
    path: String,
    text: String,
    text_height: f64,
    dim_offset: f64,
    box_height: f64,
    dx: f64,
    dy: f64,
    angle: f64,
    factor: f64,
    offset: f64,
    is3d: bool,
    yaw: f64,
    pitch: f64,
    dirty: bool,
    help: bool,
}
impl Default for Cad {
    fn default() -> Self {
        Self {
            d: Drawing::sample(),
            h: History::default(),
            tool: Tool::Select,
            points: vec![],
            selection: vec![],
            layer: 0,
            center: Point::new(65., 45.),
            zoom: 5.,
            pointer: Point::default(),
            snap_label: String::new(),
            grid: true,
            grid_snap: false,
            osnap: true,
            ortho: false,
            spacing: 10.,
            command: String::new(),
            log: vec!["RustCAD ready. The sample is editable. New starts an empty drawing.".into()],
            path: "drawing.rcad".into(),
            text: "CAD annotation".into(),
            text_height: 3.,
            dim_offset: 12.,
            box_height: 25.,
            dx: 10.,
            dy: 0.,
            angle: 45.,
            factor: 1.5,
            offset: 5.,
            is3d: false,
            yaw: 0.7,
            pitch: 0.65,
            dirty: false,
            help: false,
        }
    }
}
impl Cad {
    fn note(&mut self, s: impl Into<String>) {
        self.log.push(s.into());
        if self.log.len() > 5 {
            self.log.remove(0);
        }
    }
    fn checkpoint(&mut self) {
        self.h.checkpoint(&self.d);
        self.dirty = true
    }
    fn tool(&mut self, t: Tool) {
        self.tool = t;
        self.points.clear();
        self.note(t.prompt(0))
    }
    fn add(&mut self, s: Shape) {
        if !s.validate() {
            self.note("Invalid or degenerate geometry");
            return;
        }
        if self.d.layers[self.layer].locked {
            self.note("Current layer is locked");
            return;
        }
        self.checkpoint();
        match self.d.add(self.layer, s) {
            Ok(id) => {
                self.selection = vec![id];
                self.note(format!("Created entity #{id}"))
            }
            Err(e) => self.note(e),
        }
    }
    fn erase(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        self.checkpoint();
        self.d.erase(&self.selection);
        self.selection.clear()
    }
    fn undo(&mut self) {
        if self.h.undo(&mut self.d) {
            self.points.clear();
            self.selection.clear();
            self.layer = self.layer.min(self.d.layers.len() - 1);
            self.dirty = true;
            self.note("Undo")
        }
    }
    fn redo(&mut self) {
        if self.h.redo(&mut self.d) {
            self.selection.clear();
            self.layer = self.layer.min(self.d.layers.len() - 1);
            self.dirty = true;
            self.note("Redo")
        }
    }
    fn fit(&mut self, size: Vec2) {
        if let Some((a, b)) = self.d.bounds() {
            self.center = a.add(b).mul(0.5);
            self.zoom = ((size.x as f64 - 80.) / (b.x - a.x).max(20.))
                .min((size.y as f64 - 80.) / (b.y - a.y).max(20.))
                .clamp(0.01, 10000.)
        }
    }
    fn modify(&mut self, op: &str) -> Result<(), String> {
        if self.selection.is_empty() {
            return Err("Select entities first".into());
        }
        let ids = self.selection.clone();
        let dx = self.dx;
        let dy = self.dy;
        let angle = self.angle;
        let k = self.factor;
        let pivot = self
            .d
            .entities
            .iter()
            .filter(|e| ids.contains(&e.id))
            .flat_map(|e| e.shape.anchors().into_iter().map(|q| q.0))
            .next()
            .unwrap_or_default();
        if op == "scale" && (!k.is_finite() || k <= 0.) {
            return Err("Scale must be positive".into());
        }
        if op == "rotate"
            && self
                .d
                .entities
                .iter()
                .any(|e| ids.contains(&e.id) && matches!(e.shape, Shape::Box3D { .. }))
        {
            return Err("Rotate supports 2D entities; 3D boxes remain axis-aligned".into());
        }
        self.checkpoint();
        match op {
            "move" => {
                self.d.edit(&ids, |s| {
                    s.map_points(|p| p.add(Point::new(dx, dy)), 1., 0.)
                });
            }
            "copy" => {
                let copies = self
                    .d
                    .entities
                    .iter()
                    .filter(|e| ids.contains(&e.id) && self.d.selectable(e))
                    .cloned()
                    .collect::<Vec<_>>();
                self.selection.clear();
                for mut e in copies {
                    e.shape.map_points(|p| p.add(Point::new(dx, dy)), 1., 0.);
                    self.selection.push(self.d.add(e.layer, e.shape)?)
                }
            }
            "rotate" => {
                self.d.edit(&ids, |s| {
                    s.map_points(|p| p.rotate(pivot, angle.to_radians()), 1., angle)
                });
            }
            "scale" => {
                self.d.edit(&ids, |s| {
                    s.map_points(|p| pivot.add(p.sub(pivot).mul(k)), k, 0.)
                });
            }
            "mirror" => {
                self.d.edit(&ids, |s| {
                    s.map_points(|p| Point::new(2. * pivot.x - p.x, p.y), 1., 0.);
                    if let Shape::Arc { start, end, .. } = s {
                        let a = 180. - *end;
                        *end = 180. - *start;
                        *start = a
                    }
                    if let Shape::Dimension { offset, .. } = s {
                        *offset = -*offset
                    }
                });
            }
            "offset" => {
                for id in ids {
                    self.d.offset(id, self.offset)?;
                }
            }
            _ => return Err("Unknown edit".into()),
        }
        self.note(format!("{} completed", op.to_uppercase()));
        Ok(())
    }
    fn modify_ui(&mut self, op: &str) {
        if let Err(e) = self.modify(op) {
            self.note(e)
        }
    }
    fn save(&mut self) {
        match fs::write(&self.path, save_project(&self.d)) {
            Ok(()) => {
                self.dirty = false;
                self.note(format!("Saved {}", self.path))
            }
            Err(e) => self.note(format!("Save failed: {e}")),
        }
    }
    fn open(&mut self) {
        let r = fs::read_to_string(&self.path)
            .map_err(|e| e.to_string())
            .and_then(|s| {
                if self.path.to_lowercase().ends_with(".dxf") {
                    import_dxf(&s).map(|r| (r.drawing, r.warnings.join("; ")))
                } else {
                    load_project(&s).map(|d| (d, String::new()))
                }
            });
        match r {
            Ok((d, w)) => {
                self.checkpoint();
                self.d = d;
                self.selection.clear();
                self.points.clear();
                self.layer = 0;
                self.center = self
                    .d
                    .bounds()
                    .map(|(a, b)| a.add(b).mul(0.5))
                    .unwrap_or_default();
                self.dirty = false;
                self.note(format!("Opened {}", self.path));
                if !w.is_empty() {
                    self.note(w)
                }
            }
            Err(e) => self.note(format!("Open failed: {e}")),
        }
    }
    fn export(&mut self, ext: &str) {
        let path = std::path::Path::new(&self.path).with_extension(ext);
        let r = match ext {
            "dxf" => Ok(export_dxf(&self.d)),
            "svg" => Ok(export_svg(&self.d)),
            "stl" => export_stl(&self.d),
            _ => Err("Unknown export format".into()),
        };
        match r.and_then(|s| fs::write(&path, s).map_err(|e| e.to_string())) {
            Ok(()) => self.note(format!("Exported {}", path.display())),
            Err(e) => self.note(e),
        }
    }
    fn point(&mut self, p: Point, shift: bool) {
        match self.tool {
            Tool::Select => {
                if let Some(id) = self.d.hit(p, 9. / self.zoom) {
                    if shift {
                        if self.selection.contains(&id) {
                            self.selection.retain(|x| *x != id)
                        } else {
                            self.selection.push(id)
                        }
                    } else {
                        self.selection = vec![id]
                    }
                } else if !shift {
                    self.selection.clear()
                }
            }
            Tool::Line => {
                if let Some(a) = self.points.first().copied() {
                    self.add(Shape::Line(a, p));
                    self.points = vec![p]
                } else {
                    self.points.push(p)
                }
            }
            Tool::Polyline => self.points.push(p),
            Tool::Circle => {
                if let Some(a) = self.points.first().copied() {
                    self.add(Shape::Circle {
                        center: a,
                        radius: a.dist(p),
                    });
                    self.points.clear()
                } else {
                    self.points.push(p)
                }
            }
            Tool::Rectangle | Tool::Box3D => {
                if let Some(a) = self.points.first().copied() {
                    let s = if self.tool == Tool::Box3D {
                        Shape::Box3D {
                            min: [a.x.min(p.x), a.y.min(p.y), 0.],
                            max: [a.x.max(p.x), a.y.max(p.y), self.box_height],
                        }
                    } else {
                        Shape::Polyline {
                            points: vec![a, Point::new(p.x, a.y), p, Point::new(a.x, p.y)],
                            closed: true,
                        }
                    };
                    self.add(s);
                    self.points.clear()
                } else {
                    self.points.push(p)
                }
            }
            Tool::Arc => {
                self.points.push(p);
                if self.points.len() == 3 {
                    let c = self.points[0];
                    let a = self.points[1];
                    self.add(Shape::Arc {
                        center: c,
                        radius: c.dist(a),
                        start: (a.y - c.y).atan2(a.x - c.x).to_degrees(),
                        end: (p.y - c.y).atan2(p.x - c.x).to_degrees(),
                    });
                    self.points.clear()
                }
            }
            Tool::Text => self.add(Shape::Text {
                at: p,
                text: self.text.clone(),
                height: self.text_height,
            }),
            Tool::Dimension | Tool::Measure => {
                if let Some(a) = self.points.first().copied() {
                    if self.tool == Tool::Dimension {
                        self.add(Shape::Dimension {
                            a,
                            b: p,
                            offset: self.dim_offset,
                        })
                    } else {
                        let v = p.sub(a);
                        self.note(format!(
                            "Distance {:.4} {} | dX {:.4} | dY {:.4} | angle {:.2}°",
                            a.dist(p),
                            self.d.units,
                            v.x,
                            v.y,
                            v.y.atan2(v.x).to_degrees()
                        ))
                    }
                    self.points.clear()
                } else {
                    self.points.push(p)
                }
            }
            Tool::Pan => (),
        }
    }
    fn finish_poly(&mut self, closed: bool) {
        if self.tool == Tool::Polyline && self.points.len() >= 2 {
            self.add(Shape::Polyline {
                points: self.points.clone(),
                closed,
            });
            self.points.clear()
        }
    }
    fn command(&mut self, cmd: &str) -> Result<(), String> {
        if cmd.is_empty() {
            return Ok(());
        }
        if cmd.contains(',') && self.tool != Tool::Select {
            let relative = cmd.starts_with('@');
            let v = cmd
                .trim_start_matches('@')
                .split(',')
                .map(|n| {
                    n.trim()
                        .parse::<f64>()
                        .map_err(|_| "Use x,y or @dx,dy".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            if v.len() != 2 || v.iter().any(|n| !n.is_finite()) {
                return Err("Use two finite coordinates".into());
            }
            let p = Point::new(v[0], v[1]);
            self.point(
                if relative {
                    self.points.last().copied().unwrap_or_default().add(p)
                } else {
                    p
                },
                false,
            );
            return Ok(());
        }
        let a = cmd.split_whitespace().collect::<Vec<_>>();
        let name = a[0].to_uppercase();
        let nums = |len| -> Result<Vec<f64>, String> {
            let v = a[1..]
                .iter()
                .map(|s| s.parse::<f64>().map_err(|_| "Invalid number".to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            if v.len() != len || v.iter().any(|n| !n.is_finite()) {
                return Err(format!("{name} requires {len} finite numbers"));
            }
            Ok(v)
        };
        match name.as_str() {
            "LINE" | "L" if a.len() == 1 => self.tool(Tool::Line),
            "LINE" | "L" => {
                let v = nums(4)?;
                self.add(Shape::Line(Point::new(v[0], v[1]), Point::new(v[2], v[3])))
            }
            "CIRCLE" | "C" if a.len() == 1 => self.tool(Tool::Circle),
            "CIRCLE" | "C" => {
                let v = nums(3)?;
                self.add(Shape::Circle {
                    center: Point::new(v[0], v[1]),
                    radius: v[2],
                })
            }
            "ARC" if a.len() == 1 => self.tool(Tool::Arc),
            "ARC" => {
                let v = nums(5)?;
                self.add(Shape::Arc {
                    center: Point::new(v[0], v[1]),
                    radius: v[2],
                    start: v[3],
                    end: v[4],
                })
            }
            "RECT" | "RECTANGLE" if a.len() == 1 => self.tool(Tool::Rectangle),
            "RECT" | "RECTANGLE" => {
                let v = nums(4)?;
                self.add(Shape::Polyline {
                    points: vec![
                        Point::new(v[0], v[1]),
                        Point::new(v[2], v[1]),
                        Point::new(v[2], v[3]),
                        Point::new(v[0], v[3]),
                    ],
                    closed: true,
                })
            }
            "PLINE" => self.tool(Tool::Polyline),
            "CLOSE" => self.finish_poly(true),
            "DIM" if a.len() == 1 => self.tool(Tool::Dimension),
            "DIM" => {
                let v = nums(5)?;
                self.add(Shape::Dimension {
                    a: Point::new(v[0], v[1]),
                    b: Point::new(v[2], v[3]),
                    offset: v[4],
                })
            }
            "BOX" if a.len() == 1 => self.tool(Tool::Box3D),
            "BOX" => {
                let v = nums(6)?;
                self.add(Shape::Box3D {
                    min: [v[0], v[1], v[2]],
                    max: [v[0] + v[3], v[1] + v[4], v[2] + v[5]],
                })
            }
            "MOVE" | "COPY" => {
                let v = nums(2)?;
                self.dx = v[0];
                self.dy = v[1];
                self.modify(&name.to_lowercase())?
            }
            "ROTATE" => {
                self.angle = nums(1)?[0];
                self.modify("rotate")?
            }
            "SCALE" => {
                self.factor = nums(1)?[0];
                self.modify("scale")?
            }
            "MIRROR" => self.modify("mirror")?,
            "OFFSET" => {
                self.offset = nums(1)?[0];
                self.modify("offset")?
            }
            "TRIM" => {
                if a.len() != 5 {
                    return Err("TRIM target_id boundary_id keep_x keep_y".into());
                }
                let target = a[1].parse::<u64>().map_err(|_| "Invalid target")?;
                let boundary = a[2].parse::<u64>().map_err(|_| "Invalid boundary")?;
                let x = a[3].parse::<f64>().map_err(|_| "Invalid X")?;
                let y = a[4].parse::<f64>().map_err(|_| "Invalid Y")?;
                if !x.is_finite() || !y.is_finite() {
                    return Err("Keep point must be finite".into());
                }
                self.checkpoint();
                self.d.trim_line(target, boundary, Point::new(x, y))?
            }
            "ERASE" | "DELETE" => self.erase(),
            "UNDO" | "U" => self.undo(),
            "REDO" => self.redo(),
            "SELECT" if a.len() > 1 => {
                self.selection = a[1..]
                    .iter()
                    .map(|v| v.parse::<u64>().map_err(|_| "Invalid ID".to_string()))
                    .collect::<Result<Vec<_>, _>>()?
            }
            "SELECT" => self.tool(Tool::Select),
            "LAYER" if a.len() == 2 => {
                if let Some(n) = self.d.layers.iter().position(|l| l.name == a[1]) {
                    self.layer = n
                } else {
                    self.checkpoint();
                    self.d.layers.push(Layer {
                        name: a[1].into(),
                        visible: true,
                        locked: false,
                        color: [110, 206, 249],
                    });
                    self.layer = self.d.layers.len() - 1
                }
            }
            "OPEN" if a.len() > 1 => {
                self.path = cmd[cmd.find(' ').unwrap() + 1..].trim_matches('"').into();
                self.open()
            }
            "SAVE" => {
                if a.len() > 1 {
                    self.path = cmd[cmd.find(' ').unwrap() + 1..].trim_matches('"').into()
                }
                self.save()
            }
            "EXPORT" if a.len() == 2 => self.export(a[1]),
            "3D" => self.is3d = true,
            "2D" => self.is3d = false,
            "HELP" => self.help = true,
            _ => return Err("Unknown command or arguments. Use HELP.".into()),
        }
        Ok(())
    }
    fn top(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(5.);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("RUST / CAD")
                        .strong()
                        .color(Color32::from_rgb(90, 231, 183)),
                );
                ui.separator();
                if ui.button("New").clicked() {
                    self.checkpoint();
                    self.d = Drawing::default();
                    self.points.clear();
                    self.selection.clear();
                    self.layer = 0;
                    self.note("New document. Undo restores the previous drawing.")
                }
                if ui.button("Sample plate").clicked() {
                    self.checkpoint();
                    self.d = Drawing::sample();
                    self.layer = 0;
                    self.selection.clear();
                    self.center = Point::new(65., 45.);
                    self.zoom = 5.
                }
                if ui.button("Open").clicked() {
                    self.open()
                }
                if ui.button("Save").clicked() {
                    self.save()
                }
                ui.separator();
                if ui.button("Undo").clicked() {
                    self.undo()
                }
                if ui.button("Redo").clicked() {
                    self.redo()
                }
                ui.separator();
                if ui.selectable_label(!self.is3d, "2D Model").clicked() {
                    self.is3d = false
                }
                if ui.selectable_label(self.is3d, "3D Preview").clicked() {
                    self.is3d = true
                }
                if ui.button("Help").clicked() {
                    self.help = !self.help
                }
                ui.small(if self.dirty {
                    "● modified"
                } else {
                    "saved / sample"
                });
            });
            ui.horizontal(|ui| {
                ui.label("File");
                ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(280.));
                ui.label("Export");
                for ext in ["dxf", "svg", "stl"] {
                    if ui.button(ext.to_uppercase()).clicked() {
                        self.export(ext)
                    }
                }
                ui.separator();
                ui.checkbox(&mut self.grid, "Grid");
                ui.checkbox(&mut self.grid_snap, "Grid snap");
                ui.checkbox(&mut self.osnap, "Object snap");
                ui.checkbox(&mut self.ortho, "Ortho");
            });
            ui.add_space(5.);
        });
    }
    fn side(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").resizable(true).default_width(235.).show(ctx,|ui|{egui::ScrollArea::vertical().show(ui,|ui|{
  ui.add_space(10.);ui.heading("Draw");ui.small("MODEL SPACE · MILLIMETERS");ui.add_space(8.);
  egui::Grid::new("draw-grid").num_columns(2).spacing([5.,5.]).show(ui,|ui|{
   for(i,t)in[Tool::Select,Tool::Line,Tool::Polyline,Tool::Rectangle,Tool::Circle,Tool::Arc,Tool::Text,Tool::Dimension,Tool::Measure,Tool::Pan,Tool::Box3D].into_iter().enumerate(){if ui.add_sized([103.,30.],egui::Button::new(t.label()).selected(self.tool==t)).clicked(){self.tool(t)}if i%2==1{ui.end_row()}}
  });
  ui.separator();ui.heading("Layers");
  for i in 0..self.d.layers.len(){let before=self.d.clone();let mut l=self.d.layers[i].clone();let mut choose=false;
   ui.horizontal(|ui|{if ui.selectable_label(self.layer==i,&l.name).clicked(){choose=true}ui.checkbox(&mut l.visible,"V");ui.checkbox(&mut l.locked,"L");ui.color_edit_button_srgb(&mut l.color);});
   if choose{self.layer=i}if l!=self.d.layers[i]{self.h.checkpoint(&before);self.d.layers[i]=l;self.dirty=true}
  }
  if ui.button("+ Add layer").clicked(){self.checkpoint();let n=self.d.layers.len();self.d.layers.push(Layer{name:format!("Layer-{n}"),visible:true,locked:false,color:[110,206,249]});self.layer=n}
  ui.separator();ui.heading("Selection");ui.label(format!("{} selected / {} entities",self.selection.len(),self.d.entities.len()));
  for e in self.d.entities.iter().filter(|e|self.selection.contains(&e.id)).take(5){ui.small(format!("#{} · {}",e.id,self.d.layers[e.layer].name));}
  ui.horizontal(|ui|{if ui.button("Select all").clicked(){self.selection=self.d.entities.iter().filter(|e|self.d.selectable(e)).map(|e|e.id).collect()}if ui.button("Erase").clicked(){self.erase()}});
  ui.separator();ui.heading("Modify");
  ui.horizontal(|ui|{ui.label("ΔX");ui.add(egui::DragValue::new(&mut self.dx));ui.label("ΔY");ui.add(egui::DragValue::new(&mut self.dy));});
  ui.horizontal(|ui|{for op in["move","copy","mirror"]{if ui.button(op).clicked(){self.modify_ui(op)}}});
  ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.angle).suffix("°"));if ui.button("Rotate").clicked(){self.modify_ui("rotate")}});
  ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.factor).range(0.001..=1000.).prefix("× "));if ui.button("Scale").clicked(){self.modify_ui("scale")}});
  ui.horizontal(|ui|{ui.add(egui::DragValue::new(&mut self.offset).suffix(" mm"));if ui.button("Offset").clicked(){self.modify_ui("offset")}});
  ui.small("Pivot: first anchor of selection. Mirror: vertical axis. OFFSET: lines/circles/arcs.");
  ui.separator();ui.collapsing("Drawing settings",|ui|{
   ui.label("Annotation text");ui.text_edit_singleline(&mut self.text);
   ui.horizontal(|ui|{ui.label("Text height");ui.add(egui::DragValue::new(&mut self.text_height).range(0.1..=1000.));});
   ui.horizontal(|ui|{ui.label("Dimension offset");ui.add(egui::DragValue::new(&mut self.dim_offset));});
   ui.horizontal(|ui|{ui.label("Grid spacing");ui.add(egui::DragValue::new(&mut self.spacing).range(0.001..=10000.));});
   ui.horizontal(|ui|{ui.label("Box height");ui.add(egui::DragValue::new(&mut self.box_height).range(0.001..=10000.));});
  });
  if self.is3d{ui.separator();ui.label("3D camera");ui.add(egui::Slider::new(&mut self.yaw,-PI..=PI).text("Yaw"));ui.add(egui::Slider::new(&mut self.pitch,0.05..=1.5).text("Pitch"));ui.small("Wireframe boxes and planar geometry. BOX x y z width depth height.");}
 });});
    }
    fn bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.small(format!(
                    "X {:.3}   Y {:.3} {}",
                    self.pointer.x, self.pointer.y, self.d.units
                ));
                ui.separator();
                ui.small(&self.snap_label);
                ui.separator();
                ui.small(self.tool.prompt(self.points.len()));
            });
            for line in self.log.iter().rev().take(2).rev() {
                ui.label(
                    egui::RichText::new(line)
                        .monospace()
                        .size(11.)
                        .color(Color32::from_rgb(157, 180, 201)),
                );
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("COMMAND ›").color(Color32::from_rgb(90, 231, 183)));
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command)
                        .desired_width(f32::INFINITY)
                        .hint_text("LINE 0 0 100 0 · CIRCLE 50 50 10 · x,y · @dx,dy · HELP"),
                );
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let cmd = std::mem::take(&mut self.command);
                    self.note(format!("> {cmd}"));
                    if let Err(e) = self.command(cmd.trim()) {
                        self.note(e)
                    }
                    response.request_focus();
                }
            });
        });
    }
    fn screen(&self, p: Point, r: Rect) -> Pos2 {
        Pos2::new(
            r.center().x + ((p.x - self.center.x) * self.zoom) as f32,
            r.center().y - ((p.y - self.center.y) * self.zoom) as f32,
        )
    }
    fn world(&self, p: Pos2, r: Rect) -> Point {
        Point::new(
            (p.x - r.center().x) as f64 / self.zoom + self.center.x,
            -(p.y - r.center().y) as f64 / self.zoom + self.center.y,
        )
    }
    fn project3(&self, p: [f64; 3], r: Rect) -> Pos2 {
        let x = p[0] - self.center.x;
        let y = p[1] - self.center.y;
        let (s, c) = self.yaw.sin_cos();
        Pos2::new(
            r.center().x + ((x * c - y * s) * self.zoom) as f32,
            r.center().y
                + ((x * s + y * c) * self.pitch.sin() * self.zoom
                    - p[2] * self.pitch.cos() * self.zoom) as f32,
        )
    }
    fn paint(&self, p: &egui::Painter, r: Rect, s: &Shape, color: Color32, width: f32) {
        let stroke = Stroke::new(width, color);
        if self.is3d {
            if let Shape::Box3D { min, max } = s {
                let v = box_vertices(*min, *max);
                for (a, b) in [
                    (0, 1),
                    (1, 2),
                    (2, 3),
                    (3, 0),
                    (4, 5),
                    (5, 6),
                    (6, 7),
                    (7, 4),
                    (0, 4),
                    (1, 5),
                    (2, 6),
                    (3, 7),
                ] {
                    p.line_segment([self.project3(v[a], r), self.project3(v[b], r)], stroke);
                }
                return;
            }
            for (a, b) in s.segments() {
                p.line_segment(
                    [
                        self.project3([a.x, a.y, 0.], r),
                        self.project3([b.x, b.y, 0.], r),
                    ],
                    stroke,
                );
            }
            return;
        }
        for (a, b) in s.segments() {
            p.line_segment([self.screen(a, r), self.screen(b, r)], stroke);
        }
        match s {
            Shape::Text { at, text, height } => {
                p.text(
                    self.screen(*at, r),
                    Align2::LEFT_BOTTOM,
                    text,
                    FontId::monospace((*height * self.zoom).clamp(6., 200.) as f32),
                    color,
                );
            }
            Shape::Dimension { a, b, offset } => {
                let v = b.sub(*a);
                let n = Point::new(-v.y, v.x).mul(*offset / a.dist(*b));
                let aa = a.add(n);
                let bb = b.add(n);
                p.text(
                    self.screen(aa.add(bb).mul(0.5), r) - Vec2::new(0., 6.),
                    Align2::CENTER_BOTTOM,
                    format!("{:.2}", a.dist(*b)),
                    FontId::monospace(12.),
                    color,
                );
                let u = v.mul(1. / a.dist(*b));
                for (q, dir) in [(aa, 1.), (bb, -1.)] {
                    for side in [-1., 1.] {
                        let end = q
                            .add(u.mul(dir * 7. / self.zoom))
                            .add(Point::new(-u.y, u.x).mul(side * 3. / self.zoom));
                        p.line_segment([self.screen(q, r), self.screen(end, r)], stroke);
                    }
                }
            }
            _ => (),
        }
    }
    fn canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(13, 23, 35)))
            .show(ctx, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                let r = response.rect;
                if response.hovered() {
                    if let Some(pos) = response.hover_pos() {
                        let before = self.world(pos, r);
                        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                        if scroll.abs() > 0. {
                            self.zoom =
                                (self.zoom * (scroll as f64 * 0.002).exp()).clamp(0.005, 50000.);
                            let after = self.world(pos, r);
                            self.center = self.center.add(before.sub(after));
                        }
                        let raw = self.world(pos, r);
                        let (mut point, label) = if self.osnap {
                            self.d.snap(
                                raw,
                                10. / self.zoom,
                                if self.grid_snap {
                                    Some(self.spacing)
                                } else {
                                    None
                                },
                            )
                        } else if self.grid_snap {
                            (
                                Point::new(
                                    (raw.x / self.spacing).round() * self.spacing,
                                    (raw.y / self.spacing).round() * self.spacing,
                                ),
                                "GRID".into(),
                            )
                        } else {
                            (raw, String::new())
                        };
                        if self.ortho {
                            if let Some(a) = self.points.last() {
                                if (point.x - a.x).abs() > (point.y - a.y).abs() {
                                    point.y = a.y
                                } else {
                                    point.x = a.x
                                }
                            }
                        }
                        self.pointer = point;
                        self.snap_label = label;
                        if response.clicked() && !self.is3d {
                            self.point(point, ui.input(|i| i.modifiers.shift))
                        }
                        if response.secondary_clicked() {
                            self.finish_poly(false);
                            self.points.clear();
                        }
                    }
                    if ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle))
                        || self.tool == Tool::Pan && response.dragged()
                    {
                        let delta = ui.input(|i| i.pointer.delta());
                        self.center.x -= delta.x as f64 / self.zoom;
                        self.center.y += delta.y as f64 / self.zoom;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::F)) && !ctx.wants_keyboard_input() {
                        self.fit(r.size())
                    }
                }
                if self.grid && !self.is3d {
                    let mut step = self.spacing;
                    while step * self.zoom < 18. {
                        step *= 2.
                    }
                    let min = self.world(r.left_bottom(), r);
                    let max = self.world(r.right_top(), r);
                    let col = Color32::from_rgb(26, 43, 60);
                    for i in (min.x / step).floor() as i64..=(max.x / step).ceil() as i64 {
                        let x = self.screen(Point::new(i as f64 * step, 0.), r).x;
                        painter.line_segment(
                            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                            Stroke::new(0.5, col),
                        );
                    }
                    for i in (min.y / step).floor() as i64..=(max.y / step).ceil() as i64 {
                        let y = self.screen(Point::new(0., i as f64 * step), r).y;
                        painter.line_segment(
                            [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
                            Stroke::new(0.5, col),
                        );
                    }
                    let zero = self.screen(Point::default(), r);
                    painter.line_segment(
                        [Pos2::new(r.left(), zero.y), Pos2::new(r.right(), zero.y)],
                        Stroke::new(0.7, Color32::from_rgb(52, 82, 78)),
                    );
                    painter.line_segment(
                        [Pos2::new(zero.x, r.top()), Pos2::new(zero.x, r.bottom())],
                        Stroke::new(0.7, Color32::from_rgb(57, 64, 86)),
                    );
                }
                for e in &self.d.entities {
                    let l = &self.d.layers[e.layer];
                    if !l.visible {
                        continue;
                    }
                    let chosen = self.selection.contains(&e.id);
                    let color = if chosen {
                        Color32::from_rgb(255, 199, 92)
                    } else {
                        Color32::from_rgb(l.color[0], l.color[1], l.color[2])
                    };
                    self.paint(&painter, r, &e.shape, color, if chosen { 2.2 } else { 1.3 });
                    if chosen && !self.is3d {
                        for (a, _) in e.shape.anchors() {
                            painter.circle_filled(
                                self.screen(a, r),
                                3.,
                                Color32::from_rgb(255, 199, 92),
                            );
                        }
                    }
                }
                if !self.is3d {
                    if let Some(a) = self.points.first().copied() {
                        let point = self.pointer;
                        let ghost = match self.tool {
                            Tool::Line | Tool::Dimension | Tool::Measure => {
                                Some(Shape::Line(*self.points.last().unwrap(), point))
                            }
                            Tool::Circle => Some(Shape::Circle {
                                center: a,
                                radius: a.dist(point),
                            }),
                            Tool::Rectangle | Tool::Box3D => Some(Shape::Polyline {
                                points: vec![
                                    a,
                                    Point::new(point.x, a.y),
                                    point,
                                    Point::new(a.x, point.y),
                                ],
                                closed: true,
                            }),
                            Tool::Polyline => {
                                let mut ps = self.points.clone();
                                ps.push(point);
                                Some(Shape::Polyline {
                                    points: ps,
                                    closed: false,
                                })
                            }
                            Tool::Arc if self.points.len() == 2 => {
                                let b = self.points[1];
                                Some(Shape::Arc {
                                    center: a,
                                    radius: a.dist(b),
                                    start: (b.y - a.y).atan2(b.x - a.x).to_degrees(),
                                    end: (point.y - a.y).atan2(point.x - a.x).to_degrees(),
                                })
                            }
                            _ => None,
                        };
                        if let Some(g) = ghost {
                            self.paint(&painter, r, &g, Color32::from_rgb(90, 231, 183), 1.);
                            painter.text(
                                self.screen(point, r) + Vec2::new(12., 15.),
                                Align2::LEFT_TOP,
                                format!("{:.3} {}", a.dist(point), self.d.units),
                                FontId::monospace(11.),
                                Color32::from_rgb(90, 231, 183),
                            );
                        }
                    }
                    if response.hovered() {
                        let c = self.screen(self.pointer, r);
                        let col = Color32::from_rgb(120, 162, 191);
                        painter.line_segment(
                            [c - Vec2::new(14., 0.), c + Vec2::new(14., 0.)],
                            Stroke::new(0.8, col),
                        );
                        painter.line_segment(
                            [c - Vec2::new(0., 14.), c + Vec2::new(0., 14.)],
                            Stroke::new(0.8, col),
                        );
                        if !self.snap_label.is_empty() {
                            painter.circle_stroke(
                                c,
                                5.,
                                Stroke::new(1.5, Color32::from_rgb(90, 231, 183)),
                            );
                        }
                    }
                }
                painter.text(
                    r.left_top() + Vec2::new(20., 17.),
                    Align2::LEFT_TOP,
                    if self.is3d {
                        "3D / WIREFRAME"
                    } else {
                        "TOP / 2D MODEL SPACE"
                    },
                    FontId::monospace(11.),
                    Color32::from_rgb(97, 133, 159),
                );
                painter.text(
                    r.right_bottom() - Vec2::new(18., 15.),
                    Align2::RIGHT_BOTTOM,
                    "Wheel: zoom · Middle mouse: pan · F: fit",
                    FontId::monospace(10.),
                    Color32::from_rgb(92, 122, 146),
                );
            });
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                self.points.clear();
                self.tool = Tool::Select
            }
            if i.key_pressed(egui::Key::Delete) {
                self.erase()
            }
            if i.key_pressed(egui::Key::Enter) {
                self.finish_poly(false)
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Z) {
                if i.modifiers.shift {
                    self.redo()
                } else {
                    self.undo()
                }
            }
            if i.modifiers.command && i.key_pressed(egui::Key::Y) {
                self.redo()
            }
            if i.modifiers.command && i.key_pressed(egui::Key::S) {
                self.save()
            }
        });
    }
}
impl eframe::App for Cad {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.shortcuts(ctx);
        self.top(ctx);
        self.bottom(ctx);
        self.side(ctx);
        self.canvas(ctx);
        if self.help {
            egui::Window::new("RustCAD / Commands and scope").open(&mut self.help).default_width(610.).show(ctx,|ui|{
   ui.heading("Command examples");
   for s in["LINE 0 0 120 0","CIRCLE 15 15 5","RECT 0 0 120 80","ARC 50 50 20 0 180","DIM 0 0 120 0 -15","BOX 0 0 0 60 40 20","SELECT 1 2","MOVE 10 5 | COPY 30 0","ROTATE 45 | SCALE 2 | MIRROR","OFFSET 5","TRIM target_id boundary_id keep_x keep_y","LAYER Holes","OPEN path/to/file.dxf","SAVE drawing.rcad | EXPORT svg","LINE → 0,0 → @100,0","PLINE → coordinates → CLOSE","HELP UNDO REDO ERASE 2D 3D"]{ui.monospace(s);}
   ui.separator();ui.label("Independent Rust CAD prototype. Native RCAD preserves supported shapes/layers. DXF imports a 2D subset; dimensions export as lines + text; boxes export as 3DFACE. DWG, NURBS, solid booleans, hatches, blocks, constraints, layouts and AutoCAD scripting are not implemented.");
  });
        }
    }
}
fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360., 900.])
            .with_min_inner_size([950., 650.]),
        ..Default::default()
    };
    eframe::run_native(
        "RustCAD — Open CAD workbench",
        opts,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Cad::default()))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_geometry_and_history() {
        let mut cad = Cad::default();
        cad.d = Drawing::default();
        cad.command("RECT 0 0 120 80").unwrap();
        for (x, y) in [(15, 15), (105, 15), (105, 65), (15, 65)] {
            cad.command(&format!("CIRCLE {x} {y} 5")).unwrap()
        }
        assert_eq!(cad.d.entities.len(), 5);
        cad.command("SELECT 2 3 4 5").unwrap();
        cad.command("MOVE 2 3").unwrap();
        if let Shape::Circle { center, .. } = cad.d.entities[1].shape {
            assert_eq!(center, Point::new(17., 18.))
        } else {
            panic!()
        }
        cad.undo();
        if let Shape::Circle { center, .. } = cad.d.entities[1].shape {
            assert_eq!(center, Point::new(15., 15.))
        } else {
            panic!()
        }
        assert!(cad.command("CIRCLE NaN 0 5").is_err());
    }
    #[test]
    fn box_command_and_stl() {
        let mut cad = Cad::default();
        cad.command("BOX 0 0 0 50 30 10").unwrap();
        assert_eq!(
            export_stl(&cad.d).unwrap().matches("facet normal").count(),
            12
        );
    }
    #[test]
    fn coordinate_input_and_close() {
        let mut cad = Cad::default();
        cad.d = Drawing::default();
        cad.command("PLINE").unwrap();
        cad.command("0,0").unwrap();
        cad.command("@20,0").unwrap();
        cad.command("@0,30").unwrap();
        cad.command("CLOSE").unwrap();
        assert!(
            matches!(&cad.d.entities[0].shape,Shape::Polyline{points,closed:true} if points.len()==3)
        );
    }
    #[test]
    fn headless_interface_and_3d() {
        let ctx = egui::Context::default();
        let mut cad = Cad::default();
        for view in [false, true] {
            cad.is3d = view;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1360., 900.))),
                    ..Default::default()
                },
                |ctx| {
                    cad.top(ctx);
                    cad.bottom(ctx);
                    cad.side(ctx);
                    cad.canvas(ctx);
                },
            );
            if !view {
                if let Ok(path) = std::env::var("RUSTCAD_REVIEW_PPM") {
                    render_review(&ctx, &output, &path);
                }
            }
            assert!(!output.shapes.is_empty());
            assert!(!ctx
                .tessellate(output.shapes, output.pixels_per_point)
                .is_empty());
        }
    }
}

#[cfg(test)]
fn render_review(ctx: &egui::Context, out: &egui::FullOutput, path: &str) {
    use std::io::Write;
    let w = 1360usize;
    let h = 900usize;
    let mut pixels = vec![[0f32; 3]; w * h];
    let mut textures: std::collections::HashMap<egui::TextureId, ([usize; 2], Vec<[u8; 4]>)> =
        std::collections::HashMap::new();
    for (id, delta) in &out.textures_delta.set {
        let size = delta.image.size();
        let rgba = match &delta.image {
            egui::ImageData::Color(i) => i.pixels.iter().map(|c| c.to_array()).collect::<Vec<_>>(),
            egui::ImageData::Font(i) => i.srgba_pixels(Some(1.)).map(|c| c.to_array()).collect(),
        };
        if let Some(pos) = delta.pos {
            if let Some((full, pixels)) = textures.get_mut(id) {
                for y in 0..size[1] {
                    for x in 0..size[0] {
                        pixels[(pos[1] + y) * full[0] + pos[0] + x] = rgba[y * size[0] + x];
                    }
                }
            }
        } else {
            textures.insert(*id, (size, rgba));
        }
    }
    for clip in ctx.tessellate(out.shapes.clone(), out.pixels_per_point) {
        let egui::epaint::Primitive::Mesh(mesh) = clip.primitive else {
            continue;
        };
        let Some((size, texture)) = textures.get(&mesh.texture_id) else {
            continue;
        };
        for idx in mesh.indices.chunks_exact(3) {
            let a = mesh.vertices[idx[0] as usize];
            let b = mesh.vertices[idx[1] as usize];
            let c = mesh.vertices[idx[2] as usize];
            let edge =
                |a: Pos2, b: Pos2, p: Pos2| (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x);
            let area = edge(a.pos, b.pos, c.pos);
            if area.abs() < 0.00001 {
                continue;
            }
            let minx = a
                .pos
                .x
                .min(b.pos.x)
                .min(c.pos.x)
                .floor()
                .max(clip.clip_rect.min.x)
                .max(0.) as usize;
            let maxx = a
                .pos
                .x
                .max(b.pos.x)
                .max(c.pos.x)
                .ceil()
                .min(clip.clip_rect.max.x)
                .min(w as f32) as usize;
            let miny = a
                .pos
                .y
                .min(b.pos.y)
                .min(c.pos.y)
                .floor()
                .max(clip.clip_rect.min.y)
                .max(0.) as usize;
            let maxy = a
                .pos
                .y
                .max(b.pos.y)
                .max(c.pos.y)
                .ceil()
                .min(clip.clip_rect.max.y)
                .min(h as f32) as usize;
            let ca = a.color.to_array();
            let cb = b.color.to_array();
            let cc = c.color.to_array();
            for y in miny..maxy {
                for x in minx..maxx {
                    let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let wa = edge(b.pos, c.pos, p) / area;
                    let wb = edge(c.pos, a.pos, p) / area;
                    let wc = 1. - wa - wb;
                    if wa < -0.00001 || wb < -0.00001 || wc < -0.00001 {
                        continue;
                    }
                    let u = (a.uv.x * wa + b.uv.x * wb + c.uv.x * wc).clamp(0., 1.);
                    let v = (a.uv.y * wa + b.uv.y * wb + c.uv.y * wc).clamp(0., 1.);
                    let tx = ((u * size[0] as f32) as usize).min(size[0] - 1);
                    let ty = ((v * size[1] as f32) as usize).min(size[1] - 1);
                    let t = texture[ty * size[0] + tx];
                    let alpha = (ca[3] as f32 * wa + cb[3] as f32 * wb + cc[3] as f32 * wc) / 255.
                        * t[3] as f32
                        / 255.;
                    for k in 0..3 {
                        let src = (ca[k] as f32 * wa + cb[k] as f32 * wb + cc[k] as f32 * wc)
                            / 255.
                            * t[k] as f32
                            / 255.;
                        pixels[y * w + x][k] = src + pixels[y * w + x][k] * (1. - alpha);
                    }
                }
            }
        }
    }
    let mut file = std::fs::File::create(path).unwrap();
    write!(file, "P6\n{w} {h}\n255\n").unwrap();
    let bytes = pixels
        .into_iter()
        .flat_map(|p| p.map(|v| (v.clamp(0., 1.) * 255.).round() as u8))
        .collect::<Vec<_>>();
    file.write_all(&bytes).unwrap();
}
