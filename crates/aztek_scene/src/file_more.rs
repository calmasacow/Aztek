//! The rest of the reference's file command: session settings (-defaultExtensions, -renameToSave, -saveDiskCache,
//! -saveTextures, -uiConfiguration), the error status, compressed saves through a temporary file, strict and
//! animation exports, export previews and exports as references, import options (frame rate, time range, base layer,
//! duplicate networks, names), how an open loads references, -selectAll, -saveReference and the queries that go with
//! them. The scripts that run around a save and the snapshot callback are run by the host, which can run scripts.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandError, Commands, FlagDef, RunFn};
use serde_json::{json, Value};

use crate::graph::SelectMode;
use crate::ma_file::Format;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::ref_core::asked;
use crate::session::Session;

struct State {
    default_extensions: bool,
    error: bool,
    last_option: String,
    last_temp: String,
    rename_to_save: bool,
    save_disk_cache: String,
    save_textures: String,
    ui_configuration: bool,
    ui_load_configuration: bool,
    command: Vec<String>,
    place_holders: Vec<(String, String)>,
    replace_names: Vec<(String, String)>,
    /// What -buildLoadSettings read: the file, then its references in pre-order (selLoadSettings' IDs).
    load_settings: Vec<LoadSetting>,
    /// -proxyManager, -proxyTag and -activeProxy by reference node.
    proxies: HashMap<String, (String, String, String)>,
}

