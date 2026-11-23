//! The reference's system and query commands: scene up axis, units, display colours and preferences kept for scripts, evaluation
//! settings, node locks, and asking about attributes and nodes.
//!
//! Stubs, so scripts run: dgtimer returns zeros (nothing is timed). hitTest returns nothing: only the UI knows a panel's
//! camera and size. evaluationManager and threadCount hold what they are told; evaluation doesn't change.

use std::collections::{BTreeMap, HashSet};

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DVec3, EulerRot};
use serde_json::{json, Value};

use crate::ref_core::{asked, f};
use crate::plug::{flags, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as F, Int as I, List as L, String as S};

/// Scene and session settings the commands in this module and its neighbours hold.
#[derive(Default)]
pub struct Extras {
    pub(crate) opened_history: Option<crate::ma_file::OpenedHistory>,
    pub cycle_check: Option<bool>,
    pub evaluation_mode: Option<String>,
    pub thread_count: Option<i64>,
    pub display_pref: BTreeMap<String, Value>,
    pub display_stats: BTreeMap<String, Value>,
    pub display_surface: BTreeMap<(NodeId, String), bool>,
    pub level_of_detail: bool,
    pub select_priority: BTreeMap<String, i64>,
    pub dg_timer_on: bool,
    /// animDisplay's settings.
    pub anim_display: BTreeMap<String, Value>,
    /// filterStudioImport's options.
    pub studio_import: BTreeMap<String, Value>,
    /// recordAttr: plugs marked for recording.
    pub recording: Vec<String>,
    /// symmetricModelling's settings.
    pub symmetry: crate::mesh_edit::ref_poly::SymmetryState,
    /// The scene's file references.
    pub references: crate::references::RefTable,
    /// The state of general_cmds and its neighbours.
    pub more: crate::general_cmds::MoreState,
    /// The renderer's running render, interactive state and kept images.
    pub harold: crate::harold::commands::State,
    /// fileInfo's keyword and value pairs, in the order they were set.
    pub file_info: Vec<(String, String)>,
    /// Plug-ins the scene's requires lines named that could not be loaded (unknownPlugin).
    pub unknown_plugins: BTreeMap<String, crate::sys_extra::UnknownPlugin>,
    /// What optionVar keeps beside the values: defaults, categories, stashes, transient names.
    pub option_var_meta: crate::ref_scene::OptionVarMeta,
    /// The project, as `workspace` keeps it.
    pub workspace: crate::ref_scene::Workspace,
    /// evaluationManager's finer settings and node type overrides.
    pub eval_manager: crate::freeze::EvalManager,
    /// setAttr -caching: plugs set not to be cached (or back on), by node and attribute.
    pub plug_caching: BTreeMap<(NodeId, i32), bool>,
    /// addExtension's attributes: what they carry beyond their definitions.
    pub extensions: crate::extensions::ExtensionMeta,
    /// What sys_misc's commands hold: backgroundEvaluationManager's settings.
    pub sys: crate::sys_misc::SysState,
    /// What db_cmds' commands hold: curves' static tags, displayString's strings.
    pub db: crate::db_cmds::DbState,
    /// The custom evaluators and the deformer evaluator's settings.
    pub eval: crate::eval_cmds::EvalState,
}

/// What a query asks for: the first flag -q names.
/// The largest texture the viewport's graphics card takes, set by the viewport when it starts.
pub static MAX_HW_TEXTURE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(16384);

pub(crate) fn q_one(a: &Args) -> Option<String> {
    a.get("queried").and_then(Value::as_array).and_then(|v| v.first()).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn q_flags(c: Cmd) -> Cmd {
    c.flag(f("query", "q", B, "Query mode.")).flag(f("edit", "e", B, "Edit mode.")).flag(f("queried", "", L, "Flags asked about in query mode."))
}

fn on_off(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)
}

// ---------------------------------------------------------------------------------------------------------------------
// upAxis

fn up_axis_cmd(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        return Ok(json!(crate::world::up_name()));
    }
    let Some(axis) = a.get("axis").and_then(Value::as_str) else { fail!("upAxis: the -axis flag is mandatory") };
    let z = match axis {
        "y" | "Y" => false,
        "z" | "Z" => true,
        _ => fail!("upAxis: the axis must be y or z"),
    };
    if z != crate::world::up_z() {
        crate::world::set_up_z(z);
        // -rv turns the cameras about X with the world, so the ground stays where it was on screen.
        if a.get("rotateView").is_some_and(on_off) {
            rotate_cameras(s, z);
        }
    }
    Ok(Value::Null)
}

