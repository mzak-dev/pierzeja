//! Arkusze review modeli 3D (ProceduraSuite): każdy model z wielu widoków, z podpisami.
//! `cargo run -p pierzeja-render --example review --release -- <tabor|tlum> <lista.json> <katalog> [nazwa…]`
//! `lista.json` = { "nazwa": spec, … } (TaborSpec albo TlumSpec bez `pose`). Wynik w `<katalog>/<nazwa>/`:
//! `NN-widok.png` (1000×600) i `arkusz.png` (wszystkie widoki, 3 kolumny). Pojazd: widoki ortogonalne,
//! 3/4, z góry, zbliżenia przodu i tyłu, kabina z załogą (szyby ukryte) i skala z człowiekiem 1,78 m.
//! Człowiek: stoi (przód, bok, tył, 3/4), idzie, siedzi, prowadzi, zbliżenia głowy.
use pierzeja_render::{render_scene, scene_bounds, Image, RenderItem, RenderOptions, View};
use pierzeja_core::{Bounds, MatKey, MeshData};
use serde_json::{json, Value};
use tabor_core::{generate as vehicle, TaborSpec, VehicleModel};
use tlum_core::{generate as human, HumanModel, TlumSpec};

const TW: u32 = 1000;
const TH: u32 = 600;

/// Widok: podpis, azymut (0 = od przodu, 90 = z lewej), elewacja, kadr (None = całość).
struct Shot {
    label: &'static str,
    az: f64,
    el: f64,
    focus: Option<Bounds>,
    items: Vec<usize>,
}

fn person(spec: Value) -> HumanModel {
    human(&serde_json::from_value::<TlumSpec>(spec).expect("TlumSpec"))
}

fn without_glass(m: &MeshData) -> MeshData {
    let mut c = m.clone();
    c.parts.retain(|p| !matches!(p.material, Some(MatKey::Glass)));
    c
}

fn bx(min: [f64; 3], max: [f64; 3]) -> Bounds {
    Bounds { min, max }
}