impl Default for State {
    fn default() -> Self {
        State {
            default_extensions: true,
            error: false,
            last_option: String::new(),
            last_temp: String::new(),
            rename_to_save: false,
            save_disk_cache: "always".into(),
            save_textures: "unlessRef".into(),
            ui_configuration: true,
            ui_load_configuration: true,
            command: Vec::new(),
            place_holders: Vec::new(),
            replace_names: Vec::new(),
            load_settings: Vec::new(),
            proxies: HashMap::new(),
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn st<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// file -command, for the host to run before a file operation: the script and its type.
pub fn file_callback() -> Vec<String> {
    st(|s| s.command.clone())
}

// ---------------------------------------------------------------------------------------------------------------------
// gzip, for -compress

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

pub fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255];
    out.extend(miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend(crc32(data).to_le_bytes());
    out.extend((data.len() as u32).to_le_bytes());
    out
}

pub fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return Err("not a gzip file".into());
    }
    let flg = data[3];
    let mut at = 10usize;
    if flg & 4 != 0 {
        let n = u16::from_le_bytes([data[at], data[at + 1]]) as usize;
        at += 2 + n;
    }
    for bit in [8u8, 16] {
        if flg & bit != 0 {
            while at < data.len() && data[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    if at >= data.len() {
        return Err("a damaged gzip file".into());
    }
    miniz_oxide::inflate::decompress_to_vec(&data[at..data.len().saturating_sub(8)]).map_err(|e| format!("a damaged gzip file ({e:?})"))
}

// ---------------------------------------------------------------------------------------------------------------------
// Helpers

fn texts(v: Option<&Value>) -> Vec<String> {
    fn flat(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| flat(x, out)),
            Value::String(s) => out.push(s.clone()),
            Value::Null => {}
            o => out.push(o.to_string()),
        }
    }
    let mut out = Vec::new();
    if let Some(v) = v {
        flat(v, &mut out);
    }
    out
}

fn pairs(v: Option<&Value>) -> Vec<(String, String)> {
    texts(v).chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect()
}

fn extension(path: &str) -> String {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn is_scene_type(t: Option<&str>) -> bool {
    matches!(t, None | Some("mayaAscii") | Some("mayaBinary"))
}

fn is_scene_path(p: &str) -> bool {
    let e = extension(p);
    matches!(e.as_str(), "" | "ma" | "mb" | "gz")
}

fn names(s: &Session, ids: &[NodeId]) -> Value {
    json!(ids.iter().filter(|&&i| s.graph.node(i).is_some()).map(|&i| s.graph.path(i)).collect::<Vec<_>>())
}

/// `*` and `?` over a whole name.
fn glob(p: &str, t: &str) -> bool {
    crate::ref_core::glob(p, t)
}

fn entry_index(s: &Session, name: &str) -> Option<usize> {
    let t = &s.extras.references.entries;
    let name = name.split('{').next().unwrap_or(name);
    t.iter().position(|e| e.rfn == name).or_else(|| t.iter().position(|e| e.path == name || e.unresolved == name)).or_else(|| t.iter().position(|e| !e.namespace.is_empty() && e.namespace == name.trim_start_matches(':'))).or_else(|| t.iter().position(|e| Path::new(&e.path).file_name().and_then(|x| x.to_str()) == Some(name)))
}

/// A reference's entries, it and every one inside it.
fn entry_and_below(s: &Session, at: usize) -> Vec<u32> {
    let t = &s.extras.references.entries;
    let mut ids = vec![t[at].id];
    let mut i = 0;
    while i < ids.len() {
        let p = ids[i];
        ids.extend(t.iter().filter(|e| e.parent == p).map(|e| e.id));
        i += 1;
    }
    ids
}

/// What a scene file's text holds as fileInfo, without reading the scene.
fn file_info_of(path: &str) -> Vec<(String, String)> {
    let Ok(mut bytes) = std::fs::read(path) else { return Vec::new() };
    if bytes.starts_with(&[0x1f, 0x8b]) {
        bytes = gunzip(&bytes).unwrap_or_default();
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut out = Vec::new();
    for line in text.lines().take(400) {
        if let Some(rest) = line.trim().strip_prefix("fileInfo ") {
            let q: Vec<&str> = rest.split('"').collect();
            if q.len() >= 4 {
                out.push((q[1].to_string(), q[3].to_string()));
            }
        }
    }
    out
}

fn set_file_info(s: &mut Session, key: &str, on: bool) {
    s.extras.file_info.retain(|(k, _)| k != key);
    if on {
        s.extras.file_info.push((key.to_string(), "1".to_string()));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Queries

fn query(s: &mut Session, a: &Args) -> Option<CmdResult> {
    let path = a.pos_str(0).map(str::to_string);
    let q = |n: &str| asked(a, n);
    if q("anyModified") {
        return Some(Ok(json!(s.is_modified())));
    }
    if q("errorStatus") {
        return Some(Ok(json!(st(|x| x.error))));
    }
    if q("lastFileOption") {
        return Some(Ok(json!(st(|x| x.last_option.clone()))));
    }
    if q("lastTempFile") {
        return Some(Ok(json!(st(|x| x.last_temp.clone()))));
    }
    if q("defaultExtensions") {
        return Some(Ok(json!(st(|x| x.default_extensions))));
    }
    if q("renameToSave") {
        return Some(Ok(json!(st(|x| x.rename_to_save))));
    }
    if q("saveDiskCache") {
        return Some(Ok(json!(st(|x| x.save_disk_cache.clone()))));
    }
    if q("saveTextures") {
        return Some(Ok(json!(st(|x| x.save_textures.clone()))));
    }
    if q("uiConfiguration") {
        return Some(Ok(json!(st(|x| x.ui_configuration))));
    }
    if q("uiLoadConfiguration") {
        return Some(Ok(json!(st(|x| x.ui_load_configuration))));
    }
    if q("command") {
        return Some(Ok(json!(st(|x| x.command.clone()))));
    }
    if q("mapPlaceHolderNamespace") {
        return Some(Ok(json!(st(|x| x.place_holders.iter().flat_map(|(a, b)| [a.clone(), b.clone()]).collect::<Vec<_>>()))));
    }
    if q("replaceName") {
        return Some(Ok(json!(st(|x| x.replace_names.iter().flat_map(|(a, b)| [a.clone(), b.clone()]).collect::<Vec<_>>()))));
    }
    if q("fileMetaData") {
        let Some(p) = path else { return Some(Err(CommandError::new("file: -fileMetaData needs a file"))) };
        return Some(Ok(json!(wave_time_reference(&p))));
    }
    if q("copyNumberList") {
        let p = path.or_else(|| s.scene_path.clone()).unwrap_or_default();
        let base = p.split('{').next().unwrap_or(&p).to_string();
        let t = &s.extras.references.entries;
        let same: Vec<&crate::references::RefEntry> = match entry_index(s, &base) {
            Some(at) => t.iter().filter(|e| e.path == t[at].path).collect(),
            None => Vec::new(),
        };
        if same.is_empty() {
            return Some(Ok(json!(if s.scene_path.as_deref() == Some(base.as_str()) { vec!["0".to_string()] } else { Vec::new() })));
        }
        return Some(Ok(json!((0..same.len()).map(|i| i.to_string()).collect::<Vec<_>>())));
    }
    if q("location") {
        let p = path.or_else(|| s.scene_path.clone()).unwrap_or_default();
        if let Some(at) = entry_index(s, &p) {
            return Some(Ok(json!(s.extras.references.entries[at].path)));
        }
        let pb = PathBuf::from(&p);
        let full = if pb.is_absolute() { pb } else { std::env::current_dir().map(|d| d.join(&pb)).unwrap_or(pb) };
        if !full.exists() {
            return Some(Err(CommandError::new(format!("file: {p} not found"))));
        }
        return Some(Ok(json!(full.to_string_lossy().replace('\\', "/"))));
    }
    if q("writable") {
        let main = s.scene_path.clone();
        return Some(Ok(json!(match path.as_deref() {
            None => match &main {
                Some(m) => writable(m),
                None => true,
            },
            Some(p) if main.as_deref() == Some(p) => writable(p),
            Some(_) => false,
        })));
    }
    if q("parentNamespace") {
        let Some(at) = path.as_deref().and_then(|p| entry_index(s, p)) else { return Some(Err(CommandError::new("file: -parentNamespace needs a referenced file"))) };
        let ns = s.extras.references.entries[at].namespace.clone();
        let parent = ns.rsplit_once(':').map(|x| x.0.to_string()).unwrap_or_default();
        let out = if a.bool("absoluteName") || q("absoluteName") { format!(":{parent}") } else if parent.is_empty() { ":".to_string() } else { parent };
        return Some(Ok(json!([out])));
    }
    if q("renamingPrefixList") {
        let parent = match path.as_deref() {
            Some(p) if s.scene_path.as_deref() != Some(p) => match entry_index(s, p) {
                Some(at) => s.extras.references.entries[at].id,
                None => return Some(Err(CommandError::new(format!("file: {p} is not in the scene")))),
            },
            _ => 0,
        };
        let list: Vec<String> = s.extras.references.entries.iter().filter(|e| e.parent == parent).map(|e| if e.prefix.is_empty() { e.namespace.rsplit(':').next().unwrap_or("").to_string() } else { e.prefix.clone() }).filter(|x| !x.is_empty()).collect();
        return Some(Ok(json!(list)));
    }
    None
}

fn writable(path: &str) -> bool {
    match std::fs::metadata(path) {
        Ok(m) => !m.permissions().readonly(),
        Err(_) => Path::new(path).parent().map(|d| if d.as_os_str().is_empty() { Path::new(".") } else { d }).and_then(|d| std::fs::metadata(d).ok()).is_some_and(|m| !m.permissions().readonly()),
    }
}

/// A Broadcast Wave file's time reference (the `bext` chunk's sample count); 0 for any other file.
fn wave_time_reference(path: &str) -> i64 {
    let Ok(b) = std::fs::read(path) else { return 0 };
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return 0;
    }
    let mut at = 12;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let len = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
        let body = at + 8;
        if id == b"bext" && body + 346 <= b.len() {
            let lo = u32::from_le_bytes(b[body + 338..body + 342].try_into().unwrap()) as u64;
            let hi = u32::from_le_bytes(b[body + 342..body + 346].try_into().unwrap()) as u64;
            return ((hi << 32) | lo) as i64;
        }
        at = body + len + (len & 1);
    }
    0
}

// ---------------------------------------------------------------------------------------------------------------------
// Exports

fn export_format(s: &Session, path: &str, ty: Option<&str>) -> CmdResult<Format> {
    if let Some(t) = ty {
        return Format::of_type(t);
    }
    Ok(match s.scene_type {
        Some(f) if extension(path).is_empty() => f,
        _ => Format::of_path(path),
    })
}

fn with_extension(path: &str, f: Format) -> String {
    if extension(path).is_empty() && st(|x| x.default_extensions) {
        format!("{path}.{}", f.extension())
    } else {
        path.to_string()
    }
}

/// The animation curves an animation export takes.
fn anim_curves(s: &Session, selected: bool, referenced: bool, rfn: Option<&str>) -> CmdResult<Vec<NodeId>> {
    let g = &s.graph;
    let only_ref: Option<u32> = match rfn {
        Some(r) => match entry_index(s, r) {
            Some(at) => Some(s.extras.references.entries[at].id),
            None => fail!("file: no reference {r}"),
        },
        None => None,
    };
    let mut targets: HashSet<NodeId> = HashSet::new();
    if selected {
        for &id in g.selection() {
            targets.insert(id);
            targets.extend(g.descendants(id));
        }
    }
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("animCurve") || n.reference_id() != 0 {
            continue;
        }
        let driven = crate::anim::driven_by(g, id);
        let ok = driven.iter().any(|d| {
            let Some(t) = g.node(d.node) else { return false };
            (!selected || targets.contains(&d.node))
                && (!referenced || t.reference_id() != 0)
                && only_ref.is_none_or(|r| t.reference_id() == r)
        });
        if ok {
            out.push(id);
        }
    }
    Ok(out)
}

/// Runs `f` with nodes in `ns` named without it, as -relativeNamespace writes them; the names come back after.
fn relative_to<R>(s: &mut Session, ns: &str, path: &str, f: impl FnOnce(&mut Session) -> R) -> R {
    let ns = ns.trim_start_matches(':').trim_end_matches(':').to_string();
    if ns.is_empty() {
        return f(s);
    }
    // Scene text: the names are taken out of the namespace as written, so nothing in the scene is renamed (a node
    // of the same name outside it would make a clash).
    if Format::of_path(path) == Format::Ma {
        let r = f(s);
        if let Ok(text) = std::fs::read_to_string(path) {
            let mut text = text;
            for lead in ["\"", "|"] {
                text = text.replace(&format!("{lead}{ns}:"), lead);
            }
            let _ = std::fs::write(path, text);
        }
        return r;
    }
    let pre = format!("{ns}:");
    let mut renamed: Vec<(NodeId, String)> = Vec::new();
    let ids: Vec<NodeId> = s.graph.nodes();
    s.graph.without_undo(|g| {
        for id in ids {
            let Some(n) = g.node(id) else { continue };
            if n.reference_id() != 0 {
                continue;
            }
            let name = n.name().to_string();
            if let Some(rest) = name.strip_prefix(&pre) {
                if g.rename(id, rest).is_ok() {
                    renamed.push((id, name));
                }
            }
        }
    });
    let r = f(s);
    s.graph.without_undo(|g| {
        for (id, name) in renamed.iter().rev() {
            let _ = g.rename(*id, name);
        }
    });
    r
}

fn export(s: &mut Session, a: &Args, path: &str) -> CmdResult {
    let ty = a.get("type").and_then(Value::as_str).filter(|t| !t.is_empty());
    let format = export_format(s, path, ty)?;
    let path = with_extension(path, format);
    let strict = a.bool("exportSelectedStrict");
    let anim = ["exportAnim", "exportSelectedAnim", "exportAnimFromReference", "exportSelectedAnimFromReference"].into_iter().find(|f| a.bool(f));
    let rfn = a.get("referenceNode").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let nodes: Vec<NodeId> = if let Some(kind) = anim {
        anim_curves(s, kind.contains("Selected"), kind.ends_with("FromReference"), rfn.as_deref())?
    } else if strict || a.bool("exportSelected") || a.bool("exportAsReference") {
        let sel = s.graph.selection().to_vec();
        if sel.is_empty() {
            fail!("file: nothing selected to export");
        }
        crate::ma_file::export_closure(s, &sel, strict)
    } else {
        s.graph.nodes().into_iter().filter(|&id| s.graph.node(id).is_some_and(|n| n.reference_id() == 0)).collect()
    };
    if a.bool("preview") {
        return Ok(names(s, &nodes));
    }
    let rel = a.string("relativeNamespace", "");
    if anim.is_some() {
        relative_to(s, &rel, &path, |s| crate::ma_file::write_anim_file(s, &path, format, &nodes)).map_err(CommandError::new)?;
        return Ok(json!(path));
    }
    let only: Option<Vec<NodeId>> = (strict || a.bool("exportSelected") || a.bool("exportAsReference")).then(|| s.graph.selection().to_vec());
    crate::ma_file::EXPORT_STRICT.with(|c| c.set(strict));
    let r = relative_to(s, &rel, &path, |s| crate::ma_file::write_file(s, &path, format, only.as_deref()));
    crate::ma_file::EXPORT_STRICT.with(|c| c.set(false));
    r.map_err(CommandError::new)?;
    if a.bool("exportAsReference") {
        // The selection goes, and comes back as a reference to the file it went to.
        let sel: Vec<String> = s.graph.selection().iter().map(|&i| s.graph.path(i)).collect();
        s.run("delete", json!({"args": sel}))?;
        let stem = Path::new(&path).file_stem().and_then(|x| x.to_str()).unwrap_or("ref").to_string();
        let ns = a.get("namespace").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string).unwrap_or(stem);
        crate::references::create(s, &path, &ns, None, false).map_err(CommandError::new)?;
        let rfn = s.extras.references.entries.last().map(|e| e.rfn.clone()).unwrap_or_default();
        return Ok(json!(rfn));
    }
    Ok(json!(path))
}

// ---------------------------------------------------------------------------------------------------------------------
// Save

fn save(s: &mut Session, a: &Args) -> Option<CmdResult> {
    let ty = a.get("type").and_then(Value::as_str).filter(|t| !t.is_empty());
    if !is_scene_type(ty) {
        return None;
    }
    let given = a.pos_str(0).map(str::to_string);
    if given.as_deref().is_some_and(|p| !is_scene_path(p)) || (given.is_none() && s.scene_path.as_deref().is_some_and(|p| !is_scene_path(p))) {
        return None;
    }
    Some((|| {
        if st(|x| x.rename_to_save) && given.is_none() {
            fail!("file: the scene has to be renamed before it can be saved");
        }
        let scene_type = ty.map(Format::of_type).transpose()?.or(s.scene_type);
        let Some(path) = given.or_else(|| s.scene_path.clone()) else { fail!("file: the scene has no name; use file -rename first") };
        let path = match scene_type {
            Some(f) => with_extension(&path, f),
            None => path,
        };
        let format = match scene_type {
            Some(f) if extension(&path).is_empty() || ty.is_some() => f,
            _ => Format::of_path(path.trim_end_matches(".gz")),
        };
        let compress = a.bool("compress");
        let path = if compress && !a.bool("preserveName") && extension(&path) != "gz" { format!("{path}.gz") } else { path };
        if crate::ma_file::save_opened_history(s, &path, format, compress).map_err(CommandError::new)? {
            return Ok(json!(path));
        }
        // Written to a file beside it first, then put in its place: a save that fails leaves the old file.
        let p = Path::new(&path);
        let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
        let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("scene").to_string();
        let temp = dir.join(format!(".{name}.tmp")).to_string_lossy().replace('\\', "/");
        st(|x| x.last_temp = temp.clone());
        crate::ma_file::write_file(s, &temp, format, None).map_err(CommandError::new)?;
        if compress {
            let raw = std::fs::read(&temp).map_err(|e| CommandError::new(format!("{temp}: {e}")))?;
            std::fs::write(&temp, gzip(&raw)).map_err(|e| CommandError::new(format!("{temp}: {e}")))?;
        }
        std::fs::rename(&temp, &path).map_err(|e| CommandError::new(format!("{path}: {e}")))?;
        s.scene_path = Some(path.clone());
        s.scene_type = Some(format);
        s.mark_saved();
        Ok(json!(path))
    })())
}

/// file -saveReference: the reference's nodes as they are now, written back to its file; its edits are then part of it.
fn save_reference(s: &mut Session, name: &str) -> CmdResult {
    let Some(at) = entry_index(s, name) else { fail!("file: no reference {name}") };
    let (id, path, ns, loaded, rfn) = {
        let e = &s.extras.references.entries[at];
        (e.id, e.path.clone(), e.namespace.clone(), e.loaded, e.rfn.clone())
    };
    if !loaded {
        fail!("file: {rfn} is not loaded");
    }
    let ids: Vec<NodeId> = s.graph.nodes().into_iter().filter(|&n| s.graph.node(n).is_some_and(|x| x.reference_id() == id && x.ty().name != crate::references::REF_NODE_TYPE)).collect();
    s.graph.without_undo(|g| ids.iter().for_each(|&n| g.set_reference(n, 0)));
    let tmp = std::env::temp_dir().join(format!("aztek_saveref_{}_{id}.ma", std::process::id())).to_string_lossy().replace('\\', "/");
    crate::ma_file::EXPORT_STRICT.with(|c| c.set(true));
    crate::ma_file::NO_REFERENCES.with(|c| c.set(true));
    let r = crate::ma_file::write_file(s, &tmp, Format::Ma, Some(&ids));
    crate::ma_file::NO_REFERENCES.with(|c| c.set(false));
    crate::ma_file::EXPORT_STRICT.with(|c| c.set(false));
    s.graph.without_undo(|g| ids.iter().for_each(|&n| g.set_reference(n, id)));
    r.map_err(CommandError::new)?;
    let mut text = std::fs::read_to_string(&tmp).map_err(|e| CommandError::new(e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    if !ns.is_empty() {
        for lead in ["\"", "|"] {
            text = text.replace(&format!("{lead}{ns}:"), lead);
        }
    }
    let bytes = if Format::of_path(&path) == Format::Mb {
        fail!("file: -saveReference writes scene text; {path} is a binary file");
    } else {
        text.into_bytes()
    };
    std::fs::write(&path, bytes).map_err(|e| CommandError::new(format!("{path}: {e}")))?;
    crate::references::rebaseline(s, at);
    Ok(json!(path))
}

// ---------------------------------------------------------------------------------------------------------------------
// Import

/// -removeDuplicateNetworks: a shading network the import brought that the scene has already (same types and
/// values, upstream too) gives its objects to the scene's and goes.
fn same_network(s: &mut Session, a: NodeId, b: NodeId, depth: u32) -> bool {
    if depth > 12 {
        return false;
    }
    let g = &mut s.graph;
    let (Some(na), Some(nb)) = (g.node(a), g.node(b)) else { return false };
    if na.ty().name != nb.ty().name {
        return false;
    }
    let count = na.ty().attrs.len();
    let sg = na.ty().name == "shadingEngine";
    let mut ups: Vec<(NodeId, NodeId)> = Vec::new();
    for i in 0..count {
        // A shading group is its shaders; its members are what differ.
        if sg && !matches!(g.node(a).unwrap().ty().attrs[i].name.as_str(), "surfaceShader" | "volumeShader" | "displacementShader") {
            continue;
        }
        let (pa, pb) = (PlugRef::new(a, i as i32), PlugRef::new(b, i as i32));
        match (g.source(&pa), g.source(&pb)) {
            (Some(x), Some(y)) => {
                if x.attr != y.attr {
                    return false;
                }
                ups.push((x.node, y.node));
            }
            (None, None) => {
                let attr = &g.node(a).unwrap().ty().attrs[i];
                if attr.flags & crate::plug::flags::OUTPUT != 0 && attr.flags & crate::plug::flags::INPUT == 0 {
                    continue;
                }
                let (va, vb) = (g.value(&pa).ok(), g.value(&pb).ok());
                if va != vb {
                    return false;
                }
            }
            _ => return false,
        }
    }
    ups.into_iter().all(|(x, y)| x == y || same_network(s, x, y, depth + 1))
}

fn upstream(g: &crate::graph::Graph, from: NodeId, within: &HashSet<NodeId>, out: &mut Vec<NodeId>) {
    if !within.contains(&from) || out.contains(&from) {
        return;
    }
    out.push(from);
    let count = g.node(from).map_or(0, |n| n.ty().attrs.len());
    for i in 0..count {
        if let Some(src) = g.source(&PlugRef::new(from, i as i32)) {
            upstream(g, src.node, within, out);
        }
    }
}

fn remove_duplicate_networks(s: &mut Session, new: &[NodeId]) -> CmdResult<()> {
    let fresh: HashSet<NodeId> = new.iter().copied().collect();
    let is_sg = |s: &Session, id: NodeId| s.graph.node(id).is_some_and(|n| n.ty().name == "shadingEngine");
    let new_sgs: Vec<NodeId> = new.iter().copied().filter(|&i| is_sg(s, i)).collect();
    let old_sgs: Vec<NodeId> = s.graph.nodes().into_iter().filter(|i| !fresh.contains(i) && is_sg(s, *i)).collect();
    for sg in new_sgs {
        let Some(twin) = old_sgs.iter().copied().find(|&o| same_network(s, sg, o, 0)) else { continue };
        let name = s.graph.path(sg);
        let members = s.run("sets", json!({"query": true, "args": [name]})).ok().map(|v| texts(Some(&v))).unwrap_or_default();
        if !members.is_empty() {
            s.run("sets", json!({"forceElement": s.graph.path(twin), "args": members}))?;
        }
        let mut doomed = Vec::new();
        upstream(&s.graph, sg, &fresh, &mut doomed);
        let doomed: Vec<String> = doomed.into_iter().filter(|&d| s.graph.node(d).is_some()).map(|d| s.graph.path(d)).collect();
        for d in doomed {
            if let Ok(Some(id)) = s.graph.find(&d) {
                let _ = s.graph.delete_node(id);
            }
        }
    }
    Ok(())
}

/// -mergeBaseAnimLayer: the file's base layer gives its layers and connections to the scene's, and goes.
fn merge_base_layer(s: &mut Session, new: &[NodeId], old_base: Option<NodeId>) -> CmdResult<()> {
    let Some(base) = old_base else { return Ok(()) };
    let g = &mut s.graph;
    let imported: Vec<NodeId> = new.iter().copied().filter(|&n| crate::anim_layers::is_layer(g, n) && crate::anim_layers::layer_parent(g, n).is_none()).collect();
    for ib in imported {
        for c in crate::anim_layers::layer_children(g, ib) {
            let from = PlugRef::new(c, crate::anim_layer_nodes::L_PARENT_LAYER as i32);
            for d in g.destinations(&from) {
                g.disconnect(&d);
            }
            g.connect(&from, &PlugRef::new(base, crate::anim_layer_nodes::L_CHILDREN as i32), false).map_err(|e| CommandError::new(e.to_string()))?;
        }
        let count = g.node(ib).map_or(0, |n| n.ty().attrs.len());
        for i in 0..count {
            let src = PlugRef::new(ib, i as i32);
            for d in g.destinations(&src) {
                if d.node == base {
                    continue;
                }
                g.disconnect(&d);
                let _ = g.connect(&PlugRef { node: base, ..src }, &d, true);
            }
        }
        g.delete_node(ib).map_err(|e| CommandError::new(e.to_string()))?;
    }
    Ok(())
}

/// -replaceName pairs on a node's last name: a pattern with * or ? replaces the whole name it matches, plain text
/// replaces where it is found.
fn replaced(name: &str, rules: &[(String, String)]) -> String {
    let (ns, leaf) = match name.rsplit_once(':') {
        Some((n, l)) => (format!("{n}:"), l.to_string()),
        None => (String::new(), name.to_string()),
    };
    let mut leaf = leaf;
    for (from, to) in rules {
        if from.contains('*') || from.contains('?') {
            if glob(from, &leaf) {
                leaf = to.clone();
            }
        } else if !from.is_empty() {
            leaf = leaf.replace(from.as_str(), to);
        }
    }
    format!("{ns}{leaf}")
}

/// A placeholder namespace's target: a reference node stands for its namespace, `:` for the root.
fn placeholder_target(s: &Session, target: &str) -> String {
    if target == ":" || target.is_empty() {
        return String::new();
    }
    match entry_index(s, target) {
        Some(at) => s.extras.references.entries[at].namespace.clone(),
        None => target.trim_start_matches(':').to_string(),
    }
}

/// The file's text with its placeholder namespaces filled in, written beside the temporary folder; None when
/// nothing changes.
fn mapped_copy(s: &Session, path: &str, maps: &[(String, String)]) -> Option<String> {
    if maps.is_empty() {
        return None;
    }
    let mut bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0x1f, 0x8b]) {
        bytes = gunzip(&bytes).ok()?;
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    let before = text.clone();
    for (ph, target) in maps {
        let ph = if ph.starts_with('<') { ph.clone() } else { format!("<{ph}>") };
        let to = placeholder_target(s, target);
        text = text.replace(&format!("{ph}:"), &if to.is_empty() { String::new() } else { format!("{to}:") });
    }
    if text == before {
        return None;
    }
    let ext = extension(path);
    let tmp = std::env::temp_dir().join(format!("aztek_mapped_{}.{}", std::process::id(), if ext.is_empty() { "ma".into() } else { ext })).to_string_lossy().replace('\\', "/");
    std::fs::write(&tmp, text).ok()?;
    Some(tmp)
}

fn import(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let Some(path) = a.pos_str(0).map(str::to_string) else { return prev(s, a) };
    let mut call = a.clone();
    let mut maps = pairs(a.get("mapPlaceHolderNamespace"));
    if let Some(t) = a.get("applyTo").and_then(Value::as_str) {
        maps.push(("<main>".into(), t.to_string()));
    }
    let mapped = mapped_copy(s, &path, &maps);
    if let Some(m) = &mapped {
        call.positional[0] = json!(m);
    }
    if let Some(r) = a.get("importTimeRange").and_then(Value::as_str) {
        if !matches!(r, "keep" | "override" | "combine") {
            fail!("file: -importTimeRange takes keep, override or combine");
        }
    }
    let old_base = crate::anim_layers::base_layer(&s.graph);
    let before: HashSet<NodeId> = s.graph.nodes().into_iter().collect();
    crate::ma_file::IMPORT_FRAME_RATE.with(|c| c.set(a.bool("importFrameRate")));
    crate::ma_file::IMPORT_TIME_RANGE.with(|c| *c.borrow_mut() = a.string("importTimeRange", ""));
    let root = a.bool("defaultNamespace").then(|| std::mem::take(&mut s.graph.current_namespace));
    let r = prev(s, &call);
    if let Some(ns) = root {
        s.graph.current_namespace = ns;
    }
    crate::ma_file::IMPORT_FRAME_RATE.with(|c| c.set(false));
    crate::ma_file::IMPORT_TIME_RANGE.with(|c| c.borrow_mut().clear());
    if let Some(m) = &mapped {
        let _ = std::fs::remove_file(m);
    }
    let r = r?;
    let new: Vec<NodeId> = s.graph.nodes().into_iter().filter(|n| !before.contains(n)).collect();
    if a.bool("defaultNamespace") {
        // The nodes keep the file's names in the root namespace; one that had to be renamed is a clash.
        let clashes: Vec<String> = crate::ma_file::last_read_names().into_iter().filter(|(id, written)| new.contains(id) && s.graph.node(*id).is_some_and(|n| n.name() != written.rsplit('|').next().unwrap_or(written))).map(|x| x.1).collect();
        if !clashes.is_empty() {
            for &id in &new {
                let _ = s.graph.delete_node(id);
            }
            fail!("file: names already in the scene: {}", clashes.join(", "));
        }
    }
    let rules = pairs(a.get("replaceName"));
    if !rules.is_empty() {
        for &id in &new {
            let Some(n) = s.graph.node(id) else { continue };
            let name = n.name().to_string();
            let to = replaced(&name, &rules);
            if to != name {
                let _ = s.graph.rename(id, &to);
            }
        }
    }
    if a.bool("mergeBaseAnimLayer") {
        merge_base_layer(s, &new, old_base)?;
    }
    if a.bool("removeDuplicateNetworks") {
        remove_duplicate_networks(s, &new)?;
    }
    Ok(match (&mapped, r) {
        (Some(_), Value::String(_)) => json!(path),
        (_, r) => r,
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Open

/// One implicit load setting: the file being opened (ID 0) or one of the references under it.
#[derive(Clone, Debug, Default)]
pub(crate) struct LoadSetting {
    /// The setting this one is under; None for the file itself.
    parent: Option<usize>,
    rfn: String,
    namespace: String,
    unresolved: String,
    resolved: String,
    deferred: bool,
}

/// A `file` line of a scene's head: its flags' values and the path, the last quoted word.
struct RefLine {
    depth: u32,
    ns: String,
    rfn: String,
    deferred: bool,
    path: String,
}

fn quoted_words(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = t.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_q => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '"' => {
                if in_q {
                    out.push(std::mem::take(&mut cur));
                }
                in_q = !in_q;
            }
            c if !in_q && (c.is_whitespace() || c == ';') => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The `file -rdi` and `file -r` lines of a scene file.
fn ref_lines(path: &str) -> (Vec<RefLine>, Vec<RefLine>) {
    let Ok(mut bytes) = std::fs::read(path) else { return (Vec::new(), Vec::new()) };
    if bytes.starts_with(&[0x1f, 0x8b]) {
        bytes = gunzip(&bytes).unwrap_or_default();
    }
    let text = String::from_utf8_lossy(&bytes);
    let (mut rdi, mut r) = (Vec::new(), Vec::new());
    for line in text.lines() {
        let t = line.trim();
        // The head ends where the nodes begin.
        if t.starts_with("createNode ") {
            break;
        }
        if !t.starts_with("file ") {
            continue;
        }
        let w = quoted_words(t);
        let val = |f: &str| w.windows(2).find(|x| x[0] == f).map(|x| x[1].clone());
        let is_rdi = w.iter().any(|x| x == "-rdi");
        if !is_rdi && !w.iter().any(|x| x == "-r" || x == "-reference") {
            continue;
        }
        let Some(path) = w.last().filter(|p| !p.starts_with('-')).cloned() else { continue };
        let line = RefLine {
            depth: val("-rdi").and_then(|d| d.parse().ok()).unwrap_or(1),
            ns: val("-ns").unwrap_or_default(),
            rfn: val("-rfn").unwrap_or_default(),
            deferred: val("-dr").is_some_and(|d| d == "1" || d == "true"),
            path,
        };
        if is_rdi {
            rdi.push(line);
        } else {
            r.push(line);
        }
    }
    (rdi, r)
}

fn stem_of(path: &str) -> String {
    Path::new(&path.replace('\\', "/")).file_stem().and_then(|x| x.to_str()).unwrap_or("reference").to_string()
}

fn find_file(s: &mut Session, written: &str, beside: &str) -> String {
    let w = written.replace('\\', "/");
    if let Some(dir) = Path::new(beside).parent() {
        let p = dir.join(&w);
        if !Path::new(&w).is_absolute() && p.is_file() {
            return p.to_string_lossy().replace('\\', "/");
        }
        if let Some(name) = Path::new(&w).file_name() {
            let p = dir.join(name);
            if !Path::new(&w).is_file() && p.is_file() {
                return p.to_string_lossy().replace('\\', "/");
            }
        }
    }
    crate::references::resolve_written(s, &w).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or(w)
}

/// The references of a file one level down, each with what is under it read from its own file.
fn add_from_file(s: &mut Session, out: &mut Vec<LoadSetting>, parent: usize, file: &str, depth: u32) {
    if depth > 32 {
        return;
    }
    let (_, lines) = ref_lines(file);
    let above = out[parent].namespace.clone();
    let join = |n: &str| if above.is_empty() { n.to_string() } else { format!("{above}:{n}") };
    for l in lines {
        let resolved = find_file(s, &l.path, file);
        let ns = if l.ns == ":" { above.clone() } else if l.ns.is_empty() { join(&stem_of(&l.path)) } else { join(&l.ns) };
        let rfn = if l.rfn.is_empty() { format!("{ns}RN") } else { join(&l.rfn) };
        out.push(LoadSetting { parent: Some(parent), rfn, namespace: ns, unresolved: l.path.clone(), resolved: resolved.clone(), deferred: l.deferred });
        let me = out.len() - 1;
        add_from_file(s, out, me, &resolved, depth + 1);
    }
}

/// file -buildLoadSettings: the file, then every reference under it in pre-order. A scene the reference saved lists
/// them all with their depth and load state (`file -rdi`); otherwise each referenced file is read for its own.
fn build_load_settings(s: &mut Session, path: &str) -> Vec<LoadSetting> {
    let mut out = vec![LoadSetting { parent: None, unresolved: path.to_string(), resolved: path.to_string(), ..Default::default() }];
    let (rdi, _) = ref_lines(path);
    if rdi.is_empty() {
        add_from_file(s, &mut out, 0, path, 0);
        return out;
    }
    // Pre-order with depths: each line's parent is the last line one level up.
    let mut stack: Vec<usize> = vec![0];
    for l in rdi {
        let depth = l.depth.max(1) as usize;
        stack.truncate(depth);
        let parent = *stack.last().unwrap_or(&0);
        let above = out[parent].namespace.clone();
        let join = |n: &str| if above.is_empty() { n.to_string() } else { format!("{above}:{n}") };
        let ns = if l.ns == ":" { above.clone() } else if l.ns.is_empty() { join(&stem_of(&l.path)) } else { join(&l.ns) };
        let beside = out[parent].resolved.clone();
        let resolved = find_file(s, &l.path, &beside);
        let rfn = if l.rfn.is_empty() { format!("{ns}RN") } else { l.rfn.clone() };
        out.push(LoadSetting { parent: Some(parent), rfn, namespace: ns, unresolved: l.path.clone(), resolved, deferred: l.deferred });
        stack.push(out.len() - 1);
    }
    out
}

fn descendants(list: &[LoadSetting], of: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, l) in list.iter().enumerate() {
        let mut p = l.parent;
        while let Some(x) = p {
            if x == of {
                out.push(i);
                break;
            }
            p = list[x].parent;
        }
    }
    out
}

/// selLoadSettings: the implicit load settings -buildLoadSettings made, by ID: 0 the file, then its references.
fn sel_load_settings(_s: &mut Session, a: &Args) -> CmdResult {
    let ids: Vec<usize> = a
        .positional
        .iter()
        .filter_map(|v| v.as_u64().map(|n| n as usize).or_else(|| v.as_f64().map(|f| f as usize)).or_else(|| v.as_str().and_then(|t| t.trim().parse().ok())))
        .collect();
    let mut list = st(|x| x.load_settings.clone());
    if list.is_empty() {
        list.push(LoadSetting::default());
    }
    let n = list.len();
    if let Some(bad) = ids.iter().find(|i| **i >= n) {
        fail!("selLoadSettings: no load setting {bad}; there are {n}");
    }
    let q: Vec<String> = a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let wants = |f: &str| q.iter().any(|x| x == f) || (a.bool("query") && a.get(f).and_then(Value::as_bool) == Some(true));
    let edit = a.bool("edit");
    let query = a.bool("query") || (!edit && ["numSettings", "fileName", "referenceNode", "proxyManager", "proxyTag", "proxySetTags", "proxySetFiles"].iter().any(|f| a.get(f).is_some_and(|v| v.as_bool() == Some(true))));
    if query {
        if wants("numSettings") || a.has("numSettings") && q.is_empty() {
            return Ok(json!(n));
        }
        if ids.is_empty() {
            fail!("selLoadSettings: give the load setting IDs to query");
        }
        let proxy = |l: &LoadSetting| st(|x| x.proxies.get(&l.rfn).cloned());
        let each = |f: &dyn Fn(&LoadSetting) -> Value| -> Value {
            let v: Vec<Value> = ids.iter().map(|&i| f(&list[i])).collect();
            if v.len() == 1 {
                v.into_iter().next().unwrap()
            } else {
                Value::Array(v)
            }
        };
        let short = a.bool("shortName") || q.iter().any(|x| x == "shortName");
        let unresolved = a.bool("unresolvedName") || q.iter().any(|x| x == "unresolvedName");
        if wants("fileName") {
            return Ok(each(&|l| {
                let f = if unresolved { &l.unresolved } else { &l.resolved };
                json!(if short { f.rsplit(['/', '\\']).next().unwrap_or(f).to_string() } else { f.clone() })
            }));
        }
        if wants("deferReference") {
            return Ok(each(&|l| json!(l.deferred)));
        }
        if wants("referenceNode") {
            return Ok(each(&|l| json!(l.rfn)));
        }
        if wants("proxyManager") {
            return Ok(each(&|l| json!(proxy(l).map(|p| p.0).unwrap_or_default())));
        }
        if wants("proxyTag") {
            return Ok(each(&|l| json!(proxy(l).map(|p| p.1).unwrap_or_default())));
        }
        if wants("activeProxy") {
            return Ok(each(&|l| json!(proxy(l).map(|p| if p.2.is_empty() { p.1 } else { p.2 }).unwrap_or_default())));
        }
        // A reference with no proxies is a set of one: itself, under no tag.
        if wants("proxySetTags") {
            return Ok(each(&|l| json!([proxy(l).map(|p| p.1).unwrap_or_default()])));
        }
        if wants("proxySetFiles") {
            return Ok(each(&|l| json!([if short { l.resolved.rsplit(['/', '\\']).next().unwrap_or(&l.resolved).to_string() } else { l.resolved.clone() }])));
        }
        fail!("selLoadSettings: name what to query");
    }
    if ids.is_empty() {
        fail!("selLoadSettings: give the load setting IDs to edit");
    }
    let mut warnings = Vec::new();
    if let Some(v) = a.get("deferReference").filter(|v| !v.is_null()) {
        let defer = v.as_bool().unwrap_or(v.as_f64().is_some_and(|x| x != 0.0) || v.as_str().is_some_and(|t| matches!(t, "true" | "1" | "on")));
        for &i in &ids {
            if i == 0 {
                warnings.push("selLoadSettings: the file being opened is always loaded".to_string());
                continue;
            }
            if !defer {
                if let Some(p) = list[i].parent.filter(|&p| p != 0 && list[p].deferred) {
                    warnings.push(format!("selLoadSettings: {} can't load while {} is unloaded", list[i].rfn, list[p].rfn));
                    continue;
                }
            }
            // Children follow their parent.
            list[i].deferred = defer;
            for d in descendants(&list, i) {
                list[d].deferred = defer;
            }
        }
    }
    if let Some(tag) = a.get("activeProxy").and_then(Value::as_str) {
        for &i in &ids {
            let rfn = list[i].rfn.clone();
            st(|x| {
                let e = x.proxies.entry(rfn).or_default();
                e.2 = tag.to_string();
            });
        }
    }
    st(|x| {
        if !x.load_settings.is_empty() {
            x.load_settings = list;
        }
    });
    for w in warnings {
        log::warn!("{w}");
    }
    Ok(Value::Null)
}

fn open(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let path = a.pos_str(0).map(str::to_string);
    if a.bool("buildLoadSettings") {
        let Some(p) = path else { fail!("file: -buildLoadSettings needs a file") };
        if !Path::new(&p).is_file() {
            fail!("file: {p} not found");
        }
        let list = build_load_settings(s, &p);
        st(|x| x.load_settings = list);
        return Ok(json!(p));
    }
    use crate::references::LoadMode;
    let mode = if a.bool("loadNoReferences") {
        Some(LoadMode::None)
    } else if a.bool("loadAllReferences") || (a.has("loadAllDeferred") && a.bool("loadAllDeferred")) {
        Some(LoadMode::All)
    } else if let Some(d) = a.get("loadReferenceDepth").and_then(Value::as_str) {
        match d {
            "all" => Some(LoadMode::All),
            "none" => Some(LoadMode::None),
            "topOnly" => Some(LoadMode::TopOnly),
            _ => None,
        }
    } else if a.string("loadSettings", "") == "implicitLoadSettings" {
        Some(LoadMode::Map(st(|x| x.load_settings.iter().skip(1).map(|l| (l.rfn.clone(), l.deferred)).collect())))
    } else {
        None
    };
    if let Some(ls) = a.get("loadSettings").and_then(Value::as_str) {
        if !ls.is_empty() && ls != "implicitLoadSettings" {
            fail!("file: no load settings named {ls}");
        }
    }
    crate::references::LOAD_MODE.with(|m| *m.borrow_mut() = mode);
    let r = prev(s, a);
    crate::references::LOAD_MODE.with(|m| *m.borrow_mut() = None);
    let r = r?;
    if a.bool("reserveNamespaces") {
        let spaces: Vec<String> = s.extras.references.entries.iter().filter(|e| !e.loaded && !e.namespace.is_empty()).map(|e| e.namespace.clone()).collect();
        for ns in spaces {
            s.graph.set_namespace_known(&ns, true);
        }
    }
    Ok(r)
}

// ---------------------------------------------------------------------------------------------------------------------
// The command

fn select_all(s: &mut Session, a: &Args) -> CmdResult {
    let path = a.pos_str(0).map(str::to_string);
    let main = path.is_none() || path == s.scene_path;
    let g = &s.graph;
    let ids: Vec<NodeId> = if main {
        g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().dag && !(n.namespace().is_empty() && crate::defaults::is_default_node(n.name())) && !crate::cameras::is_startup(g, crate::dag::transform_of(g, id)))).collect()
    } else {
        let p = path.unwrap();
        let Some(at) = entry_index(s, &p) else { fail!("file: {p} is not in the scene") };
        let ids = entry_and_below(s, at);
        g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| ids.contains(&n.reference_id()) && n.ty().name != crate::references::REF_NODE_TYPE)).collect()
    };
    s.graph.select(&ids, if a.bool("add") { SelectMode::Add } else { SelectMode::Replace });
    Ok(names(s, &ids))
}

fn settings(s: &mut Session, a: &Args) -> CmdResult<bool> {
    let mut any = false;
    if a.has("defaultExtensions") {
        let on = a.bool("defaultExtensions");
        st(|x| x.default_extensions = on);
        crate::ma_file::DEFAULT_EXTENSIONS.with(|c| c.set(on));
        any = true;
    }
    if a.has("renameToSave") {
        let on = a.bool("renameToSave");
        st(|x| x.rename_to_save = on);
        any = true;
    }
    if let Some(v) = a.get("saveDiskCache").and_then(Value::as_str) {
        if !matches!(v, "always" | "never") {
            fail!("file: -saveDiskCache takes always or never");
        }
        st(|x| x.save_disk_cache = v.to_string());
        any = true;
    }
    if let Some(v) = a.get("saveTextures").and_then(Value::as_str) {
        if !matches!(v, "always" | "unlessRef" | "never") {
            fail!("file: -saveTextures takes always, unlessRef or never");
        }
        st(|x| x.save_textures = v.to_string());
        any = true;
    }
    if a.has("uiConfiguration") {
        let on = a.bool("uiConfiguration");
        st(|x| x.ui_configuration = on);
        any = true;
    }
    if a.has("uiLoadConfiguration") {
        let on = a.bool("uiLoadConfiguration");
        st(|x| x.ui_load_configuration = on);
        any = true;
    }
    if a.has("command") {
        let c = texts(a.get("command"));
        st(|x| x.command = c);
        any = true;
    }
    if a.has("lockFile") {
        set_file_info(s, "lockFile", a.bool("lockFile"));
        any = true;
    }
    if a.has("lockContainerUnpublished") {
        set_file_info(s, "lockContainerUnpublished", a.bool("lockContainerUnpublished"));
        any = true;
    }
    if a.has("mapPlaceHolderNamespace") && a.bool("edit") {
        let p = pairs(a.get("mapPlaceHolderNamespace"));
        st(|x| x.place_holders.extend(p));
        any = true;
    }
    if a.has("replaceName") && a.bool("edit") {
        let p = pairs(a.get("replaceName"));
        st(|x| x.replace_names.extend(p));
        any = true;
    }
    any |= a.bool("moveSelected");
    Ok(any)
}

const ACTIONS: &[&str] = &[
    "new", "open", "save", "rename", "import", "exportAll", "exportSelected", "exportSelectedStrict", "exportAnim", "exportSelectedAnim", "exportAnimFromReference", "exportSelectedAnimFromReference", "exportAsReference", "reference",
    "loadReference", "unloadReference", "removeReference", "importReference", "cleanReference", "flushReference", "saveReference", "selectAll", "buildLoadSettings", "loadReferencePreview",
];

fn run(s: &mut Session, given: &Args, prev: &RunFn<Session>) -> CmdResult {
    let mut a = given.clone();
    if let Some(v) = a.flags.remove("newFile") {
        a.flags.insert("new".into(), v);
    }
    if a.bool("query") {
        if let Some(r) = query(s, &a) {
            return r;
        }
        let r = prev(s, &a)?;
        if asked(&a, "namespace") && a.bool("absoluteName") {
            if let Some(ns) = r.as_str() {
                return Ok(json!(format!(":{}", ns.trim_start_matches(':'))));
            }
        }
        return Ok(r);
    }
    if let Some(o) = a.get("options").and_then(Value::as_str) {
        let o = o.to_string();
        st(|x| x.last_option = o);
    }
    let action = ACTIONS.iter().any(|f| a.bool(f) || (a.has(f) && !matches!(a.get(f), Some(Value::Bool(_)))));
    if a.bool("resetError") {
        st(|x| x.error = false);
        if !action {
            return Ok(Value::Null);
        }
    }
    let set = settings(s, &a)?;
    if set && !action {
        return Ok(Value::Null);
    }
    let r = act(s, &a, prev);
    if r.is_err() && ["open", "import", "reference", "loadReference"].iter().any(|f| a.has(f) && a.get(f) != Some(&Value::Bool(false))) {
        st(|x| x.error = true);
    }
    r
}

fn act(s: &mut Session, a: &Args, prev: &RunFn<Session>) -> CmdResult {
    let path = a.pos_str(0).map(str::to_string);
    // -strict: a relative path is the project's, and is looked for nowhere else.
    if a.bool("strict") {
        if let Some(p) = &path {
            if !Path::new(p).is_absolute() && (a.bool("open") || a.bool("import") || a.bool("reference") || a.bool("importReference")) {
                let root = crate::shading::project_dir(s).unwrap_or_else(|| std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default());
                let full = PathBuf::from(&root).join(p);
                if !full.is_file() {
                    fail!("file: {p} not found in the project {root}");
                }
                let mut call = a.clone();
                call.flags.remove("strict");
                call.positional[0] = json!(full.to_string_lossy().replace('\\', "/"));
                crate::references::STRICT_PATHS.with(|c| c.set(true));
                let r = act(s, &call, prev);
                crate::references::STRICT_PATHS.with(|c| c.set(false));
                return r;
            }
        }
    }
    if a.bool("selectAll") {
        return select_all(s, a);
    }
    if a.has("saveReference") || (a.bool("saveReference")) {
        let name = a.get("referenceNode").and_then(Value::as_str).map(str::to_string).or(path).unwrap_or_default();
        return save_reference(s, &name);
    }
    let anim = ["exportAnim", "exportSelectedAnim", "exportAnimFromReference", "exportSelectedAnimFromReference", "exportSelectedStrict", "exportAsReference"].iter().any(|f| a.bool(f));
    let exporting = anim || a.bool("exportAll") || a.bool("exportSelected");
    let plain_type = is_scene_type(a.get("type").and_then(Value::as_str).filter(|t| !t.is_empty()));
    if exporting && (anim || ((a.bool("preview") || a.has("relativeNamespace")) && plain_type)) {
        let Some(p) = path else { fail!("file: export needs a path") };
        if a.has("relativeNamespace") && a.bool("preserveReferences") {
            fail!("file: -relativeNamespace can't be used with -preserveReferences");
        }
        return export(s, a, &p);
    }
    if a.bool("save") {
        if let Some(r) = save(s, a) {
            return r;
        }
    }
    if a.bool("open") {
        return open(s, a, prev);
    }
    if a.bool("import") {
        return import(s, a, prev);
    }
    if a.bool("reference") {
        let mut call = a.clone();
        if let Some(p) = &path {
            // A file saved with -lockFile comes in locked; with -lockContainerUnpublished its containers' members that
            // aren't published are locked.
            let info = file_info_of(p);
            if info.iter().any(|(k, v)| k == "lockFile" && v == "1") {
                call.flags.insert("lockReference".into(), json!(true));
            }
            let before: HashSet<NodeId> = s.graph.nodes().into_iter().collect();
            let r = prev(s, &call)?;
            let new: Vec<NodeId> = s.graph.nodes().into_iter().filter(|n| !before.contains(n)).collect();
            if info.iter().any(|(k, v)| k == "lockContainerUnpublished" && v == "1") {
                lock_unpublished(s, &new);
            }
            let rules = pairs(a.get("replaceName"));
            if !rules.is_empty() {
                for &id in &new {
                    let Some(n) = s.graph.node(id) else { continue };
                    if n.ty().name == crate::references::REF_NODE_TYPE {
                        continue;
                    }
                    let name = n.name().to_string();
                    let to = replaced(&name, &rules);
                    if to != name {
                        let _ = s.graph.rename(id, &to);
                    }
                }
            }
            if a.has("proxyManager") || a.has("proxyTag") || a.has("activeProxy") {
                if a.has("proxyTag") && !a.has("proxyManager") {
                    fail!("file: -proxyTag needs -proxyManager");
                }
                let rfn = s.extras.references.entries.last().map(|e| e.rfn.clone()).unwrap_or_default();
                let tag = a.string("proxyTag", "");
                if let Ok(p) = s.graph.plug(&format!("{rfn}.proxyTag")) {
                    let _ = s.graph.without_undo(|g| g.set_value(p, PlugValue::String(tag.clone())));
                }
                st(|x| x.proxies.insert(rfn, (a.string("proxyManager", ""), tag, a.string("activeProxy", ""))));
            }
            return Ok(r);
        }
    }
    if a.bool("removeReference") && (a.bool("mergeNamespaceWithParent") || a.bool("mergeNamespaceWithRoot")) {
        let name = path.clone().or_else(|| a.get("referenceNode").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        let Some(at) = entry_index(s, &name) else { fail!("no reference {name}") };
        let ns = s.extras.references.entries[at].namespace.clone();
        if ns.is_empty() {
            fail!("file: {name} is in the root namespace; there is no namespace to merge");
        }
        let r = prev(s, a)?;
        if crate::ref_scene::namespace_exists(&s.graph, &ns) {
            let flag = if a.bool("mergeNamespaceWithRoot") { "mergeNamespaceWithRoot" } else { "mergeNamespaceWithParent" };
            s.run("namespace", json!({"removeNamespace": format!(":{ns}"), flag: true}))?;
        }
        return Ok(r);
    }
    prev(s, a)
}

/// The members of the given containers that aren't published, locked.
fn lock_unpublished(s: &mut Session, among: &[NodeId]) {
    let g = &mut s.graph;
    for &c in among {
        if !g.node(c).is_some_and(|n| n.ty().name == "container") {
            continue;
        }
        let published: Vec<String> = g.plug(&format!("{}.publishedNode", g.path(c))).ok().map(|p| g.elements(&p)).unwrap_or_default().into_iter().filter_map(|e| g.value(&e).ok()).filter_map(|v| if let PlugValue::String(t) = v { Some(t) } else { None }).collect();
        for (_, m) in crate::rig_misc::container_members(g, c) {
            if m == NO_NODE {
                continue;
            }
            let name = g.path(m);
            if !published.iter().any(|p| *p == name || name.ends_with(&format!("|{p}")) || name.ends_with(&format!(":{p}"))) {
                g.set_node_locked(m, true);
            }
        }
    }
}

pub fn register(c: &mut Commands<Session>) {
    register_sel_load_settings(c);
    let Some(old) = c.find("file").cloned() else { return };
    let mut def = old.clone();
    let prev = def.run.clone();
    let b = |n: &str, sh: &str, doc: &str| FlagDef::new(n, sh, ArgType::Bool, Value::Null, doc);
    let t = |n: &str, sh: &str, doc: &str| FlagDef::new(n, sh, ArgType::String, Value::Null, doc);
    let p = |n: &str, sh: &str, doc: &str| FlagDef::new(n, sh, ArgType::Any, Value::Null, doc).multi(2);
    // -new becomes -newFile, as the reference names it; -new is its short name.
    def.flags.retain(|f| f.name != "new");
    let flags = vec![
        b("newFile", "new", "A new, empty scene."),
        b("absoluteName", "an", "Namespaces as full paths from the root, :a:b."),
        t("activeProxy", "ap", "With -reference: the proxy that is active."),
        b("add", "add", "With -selectAll: add to the selection rather than replace it."),
        b("anyModified", "amf", "Query: whether the scene has edits (as -modified)."),
        t("applyTo", "at", "Importing or referencing an edits file: apply it to this reference node; : is the root namespace."),
        b("buildLoadSettings", "bls", "With -open: read only which references the file holds, as implicitLoadSettings, without opening it."),
        p("command", "c", "A script and its type, run before each file operation. Query: what they are."),
        b("compress", "cmp", "With -save: gzip the file (adding .gz unless -preserveName)."),
        b("copyNumberList", "cnl", "Query: a number for each time the file is in use."),
        b("defaultExtensions", "de", "Whether a name with no extension takes the type's on save (on, for the session)."),
        b("defaultNamespace", "dns", "Import into the root namespace with no clash handling: a name the scene has is an error."),
        b("errorStatus", "err", "Query: whether the last file read failed."),
        b("executeScriptNodes", "esn", "Whether the file's script nodes run when it opens (on)."),
        b("exportAnim", "ean", "Export the scene's animation curves, connected by name to what they drive."),
        b("exportAnimFromReference", "ear", "Export the animation on referenced nodes; -rfn picks one reference."),
        b("exportAsReference", "er", "Export the selection to the file and bring it back as a reference to it. Returns the reference node."),
        b("exportSelectedAnim", "eas", "Export the animation on the selected objects."),
        b("exportSelectedAnimFromReference", "esa", "Export the animation on the selected referenced objects; -rfn picks one reference."),
        b("exportSelectedStrict", "ess", "Export the selected nodes and nothing they need."),
        p("exportSnapshotCallback", "esc", "A script and a namespace: export copies of what is exported, made in that namespace, after the script has run on them."),
        b("fileMetaData", "fmd", "Query: a Broadcast Wave file's time reference in samples; 0 for other files."),
        b("importFrameRate", "ifr", "With -import: the file's frame rate becomes the scene's."),
        t("importTimeRange", "itr", "With -import: keep, override or combine the playback range with the file's."),
        b("lastFileOption", "lfo", "Query: the last -options string given."),
        b("lastTempFile", "ltf", "Query: the temporary file the last save wrote first."),
        b("loadAllDeferred", "lad", "With -open: on loads every reference, deferred ones too."),
        b("loadAllReferences", "lar", "With -open: load every reference."),
        b("loadNoReferences", "lnr", "With -open: load no references."),
        t("loadSettings", "ls", "With -open: the reference load settings to use: empty for the file's, or implicitLoadSettings."),
        b("location", "loc", "Query: where the file is."),
        b("lockContainerUnpublished", "lcu", "Saved with the scene: a file referencing it locks its containers' unpublished members."),
        b("lockFile", "lf", "Saved with the scene: a file referencing it locks all of it."),
        p("mapPlaceHolderNamespace", "mns", "With -import or -reference: a placeholder namespace (<name>) and the reference node it stands for; : is the root."),
        b("mergeBaseAnimLayer", "mbl", "With -import: the file's base animation layer merges into the scene's."),
        b("mergeNamespaceWithParent", "mnp", "With -removeReference: what is left in its namespace goes to the parent namespace."),
        b("mergeNamespaceWithRoot", "mnr", "With -removeReference: what is left in its namespace goes to the root namespace."),
        b("moveSelected", "ms", "Obsolete; does nothing."),
        b("parentNamespace", "pns", "Query: a referenced file's parent namespace."),
        t("postSaveScript", "pos", "With -save: a script run after the save."),
        t("preSaveScript", "prs", "With -save: a script run before the save."),
        b("preserveName", "pn", "With -compress: keep the name as given, with no .gz."),
        b("preview", "pv", "With an export flag: the nodes it would write, and nothing written."),
        t("proxyManager", "pm", "With -reference: the proxy manager node of the file's proxies."),
        t("proxyTag", "pt", "With -reference and -proxyManager: the proxy's name in its set."),
        t("relativeNamespace", "rns", "With an export: nodes in this namespace are written without it."),
        b("removeDuplicateNetworks", "rdn", "With -import: shading networks the scene has already are not added."),
        b("renameToSave", "rts", "Whether a save needs the scene renamed first. Query: whether it does."),
        b("renamingPrefixList", "rpl", "Query: the renaming prefixes the file's references use."),
        p("replaceName", "rep", "With -import or -reference: a name pattern and what it becomes."),
        b("reserveNamespaces", "rvn", "With -open: the namespaces of unloaded references are made."),
        b("resetError", "rer", "Clear the error status."),
        t("saveDiskCache", "sdc", "always or never: whether disk caches are copied on save as. Query: which."),
        b("saveReference", "sr", "Write a reference's nodes as they are now back to its file."),
        t("saveTextures", "stx", "always, unlessRef or never: whether painted textures are saved on save as. Query: which."),
        b("selectAll", "sa", "Select everything the file (and the files in it) brought in."),
        b("strict", "str", "With -open, -import, -importReference or -reference: a relative path is the project's only."),
        b("uiConfiguration", "uc", "Whether panels are saved with the scene (on). Query: whether they are."),
        b("uiLoadConfiguration", "ulc", "Whether panels saved with a scene are read (on). Query: whether they are."),
        b("writable", "w", "Query: whether the file can be written: the scene's file by the file system, a referenced one never."),
    ];
    for fl in flags {
        def.flags.retain(|o| o.name != fl.name);
        def.flags.push(fl);
    }
    def.run = std::sync::Arc::new(move |s: &mut Session, a: &Args| run(s, a, &prev));
    c.register(def);
}

#[cfg(test)]
mod tests {
    #[test]
    fn gzip_round_trip() {
        let text = b"//Maya ASCII 2024 scene\ncreateNode transform -n \"a\";\n".repeat(50);
        let z = super::gzip(&text);
        assert_eq!(super::gunzip(&z).unwrap(), text);
    }
}

fn register_sel_load_settings(c: &mut Commands<Session>) {
    use aztek_ops::CommandDef;
    let f = |n: &str, sh: &str, ty: ArgType, doc: &str| FlagDef::new(n, sh, ty, Value::Null, doc);
    c.register(
        CommandDef::new(
            "selLoadSettings",
            "The implicit load settings of the file file -buildLoadSettings read, for the Preload Reference Editor: ID 0 is the file, then its references in pre-order. -e -deferReference sets which references the next file -open -loadSettings implicitLoadSettings loads; children follow their parent.",
            sel_load_settings,
        )
        .undoable(false)
        .positional("load setting IDs")
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("edit", "e", ArgType::Bool, "Edit mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode."))
        .flag(f("activeProxy", "ap", ArgType::String, "The tag of the proxy set's active proxy; in edit mode, makes the proxy of that tag active."))
        .flag(f("deferReference", "dr", ArgType::Bool, "Whether the reference stays unloaded."))
        .flag(f("fileName", "fn", ArgType::String, "The file of each load setting, resolved unless -unresolvedName."))
        .flag(f("numSettings", "ns", ArgType::Int, "How many load settings: the references plus one."))
        .flag(f("proxyManager", "pm", ArgType::String, "The proxy manager of each load setting."))
        .flag(f("proxySetFiles", "psf", ArgType::String, "The files of the proxies in each load setting's proxy set."))
        .flag(f("proxySetTags", "pst", ArgType::String, "The tags of the proxies in each load setting's proxy set."))
        .flag(f("proxyTag", "pt", ArgType::String, "The proxy tag of each load setting."))
        .flag(f("referenceNode", "rfn", ArgType::String, "The reference node of each load setting."))
        .flag(f("shortName", "shn", ArgType::Bool, "With -fileName: the file's name only."))
        .flag(f("unresolvedName", "un", ArgType::Bool, "With -fileName: the name as the reference was made, found or not.")),
    );
}

/// The load settings as the Preload Reference Editor shows them: ID, parent ID (-1 for the file), name, file, unloaded.
pub fn load_settings_rows() -> Value {
    Value::Array(
        st(|x| x.load_settings.clone())
            .iter()
            .enumerate()
            .map(|(i, l)| json!({"id": i, "parent": l.parent.map_or(-1, |p| p as i64), "node": l.rfn, "file": l.resolved, "unloaded": l.deferred}))
            .collect(),
    )
}