/// The views' cameras turned a quarter about X with the world, as `upAxis -rv` turns them: the ground stays where it
/// was on screen. Other cameras are scene content and stay.
pub fn rotate_cameras(s: &mut Session, z: bool) {
    let g = &mut s.graph;
    let turn = DMat3::from_rotation_x(if z { 90f64.to_radians() } else { (-90f64).to_radians() });
    g.without_undo(|g| {
        for name in ["persp", "top", "front", "side"] {
            let Ok(Some(id)) = g.find(name) else { continue };
            let n = g.node(id).unwrap();
            if !n.ty().is_a("transform") {
                continue;
            }
            let (t, r) = (n.vec3(Transform::TRANSLATE), n.vec3(Transform::ROTATE));
            // The Euler angles in xyz order: R = Rz Ry Rx.
            let m = DMat3::from_euler(EulerRot::ZYX, r.z, r.y, r.x);
            let (rz, ry, rx) = (turn * m).to_euler(EulerRot::ZYX);
            let _ = g.set_value(PlugRef::new(id, Transform::TRANSLATE as i32), PlugValue::Vec3(turn * t));
            let _ = g.set_value(PlugRef::new(id, Transform::ROTATE as i32), PlugValue::Vec3(DVec3::new(rx, ry, rz)));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// convertUnit

/// A unit's size in its family's base unit: (family, factor).
fn unit_of(u: &str) -> Option<(u8, f64)> {
    Some(match u {
        "mm" | "millimeter" => (0, 0.1),
        "cm" | "centimeter" => (0, 1.0),
        "m" | "meter" => (0, 100.0),
        "km" | "kilometer" => (0, 100_000.0),
        "in" | "inch" => (0, 2.54),
        "ft" | "foot" => (0, 30.48),
        "yd" | "yard" => (0, 91.44),
        "mi" | "mile" => (0, 160_934.4),
        "deg" | "degree" => (1, 1.0),
        "rad" | "radian" => (1, 180.0 / std::f64::consts::PI),
        "min" | "arcminute" => (1, 1.0 / 60.0),
        "sec" | "arcsecond" => (1, 1.0 / 3600.0),
        _ => return None,
    })
}

fn fmt_number(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

fn convert_unit(_s: &mut Session, a: &Args) -> CmdResult {
    let text = match a.positional.first() {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => fail!("convertUnit: give a value"),
    };
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(text.len());
    let (num, suffix) = text.split_at(split);
    let Ok(v) = num.parse::<f64>() else { fail!("convertUnit: {text} is not a value") };
    let from = a.get("fromUnit").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string).or((!suffix.is_empty()).then(|| suffix.to_string())).unwrap_or("cm".to_string());
    let Some((fa, fb)) = unit_of(&from) else { fail!("convertUnit: unknown unit {from}") };
    let to = a.get("toUnit").and_then(Value::as_str).map(str::to_string).unwrap_or(if fa == 0 { "cm".to_string() } else { "deg".to_string() });
    let Some((ta, tb)) = unit_of(&to) else { fail!("convertUnit: unknown unit {to}") };
    if fa != ta {
        fail!("convertUnit: {from} and {to} don't measure the same thing");
    }
    Ok(json!(format!("{}{to}", fmt_number(v * fb / tb))))
}

// ---------------------------------------------------------------------------------------------------------------------
// copyAttr

fn copy_attr(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let [src, dst] = names.as_slice() else { fail!("copyAttr: name or select a source and a target") };
    let (Some(from), Some(to)) = (g.find(src)?, g.find(dst)?) else { fail!("copyAttr: no such object") };
    let only: Option<HashSet<String>> = a.get("attribute").map(|v| match v {
        Value::Array(l) => l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Value::String(t) => [t.clone()].into_iter().collect(),
        _ => HashSet::new(),
    });
    let wants = |at: &crate::plug::AttrDef| only.as_ref().is_none_or(|o| o.contains(&at.name) || o.contains(&at.short_name));
    let (values, ic, oc, keep) = (a.bool("values"), a.bool("inConnections"), a.bool("outConnections"), a.bool("keepSourceConnections"));
    let t_src = g.node(from).unwrap().type_arc().clone();
    let t_dst = g.node(to).unwrap().type_arc().clone();
    let pairs: Vec<(usize, usize)> = t_src.attrs.iter().enumerate().filter(|(_, x)| wants(x)).filter_map(|(i, x)| t_dst.attr_index(&x.name).map(|j| (i, j))).collect();
    g.open_chunk("copyAttr");
    let r = (|| -> CmdResult<()> {
        for &(i, j) in &pairs {
            let def = &t_src.attrs[i];
            if t_dst.attrs[j].ty != def.ty {
                continue;
            }
            let (ps, pd) = (PlugRef::new(from, i as i32), PlugRef::new(to, j as i32));
            if values && def.has(flags::INPUT) && !def.is_array() && g.source(&pd).is_none() && !g.is_locked(&pd) && !matches!(def.ty, PlugType::Mesh | PlugType::Data) {
                let v = g.value(&ps)?;
                g.set_value(pd, v)?;
            }
            // Components and elements keep their place on the new plug.
            if ic || oc {
                for (d, up) in g.connections() {
                    if ic && d.node == from && d.attr == i as i32 {
                        let mut to_plug = d;
                        to_plug.node = to;
                        to_plug.attr = j as i32;
                        g.connect(&up, &to_plug, true)?;
                        if !keep {
                            g.disconnect(&d);
                        }
                    }
                    if oc && !keep && up.node == from && up.attr == i as i32 && d.node != to {
                        let mut from_plug = up;
                        from_plug.node = to;
                        from_plug.attr = j as i32;
                        g.connect(&from_plug, &d, true)?;
                    }
                }
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    g.evaluate();
    Ok(json!(true))
}

// ---------------------------------------------------------------------------------------------------------------------
// cycleCheck

/// A plug with its element and component, as connections name them.
type PlugKey = (NodeId, i32, i32, i32);

fn key(p: &PlugRef) -> PlugKey {
    (p.node, p.attr, p.element, p.component)
}

fn cycle_walk(at: PlugKey, next: &BTreeMap<PlugKey, Vec<PlugKey>>, state: &mut BTreeMap<PlugKey, u8>, stack: &mut Vec<PlugKey>) -> Option<Vec<PlugKey>> {
    match state.get(&at) {
        Some(1) => {
            let pos = stack.iter().position(|x| *x == at).unwrap_or(0);
            return Some(stack[pos..].to_vec());
        }
        Some(_) => return None,
        None => {}
    }
    state.insert(at, 1);
    stack.push(at);
    for &n in next.get(&at).map(Vec::as_slice).unwrap_or(&[]) {
        if let Some(c) = cycle_walk(n, next, state, stack) {
            return Some(c);
        }
    }
    stack.pop();
    state.insert(at, 2);
    None
}

/// Where a cycle search starts: every plug, those of some nodes, or some plugs.
enum Starts {
    All,
    Nodes(Vec<NodeId>),
    Plugs(Vec<PlugKey>),
}

/// Cycles of plugs reachable from `from`: a connection feeds its destination plug, and an input feeds whatever of its
/// node's computed outputs drive something. A stored plug feeds only itself, so tx through a node into ty is no cycle,
/// as in the reference. With `dag`, a parent's world matrix feeds each child's parent matrix too, and what a node takes
/// in feeds its world matrix. Each cycle once; the search stops at the first with `first_only`, or at `deadline`.
fn find_cycles(g: &crate::graph::Graph, from: &Starts, dag: bool, first_only: bool, deadline: Option<std::time::Instant>) -> Vec<Vec<PlugRef>> {
    let conns = g.connections();
    let mut next: BTreeMap<PlugKey, Vec<PlugKey>> = BTreeMap::new();
    let mut refs: BTreeMap<PlugKey, PlugRef> = BTreeMap::new();
    for (dst, src) in &conns {
        refs.insert(key(src), *src);
        refs.insert(key(dst), *dst);
        next.entry(key(src)).or_default().push(key(dst));
        for (_, out) in conns.iter().filter(|(_, o)| o.node == dst.node) {
            if g.attr(out).has(flags::OUTPUT) {
                next.entry(key(dst)).or_default().push(key(out));
            }
        }
    }
    if dag {
        use crate::transform::DagNode;
        for id in g.nodes() {
            let p = g.parent(id);
            if p == crate::plug::NO_NODE || !g.node(id).is_some_and(|n| n.ty().dag) {
                continue;
            }
            let (pw, cp) = (PlugRef::new(p, DagNode::WORLD_MATRIX as i32), PlugRef::new(id, DagNode::PARENT_MATRIX as i32));
            refs.insert(key(&pw), pw);
            refs.insert(key(&cp), cp);
            next.entry(key(&pw)).or_default().push(key(&cp));
            // The child's parent matrix moves whatever the child gives out; the parent's inputs move its world matrix.
            let outs: Vec<PlugKey> = conns.iter().filter(|(_, o)| o.node == id).map(|(_, o)| key(o)).collect();
            next.entry(key(&cp)).or_default().extend(outs);
            for (dst, _) in conns.iter().filter(|(d, _)| d.node == p) {
                next.entry(key(dst)).or_default().push(key(&pw));
            }
        }
    }
    let starts: Vec<PlugKey> = match from {
        Starts::All => next.keys().copied().collect(),
        Starts::Nodes(ids) => next.keys().filter(|k| ids.contains(&k.0)).copied().collect(),
        Starts::Plugs(ks) => next.keys().filter(|k| ks.iter().any(|w| w.0 == k.0 && w.1 == k.1 && (w.2 < 0 || w.2 == k.2) && (w.3 < 0 || w.3 == k.3))).copied().collect(),
    };
    let mut found: Vec<Vec<PlugKey>> = Vec::new();
    for st in starts {
        if deadline.is_some_and(|d| std::time::Instant::now() > d) {
            break;
        }
        // A fresh walk from each start finds the cycles through it.
        let (mut state, mut stack) = (BTreeMap::new(), Vec::new());
        if let Some(c) = cycle_walk(st, &next, &mut state, &mut stack) {
            let mut sorted = c.clone();
            sorted.sort();
            if !found.iter().any(|f| {
                let mut x = f.clone();
                x.sort();
                x == sorted
            }) {
                found.push(c);
                if first_only {
                    break;
                }
            }
        }
    }
    found.into_iter().map(|c| c.into_iter().filter_map(|k| refs.get(&k).copied()).collect()).collect()
}

/// A time limit: seconds, or a number with a unit (10sec, 2min, 500ms, 24 frames counted as seconds at 24 fps).
fn time_limit(v: &Value) -> Option<std::time::Duration> {
    let secs = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(t) => {
            let t = t.trim();
            let at = t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len());
            let (n, unit) = t.split_at(at);
            let n: f64 = n.trim().parse().ok()?;
            match unit {
                "" | "s" | "sec" => n,
                "ms" | "millisec" => n / 1000.0,
                "min" => n * 60.0,
                "hour" => n * 3600.0,
                _ => n / 24.0,
            }
        }
        _ => return None,
    };
    Some(std::time::Duration::from_secs_f64(secs.max(0.0)))
}

fn cycle_check(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        return Ok(json!(s.extras.cycle_check.unwrap_or(false)));
    }
    if let Some(v) = a.get("evaluation") {
        s.extras.cycle_check = Some(on_off(v) || v.as_str() == Some("on"));
        return Ok(Value::Null);
    }
    // The reference's -e here is evaluation: cycleCheck -e on, or -e alone for on.
    if a.bool("edit") {
        let off = matches!(a.positional.first(), Some(v) if v.as_str() == Some("off") || v == &json!(false) || v == &json!(0));
        s.extras.cycle_check = Some(!off);
        return Ok(Value::Null);
    }
    let names = objects(s, a);
    let g = &s.graph;
    let all = a.bool("all") || names.is_empty();
    // -secondary (on for -all) looks at the plugs related to those named too: their nodes' other plugs.
    let secondary = a.get("secondary").map_or(all, |v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    let starts = if all {
        Starts::All
    } else if names.iter().any(|n| n.contains('.')) && !secondary {
        Starts::Plugs(names.iter().filter_map(|n| g.plug(n).ok()).map(|p| key(&p)).collect())
    } else {
        Starts::Nodes(names.iter().filter_map(|n| g.find(n.split('.').next().unwrap_or(n)).ok().flatten()).collect())
    };
    let deadline = a.get("timeLimit").and_then(time_limit).map(|d| std::time::Instant::now() + d);
    let list = a.bool("list");
    let cycles = find_cycles(g, &starts, a.bool("dag"), !list || a.bool("firstCycleOnly"), deadline);
    if !list {
        return Ok(json!(!cycles.is_empty()));
    }
    let sep = a.get("listSeparator").and_then(Value::as_str);
    let (first, last) = (a.bool("firstPlugPerNode"), a.bool("lastPlugPerNode"));
    let mut out = Vec::new();
    for (k, c) in cycles.iter().enumerate() {
        if k > 0 {
            if let Some(sep) = sep {
                out.push(json!(sep));
            }
        }
        let mut plugs: Vec<PlugRef> = c.clone();
        if first || last {
            // One plug a node: the first or the last it has on the cycle.
            let mut kept: Vec<PlugRef> = Vec::new();
            for p in &plugs {
                match kept.iter().position(|q| q.node == p.node) {
                    Some(i) if last => kept[i] = *p,
                    Some(_) => {}
                    None => kept.push(*p),
                }
            }
            plugs = kept;
        }
        out.extend(plugs.iter().map(|p| json!(g.plug_name(p))));
    }
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// Display colours

use crate::draw_style::{self, DISPLAY_COLORS};

fn display_color(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("list") {
        return Ok(Value::Array(DISPLAY_COLORS.iter().map(|c| json!(c.0)).collect()));
    }
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_display_indices();
        return Ok(Value::Null);
    }
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("displayColor: name a colour") };
    let active = a.bool("active");
    let known = DISPLAY_COLORS.iter().find(|c| c.0 == name);
    if a.bool("query") {
        let Some(i) = draw_style::display_index(&name, active) else { fail!("displayColor: no colour named {name}") };
        return Ok(json!(i));
    }
    let Some(i) = a.positional.get(1).and_then(Value::as_i64) else { fail!("displayColor: give an index, 1 to 31") };
    if !(0..=31).contains(&i) {
        fail!("displayColor: the index must be 0 to 31");
    }
    if known.is_none() && !a.bool("create") {
        fail!("displayColor: no colour named {name}; use -create");
    }
    draw_style::set_display_index(&name, active, i);
    Ok(Value::Null)
}

/// A named view colour: what displayRGBColor was given, else the reference's default.
pub fn view_rgb(name: &str) -> Option<[f64; 4]> {
    draw_style::display_rgb(name).or_else(|| NAMED_RGB.iter().find(|c| c.0 == name).map(|c| c.1))
}

/// Whether views show the gradient from backgroundTop to backgroundBottom (displayPref -displayGradient).
pub fn display_gradient(s: &Session) -> bool {
    match s.extras.display_pref.get("displayGradient") {
        Some(Value::Bool(b)) => *b,
        Some(v) => v.as_f64().is_some_and(|n| n != 0.0),
        None => false,
    }
}

const NAMED_RGB: &[(&str, [f64; 4])] = &[
    ("background", [0.63, 0.63, 0.63, 1.0]),
    ("backgroundTop", [0.54, 0.62, 0.70, 1.0]),
    ("backgroundBottom", [0.05, 0.05, 0.05, 1.0]),
    ("gridColor", [0.33, 0.33, 0.33, 1.0]),
    ("gridAxis", [0.0, 0.0, 0.0, 1.0]),
    ("gridHighlight", [0.0, 0.0, 0.0, 1.0]),
    ("lead", [0.4, 1.0, 0.647, 1.0]),
];

fn display_rgb(_s: &mut Session, a: &Args) -> CmdResult {
    const NAMED: &[(&str, [f64; 4])] = NAMED_RGB;
    if a.bool("list") {
        return Ok(Value::Array(NAMED.iter().map(|c| json!(format!("{} {} {} {}", c.0, c.1[0], c.1[1], c.1[2]))).collect()));
    }
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_display_rgb();
        return Ok(Value::Null);
    }
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("displayRGBColor: name a colour") };
    let known = NAMED.iter().find(|c| c.0 == name).map(|c| c.1);
    if a.bool("query") {
        let Some(v) = draw_style::display_rgb(&name).or(known) else { fail!("displayRGBColor: no colour named {name}") };
        return Ok(if a.bool("alpha") { json!([v[0], v[1], v[2], v[3]]) } else { json!([v[0], v[1], v[2]]) });
    }
    let n: Vec<f64> = a.positional.iter().skip(1).filter_map(Value::as_f64).collect();
    if n.len() < 3 {
        fail!("displayRGBColor: give r g b");
    }
    if known.is_none() && !a.bool("create") {
        fail!("displayRGBColor: no colour named {name}; use -create");
    }
    draw_style::set_display_rgb(&name, [n[0], n[1], n[2], n.get(3).copied().unwrap_or(1.0)]);
    Ok(Value::Null)
}

/// RGB 0 to 1 from hue in degrees, saturation and value 0 to 1.
pub(crate) fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i64 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

/// Hue in degrees, saturation and value from RGB 0 to 1.
pub(crate) fn rgb_to_hsv(c: [f64; 3]) -> [f64; 3] {
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = max - min;
    let h = if d <= 0.0 {
        0.0
    } else if max == c[0] {
        60.0 * ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if max == c[1] {
        60.0 * ((c[2] - c[0]) / d + 2.0)
    } else {
        60.0 * ((c[0] - c[1]) / d + 4.0)
    };
    [h, if max <= 0.0 { 0.0 } else { d / max }, max]
}

/// colorIndex: the dormant palette (the default), -active the colours selected objects draw in, -userColor the eight
/// user colours (1 to 8). -hsv gives and answers hue (degrees), saturation and value.
fn color_index(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_palette();
        return Ok(json!(1));
    }
    let Some(i) = a.positional.first().and_then(Value::as_i64) else { fail!("colorIndex: give an index") };
    let user = a.bool("userColor");
    let active = a.bool("active");
    let hsv = a.bool("hueSaturationValue");
    // The user colours are the palette's last eight entries.
    let entry = if user {
        if !(1..=8).contains(&i) {
            fail!("colorIndex: a user colour is 1 to 8");
        }
        draw_style::USER_COLOR_BASE + i - 1
    } else {
        i
    };
    // Entry 0 means no colour: it can be asked about, not set.
    if a.bool("query") && (0..=31).contains(&entry) {
        let c = if active { draw_style::active_palette(entry) } else { draw_style::palette(entry) };
        return Ok(json!(if hsv { rgb_to_hsv(c) } else { c }));
    }
    if !(1..=31).contains(&entry) {
        fail!("colorIndex: the index must be 1 to 31");
    }
    let n: Vec<f64> = a.positional.iter().skip(1).filter_map(Value::as_f64).collect();
    if n.len() < 3 {
        fail!("colorIndex: give r g b");
    }
    let c = if hsv { hsv_to_rgb(n[0], n[1], n[2]) } else { [n[0], n[1], n[2]] };
    if active {
        draw_style::set_active_palette(entry, c);
    } else {
        draw_style::set_palette(entry, c);
    }
    Ok(json!(1))
}

/// A default for a settings command.
#[derive(Clone, Copy)]
pub(crate) enum Def {
    B(bool),
    I(i64),
    F(f64),
    S(&'static str),
}

impl Def {
    fn json(self) -> Value {
        match self {
            Def::B(v) => json!(v),
            Def::I(v) => json!(v),
            Def::F(v) => json!(v),
            Def::S(v) => json!(v),
        }
    }
}

type Setting = (&'static str, &'static str, ArgType, Def);

/// A settings command: each flag of the list holds a value; -q reads it back, falling on the default.
pub(crate) fn setting_store(c: &mut Commands<Session>, name: &'static str, doc: &'static str, list: &'static [Setting], field: fn(&mut Session) -> &mut BTreeMap<String, Value>) {
    let mut cmd = q_flags(Cmd::new(name, doc, move |s: &mut Session, a: &Args| {
        let map = field(s);
        if a.bool("query") {
            let Some(q) = q_one(a).or_else(|| list.iter().find(|x| a.has(x.0)).map(|x| x.0.to_string())) else { fail!("{name}: query what?") };
            let Some(item) = list.iter().find(|x| x.0 == q) else { fail!("{name}: can't query {q}") };
            return Ok(map.get(&q).cloned().unwrap_or(item.3.json()));
        }
        for item in list {
            if let Some(v) = a.get(item.0) {
                map.insert(item.0.to_string(), v.clone());
            }
        }
        Ok(Value::Null)
    }))
    .undoable(false);
    for &(long, short, ty, _) in list {
        cmd = cmd.flag(f(long, short, ty, "A setting."));
    }
    c.register(cmd);
}

const DISPLAY_PREF: &[Setting] = &[
    ("activeObjectPivots", "aop", B, Def::B(false)),
    ("defaultFontSize", "dfs", I, Def::I(9)),
    ("displayAffected", "da", B, Def::B(true)),
    ("displayGradient", "dgr", B, Def::B(false)),
    ("fontSettingMode", "fm", I, Def::I(0)),
    ("lineWidth", "lw", F, Def::F(1.0)),
    ("materialLoadingMode", "mld", S, Def::S("immediate")),
    ("maxTextureResolution", "mtr", I, Def::I(4096)),
    ("purgeExistingTextures", "pet", B, Def::B(false)),
    ("regionOfEffect", "roe", B, Def::B(false)),
    ("shadeTemplates", "st", B, Def::B(false)),
    ("smallFontSize", "sfs", I, Def::I(9)),
    ("textureDrawPixel", "tdp", B, Def::B(false)),
    ("wireframeOnShadedActive", "wsa", S, Def::S("full")),
    ("wireframeOnBackgroundFaces", "wbf", B, Def::B(false)),
    ("workspaceMode", "wsm", S, Def::S("")),
    ("xrayActiveComponents", "xac", B, Def::B(false)),
];

const DISPLAY_STATS: &[Setting] = &[
    ("frameRate", "fr", B, Def::B(false)),
    ("headsUpDisplay", "hud", B, Def::B(false)),
    ("polyCount", "pc", B, Def::B(false)),
    ("cameraName", "cn", B, Def::B(true)),
    ("viewAxis", "va", B, Def::B(true)),
    ("selectionDetails", "sd", B, Def::B(false)),
];

/// (flag, short, the reference's default priority.)
const SELECT_PRIORITY: &[(&str, &str, i64)] = &[
    ("allComponents", "alc", 1), ("allObjects", "alo", 1), ("animBreakdown", "abd", 3), ("animCurve", "ac", 3), ("animInTangent", "ait", 3), ("animKeyframe", "ak", 3),
    ("animOutTangent", "aot", 3), ("byName", "bn", 2), ("camera", "ca", 1), ("cluster", "cl", 1), ("collisionModel", "clm", 1), ("controlVertex", "cv", 5),
    ("curve", "c", 1), ("curveKnot", "ck", 5), ("curveOnSurface", "cos", 1), ("curveParameterPoint", "cpp", 5), ("dimension", "dim", 1), ("dynamicConstraint", "dc", 1),
    ("edge", "eg", 5), ("editPoint", "ep", 5), ("emitter", "em", 1), ("facet", "fc", 5), ("field", "fi", 1), ("fluid", "fl", 1), ("follicle", "fo", 1), ("hairSystem", "hs", 1), ("handle", "ha", 2), ("hull", "hl", 5),
    ("ikEndEffector", "iee", 2), ("ikHandle", "ikh", 2), ("imagePlane", "ip", 1), ("implicitGeometry", "ig", 1), ("isoparm", "iso", 5), ("joint", "j", 3),
    ("jointPivot", "jp", 3), ("lattice", "la", 1), ("latticePoint", "lp", 5), ("light", "lt", 1), ("localRotationAxis", "ra", 3), ("locator", "lc", 1),
    ("locatorUV", "luv", 1), ("locatorXYZ", "xyz", 1), ("meshUVShell", "msh", 5), ("motionTrailPoint", "mtp", 5), ("motionTrailTangent", "mtt", 5), ("nCloth", "ncl", 1), ("nParticle", "npr", 1), ("nParticleShape", "nps", 1), ("nRigid", "nr", 1), ("nonlinear", "nl", 1), ("nurbsCurve", "nc", 1), ("nurbsSurface", "ns", 1),
    ("orientationLocator", "ol", 1), ("particle", "pr", 1), ("particleShape", "ps", 1), ("plane", "pl", 1), ("polymesh", "p", 1), ("polymeshEdge", "pe", 5), ("polymeshFace", "pf", 5),
    ("polymeshFreeEdge", "pfe", 5), ("polymeshUV", "puv", 5), ("polymeshVertex", "pv", 5), ("polymeshVtxFace", "pvf", 5), ("queueTangent", "qt", 3), ("rigidBody", "rb", 1),
    ("rigidConstraint", "rc", 1), ("rotatePivot", "rp", 3), ("scalePivot", "sp", 3), ("sculpt", "sc", 1), ("selectHandle", "sh", 2), ("spring", "spr", 1), ("springComponent", "spc", 5),
    ("stroke", "str", 1), ("subdiv", "sd", 1), ("subdivMeshEdge", "sme", 5), ("subdivMeshFace", "smf", 5), ("subdivMeshPoint", "smp", 5), ("subdivMeshUV", "smu", 5), ("surfaceEdge", "se", 5), ("surfaceFace", "sf", 5), ("surfaceKnot", "sk", 5), ("surfaceParameterPoint", "spp", 5),
    ("surfaceRange", "sr", 5), ("surfaceUV", "suv", 5), ("texture", "tx", 1), ("vertex", "v", 5),
];

fn select_priority(s: &mut Session, a: &Args) -> CmdResult {
    // A user-defined selection type, by name: -byName name on|off, -q -queryByName name.
    let by_name = |n: &str| format!("byName:{n}");
    if a.bool("query") {
        if let Some(n) = a.get("queryByName").and_then(Value::as_str) {
            return Ok(json!(s.extras.select_priority.get(&by_name(n)).copied().unwrap_or(0)));
        }
        let Some(q) = q_one(a) else { fail!("selectPriority: query what?") };
        let Some(e) = SELECT_PRIORITY.iter().find(|x| x.0 == q) else { fail!("selectPriority: no {q}") };
        return Ok(json!(s.extras.select_priority.get(&q).copied().unwrap_or(e.2)));
    }
    if let Some(Value::Array(v)) = a.get("byName") {
        for pair in v.chunks(2) {
            let (Some(n), Some(on)) = (pair.first().and_then(Value::as_str), pair.get(1)) else { continue };
            let on = on.as_bool().or_else(|| on.as_i64().map(|i| i != 0)).unwrap_or(false);
            s.extras.select_priority.insert(by_name(n), on as i64);
        }
    }
    for e in SELECT_PRIORITY {
        if e.0 == "byName" {
            continue;
        }
        if a.has(e.0) {
            s.extras.select_priority.insert(e.0.to_string(), a.int(e.0, e.2));
        }
    }
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// Asking about nodes and attributes

fn attribute_name(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let Some(text) = a.pos_str(0) else { fail!("attributeName: name a plug") };
    let p = g.plug(text)?;
    let def = g.attr(&p);
    let comp = (p.component >= 0 && p.component < 3).then_some(p.component as usize);
    let long = comp.map_or(def.name.clone(), |c| def.child_names[c].clone());
    let short = comp.map_or(def.short_name.clone(), |c| def.child_short[c].clone());
    if a.bool("long") {
        return Ok(json!(long));
    }
    if a.bool("short") {
        return Ok(json!(if short.is_empty() { long } else { short }));
    }
    // A compound's child reads "Translate X" whole; -leaf leaves the parent's part out.
    if let (Some(c), true) = (comp, a.bool("leaf")) {
        let child = &def.child_names[c];
        return Ok(json!(crate::ref_attrs::nice(child.strip_prefix(def.name.as_str()).unwrap_or(child))));
    }
    Ok(json!(if !def.nice_name.is_empty() && comp.is_none() { def.nice_name.clone() } else { crate::ref_attrs::nice(&long) }))
}

fn attribute_info(s: &mut Session, a: &Args) -> CmdResult {
    let t = if let Some(ty) = a.get("type").and_then(Value::as_str) {
        let Some(t) = crate::node::types::find(ty) else { fail!("attributeInfo: no node type {ty}") };
        t
    } else {
        let names = objects(s, a);
        let g = &s.graph;
        let Some(n) = names.first() else { fail!("attributeInfo: name an object or -type") };
        let Some(id) = g.find(n)? else { fail!("attributeInfo: no object {n}") };
        g.node(id).unwrap().type_arc().clone()
    };
    let and = a.bool("logicalAnd");
    let want = |at: &crate::plug::AttrDef| -> bool {
        let list: [(&str, bool); 7] = [
            ("bool", at.ty == PlugType::Bool),
            ("enumerated", at.ty == PlugType::Enum),
            ("hidden", at.has(flags::HIDDEN)),
            ("inherited", t.base.as_ref().is_some_and(|b| b.attr_index(&at.name).is_some())),
            // A child of a compound with no children of its own.
            ("leaf", at.name.contains('.') && at.ty != PlugType::Vec3),
            ("multi", at.is_array()),
            ("writable", at.has(flags::INPUT)),
        ];
        let on: Vec<bool> = list.iter().filter(|(flag, _)| a.has(flag)).map(|(flag, has)| a.bool(flag) == *has).collect();
        if on.is_empty() {
            return true;
        }
        if and {
            on.iter().all(|x| *x)
        } else {
            on.iter().any(|x| *x)
        }
    };
    let short = a.bool("short");
    let out: Vec<Value> = t.attrs.iter().filter(|x| want(x)).map(|x| json!(if short && !x.short_name.is_empty() { &x.short_name } else { &x.name })).collect();
    Ok(Value::Array(out))
}

fn same_transform(g: &crate::graph::Graph, a: NodeId, b: NodeId, root: bool) -> bool {
    let local = |n: NodeId| g.node(n).filter(|x| x.ty().is_a("transform")).map(|x| x.matrix(Transform::MATRIX));
    let close = match (local(a), local(b)) {
        (Some(p), Some(q)) => p.abs_diff_eq(q, 1e-6),
        _ => false,
    };
    if !close {
        return false;
    }
    if root {
        return true;
    }
    let kids = |n: NodeId| -> Vec<NodeId> { g.children(n).iter().copied().filter(|&c| crate::dag::is_transform(g, c)).collect() };
    let (ka, kb) = (kids(a), kids(b));
    ka.len() == kb.len() && ka.iter().zip(&kb).all(|(&p, &q)| same_transform(g, p, q, false))
}

fn transform_compare(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    g.evaluate();
    let [x, y] = names.as_slice() else { fail!("transformCompare: name or select two transforms") };
    let (Some(x), Some(y)) = (g.find(x)?, g.find(y)?) else { fail!("transformCompare: no such object") };
    Ok(json!(if same_transform(g, x, y, a.bool("root")) { 0 } else { 1 }))
}

fn lock_node(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let mut ids = Vec::new();
    for n in &names {
        let Some(id) = g.find(n)? else { fail!("lockNode: no object {n}") };
        ids.push(id);
    }
    if a.bool("query") {
        return Ok(Value::Array(ids.iter().map(|&i| json!(g.node(i).unwrap().is_locked())).collect()));
    }
    let on = a.get("lock").map_or(true, on_off);
    g.open_chunk("lockNode");
    for &i in &ids {
        g.set_node_locked(i, on);
    }
    g.close_chunk();
    Ok(Value::Null)
}

fn is_true(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0) else { fail!("isTrue: name a condition") };
    let g = &s.graph;
    Ok(json!(match name {
        "SomethingSelected" => !g.selection().is_empty(),
        "MayaInitialized" | "opengl" => true,
        "BatchMode" | "playingBack" | "playingBackAuto" | "delete" | "dynamicsIsRunning" | "readingFile" | "writingFile" => false,
        "autoKeyframeState" | "autoKeyframe" => crate::anim::auto_key(),
        _ => fail!("isTrue: no condition {name}"),
    }))
}

fn encode_string(a: &Args) -> CmdResult {
    let Some(t) = a.pos_str(0) else { fail!("encodeString: give a string") };
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 32 || c as u32 == 127 => out.push_str(&format!("\\{:03o}", c as u32)),
            c => out.push(c),
        }
    }
    Ok(json!(out))
}

fn find_type(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &s.graph;
    let ty = a.string("type", "");
    if ty.is_empty() {
        fail!("findType: give -type");
    }
    let fits = |id: NodeId| g.node(id).is_some_and(|n| if a.bool("exact") { n.ty().name == ty } else { n.ty().is_a(&ty) });
    let conns = g.connections();
    let (forward, deep) = (a.bool("forward"), a.bool("deep"));
    let mut out = Vec::new();
    for name in names {
        let Some(id) = g.find(&name)? else { continue };
        let mut seen: HashSet<NodeId> = HashSet::from([id]);
        let mut front = std::collections::VecDeque::from([id]);
        // The node itself counts: the search starts there.
        if fits(id) {
            out.push(json!(g.path(id)));
            if !deep {
                continue;
            }
        }
        'walk: while let Some(n) = front.pop_front() {
            let near: Vec<NodeId> = conns.iter().filter_map(|(d, s)| if forward { (s.node == n).then_some(d.node) } else { (d.node == n).then_some(s.node) }).collect();
            for x in near {
                if !seen.insert(x) {
                    continue;
                }
                if fits(x) {
                    out.push(json!(g.path(x)));
                    if !deep {
                        break 'walk;
                    }
                }
                front.push_back(x);
            }
        }
    }
    Ok(Value::Array(out))
}

fn object_center(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    g.evaluate();
    let Some(name) = names.first() else { fail!("objectCenter: name or select an object") };
    let Some(id) = g.find(name)? else { fail!("objectCenter: no object {name}") };
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    let mut under = g.descendants(id);
    under.push(id);
    for u in under {
        if !crate::dag::is_shape(g, u) || g.node(u).is_some_and(|n| n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT)) {
            continue;
        }
        let b = crate::node_bounds::world_bounds(g, u);
        if b.valid() {
            lo = lo.min(b.min);
            hi = hi.max(b.max);
        }
    }
    let world = g.node(id).map_or(glam::DMat4::IDENTITY, |n| if n.ty().dag { n.matrix(Transform::WORLD_MATRIX) } else { glam::DMat4::IDENTITY });
    let mut c = if lo.x <= hi.x { (lo + hi) * 0.5 } else { world.w_axis.truncate() };
    if a.bool("local") {
        c = world.inverse().transform_point3(c);
    }
    Ok(if a.bool("x") {
        json!(c.x)
    } else if a.bool("y") {
        json!(c.y)
    } else if a.bool("z") {
        json!(c.z)
    } else {
        json!([c.x, c.y, c.z])
    })
}