fn tabor(spec: &Value, dir: &str) {
    let m: VehicleModel = vehicle(&serde_json::from_value::<TaborSpec>(spec.clone()).expect("TaborSpec"));
    let b = m.bounds();
    // załoga: kierowca w pozie `drive`, reszta `sit`; biodro na punkcie Seat_N
    // kierowca `drive`, pasażerowie `ride`; tylne rzędy kabiny brygadowej siedzą jak na krześle (`sit`)
    let heavy = m.stats.body_type == "podwozie";
    let pose = |i: usize| match i {
        0 => "drive",
        _ if heavy && m.seats[i][2] < m.seats[0][2] - 0.3 => "sit",
        _ => "ride",
    };
    let crew: Vec<HumanModel> = (0..m.seats.len()).map(|i| person(json!({ "archetype": "przechodzien_dorosly", "seed": 11 + i, "pose": pose(i) }))).collect();
    let walker = person(json!({ "archetype": "przechodzien_dorosly", "seed": 5 }));
    let open = without_glass(&m.body);

    // 0 = nadwozie, 1..=4 koła, 5 belka; dalej nadwozie bez szyb, załoga, człowiek obok
    let mut meshes: Vec<(&MeshData, [f64; 3])> = vec![(&m.body, [0.0; 3])];
    meshes.extend(m.wheels.iter().map(|(_, p)| (&m.wheel, *p)));
    let base: Vec<usize> = (0..meshes.len()).collect();
    if let Some((l, o)) = &m.lightbar {
        meshes.push((l, *o));
    }
    let with_bar: Vec<usize> = (0..meshes.len()).collect();
    let i_open = meshes.len();
    meshes.push((&open, [0.0; 3]));
    let mut cabin: Vec<usize> = with_bar.iter().copied().filter(|&i| i != 0).collect();
    cabin.push(i_open);
    for (h, s) in crew.iter().zip(&m.seats) {
        cabin.push(meshes.len());
        meshes.push((&h.body, [s[0] - h.hip[0], s[1] - h.hip[1], s[2] - h.hip[2]]));
    }
    let mut scale = with_bar.clone();
    scale.push(meshes.len());
    meshes.push((&walker.body, [b.max[0] + 0.45, 0.0, b.max[2] - 0.6]));
    let _ = base;

    let (len, hgt) = (b.max[2] - b.min[2], b.max[1] - b.min[1]);
    let near = 1.4f64.min(0.4 * len);
    let front = bx([b.min[0] - 0.05, 0.0, b.max[2] - near], [b.max[0] + 0.05, (hgt * 0.75).max(1.1), b.max[2] + 0.05]);
    let rear = bx([b.min[0] - 0.05, 0.0, b.min[2] - 0.05], [b.max[0] + 0.05, (hgt * 0.75).max(1.1), b.min[2] + near]);
    // kabina: przednie 45% długości osobówki/vana, 40% ciężkiego
    let cab_len = if m.stats.body_type == "podwozie" { 3.6f64.min(0.45 * len) } else { 0.62 * len };
    let cab = bx([b.min[0], 0.0, b.max[2] - cab_len], [b.max[0], b.max[1], b.max[2]]);
    let shots = vec![
        Shot { label: "PRZOD", az: 0.0, el: 3.0, focus: None, items: with_bar.clone() },
        Shot { label: "BOK LEWY (KIEROWCA, +X)", az: 90.0, el: 3.0, focus: None, items: with_bar.clone() },
        Shot { label: "TYL", az: 180.0, el: 5.0, focus: None, items: with_bar.clone() },
        Shot { label: "BOK PRAWY", az: 270.0, el: 3.0, focus: None, items: with_bar.clone() },
        Shot { label: "3/4 PRZOD Z GORY", az: 38.0, el: 24.0, focus: None, items: with_bar.clone() },
        Shot { label: "3/4 TYL", az: 218.0, el: 16.0, focus: None, items: with_bar.clone() },
        Shot { label: "Z GORY", az: 20.0, el: 72.0, focus: None, items: with_bar.clone() },
        Shot { label: "ZBLIZENIE PRZOD", az: 28.0, el: 9.0, focus: Some(front), items: with_bar.clone() },
        Shot { label: "ZBLIZENIE TYL", az: 205.0, el: 9.0, focus: Some(rear), items: with_bar.clone() },
        Shot { label: "KABINA Z ZALOGA (BEZ SZYB)", az: 75.0, el: 12.0, focus: Some(cab), items: cabin },
        Shot { label: "SKALA: CZLOWIEK 1,78 M", az: 60.0, el: 6.0, focus: None, items: scale },
        Shot { label: "PRZOD NISKO 3/4 PRAWY", az: 322.0, el: 2.0, focus: None, items: with_bar },
    ];
    write(dir, &shots, &meshes);
    std::fs::write(format!("{dir}/stats.json"), serde_json::to_string_pretty(&json!({ "stats": m.stats, "seats": m.seats, "wheels": m.wheels.iter().map(|(n, p)| json!({ "name": n, "pos": p })).collect::<Vec<_>>(), "lightbar": m.lightbar.as_ref().map(|l| l.1), "bounds": [b.min, b.max] })).unwrap()).unwrap();
    println!("{dir}: {}", serde_json::to_string(&m.stats).unwrap());
}

