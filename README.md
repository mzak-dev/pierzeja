# Pierzeja — proceduralny generator budynków (Polska 1800–2015)

Jeden rdzeń w Rust, trzy frontendy. Algorytm, dane i źródła: [docs/HANDOFF.md](docs/HANDOFF.md). Decyzja: [ADR 0020](../../Docs/0020-generator-budynkow-pierzeja-w-rust.md).

| Crate | Co to jest |
|---|---|
| `crates/core` | `BuildingSpec` → mieszanie 19 archetypów w 5 wektorach → gramatyka budynku → `MeshData`; eksport GLB/OBJ; ulice z profili miast. Dane: `data/pierzeja-dane.json`. |
| `crates/godot` | GDExtension: węzeł `PierzejaBuilding` (`@tool`) + statyczne API `Pierzeja` dla GDScript. |
| `crates/mcp` | Serwer MCP (stdio), 9 narzędzi jak w prototypie, podglądy PNG. |
| `crates/render` | Programowy renderer PNG (bez GPU) dla MCP. |
| `crates/app` | Aplikacja natywna: Bevy (podgląd 3D) + egui (parametry, mapa typologii, ulica). |

## Godot

```powershell
./install-godot.ps1   # przy zamkniętym edytorze: build + kopia DLL do RapidResponse/src/Host/addons/pierzeja/bin
```

Dodatek jest włączony w `project.godot`. W edytorze pojawia się zakładka **Pierzeja** obok 2D / 3D / Script: wybór archetypu albo punkt na mapie typologii, 71 parametrów (zmiana = nadpisanie, ↺ cofa), podgląd na żywo, „Wstaw do sceny”, „Eksport GLB…”. Wstawiony węzeł `PierzejaBuilding` ma pole `spec` (JSON) — edycja w inspektorze przebudowuje budynek. Siatka nie trafia do `.tscn`.

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

Mysz: lewy/prawy — obrót, środkowy — przesunięcie, kółko — zoom, `F` — kadruj. Eksport GLB do `PIERZEJA_OUT`.

## Testy

```bash
cargo test -p pierzeja-core --release
```

Wektory z HANDOFF, załącznik G: wagi IDW, mieszanie 50/50, wariacja, metryka i liczba trójkątów wszystkich 19 archetypów — zgodne z prototypem TS co do trójkąta.

## Poza zakresem (jak w prototypie)

Rzuty tylko prostokątne (segmenty, oficyny, uskoki), brak obrysów wielokątnych z OSM i dachów ze straight skeleton, tekstury tylko jako nazwa w `MaterialSpec` (kolor + UV w metrach), brak instancjonowania okien i LOD.