fn list_node_types(a: &Args) -> CmdResult {
    let Some(class) = a.pos_str(0) else { fail!("listNodeTypes: give a classification") };
    let exclude = a.get("exclude").and_then(Value::as_str).map(str::to_string);
    let mut out = Vec::new();
    for t in crate::node::types::all() {
        let Some(c) = crate::utility_nodes::classification(&t.name) else { continue };
        // A classification can have several parts, split by colons: any of them counts.
        let under = |want: &str| c.split(':').any(|p| p == want || p.starts_with(&format!("{want}/")) || (want.contains('*') && crate::ref_core::glob(want, p)));
        let ok = under(class) || c == class;
        let no = exclude.as_ref().is_some_and(|e| under(e));
        if ok && !no {
            out.push(t.name.clone());
        }
    }
    out.sort();
    Ok(json!(out))
}

/// About how much memory a value takes.
fn value_bytes(v: &PlugValue) -> usize {
    match v {
        PlugValue::Bool(_) => 1,
        PlugValue::Int(_) | PlugValue::Float(_) => 8,
        PlugValue::Vec3(_) => 24,
        PlugValue::Matrix(_) => 128,
        PlugValue::String(s) => s.len() + 24,
        PlugValue::Mesh(Some(m)) => m.point_capacity() as usize * 24 + m.face_capacity() as usize * 16,
        PlugValue::Mesh(None) | PlugValue::Data(None) => 8,
        PlugValue::Data(Some(_)) => 64,
    }
}