fn tlum(spec: &Value, dir: &str) {
    let pose = |p: &str, phase: f64| {
        let mut s = spec.clone();
        s["pose"] = json!(p);
        s["phase"] = json!(phase);
        person(s)
    };
    let models = [pose("stand", 0.0), pose("walk", 0.25), pose("sit", 0.0), pose("drive", 0.0), pose("rest", 0.0), pose("ride", 0.0)];
    let meshes: Vec<(&MeshData, [f64; 3])> = models.iter().map(|m| (&m.body, [0.0; 3])).collect();
    let top = models[0].body.bounds.max[1];
    let hh = 0.30 * (top / 1.75).max(0.75);
    let head = bx([-0.2, top - hh, -0.2], [0.2, top + 0.02, 0.2]);
    let hands = {
        let b = models[0].body.bounds;
        bx([b.min[0], 0.55 * top - 0.25, -0.2], [b.max[0], 0.55 * top + 0.05, 0.2])
    };
    // zbliżenie lewej dłoni (stoi) i dłoni na kierownicy (prowadzi)
    let around = |m: &HumanModel, name: &str, r: f64| {
        let j = m.joints.iter().find(|j| j.name == name).expect("kość");
        let c: [f64; 3] = std::array::from_fn(|k| (j.head[k] + j.tail[k]) / 2.0);
        bx([c[0] - r, c[1] - r, c[2] - r], [c[0] + r, c[1] + r, c[2] + r])
    };
    let hand = around(&models[0], "LeftHand", 0.11);
    let grip = {
        let (a, b) = (around(&models[3], "LeftHand", 0.1), around(&models[3], "RightHand", 0.1));
        bx(std::array::from_fn(|k| a.min[k].min(b.min[k])), std::array::from_fn(|k| a.max[k].max(b.max[k])))
    };
    let feet = bx([-0.3, 0.0, -0.2], [0.3, 0.3, 0.3]);
    let s = |label, az, el, focus, i: usize| Shot { label, az, el, focus, items: vec![i] };
    let shots = vec![
        s("STOI PRZOD", 0.0, 6.0, None, 0),
        s("STOI BOK LEWY", 90.0, 6.0, None, 0),
        s("STOI TYL", 180.0, 6.0, None, 0),
        s("STOI 3/4", 35.0, 12.0, None, 0),
        s("IDZIE BOK", 90.0, 6.0, None, 1),
        s("IDZIE 3/4", 35.0, 10.0, None, 1),
        s("SIEDZI BOK", 90.0, 4.0, None, 2),
        s("SIEDZI 3/4", 35.0, 14.0, None, 2),
        s("PROWADZI BOK", 90.0, 4.0, None, 3),
        s("GLOWA PRZOD", 0.0, 4.0, Some(head), 0),
        s("GLOWA 3/4", 40.0, 6.0, Some(head), 0),
        s("GLOWA BOK", 90.0, 4.0, Some(head), 0),
        s("DLONIE PRZOD", 0.0, 6.0, Some(hands), 0),
        s("STOPY 3/4", 35.0, 18.0, Some(feet), 0),
        s("T-POSE PRZOD", 0.0, 4.0, None, 4),
        s("PROWADZI 3/4", 40.0, 14.0, None, 3),
        s("PASAZER BOK", 90.0, 4.0, None, 5),
        s("DLON LEWA 3/4", 50.0, 20.0, Some(hand), 0),
        s("DLONIE NA KIEROWNICY", 30.0, 25.0, Some(grip), 3),
        s("GLOWA TYL 3/4", 150.0, 8.0, Some(head), 0),
        s("STOI 3/4 TYL", 215.0, 10.0, None, 0),
    ];
    write(dir, &shots, &meshes);
    let stats: Vec<Value> = models.iter().map(|m| json!(m.stats)).collect();
    std::fs::write(format!("{dir}/stats.json"), serde_json::to_string_pretty(&json!({ "poses": stats, "hip_stand": models[0].hip, "hip_sit": models[2].hip, "hip_drive": models[3].hip, "hip_ride": models[5].hip, "min_y": models.iter().map(|m| m.body.bounds.min[1]).collect::<Vec<_>>() })).unwrap()).unwrap();
    println!("{dir}: {}", serde_json::to_string(&stats).unwrap());
}

fn write(dir: &str, shots: &[Shot], meshes: &[(&MeshData, [f64; 3])]) {
    std::fs::create_dir_all(dir).expect("katalog");
    let mut tiles = vec![];
    for (k, sh) in shots.iter().enumerate() {
        let items: Vec<RenderItem> = sh.items.iter().map(|&i| RenderItem { mesh: meshes[i].0, offset: meshes[i].1 }).collect();
        let _ = scene_bounds(&items);
        let mut img = render_scene(&items, &RenderOptions { width: TW, height: TH, view: View::Iso, azimuth: Some(sh.az), elevation: Some(sh.el), focus: sh.focus, smooth: true, ..Default::default() });
        label(&mut img, sh.label, 3);
        let slug: String = sh.label.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        std::fs::write(format!("{dir}/{k:02}-{}.png", slug.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")), img.to_png()).expect("png");
        tiles.push(img);
    }
    let sheet = compose(&tiles, 3, 2);
    std::fs::write(format!("{dir}/arkusz.png"), sheet.to_png()).expect("png");
}

/// Arkusz: kafelki pomniejszone `down` razy (średnia z bloku), `cols` kolumn, 4 px odstępu.
fn compose(tiles: &[Image], cols: usize, down: usize) -> Image {
    let (w, h) = (TW as usize / down, TH as usize / down);
    let rows = tiles.len().div_ceil(cols);
    let (sw, sh) = (cols * (w + 4) + 4, rows * (h + 4) + 4);
    let mut data = vec![40u8; sw * sh * 4];
    for (i, t) in tiles.iter().enumerate() {
        let (ox, oy) = (4 + (i % cols) * (w + 4), 4 + (i / cols) * (h + 4));
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 4];
                for dy in 0..down {
                    for dx in 0..down {
                        let s = ((y * down + dy) * TW as usize + x * down + dx) * 4;
                        for c in 0..4 {
                            acc[c] += t.data[s + c] as u32;
                        }
                    }
                }
                let d = ((oy + y) * sw + ox + x) * 4;
                for c in 0..4 {
                    data[d + c] = (acc[c] / (down * down) as u32) as u8;
                }
            }
        }
    }
    for px in data.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Image { width: sw as u32, height: sh as u32, data }
}

