# Pierzeja — proceduralny generator budynków (Polska 1800–2015)

Jeden rdzeń w Rust, trzy frontendy. Algorytm, dane i źródła: [docs/HANDOFF.md](docs/HANDOFF.md). Decyzja: [ADR 0020](../../Docs/0020-generator-budynkow-pierzeja-w-rust.md).

| Crate | Co to jest |
|---|---|
| `crates/core` | `BuildingSpec` → mieszanie 19 archetypów w 5 wektorach → gramatyka budynku → `MeshData`; obrysy wielokątne (`footprint`); ulice, osiedle bloków, skrzyżowanie, rondo; eksport GLB/OBJ. Dane: `data/pierzeja-dane.json`. |
| `crates/godot` | GDExtension: węzły `PierzejaBuilding` i `PierzejaScene` (`@tool`) + statyczne API `Pierzeja` dla GDScript. |
| `crates/mcp` | Serwer MCP (stdio), 10 narzędzi (9 z prototypu + `generate_scene`), podglądy PNG. |
| `crates/render` | Programowy renderer PNG (bez GPU) dla MCP; przykłady `preview` (spec → PNG) i `scene`. |
| `crates/app` | Aplikacja natywna: Bevy (podgląd 3D) + egui (parametry, mapa typologii, ulica). |

## Obrys wielokątny

Pole `footprint` w spec zastępuje prostokąt z parametrów bryły (np. obrys z OSM):

```json
{ "archetype": "kamienica_secesyjna", "overrides": { "turret": 1 },
  "footprint": { "points": [[-10,12],[-13,9],[-13,-10],[-1,-10],[-1,0],[13,0],[13,12]], "front": [0,1,6], "blind": [2,5] } }
```

Punkty `[x, z]` w metrach, ulica po stronie +z; krawędź `i` = `points[i] → points[i+1]`. `front` — elewacje od ulicy (wejście na najdłuższej, klatki co `segmentWidth`), `blind` — ściany graniczne bez okien, pozostałe — podwórze/boki. Obieg dowolny (normalizowany), punkty współliniowe i duplikaty usuwane; obrys z samoprzecięciem → prostokąt z parametrów i uwaga w metryce. Dach: płaski z attyką/okapem albo kopertowy/mansardowy z odsunięć krawędzi (do pierwszego zaniku krawędzi, potem płaska kalenica — przybliżenie szkieletu prostego). Wieżyczka staje na wypukłym narożniku między frontami.

## Zespoły zabudowy

`SceneSpec { kind: estate | crossroads | roundabout, seed, spacing, rows, profile, arm }`:

- **Osiedle bloków** — rzędy płyty 5- i 11-kondygnacyjnej, punktowiec; odstęp rzędów `spacing × H` (WT 2002 § 13: co najmniej H), drogi dojazdowe z parkingiem prostopadłym od strony wejść, chodniki przy frontach, ciąg pieszy z placem zabaw, pawilon usługowy, udział zieleni (normatyw 1974: ≥ 50%).
- **Skrzyżowanie** — 4 narożniki na obrysach: ścięty, ścięty z wieżyczką, obły (modernizm), ostry; pierzeje z profilu miasta na ramionach, budynki kolidujące we wnętrzu kwartału tracą oficyny albo zostaje luka.
- **Rondo** — jednopasowe Ø 35 m (katalog GDDKiA), narożniki z frontem po łuku (obrys wklęsły).

```bash
cargo run -p pierzeja-render --example scene --release -- roundabout rondo.png
cargo run -p pierzeja-render --example preview --release -- spec.json budynek.png
```

## Godot

```powershell
./install-godot.ps1   # przy zamkniętym edytorze: build + kopia DLL do RapidResponse/src/Host/addons/pierzeja/bin
```

Dodatek jest włączony w `project.godot`. W edytorze pojawia się zakładka **Pierzeja** obok 2D / 3D / Script z trybami Budynek / Wielokąt (edytor obrysu 2D) / Osiedle / Skrzyżowanie / Rondo: archetyp albo punkt na mapie typologii, 71 parametrów (zmiana = nadpisanie, ↺ cofa), podgląd na żywo, „Wstaw do sceny” (`PierzejaBuilding` albo `PierzejaScene`), „Eksport GLB…”. Wstawiony węzeł `PierzejaBuilding` ma pole `spec` (JSON) — edycja w inspektorze przebudowuje budynek. Siatka nie trafia do `.tscn`.

GDScript: `Pierzeja.build(spec_json, night)` → `{mesh, stats, error}`, `Pierzeja.resolve(spec_json)`, `Pierzeja.export_glb(spec_json, path)`, `Pierzeja.archetypes_json()`, `Pierzeja.params_json()`.

## MCP

```bash
cargo build -p pierzeja-mcp --release
claude mcp add pierzeja -- "<ścieżka>/Tools/pierzeja/target/release/pierzeja-mcp.exe"
```

Pliki trafiają do `PIERZEJA_OUT` (domyślnie `~/pierzeja-out`).

## Aplikacja natywna

```bash
cargo run -p pierzeja-app --release
```

Tryby: Budynek, Wielokąt (edytor obrysu: klik przy krawędzi — punkt, przeciąganie, prawy klik — usuń, kwadrat na krawędzi — rola), Ulica, Osiedle, Skrzyżowanie, Rondo. Mysz: lewy/prawy — obrót, środkowy — przesunięcie, kółko — zoom, `F` — kadruj. Eksport GLB do `PIERZEJA_OUT`. `PIERZEJA_MODE=wielokat|ulica|osiedle|skrzyzowanie|rondo` + `PIERZEJA_SCREENSHOT=plik.png` — zrzut i wyjście.

## Testy

```bash
cargo test -p pierzeja-core --release
```

Wektory z HANDOFF, załącznik G: wagi IDW, mieszanie 50/50, wariacja, metryka i liczba trójkątów wszystkich 19 archetypów — zgodne z prototypem TS co do trójkąta.

## Poza zakresem (jak w prototypie)

Dachy wielokątne z odsunięć zamiast pełnego straight skeleton (przy rzutach L/U płaska kalenica zamiast grzbietów nad skrzydłami), w trybie wielokąta brak szczytów dwuspadowych i lukarn, tekstury tylko jako nazwa w `MaterialSpec` (kolor + UV w metrach), brak instancjonowania okien i LOD, sceny z budynkami prostokątnymi na ramionach (wspólne ściany w kwartale nie są usuwane).