fn dg_info(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let names = a.named();
    let ids: Vec<NodeId> = if names.is_empty() || a.bool("allNodes") { g.nodes() } else { names.iter().filter_map(|n| g.find(n).ok().flatten()).collect() };
    let ty = a.get("type").and_then(Value::as_str).map(str::to_string);
    // Nodes every scene has are left out unless -nonDeletable.
    let ids: Vec<NodeId> = ids.into_iter().filter(|&id| a.bool("nonDeletable") || !crate::defaults::undeletable(g, id)).filter(|&id| ty.as_ref().is_none_or(|t| g.node(id).is_some_and(|n| n.ty().is_a(t)))).collect();
    if a.bool("size") {
        // Nodes, datablocks (one each here) and the memory their values take.
        let bytes: usize = ids.iter().filter_map(|&id| g.node(id)).map(|n| n.data().values.iter().map(value_bytes).sum::<usize>()).sum();
        return Ok(json!([ids.len(), ids.len(), bytes]));
    }
    let mut out: Vec<String> = Vec::new();
    if a.bool("connections") {
        for (d, sr) in g.connections() {
            if ids.contains(&d.node) || ids.contains(&sr.node) {
                out.push(format!("{} -> {}", g.plug_name(&sr), g.plug_name(&d)));
            }
        }
    } else {
        for id in ids {
            let n = g.node(id).unwrap();
            if a.has("dirty") && a.bool("dirty") != n.is_dirty() {
                continue;
            }
            out.push(g.path(id));
        }
    }
    if let Some(path) = a.get("outputFile").and_then(Value::as_str) {
        std::fs::write(path, out.join("\n")).map_err(|e| aztek_ops::CommandError::new(format!("dgInfo: {path}: {e}")))?;
    }
    Ok(json!(out))
}