/// Podpis w lewym górnym rogu: biały tekst 5×7 na ciemnym pasku, piksel fontu = `k` px.
fn label(img: &mut Image, s: &str, k: usize) {
    let w = img.width as usize;
    let (pw, ph) = (s.chars().count() * 6 * k + 2 * k, 9 * k);
    let mut put = |x: usize, y: usize, c: [u8; 3]| {
        if x < w && y < img.height as usize {
            let i = (y * w + x) * 4;
            img.data[i..i + 3].copy_from_slice(&c);
            img.data[i + 3] = 255;
        }
    };
    for y in 0..ph {
        for x in 0..pw {
            put(x, y, [20, 22, 26]);
        }
    }
    for (ci, ch) in s.chars().enumerate() {
        for (r, row) in glyph(ch).iter().enumerate() {
            for (c, px) in row.chars().enumerate() {
                if px == '#' {
                    for dy in 0..k {
                        for dx in 0..k {
                            put(k + (ci * 6 + c) * k + dx, k + r * k + dy, [255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
}

fn glyph(c: char) -> [&'static str; 7] {
    match c {
        'A' => [" ### ", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"],
        'B' => ["#### ", "#   #", "#   #", "#### ", "#   #", "#   #", "#### "],
        'C' => [" ### ", "#   #", "#    ", "#    ", "#    ", "#   #", " ### "],
        'D' => ["#### ", "#   #", "#   #", "#   #", "#   #", "#   #", "#### "],
        'E' => ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#####"],
        'F' => ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#    "],
        'G' => [" ### ", "#   #", "#    ", "# ###", "#   #", "#   #", " ####"],
        'H' => ["#   #", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"],
        'I' => [" ### ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "],
        'J' => ["  ###", "   # ", "   # ", "   # ", "   # ", "#  # ", " ##  "],
        'K' => ["#   #", "#  # ", "# #  ", "##   ", "# #  ", "#  # ", "#   #"],
        'L' => ["#    ", "#    ", "#    ", "#    ", "#    ", "#    ", "#####"],
        'M' => ["#   #", "## ##", "# # #", "# # #", "#   #", "#   #", "#   #"],
        'N' => ["#   #", "##  #", "# # #", "#  ##", "#   #", "#   #", "#   #"],
        'O' => [" ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "],
        'P' => ["#### ", "#   #", "#   #", "#### ", "#    ", "#    ", "#    "],
        'R' => ["#### ", "#   #", "#   #", "#### ", "# #  ", "#  # ", "#   #"],
        'S' => [" ####", "#    ", "#    ", " ### ", "    #", "    #", "#### "],
        'T' => ["#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  "],
        'U' => ["#   #", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "],
        'W' => ["#   #", "#   #", "#   #", "# # #", "# # #", "## ##", "#   #"],
        'Y' => ["#   #", "#   #", " # # ", "  #  ", "  #  ", "  #  ", "  #  "],
        'Z' => ["#####", "    #", "   # ", "  #  ", " #   ", "#    ", "#####"],
        '0' => [" ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### "],
        '1' => ["  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "],
        '3' => ["#### ", "    #", "    #", " ### ", "    #", "    #", "#### "],
        '4' => ["   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # "],
        '7' => ["#####", "    #", "   # ", "  #  ", " #   ", " #   ", " #   "],
        '8' => [" ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### "],
        '/' => ["    #", "    #", "   # ", "  #  ", " #   ", "#    ", "#    "],
        ',' => ["     ", "     ", "     ", "     ", "     ", "  #  ", " #   "],
        '+' => ["     ", "  #  ", "  #  ", "#####", "  #  ", "  #  ", "     "],
        '-' => ["     ", "     ", "     ", "#####", "     ", "     ", "     "],
        ':' => ["     ", "  #  ", "     ", "     ", "     ", "  #  ", "     "],
        '(' => ["   # ", "  #  ", " #   ", " #   ", " #   ", "  #  ", "   # "],
        ')' => [" #   ", "  #  ", "   # ", "   # ", "   # ", "  #  ", " #   "],
        _ => ["     "; 7],
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let list: serde_json::Map<String, Value> = serde_json::from_str(&std::fs::read_to_string(&a[1]).expect("lista")).expect("json");
    let only: Vec<&String> = a[3..].iter().collect();
    for (name, spec) in &list {
        if !only.is_empty() && !only.contains(&name) {
            continue;
        }
        let dir = format!("{}/{name}", a[2]);
        match a[0].as_str() {
            "tabor" => tabor(spec, &dir),
            "tlum" => tlum(spec, &dir),
            m => panic!("tryb {m}: tabor albo tlum"),
        }
    }
}