/// getModulePath -mn: where a module's .mod file says it lives, searched along AZTEK_MODULE_PATH and MAYA_MODULE_PATH.
fn module_path(a: &Args) -> CmdResult {
    let name = a.string("moduleName", "");
    if name.is_empty() {
        fail!("getModulePath: give -moduleName");
    }
    for var in ["AZTEK_MODULE_PATH", "MAYA_MODULE_PATH"] {
        let Ok(list) = std::env::var(var) else { continue };
        for dir in std::env::split_paths(&list) {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if e.path().extension().and_then(|x| x.to_str()) != Some("mod") {
                    continue;
                }
                let Ok(t) = std::fs::read_to_string(e.path()) else { continue };
                for line in t.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 && parts[0] == "+" && parts[1] == name {
                        let p = parts[parts.len() - 1];
                        return Ok(json!(if p.starts_with('.') { dir.join(p).to_string_lossy().to_string() } else { p.to_string() }));
                    }
                }
            }
        }
    }
    fail!("getModulePath: no module named {name}")
}

/// closestPointOnMesh and nearestPointOnMesh: a node on the mesh, answering in world space.
fn closest_cmd(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let Some(name) = names.first() else { fail!("{ty}: name or select a mesh") };
    let Some(id) = g.find(name)? else { fail!("{ty}: no object {name}") };
    let Some(mesh) = crate::components::as_mesh(g, id) else { fail!("{ty}: {name} is not a mesh") };
    let pos = a.vec3("inPosition").map_or(DVec3::ZERO, DVec3::from_array);
    let t = crate::node::types::find(ty).or_else(|| crate::node::types::find("closestPointOnMesh")).expect("closestPointOnMesh");
    g.open_chunk(ty);
    let r = (|| -> CmdResult<NodeId> {
        let n = g.create_node(&t, &a.string("name", &format!("{ty}1")), crate::plug::NO_NODE, true)?;
        let at = |k: &str| PlugRef::new(n, t.attr_index(k).unwrap() as i32);
        g.set_value(at("inPosition"), PlugValue::Vec3(pos))?;
        g.connect(&PlugRef::new(mesh, crate::mesh_nodes::MeshNode::OUT_MESH as i32), &at("inMesh"), false)?;
        g.connect(&PlugRef::new(mesh, Transform::WORLD_MATRIX as i32), &at("inputMatrix"), false)?;
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    g.evaluate();
    Ok(json!(g.path(n)))
}

thread_local! {
    /// translator -fileCompression, by translator.
    static COMPRESSION: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "translator",
            "The file types file -type knows, as the reference's translators: -q -list their names (those whose plug-in is \
             loaded), and of one named: -q -extension, -filter, -readSupport, -writeSupport, -loaded, -defaultOptions.",
            |s, a| {
                let known = |cmd: &Option<&str>| cmd.is_none_or(|c| s.commands.find(c).is_some());
                // -findTranslator file mode: the loaded translator for the file's type that reads (0), writes (1) or both (2).
                if let Some(v) = a.get("findTranslator") {
                    let parts: Vec<Value> = match v {
                        Value::Array(x) => x.clone(),
                        o => vec![o.clone()],
                    };
                    let file = parts.first().and_then(Value::as_str).unwrap_or("");
                    let mode = parts.get(1).and_then(Value::as_i64).unwrap_or(0);
                    let ext = std::path::Path::new(file).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                    let fits = |r: bool, w: bool| match mode {
                        1 => w,
                        2 => r && w,
                        _ => r,
                    };
                    let builtin = crate::ma_file::TRANSLATORS.iter().find(|t| t.1 == ext && known(&t.4) && fits(t.2, t.3)).map(|t| t.0.to_string());
                    let found = builtin.or_else(|| crate::ma_file::plugin_translators().into_iter().find(|t| t.extension.eq_ignore_ascii_case(&ext) && fits(t.read, t.write)).map(|t| t.name));
                    return Ok(json!(found.unwrap_or_default()));
                }
                // -fileCompression given a value outside query: what saving files of that type is to do.
                if !a.bool("query") {
                    if let (Some(c), Some(n)) = (a.get("fileCompression").and_then(Value::as_str), a.named().first()) {
                        if !["compressed", "uncompressed", "asCompressed"].contains(&c) {
                            fail!("translator: -fileCompression is compressed, uncompressed or asCompressed, not {c}");
                        }
                        COMPRESSION.with(|m| m.borrow_mut().insert(n.to_lowercase(), c.to_string()));
                        return Ok(json!(true));
                    }
                    fail!("translator: use -q, -findTranslator or -fileCompression");
                }
                let q: Vec<String> = a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let asked = |f: &str| a.bool(f) || q.iter().any(|x| x == f) || (f == "fileCompression" && a.get(f).is_some_and(Value::is_string));
                if let Some(n) = a.named().first().filter(|_| asked("fileCompression")) {
                    return Ok(json!(COMPRESSION.with(|m| m.borrow().get(&n.to_lowercase()).cloned()).unwrap_or_else(|| "uncompressed".into())));
                }
                if asked("list") {
                    let mut l: Vec<String> = crate::ma_file::TRANSLATORS.iter().filter(|t| known(&t.4)).map(|t| t.0.to_string()).collect();
                    l.extend(crate::ma_file::plugin_translators().into_iter().map(|t| t.name));
                    return Ok(json!(l));
                }
                let names = a.named();
                let Some(n) = names.first() else { fail!("translator: name a file type") };
                if let Some(t) = crate::ma_file::plugin_translators().into_iter().find(|t| t.name.eq_ignore_ascii_case(n)) {
                    return Ok(if asked("extension") {
                        json!(t.extension)
                    } else if asked("filter") {
                        json!(format!("*.{}", t.extension))
                    } else if asked("readSupport") {
                        json!(t.read)
                    } else if asked("writeSupport") {
                        json!(t.write)
                    } else if asked("loaded") {
                        json!(true)
                    } else if asked("defaultOptions") {
                        json!(t.default_options)
                    } else if asked("optionsScript") {
                        json!(t.options_script)
                    } else if asked("defaultFileRule") || asked("objectType") {
                        json!(t.name)
                    } else {
                        json!(t.name)
                    });
                }
                let Some(t) = crate::ma_file::TRANSLATORS.iter().find(|t| t.0.eq_ignore_ascii_case(n)) else { fail!("translator: no file type {n}") };
                Ok(if asked("extension") {
                    json!(t.1)
                } else if asked("filter") {
                    json!(format!("*.{}", t.1))
                } else if asked("readSupport") {
                    json!(t.2)
                } else if asked("writeSupport") {
                    json!(t.3)
                } else if asked("loaded") {
                    json!(known(&t.4))
                } else if asked("defaultOptions") {
                    json!("")
                } else if asked("optionsScript") {
                    json!("")
                } else if asked("defaultFileRule") || asked("objectType") {
                    // The workspace file rule the type's files go by: its own name, as the default workspace lists them.
                    json!(t.0)
                } else {
                    fail!("translator: query what?")
                })
            },
        )
        .positional("file type")
        .flag(FlagDef::new("list", "l", ArgType::Bool, Value::Null, "Every file type."))
        .flag(FlagDef::new("extension", "ext", ArgType::Bool, Value::Null, "Its extension."))
        .flag(FlagDef::new("filter", "f", ArgType::Bool, Value::Null, "Its file filter."))
        .flag(FlagDef::new("readSupport", "rs", ArgType::Bool, Value::Null, "Whether it reads."))
        .flag(FlagDef::new("writeSupport", "ws", ArgType::Bool, Value::Null, "Whether it writes."))
        .flag(FlagDef::new("loaded", "ld", ArgType::Bool, Value::Null, "Whether its plug-in is loaded."))
        .flag(FlagDef::new("defaultOptions", "do", ArgType::Bool, Value::Null, "Its default options string."))
        .flag(FlagDef::new("defaultFileRule", "dfr", ArgType::Bool, Value::Null, "The workspace file rule its files go by."))
        .flag(FlagDef::new("objectType", "ot", ArgType::Bool, Value::Null, "Obsolete: the same as -defaultFileRule."))
        .flag(FlagDef::new("optionsScript", "os", ArgType::Bool, Value::Null, "The MEL procedure that posts its options, or \"\"."))
        .flag(FlagDef::new("fileCompression", "cmp", ArgType::Any, Value::Null, "compressed, uncompressed or asCompressed: what saving its files does; -q gives it."))
        .flag(FlagDef::new("findTranslator", "ft", ArgType::Any, Value::Null, "A file and 0 read, 1 write or 2 both: the translator that handles it.").multi(2))
        .flag(FlagDef::new("query", "q", ArgType::Bool, Value::Null, "Read."))
        .flag(FlagDef::new("queried", "", ArgType::List, Value::Null, "What -q asks for."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("upAxis", "The world's up axis, a preference kept between scenes: -ax y or z; -rv turns the cameras with it so the ground stays put on screen. -q -ax asks.", up_axis_cmd))
            .flag(f("axis", "ax", ArgType::Any, "y or z."))
            .flag(f("rotateView", "rv", B, "Turn the default cameras too."))
            .undoable(false),
    );
    c.register(
        Cmd::new("convertUnit", "Convert a value between units: convertUnit -f inch -t cm \"1\". Returns the value with its unit.", convert_unit)
            .positional("value")
            .flag(f("fromUnit", "f", S, "The unit of the value (centimeters, or as the value spells it)."))
            .flag(f("toUnit", "t", S, "The unit wanted."))
            .undoable(false),
    );
    c.register(
        Cmd::new("copyAttr", "Copy attributes between nodes: -v values, -ic and -oc move connections (-ksc leaves the source's), -at which attributes.", copy_attr)
            .positional("source target")
            .flag(f("attribute", "at", ArgType::Any, "Attributes to copy (all that both nodes have)."))
            .flag(f("values", "v", B, "Copy values."))
            .flag(f("inConnections", "ic", B, "Move incoming connections."))
            .flag(f("outConnections", "oc", B, "Move outgoing connections."))
            .flag(f("keepSourceConnections", "ksc", B, "Leave the source its connections."))
            .flag(f("containerParentChild", "cpc", B, "Accepted for existing scripts."))
            .flag(f("renameTargetContainer", "rtc", B, "Accepted for existing scripts.")),
    );
    c.register(
        q_flags(Cmd::new("cycleCheck", "Look for dependency cycles: -evaluation on|off the warning, -q asks; objects or -all: true when one is found, -l the plugs on it.", cycle_check))
            .positional("objects")
            .flag(f("evaluation", "ev", ArgType::Any, "Warn about cycles while evaluating."))
            .flag(f("all", "all", B, "Look through the whole scene."))
            .flag(f("list", "l", B, "List the plugs on a cycle."))
            .flag(f("firstCycleOnly", "fco", B, "With -l: only the first cycle found."))
            .flag(f("firstPlugPerNode", "fpn", B, "With -l: only the first plug of each node on a cycle."))
            .flag(f("lastPlugPerNode", "lpn", B, "With -l: only the last plug of each node on a cycle."))
            .flag(f("listSeparator", "ls", S, "With -l: this string goes between one cycle's plugs and the next's."))
            .flag(f("secondary", "s", ArgType::Any, "Look at the plugs related to those named too (on with -all)."))
            .flag(f("timeLimit", "tl", ArgType::Any, "Stop looking after this long: 10sec, 2min, or seconds."))
            .flag(f("children", "c", B, "Accepted for existing scripts."))
            .flag(f("parents", "p", B, "Accepted for existing scripts."))
            .flag(f("dag", "dag", B, "Count parenting too: a parent's world matrix feeds its children."))
            .undoable(false),
    );
    c.register(
        crate::freeze::evaluation_manager_flags(q_flags(Cmd::new("evaluationManager", "The evaluation mode, as a setting scripts can read: -mode off|serial|parallel|emp; -dst, -ust and -ccl walk the graph; node type overrides and settings are held. The graph evaluates as before whatever the mode.", |s, a| {
            if let Some(r) = crate::freeze::evaluation_manager(s, a) {
                if !a.has("mode") || a.bool("query") {
                    return r;
                }
                r?;
            }
            if a.bool("query") {
                let m = s.extras.evaluation_mode.clone().unwrap_or("off".to_string());
                if asked(a, "enabled") {
                    return Ok(json!(m != "off"));
                }
                if asked(a, "manipulation") || asked(a, "invalidate") || asked(a, "idleBuild") {
                    return Ok(json!(false));
                }
                return Ok(json!([m]));
            }
            if let Some(m) = a.get("mode").and_then(Value::as_str) {
                if !["off", "serial", "serialUncached", "parallel", "emp", "dg"].contains(&m) {
                    fail!("evaluationManager: unknown mode {m}");
                }
                s.extras.evaluation_mode = Some(if m == "dg" { "off".to_string() } else { m.to_string() });
            }
            Ok(Value::Null)
        }))
        .flag(f("mode", "m", ArgType::Any, "off, serial, parallel or emp."))
        .flag(f("enabled", "en", B, "Whether a mode other than off is set."))
        .flag(f("invalidate", "inv", B, "Accepted for existing scripts."))
        .flag(f("manipulation", "man", B, "Accepted for existing scripts."))
        .flag(f("idleBuild", "ib", B, "Accepted for existing scripts."))
        .positional("node types, for the node type overrides")
        .undoable(false)),
    );
    c.register(
        q_flags(Cmd::new("threadCount", "Threads to evaluate with, kept for scripts: -n count, 0 for the machine's; -q -n asks.", |s, a| {
            if a.bool("query") {
                let n = s.extras.thread_count.unwrap_or(0);
                return Ok(json!(if n > 0 { n } else { std::thread::available_parallelism().map_or(1, |x| x.get() as i64) }));
            }
            s.extras.thread_count = Some(a.int("numberOfThreads", 0));
            Ok(Value::Null)
        }))
        .flag(f("numberOfThreads", "n", I, "How many."))
        .undoable(false),
    );
    c.register(
        Cmd::new("dgInfo", "The dependency graph as text: -nd nodes, -c connections, -nt a type, -d dirty ones, -of a file.", dg_info)
            .positional("objects")
            .flag(f("allNodes", "all", B, "Every node."))
            .flag(f("nodes", "n", B, "List nodes (what it lists anyway)."))
            .flag(f("connections", "c", B, "List connections."))
            .flag(f("dirty", "d", B, "Only nodes in this dirty state."))
            .flag(f("nonDeletable", "nd", B, "Take in the nodes every scene has, left out otherwise."))
            .flag(f("size", "sz", B, "The nodes' sizes: how many nodes, datablocks and bytes."))
            .flag(f("outputFile", "of", S, "Write to this file."))
            .flag(f("propagation", "p", B, "Accepted for existing scripts."))
            .flag(f("short", "s", B, "Accepted for existing scripts."))
            .flag(f("subgraph", "sub", B, "Accepted for existing scripts."))
            .flag(f("type", "nt", S, "Only this type."))
            .undoable(false),
    );

    c.register(
        q_flags(Cmd::new("displayColor", "A display colour's index: name index, -a active or dormant, -q asks, -l lists, -c creates, -rf resets.", display_color))
            .positional("name [index]")
            .flag(f("active", "a", B, "The active colour."))
            .flag(f("dormant", "d", B, "The dormant colour (the default)."))
            .flag(f("create", "c", B, "Create the name."))
            .flag(f("list", "l", B, "List the names."))
            .flag(f("queryIndex", "qi", I, "Accepted for existing scripts."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .flag(f("userDefined", "ud", I, "Accepted for existing scripts."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("displayRGBColor", "A display colour's RGB: name r g b [a], -q asks, -l lists, -c creates.", display_rgb))
            .positional("name r g b [a]")
            .flag(f("alpha", "a", B, "With alpha."))
            .flag(f("create", "c", B, "Create the name."))
            .flag(f("hueSaturationValue", "hsv", B, "Accepted for existing scripts."))
            .flag(f("list", "l", B, "List the names."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("colorIndex", "A palette entry's colour: index r g b, 1 to 31, -q asks.", color_index))
            .positional("index [r g b]")
            .flag(f("hueSaturationValue", "hsv", B, "The values are hue (degrees), saturation and value; -q answers so."))
            .flag(f("active", "atv", B, "The active palette: the colours selected objects draw in."))
            .flag(f("dormant", "dor", B, "The dormant palette (the default)."))
            .flag(f("userColor", "uc", B, "The eight user colours, 1 to 8."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("displayLevelOfDetail", "Level of detail display, as a setting scripts can read: -lod on|off.", |s, a| {
            if a.bool("query") {
                return Ok(json!(s.extras.level_of_detail));
            }
            if let Some(v) = a.positional.first().or(a.get("levelOfDetail")) {
                s.extras.level_of_detail = on_off(v);
            }
            Ok(Value::Null)
        }))
        .positional("on or off")
        .flag(f("levelOfDetail", "lod", ArgType::Any, "On or off."))
        .undoable(false),
    );
    setting_store(c, "displayPref", "Viewport display preferences, kept for scripts: each flag sets, -q reads.", DISPLAY_PREF, |s| &mut s.extras.display_pref);
    // -ghostFrames sets the ghosting options as the ghosting command does; -maxHardwareTextureResolution asks the viewport.
    crate::ref_core::extend(
        c,
        "displayPref",
        vec![
            f("ghostFrames", "gf", ArgType::List, "Obsolete: the ghosting command's frames before, frames after and step, as three numbers.").multi(3),
            f("maxHardwareTextureResolution", "mhr", B, "Query: the largest texture the viewport's graphics card takes."),
        ],
        |s, a, prev| {
            let q = a.bool("query");
            if q && q_one(a).as_deref() == Some("maxHardwareTextureResolution") || (q && a.bool("maxHardwareTextureResolution")) {
                return Ok(json!(MAX_HW_TEXTURE.load(std::sync::atomic::Ordering::Relaxed)));
            }
            if q && (q_one(a).as_deref() == Some("ghostFrames") || a.has("ghostFrames")) {
                return Ok(json!(["preFrames", "postFrames", "ghostsStep"].iter().map(|k| crate::anim_extra::ghost_option(s, k).as_i64().unwrap_or(1)).collect::<Vec<_>>()));
            }
            if !q {
                if let Some(Value::Array(v)) = a.get("ghostFrames") {
                    let n: Vec<i64> = v.iter().filter_map(|x| x.as_i64().or_else(|| x.as_f64().map(|f| f as i64))).collect();
                    if n.len() != 3 {
                        fail!("displayPref: -ghostFrames takes three numbers: frames before, frames after, step");
                    }
                    for (k, x) in ["preFrames", "postFrames", "ghostsStep"].iter().zip(n) {
                        s.extras.more.ghosting.insert(k.to_string(), json!(x));
                    }
                }
                if a.bool("maxHardwareTextureResolution") {
                    fail!("displayPref: -maxHardwareTextureResolution can only be queried");
                }
            }
            prev(s, a)
        },
    );
    setting_store(c, "displayStats", "Which statistics a viewport shows, kept for scripts: each flag sets, -q reads.", DISPLAY_STATS, |s| &mut s.extras.display_stats);
    c.register(
        q_flags(Cmd::new("displaySurface", "How the named or selected surfaces draw: -x X-ray (see-through), -two two-sided (off culls the back faces, and the shape renders single sided), -flp flips the normals drawn. -q reads the first.", |s, a| {
            let names = objects(s, a);
            let ids: Vec<NodeId> = names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).collect();
            if a.bool("query") {
                let Some(q) = q_one(a) else { fail!("displaySurface: query what?") };
                let Some(&id) = ids.first() else { fail!("displaySurface: name an object") };
                let def = q == "twoSidedLighting";
                return Ok(json!([s.extras.display_surface.get(&(id, q)).copied().unwrap_or(def)]));
            }
            // NURBS surfaces and meshes only: other objects are passed over.
            let ids: Vec<NodeId> = ids
                .into_iter()
                .filter(|&id| {
                    let g = &s.graph;
                    let mut all = vec![id];
                    all.extend(crate::dag::shapes(g, id, false));
                    all.iter().any(|&n| g.node(n).is_some_and(|x| x.ty().is_a("mesh") || x.ty().is_a("nurbsSurface")))
                })
                .collect();
            for flag in ["xRay", "twoSidedLighting", "flipNormals"] {
                if a.has(flag) {
                    let on = a.bool(flag);
                    for &id in &ids {
                        s.extras.display_surface.insert((id, flag.to_string()), on);
                        if flag == "twoSidedLighting" {
                            // The shapes' doubleSided render setting follows.
                            let mut shapes = crate::dag::shapes(&s.graph, id, false);
                            shapes.push(id);
                            for sh in shapes {
                                if let Some(i) = s.graph.node(sh).and_then(|x| x.ty().attr_index("doubleSided")) {
                                    s.graph.set_value(crate::plug::PlugRef::new(sh, i as i32), crate::plug::PlugValue::Bool(on))?;
                                }
                            }
                        }
                    }
                }
            }
            Ok(Value::Null)
        }))
        .positional("objects")
        .flag(f("xRay", "x", B, "Draw see-through."))
        .flag(f("twoSidedLighting", "two", B, "Two-sided (on by default); off, single sided: the back faces are culled."))
        .flag(f("flipNormals", "flp", B, "Flip the normals drawn."))
        .undoable(false),
    );
    c.register(
        Cmd::new("allNodeTypes", "Every node type there is, by name. -ia is taken: Aztek has no abstract types to add.", |_s, _a| {
            let mut names: Vec<String> = crate::node::types::all().iter().map(|t| t.name.clone()).collect();
            names.sort();
            Ok(json!(names))
        })
        .flag(f("includeAbstract", "ia", B, "Taken; there are no abstract types."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new(
            "dirmap",
            "Folders mapped to other folders, for scenes moved between machines: -m from to adds a mapping, -um from removes one, -cd path gives the path with its mapping applied (or as it is), -gmd from the folder one maps to, -gam all of them, -en turns mapping on or off.",
            |s, a| {
                let mut maps: Vec<(String, String)> = s.option_vars.get("__dirmap").and_then(Value::as_array).map(|v| v.iter().filter_map(|p| Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()))).collect()).unwrap_or_default();
                let enabled = s.option_vars.get("__dirmapEnabled").and_then(Value::as_bool).unwrap_or(false);
                let texts = |v: Option<&Value>| -> Vec<String> {
                    match v {
                        Some(Value::Array(x)) => x.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                        Some(Value::String(x)) => vec![x.clone()],
                        _ => Vec::new(),
                    }
                };
                if let Some(path) = a.get("convertDirectory").and_then(Value::as_str) {
                    if enabled {
                        // The longest mapped folder the path is under wins.
                        if let Some((from, to)) = maps.iter().filter(|(from, _)| path.starts_with(from.as_str())).max_by_key(|(from, _)| from.len()) {
                            return Ok(json!(format!("{to}{}", &path[from.len()..])));
                        }
                    }
                    return Ok(json!(path));
                }
                if let Some(from) = a.get("getMappedDirectory").and_then(Value::as_str) {
                    return Ok(json!(maps.iter().find(|m| m.0 == from).map(|m| m.1.clone()).unwrap_or_default()));
                }
                if a.bool("getAllMappings") {
                    return Ok(json!(maps.iter().flat_map(|m| [m.0.clone(), m.1.clone()]).collect::<Vec<_>>()));
                }
                if a.bool("query") {
                    return Ok(json!(enabled));
                }
                let pair = texts(a.get("mapDirectory"));
                if pair.len() == 2 {
                    maps.retain(|m| m.0 != pair[0]);
                    maps.push((pair[0].clone(), pair[1].clone()));
                }
                if let Some(from) = a.get("unmapDirectory").and_then(Value::as_str) {
                    maps.retain(|m| m.0 != from);
                }
                if a.has("enable") {
                    s.option_vars.insert("__dirmapEnabled".into(), json!(a.bool("enable")));
                }
                s.option_vars.insert("__dirmap".into(), json!(maps.iter().map(|m| json!([m.0, m.1])).collect::<Vec<_>>()));
                Ok(Value::Null)
            },
        ))
        .flag(f("convertDirectory", "cd", S, "A path, with its mapping applied."))
        .flag(f("enable", "en", B, "Mapping on or off."))
        .flag(f("getAllMappings", "gam", B, "Every mapping: from, to, from, to."))
        .flag(f("getMappedDirectory", "gmd", S, "What a folder maps to."))
        .flag(f("mapDirectory", "m", L, "From and to.").multi(2))
        .flag(f("unmapDirectory", "um", S, "Remove this folder's mapping."))
        .undoable(false),
    );
    c.register(
        Cmd::new("listNodeTypes", "Node types of a classification: listNodeTypes \"utility/general\", -ex leaves one out.", |_s, a| list_node_types(a))
            .positional("classification")
            .flag(f("exclude", "ex", S, "A classification to leave out."))
            .undoable(false),
    );
    let mut sp = q_flags(Cmd::new("selectPriority", "Selection priorities by component and object type: -cv 5, -q -cv. Kept for scripts.", select_priority)).undoable(false);
    for e in SELECT_PRIORITY {
        if e.0 == "byName" {
            sp = sp.flag(f("byName", "bn", ArgType::Any, "A user-defined selection type and whether it takes priority: -bn name on.").multi(2));
            continue;
        }
        sp = sp.flag(f(e.0, e.1, I, "The priority."));
    }
    sp = sp.flag(f("queryByName", "qbn", S, "With -q: a user-defined selection type's priority, by name."));
    c.register(sp);
    c.register(
        Cmd::new("selectedNodes", "The selection as full paths: -do leaves out what isn't in the DAG.", |s, a| {
            let g = &s.graph;
            let mut out = Vec::new();
            for &id in g.selection() {
                let n = g.node(id).unwrap();
                if a.bool("dagObjects") && !n.ty().dag {
                    continue;
                }
                out.push(g.full_path(id));
            }
            Ok(json!(out))
        })
        .flag(f("dagObjects", "do", B, "Only DAG objects."))
        .undoable(false),
    );
    c.register(
        Cmd::new("findType", "Nodes of a type connected upstream of the objects (the selection when none), or with -f downstream, the objects themselves included: the nearest, or every one with -d. -e matches the type exactly.", find_type)
            .positional("objects")
            .flag(f("type", "t", S, "The type."))
            .flag(f("deep", "d", B, "Every one, not just the nearest."))
            .flag(f("exact", "e", B, "This type exactly."))
            .flag(f("forward", "f", B, "Look downstream."))
            .undoable(false),
    );
    c.register(
        Cmd::new("objectCenter", "An object's bounding box centre: world (-gl, the default) or -l its own space; -x -y -z one value.", object_center)
            .positional("object")
            .flag(f("gl", "gl", B, "World space (the default)."))
            .flag(f("local", "l", B, "The object's space."))
            .flag(f("x", "x", B, "X alone."))
            .flag(f("y", "y", B, "Y alone."))
            .flag(f("z", "z", B, "Z alone."))
            .undoable(false),
    );
    for name in ["closestPointOnMesh", "nearestPointOnMesh"] {
        c.register(
            Cmd::new(name, "Make a node on a mesh that answers the point on it closest to -ip. Returns the node.", move |s, a| closest_cmd(s, a, name))
                .positional("mesh")
                .flag(f("inPosition", "ip", ArgType::Vec3, "The position."))
                .flag(f("name", "n", S, "The node's name.")),
        );
    }
    c.register(Cmd::new("hitTest", "A stub outside the UI: returns nothing, since panels and their cameras live there.", |_s, _a| Ok(json!([]))).positional("control x y").undoable(false));
    c.register(
        Cmd::new("isDirty", "Whether a plug's node needs evaluating: -c through connections, -d the datablock.", |s, a| {
            let g = &s.graph;
            let Some(t) = a.pos_str(0) else { fail!("isDirty: name a plug") };
            let p = g.plug(t)?;
            Ok(json!(g.node(p.node).is_some_and(|n| n.is_dirty())))
        })
        .positional("plug")
        .flag(f("connection", "c", B, "Accepted: dirt is per node here."))
        .flag(f("datablock", "d", B, "Accepted: dirt is per node here."))
        .undoable(false),
    );
    c.register(Cmd::new("isTrue", "Is a named condition true: SomethingSelected, playingBack, autoKeyframeState, ...", is_true).positional("condition").undoable(false));
    c.register(Cmd::new("getModulePath", "Where a module lives: -mn name, along AZTEK_MODULE_PATH and MAYA_MODULE_PATH.", |_s, a| module_path(a)).flag(f("moduleName", "mn", S, "The module.")).undoable(false));
    c.register(Cmd::new("encodeString", "A string with quotes, backslashes and control characters escaped, as MEL would need them.", |_s, a| encode_string(a)).positional("string").undoable(false));
    c.register(
        Cmd::new("attributeName", "An attribute's name: -n nice (the default), -l long, -s short, -lf the leaf.", attribute_name)
            .positional("plug")
            .flag(f("nice", "n", B, "The name editors show."))
            .flag(f("long", "l", B, "The long name."))
            .flag(f("short", "s", B, "The short name."))
            .flag(f("leaf", "lf", B, "Without its parent's name."))
            .undoable(false),
    );
    c.register(
        Cmd::new("attributeInfo", "A node's attributes filtered: -b boolean, -e enumerated, -h hidden, -w writable, -m multi, -l leaf, -in inherited; -t a type in place of a node; -s short names.", attribute_info)
            .positional("object")
            .flag(f("allAttributes", "all", B, "All (what no filter gives)."))
            .flag(f("bool", "b", B, "Booleans."))
            .flag(f("enumerated", "e", B, "Enums."))
            .flag(f("hidden", "h", B, "Hidden ones."))
            .flag(f("inherited", "in", B, "Inherited from the base type."))
            .flag(f("internal", "i", B, "Internal ones: none here."))
            .flag(f("leaf", "l", B, "Leaves of compounds: attributes with a parent and no children."))
            .flag(f("logicalAnd", "and", B, "All filters must hold, not any."))
            .flag(f("multi", "m", B, "Arrays."))
            .flag(f("short", "s", B, "Short names."))
            .flag(f("type", "t", S, "A type rather than a node."))
            .flag(f("userInterface", "ui", B, "Accepted for existing scripts."))
            .flag(f("writable", "w", B, "Writable ones."))
            .undoable(false),
    );
    c.register(Cmd::new("transformCompare", "0 when two transforms match, with what's under them, 1 otherwise. -r the root alone.", transform_compare).positional("two transforms").flag(f("root", "r", B, "Only the root.")).undoable(false));
    c.register(
        q_flags(Cmd::new("lockNode", "Lock nodes against deleting and renaming: -l on or off (on with no flag); -q -l reads each.", lock_node))
            .positional("objects")
            .flag(f("lock", "l", ArgType::Any, "On or off."))
            .flag(f("lockName", "ln", B, "Accepted: a locked node keeps its name anyway."))
            .flag(f("lockUnpublished", "lu", B, "Accepted for existing scripts."))
            .flag(f("ignoreComponents", "ic", B, "Accepted for existing scripts.")),
    );
}
