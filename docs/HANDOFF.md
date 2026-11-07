# HANDOFF — Pierzeja: proceduralny generator budynków (Polska 1800–2015)

> **Dla kogo:** agent (lub zespół), który ma zbudować „prawdziwy” generator — w Rust, Pythonie albo TypeScripcie — na bazie działającego prototypu.
> **Stan na:** 7 listopada 2026. Prototyp: React + TypeScript + three.js + serwer MCP (repozytorium `budynki-generator`, paczka `pierzeja-generator.zip`).
> **Jak czytać:** rozdziały 1–3 mówią *co i dlaczego*, 4–9 *jak dokładnie działa* (z formułami), 10–11 opisują podgląd i MCP, 12–14 *jak sprawdzić i co poprawić*. Załączniki A–G zawierają dane i wektory testowe wygenerowane bezpośrednio z kodu.

---

## 0. TL;DR

1. **Budynek = 71 parametrów w 5 wektorach** (bryła, kondygnacje, dach, elewacja, materiał). Styl nie jest osobną gramatyką, tylko **punktem w przestrzeni parametrów**.
2. **19 archetypów** (dworek → kamienice → familok → wille → socrealizm → bloki z płyty → falowiec → kostka → lata 90. → deweloperka → plomba → apartamentowiec → stodoła) to skalibrowane na źródłach pełne wektory. Leżą na **mapie typologii** rok × skala zabudowy.
3. **Interpolacja:** wagi archetypów z kursora na mapie liczone metodą **IDW** (k najbliższych, wykładnik p) albo podane jawnie; **każdy z 5 wektorów może mieć własne wagi** (np. bryła bloku + elewacja kamienicy). Liczby — średnia ważona, kolory — średnia w **OKLab**, wyliczenia — głosowanie wag.
4. **Geometria** powstaje stałym zestawem reguł podziału w duchu *split grammar* / *CGA shape*: bryły → odsłonięte ściany → osie × kondygnacje → komórki z otworami → elementy → dachy. Te same reguły dla każdego punktu przestrzeni ⇒ każda mieszanka daje poprawny budynek.
5. **Reguły z przepisów** (WT 2002: § 8 grupy wysokości, § 54 winda, § 303 balkony/loggie) modyfikują wynik i trafiają do metryki.
6. **Determinizm:** ten sam `BuildingSpec` ⇒ identyczna siatka. Losowe decyzje geometryczne idą przez hash (seed, ścieżka elementu), nie przez sekwencyjny strumień.
7. **Kontrakt wymiany:** JSON `BuildingSpec` (rozdz. 4.4) — wspólny dla UI, MCP i eksportu. Utrzymaj zgodność.
8. **Dane referencyjne:** `docs/pierzeja-dane.json` (parametry, archetypy z pełnymi wektorami, profile ulic) + załączniki A–G. Wektory testowe z załącznika G przenieś do testów jednostkowych nowej implementacji.

---

## 1. Zakres i wymagania

Prośba użytkownika (oryginał): *„Stwórz tool do proceduralnego generowania modeli budynków. Piętra, styl, kształt za pomocą kilku wektorów i uzupełnianie pomiędzy tymi wektorami. Ma generować od domu jednorodzinnego, kamienicy do PRL-owskiego bloku a nawet nowoczesnych apartamentów. Ale najpierw zrób dogłębny research nawet jakieś whitepapery architektoniczne na temat budynków w Polsce od 19 wieku do 2015 roku, bo zazwyczaj jest mix takich budowli w miastach. Tool może mieć podgląd w threejs i tam też UI, ale ma też mieć MCP. Prototyp zrób w React i TypeScript.”*

Interpretacja przyjęta w prototypie:

| Wymaganie | Realizacja |
|---|---|
| „kilka wektorów” sterujących piętrami, stylem, kształtem | 5 grup parametrów = 5 wektorów; każdy można mieszać osobno |
| „uzupełnianie pomiędzy wektorami” | interpolacja wag archetypów (IDW na mapie, jawne wagi, A→B) i ciągłe mieszanie wartości |
| od domu jednorodzinnego po apartamentowce | 19 archetypów 1800–2015 |
| „mix takich budowli w miastach” | tryb ulicy: profile miast + hybrydy dwóch typów w jednym budynku |
| research, whitepapery | dokument z researchem (rozdz. 3) z ponad 80 podlinkowanymi źródłami, w tym artykuły naukowe i przepisy |
| podgląd three.js + UI | aplikacja React (rozdz. 10) |
| MCP | serwer stdio z 9 narzędziami (rozdz. 11) |

Poza zakresem prototypu (świadomie): działki narożne i nieregularne, wnętrza, wspólne ściany w kwartale, prawdziwe przekroje murów, LOD-y, tekstury fotograficzne.

---

## 2. Metody — przegląd

```
BuildingSpec (JSON)
   │  1. wagi archetypów dla każdego z 5 wektorów (jawne / IDW z mapy / archetyp)
   ▼
ResolvedBuilding  ← 2. mieszanie wartości (liczby, OKLab, głosowanie) → 3. wariacja (seed) → 4. nadpisania
   │
   ▼  5. gramatyka podziału (stałe reguły, sterowane parametrami)
 bryły (segmenty, oficyny, uskoki) → odsłonięte ściany → pasy elewacji → osie × kondygnacje
   → komórki ściany z otworami → okna, balkony/loggie, wejścia, witryny, wykusze, detal → dachy, wieżyczka
   │
   ▼  6. reguły z przepisów (WT 2002) + metryka (wysokość, klasa, PC, mieszkania)
MeshData (części per materiał, UV w metrach) ──► podgląd three.js · eksport GLB/OBJ · renderer PNG (MCP)
```

| Metoda | Po co | Gdzie w kodzie |
|---|---|---|
| Przestrzeń parametrów z archetypami jako punktami kalibracyjnymi | styl jako dane, ciągłe przejścia | `params.ts`, `archetypes.ts` |
| IDW (Shepard) na mapie rok × skala | z jednego kursora → wagi kilku archetypów | `blend.ts: mapWeights` |
| Osobne wagi per wektor | hybrydy (bryła z jednego typu, elewacja z innego) | `blend.ts: groupWeights` |
| Mieszanie kolorów w OKLab (Ottosson 2020) | przejścia bez „brudnej szarości” | `color.ts` |
| Głosowanie wag dla wyliczeń | parametry dyskretne (materiał, typ wejścia) | `blend.ts: blendValue` |
| Wariacja z jitterem per parametr | różnice między realizacjami tego samego typu | `blend.ts: applyVariation` |
| Split grammar / CGA shape (Wonka 2003, Müller 2006) — w wersji stałych reguł | geometria budynku | `generator.ts` |
| Rozkład ściany na siatkę prostokątów zamiast CSG | otwory bez operacji boolowskich | `generator.ts: wallRegion` |
| Hash-RNG po ścieżce elementu | determinizm odporny na zmiany | `random.ts: hash01` |
| Reguły z WT 2002 | realizm „prawny” i metryka | `generator.ts: balcony, stats` |
| Losowanie pierzei z profilu miasta | mieszanka typów w ulicy | `street.ts`, `cities.ts` |
| Renderer programowy (z-bufor, mapa cieni) | podglądy PNG w MCP bez GPU | `render/raster.ts` |

---

## 3. Źródła wiedzy

### 3.1 Jak powstał research

- Dokument „Budownictwo mieszkaniowe w Polsce 1800–2015 — research do generatora budynków” (Claude Docs: `https://claude.ai/artifact/HLripjtFagnkLnJV11SEzC`). Sekcje: kontekst (zabory, granice, zniszczenia), XIX w. (dworek, kamienica), budownictwo robotnicze, secesja i drewno, dwudziestolecie, socrealizm, wielka płyta, dom-kostka, 1989–2015, przepisy, statystyki GUS/NSP, metody proceduralne, tabela archetypów, źródła.
- Metoda: wyszukiwanie po polsku i angielsku, priorytet dla publikacji naukowych (Ochrona Zabytków NID, Kwartalnik Architektury i Urbanistyki PAN, Teka KAiU PAN, BazTech, repozytoria uczelni), tekstów przepisów i opracowań urzędowych; serwisy popularne tylko jako uzupełnienie. Fakty liczbowe przechodziły do parametrów jako **kalibracja projektowa** (nie pomiary konkretnych budynków).
- Każdy archetyp w kodzie ma listę źródeł (`Archetype.sources`) — patrz załącznik C/E.

### 3.2 Kluczowe fakty → parametry

| Fakt ze źródeł | Skutek w modelu | Źródło |
|---|---|---|
| Warszawskie parcele kamienic: narożne 29–36 m, środkowe 22–29 m, głębokość 25–33 m (czasem >70 m); front + oficyny wokół podwórza, wjazd bramą; ściany na granicy działki bez okien | `segmentWidth` 20–24 m, `wingLeft/Right/Rear`, `wingLength` 18–20 m, `entranceType: gate`, `sideWindows: 0` | Kilanowski, *Ochrona Zabytków* 1/2015 |
| Do 1904–1905 limit wysokości kamienic w Warszawie ok. 3 pięter; 1905–1914 prawie wyłącznie ≥5 pięter | kamienica eklektyczna 5 kond., secesyjna 6 | Kilanowski |
| „Im wyżej, tym lżej”: ciężkie balustrady na piano nobile, lżejsze wyżej; wykusze ok. 1900–1910 | `floorHeightDecay`, ornament malejący z piętrem, balustrada pełniejsza na 1. piętrze, `oriels` | Roguska, *KAiU* 4/2010 |
| Familoki: nietynkowana czerwona cegła, czerwone ramy okien, 2–3 piętra | `wallTexture: brick`, `frameColor` czerwony, 3 kond. | Wikipedia (Familok), Murator, dzieje.pl (Nikiszowiec) |
| Secesja krakowska: asymetria, spłaszczone łuki, słabo wysunięte wykusze | `asymmetry`, `windowArch`, `mansard`, `dormers`, `turret` | Janicka, *Ochrona Zabytków* 4/2001 |
| Gdyński modernizm: płaskie dachy, okna pasmowe, loggie, obłe narożniki, okładziny | `ribbon`, `loggia`, `parapetHeight`, `railing` rurowy | gdynia.pl, Muzeum Piłsudskiego |
| Socrealizm 1949–1956: pierzeje, osie, arkady, attyki, gzymsy | `groundRecess` (podcienia), `rustication`, `cornice`, `parapetHeight`, 3 segmenty × 20 m | PAP, dzieje.pl, Przesmycka, Sumorok |
| Wielka płyta: kondygnacja 2,7–2,8 m, siatka 1,2/2,4/4,8 m, trakty 4,80 m, bloki 5 i 11 kond.; typowy 11k ok. 11,8 × 38,8 m | `floorHeight` 2,75, `depth` 11–11,8, `segmentWidth` 13–14,4, `panelJoints`, `stairWindows`, `rooftopBoxes` | Woźniczka 2021, Ostańska 2009, Płachecki/Koziński |
| Falowiec (Gdańsk): ok. 860 m, 11 kond., 16 klatek, galerie | `stagger`, `balconyWidth: 1` (galeria), 11 kond. — **skrócony do 6 × 54 m** (wydajność) | National Geographic, kb.pl |
| Normatywy mieszkań 1959/1974 (M-1…M-5) | szacunek liczby mieszkań (`avgFlat` wg epoki) | Builder 02/2017 |
| Dom-kostka: limit 110 m² p.u., płaski dach, wysoki parter, loggia | 10 × 10 m, `plinthHeight` ~1,1, `basementWindows`, płaski dach z okapem | Murator, kb.pl |
| Lata 90.: daszki, wieżyczki, kolumienki, kolor; plomby; osiedla grodzone; „pasteloza” | `turret`, `dormers`, `accentPanels`, `streetSetback` | PAP (Cymer), Tokajuk, Owczarek, Nikolić |
| Nowoczesna stodoła: dwuspadowy bez okapu 35–40°, duże przeszklenia | `roofOverhang: 0`, `gableGlazing`, `ridgeAlongFront: 0` | kb.pl, Murator |
| Zasób wg okresu budowy (NSP 2021): szczyt 1971–1978 (~270 tys./rok), sprzed 1918 r. 6,9% | wagi profili ulic (blokowiska, śródmieścia) | PKO BP, *Puls Nieruchomości* 2023 |

### 3.3 Przepisy (zweryfikowane w tekście rozporządzenia)

| Przepis | Treść (skrót) | Zastosowanie w generatorze |
|---|---|---|
| WT 2002 § 8 | grupy wysokości: N ≤12 m lub mieszkalne ≤4 kond.; SW ≤25 m lub 5–9 kond.; W ≤55 m lub 10–18 kond.; WW >55 m | `stats.heightClass` (logika „lub”) |
| WT 2002 § 54 ust. 1 | budynek wielorodzinny (i in.), w którym różnica poziomów posadzek pierwszej i najwyższej kondygnacji nadziemnej przekracza 9,5 m, wyposaża się w dźwig | uwaga w metryce dla `year ≥ 2002` |
| WT 2002 § 303 | powyżej 25 m nad terenem zakaz balkonów; loggie tylko z balustradą pełną; powyżej 55 m także bez loggii | `regulations`: dla `year ≥ 1990` balkon → loggia z pełną balustradą (>25 m), brak (>55 m) |
| PRL (normy z epoki, wg wtórnego źródła) | „budynki do 5 pięter nie wymagały wind” (poradnik Krakowa 2025, bez nazwy normy) | prototyp czyta to jako ≤ 5 kondygnacji: uwaga „dodaj maszynownię” dla bloków > 5 kond. bez `rooftopBoxes` — **do weryfikacji w normach z epoki** |
| Do 1904–1905 (Warszawa) | limit wysokości kamienic | kalibracja archetypów |

Uwagi:
- **WT 2002 uchylono z dniem 20.09.2026**; nowe warunki techniczne (np. winda od 3 kondygnacji w nowych budynkach wielorodzinnych) są poza zakresem 1800–2015. Dla zabudowy z lat 2002–2015 WT 2002 nadal opisuje obowiązujące wtedy reguły.
- W dokumencie z researchem tabela przepisów ma uproszczony wiersz „winda od 5 kondygnacji i powyżej 12 m”. **Kod używa dokładnej reguły § 54 (9,5 m)** — w nowej implementacji trzymaj się tekstu przepisu.
- Klasyfikacja wysokości w prototypie liczy H jako wierzch attyki (dach płaski) albo najwyższy punkt dachu/wieżyczki (dach spadzisty). Definicja wysokości budynku w WT (§ 6) jest inna — **do dopracowania**.

### 3.4 Metody i algorytmy — literatura

- Wonka P., Wimmer M., Sillion F., Ribarsky W., *Instant Architecture*, ACM TOG 22(3), 2003 — split grammar.
- Müller P., Wonka P., Haegler S., Ulmer A., Van Gool L., *Procedural Modeling of Buildings*, ACM TOG 25(3), 2006 — CGA shape (bryły → podział elewacji: split, repeat).
- Shepard D., *A two-dimensional interpolation function for irregularly-spaced data*, ACM 1968 — IDW.
- Ottosson B., *A perceptual color space for image processing* (OKLab), 2020 — mieszanie kolorów (stałe macierzy w `color.ts`).
- mulberry32 (generator PRNG z domeny publicznej) i hash FNV-podobny — deterministyczna losowość.

### 3.5 Lista źródeł

Pełne listy: **załącznik E** (źródła przypięte do archetypów) i sekcja „Źródła” dokumentu z researchem (prace naukowe: Kilanowski, Roguska, Domińczak i Zaguła, Łupienko, Janicka, Sumorok, Przesmycka, Woźniczka, Ostańska, ABiD 4/2020, Builder 02/2017, Owczarek, Nikolić, Cymer, PKO BP/NSP 2021, tekst WT, Wonka 2003, Müller 2006).

### 3.6 Zastrzeżenia do danych

- Wartości archetypów to **pojedyncze punkty kalibracyjne**, a nie rozkłady. Realne budynki danego typu mają rozrzut — w prototypie przybliża go `variation` (jednakowy jitter dla wszystkich archetypów).
- Falowiec skrócono (6 segmentów × 54 m zamiast ~860 m): 324 m to już ~283 tys. trójkątów, pełna długość dałaby szacunkowo ~750 tys.
- Jedno ze źródeł familoka to kopia Wikipedii (kiwix) — przy rozbudowie zastąp źródłem naukowym.
- Szacunek mieszkań (`PC × 0,72 / średni metraż epoki`) i progi „domu jednorodzinnego” są heurystyką (np. dworek 264 m² daje 3 „mieszkania”).

---

## 4. Model danych

### 4.1 Parametry

71 parametrów w 5 grupach; pełna tabela w **załączniku A** i w `docs/pierzeja-dane.json` (`params`). Typy:

| `kind` | Przechowywanie | Mieszanie | Normalizacja (`clampParam`) |
|---|---|---|---|
| `number` | float | średnia ważona | przycięcie do [min, max]; NaN → domyślna |
| `int` | float w trakcie, int na końcu | średnia ważona, potem `round` | przycięcie + `round` |
| `color` | sRGB `#rrggbb` (małe litery) | średnia ważona w OKLab | regex `^#[0-9a-f]{6}$`, inaczej domyślna |
| `enum` | string | opcja o największej sumie wag | spoza listy → domyślna |

Każdy parametr ma `jitter` = amplituda wariacji jako ułamek zakresu (dla `variation = 1`).

**Ważne semantyki** (łatwe do przeoczenia):
- Parametry „gęstości” 0–1 (`balconies`, `oriels`, `dormers`, `accentPanels`, `groundShop`, `basementWindows`, `sideWindows`, `chimneys`) są prawdopodobieństwami albo gęstością wzoru — nie liczbą sztuk.
- Progi dyskretne w generatorze (np. `mansard > 0.05`, `roofHip < 0.15`, `turret ≥ 0.5`, `rustication > 0.5`, `railing < 0.33 / < 0.66`, `wingRear ≥ 0.5`, `ridgeAlongFront ≥ 0.5`, `roofPitch < 3`) powodują, że przy płynnej interpolacji niektóre cechy przełączają się skokowo — to zamierzone (element albo jest, albo go nie ma).
- `groundFloorHeight` to wysokość parteru liczona od poziomu posadzki parteru (wierzch cokołu); `floorHeight` — wysokość kondygnacji brutto (strop–strop).
- `balconyWidth` jest ułamkiem rozstawu osi; `≥ 0.97` = galeria ciągła.
- `railing`: `< 0.33` pełna, `< 0.66` prętowa, `≥ 0.66` szklana.
- `entranceType`: `door` | `gate` (przejazd bramny) | `portico` | `glass`.

### 4.2 Archetypy

```ts
interface Archetype {
  id: string; name: string; short: string;
  years: [number, number];          // okres występowania
  map: { year: number; scale: number }; // położenie na mapie typologii
  description: string; features: string[]; sources: { title: string; url: string }[];
  params: Partial<BuildingParams>;  // tylko wartości różne od domyślnych
}
// pełny wektor = normalizeParams({ ...DEFAULT_PARAMS, ...archetype.params })
```

Lista, położenia i nadpisania: **załączniki B i C**. Archetyp domyślny: `kamienica_eklektyczna`.

### 4.3 Mapa typologii

- Oś X: rok 1800–2015 (`MAP_YEARS`), oś Y: skala zabudowy 0 (dom) – 1 (wielki blok, falowiec).
- Epoki (tylko do opisu UI): Zabory 1800–1918, II RP 1918–1939, Wojna 1939–1945, PRL 1945–1989, III RP 1989–2015.
- Do odległości obie osie są normalizowane do [0, 1] (`mapToUnit`), więc 1 jednostka X = 215 lat, 1 jednostka Y = cała skala.

### 4.4 `BuildingSpec` — kontrakt JSON

```ts
interface BuildingSpec {
  archetype?: string;                                   // 100% jednego archetypu
  weights?: Record<string, number>;                     // jawne wagi (normalizowane)
  cursor?: { year: number; scale: number };             // punkt na mapie → IDW
  groupWeights?: Partial<Record<GroupId, Record<string, number>>>; // osobno per wektor (pierwszeństwo)
  groupCursors?: Partial<Record<GroupId, { year: number; scale: number }>>;
  overrides?: Partial<BuildingParams>;                  // stosowane na końcu
  seed?: number;          // domyślnie 1
  variation?: number;     // 0–1, domyślnie 0
  sharpness?: number;     // wykładnik IDW, domyślnie 3
  neighbors?: number;     // k najbliższych, domyślnie 3
  regulations?: boolean;  // reguły WT, domyślnie true
}
type GroupId = 'massing' | 'storeys' | 'roof' | 'facade' | 'material';
```

**Pierwszeństwo źródeł wag dla wektora `g`** (`groupWeights(spec, g)`):
1. `groupWeights[g]` → `normalizeWeights`
2. `groupCursors[g]` → `mapWeights(cursor, sharpness, neighbors)`
3. `archetype` (jeśli istnieje) → `{archetype: 1}`
4. `weights` → `normalizeWeights`
5. `cursor` → `mapWeights`
6. domyślnie `{kamienica_eklektyczna: 1}`

`normalizeWeights` odrzuca nieznane id i wagi ≤ 0, dzieli przez sumę; pusta mapa → archetyp domyślny. (Serwer MCP zamiast cicho odrzucać zwraca błąd z listą poprawnych id — tak też rób w API.)

Przykłady: załącznik G oraz `README.md`.

### 4.5 `ResolvedBuilding`

```ts
{ params: BuildingParams;                 // pełny wektor po mieszaniu, wariacji i nadpisaniach
  weights: Record<GroupId, WeightMap>;    // wagi każdego wektora
  year: number;                           // round(Σ_g Σ_a w·year_a / Σ w) — „datowanie”
  seed: number; regulations: boolean;
  dominant: string }                      // archetyp o największej sumie wag po wektorach
```

---

## 5. Mieszanie wektorów (`resolveSpec`) — algorytm

### 5.1 IDW na mapie

```text
mapWeights(cursor, p = sharpness, k = neighbors):
  (cx, cy) = ((year − 1800) / 215, scale)
  dla każdego archetypu a: d_a = hypot(ax − cx, ay − cy)   // w jednostkach znormalizowanych
  posortuj rosnąco po d
  jeśli d_min < 0.012: zwróć {najbliższy: 1}                // „przyciąganie” do archetypu
  weź k najbliższych; w_a = 1 / d_a^p; znormalizuj do sumy 1
```

Uwagi: p = 3 daje wyraźne „strefy”, p ≈ 1–1,5 miesza łagodnie. Ograniczenie do k sąsiadów zapobiega „rozmyciu” wszystkich 19 typów. Wektory testowe: G.1.

### 5.2 Mieszanie wartości

```text
dla każdego parametru def (w kolejności PARAM_DEFS):
  w = wagi wektora def.group
  number/int: v = Σ w_a · vec_a[key] → clampParam (int: round)
  color:      v = OKLab⁻¹( Σ w_a · OKLab(vec_a[key]) / Σ w_a )   // sRGB → linear → OKLab
  enum:       v = argmax_opcja Σ w_a·[vec_a[key] = opcja]          // remis: pierwsza w kolejności wpisów wag
```

Konwersje kolorów (dokładne stałe w `src/core/color.ts`, wzory Ottossona): sRGB→linear (próg 0,04045, wykładnik 2,4), linear→LMS→pierwiastek sześcienny→OKLab i z powrotem; ujemne składowe po konwersji przycinane do 0. Kontrola: G.2.

### 5.3 Wariacja

```text
applyVariation(p, seed, variation):            // variation ∈ [0, 1]
  rnd = mulberry32((seed XOR 0x9E3779B9) >>> 0)
  dla każdego def w PARAM_DEFS:
    r1, r2, r3 = 2·rnd()−1  (TRZY losowania zawsze, także gdy jitter = 0 — zachowuje sekwencję)
    jeśli jitter ≤ 0: dalej
    number/int: nv = v + r1·variation·jitter·(max − min)
                jeśli v == 0 i min == 0: nv = 0     // nie twórz elementów z niczego (np. lukarn)
                v = clampParam(nv)
    color:      v = adjustHex(v, dL = r1·variation·jitter·1.4, chroma×(1 + r2·variation·0.25), dHue = r3·variation·0.18 rad)
```

### 5.4 Nadpisania, datowanie, typ dominujący

`params = normalizeParams({...params, ...overrides})`; `year = round(Σ_g Σ_a w·year_a / Σ w)`; `dominant = argmax_a Σ_g w_a`. Wagi „łączne” do opisów: średnia wag po 5 wektorach (`overallWeights`).

### 5.5 Losowość referencyjna (port 1:1)

Aby odtworzyć identyczne budynki, zachowaj dokładną arytmetykę 32-bitową (w Rust: `wrapping_mul`, `>>` na `u32`; w Pythonie maskuj `& 0xFFFFFFFF`).

```ts
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
export function hash01(...xs: number[]): number {   // → [0, 1)
  let h = 0x811c9dc5;
  for (const x of xs) {
    let k = Math.floor(x * 1000003) | 0;               // ToInt32 (modulo 2^32, ze znakiem)
    k = Math.imul(k ^ (k >>> 16), 0x45d9f3b);
    k = Math.imul(k ^ (k >>> 16), 0x45d9f3b);
    k ^= k >>> 16;
    h = Math.imul(h ^ k, 0x01000193);
  }
  h ^= h >>> 15; h = Math.imul(h, 0x2c1b3c6d); h ^= h >>> 12;
  return (h >>> 0) / 4294967296;
}
```

Generator geometrii używa wyłącznie `hash01(seed, ...sól)` (w pseudokodzie skrót `h(...)` = `hash01(seed, ...)`) — każda decyzja ma własną „ścieżkę” (np. `salt, bay, floor, 41` dla oświetlonych okien). Dzięki temu dodanie nowego elementu nie przesuwa innych losowań. **Zachowaj tę zasadę.**

---

## 6. Generator geometrii (`generateBuilding`)

### 6.0 Konwencje

- Jednostki: metry. Oś **Y w górę**. Teren y = 0. Front budynku na **z = 0**, zwrócony w stronę **+Z** (ulica); budynek sięga w −Z. Oś X wzdłuż ulicy, budynek wyśrodkowany w x.
- Układ elewacji (`Frame {o, u, v, n}`): `n` — normalna na zewnątrz, `v` = (0,1,0), `u = cross(v, n)` — „w prawo” patrząc z zewnątrz; `o` — lewy dolny narożnik ściany (y = 0). Współrzędne lokalne: `a` wzdłuż ściany, `b` wysokość, `c` odsunięcie wzdłuż normalnej (c < 0 = w głąb muru).
- Dla prostokąta `r = {x0, x1, z0, z1}`: front `o=(x0,0,z1), n=+Z`; tył `o=(x1,0,z0), n=−Z`; prawa `o=(x1,0,z1), n=+X`; lewa `o=(x0,0,z0), n=−X`.
- Siatka „flat shaded”: każdy wielokąt ma własne wierzchołki i normalną; wielokąty wypukłe rozbijane wachlarzem trójkątów; `poly()` odwraca kolejność, jeśli normalna nie zgadza się z zamierzoną (`want`).
- UV w metrach, rzut planarny wg dominującej osi normalnej (|n_y| > 0,72 → (x, z); |n_x| > |n_z| → (±z, y); inaczej (±x, y)); w podglądzie tekstura powtarza się co `textureScale` metrów.
- Części siatki grupowane per materiał (`MatKey`), patrz 6.17.

### 6.1 Kondygnacje (`rowsFor`)

```text
F = max(1, round(floors))
plinth = clamp(plinthHeight, 0, max(0, groundFloorHeight − 1.2))
groundTop = plinth + groundFloorHeight
wiersz 0 (parter): b0 = 0, b1 = groundTop, level = plinth
wiersz k ≥ 1:      t = (F > 2) ? (k−1)/(F−2) : 0
                   h = max(2.3, floorHeight · (1 − floorHeightDecay · t))
                   b0 = b1 poprzedniego, b1 = b0 + h, level = b0
```

`b0..b1` to pas elewacji kondygnacji, `level` — poziom posadzki (balkony, okna).

### 6.2 Bryła (`layout`)

```text
S = clamp(round(setbackFloors), 0, F−1); segs = max(1, round(segments)); sw = segmentWidth; D = depth; W = segs·sw
canPent = S > 0 && D − setbackDepth ≥ 3.5 && setbackDepth > 0.4
rowsMain = canPent ? wiersze 0..F−1−S : wiersze 0..F−1
ridge = ridgeAlongFront ≥ 0.5 ? 'x' : 'z'
dla k = 0..segs−1:
  x0 = −W/2 + k·sw;  zf = (k nieparzyste) ? −stagger : 0          // falowiec: co drugi segment cofnięty
  main:  x ∈ [x0, x0+sw], z ∈ [zf−D, zf], y ∈ [0, top(rowsMain)], hasAbove = canPent
  pent (jeśli canPent): x ∈ [x0, x0+sw], z ∈ [zf−D, zf−setbackDepth], wiersze F−S..F−1 (cofnięcie od frontu)
oficyny (wysokość części głównej):
  ww = clamp(wingWidth, 3, max(3, W/2 − 1));  LL = wingLength·wingLeft;  LR = wingLength·wingRight
  hasL = LL ≥ 2 && W ≥ 2·ww + 2;  hasR analogicznie
  rear = hasL && hasR && wingRear ≥ 0.5  → obie oficyny długości min(LL, LR)
  wingL: x ∈ [−W/2, −W/2+ww], z ∈ [backL−L, backL]   (backL = z0 pierwszego segmentu), kalenica 'z'
  wingR: x ∈ [W/2−ww, W/2],   z ∈ [backR−L, backR]
  rear:  x ∈ [−W/2, W/2],     z ∈ [zb−ww, zb], zb = min(backL, backR) − L, kalenica 'x'  // zamyka podwórze-studnię
```

### 6.3 Odsłonięte ściany (`exposed`) i role

Dla każdej bryły `v` i ściany (front/back/left/right) liczony jest zbiór przedziałów `[s0, s1]` (we współrzędnej `a`) **nie zasłoniętych** przez inne bryły tego budynku:

```text
L = długość ściany; cov = []
dla każdej innej bryły o, która nakłada się pionowo (min(yHi, o.top) − max(yLo, o.y0) ≥ 0.1):
  jeśli o leży „za” płaszczyzną ściany (z tolerancją EPS = 0.02) — dodaj rzut części wspólnej na oś a
scal przedziały cov; zwróć dopełnienie w [0, L] (pomijając szczeliny ≤ 0,05 m)
```

To obsługuje styki segmentów, przesunięcia falowca, połączenia oficyn i penthouse. Elewacje i attyki generuje się tylko na przedziałach odsłoniętych; szczyty dachów dwuspadowych tylko gdy odsłonięte ≥ 50% szerokości.

Role ścian (`roleOf`) sterują ilością detalu: dla `main/pent`: front → `front`, tył → `back`, boki → `side`; oficyna lewa: ściana prawa → `court` (podwórze), reszta `side`; oficyna prawa: lewa → `court`; oficyna tylna: front → `court`. Mnożnik ornamentu: front 1, side 0,3, back/court 0,15.

### 6.4 Elewacja (`facade(v, face, s0, s1)`)

```text
len = s1 − s0; margin = min(cornerMargin, 0.22·len); usable = len − 2·margin
n = max(1, round(usable / max(0.8, bayWidth))); jeśli usable < 1.1: n = 0;  bw = usable / n
recess = (rola front, bryła main, groundRecess > 0.3) ? groundRecess : 0          // podcienia
narożniki [s0, s0+margin] i [s1−margin, s1]: pełny mur na całą wysokość (boniowanie parteru jeśli rustication > 0.5 i rola ≠ side)
jeśli n = 0: pełna ściana + gzymsy; koniec
centerBay = min(n−1, ⌊n/2⌋ + (asymmetry ≥ 0.5 && n > 2 ? (h < 0.5 ? −1 : 0) : 0))
entranceBay = (front bryły main && len ≥ 2.5) ? centerBay : −1          // jedno wejście na segment = klatkę
stairs = entranceBay ≥ 0 && stairWindows ≥ 0.5 && liczba wierszy ≥ 3
balcBays  = (rola front|back) ? pick(n, balconies, salt+1) : brak
orielBays = (rola front, nie pent, n ≥ 2) ? pick(n, oriels, salt+2) : brak
jeśli stairs: oś wejścia bez balkonu i wykusza
ww = clamp(lerp(windowWidth, bw − 0.08, ribbon), 0.35, bw − (ribbon > 0.6 ? 0.06 : 0.2))   // okna pasmowe = szerokie
orielTo = (wierszy ≥ 4) ? piętro wiersza [len − (roofPitch > 3 ? 1 : 2)] : −1
```

Dla każdej osi `i` i kondygnacji `row`:

```text
rowH = row.b1 − row.level; sill = sillHeight; wh = min(windowHeight, rowH − sill − 0.22)
jeśli wh < 0.5: sill = max(0.1, rowH − 0.72); wh = min(windowHeight, rowH − sill − 0.22)
winTop = row.level + sill + wh
parter (nie pent):
  oś wejścia → entrance(...)                                   (6.10)
  albo rola front i h(salt,i,21) < groundShop → witryna: szer. min(bw−0.3, max(ww, 0.78·bw)), od level+0.05 do b1−0.45
  okienko piwnicy: plinth ≥ 0.45 i basementWindows > h(salt,i,23) i brak innych otworów → 0,66 m, od 0,1 do min(plinth−0.08, 0.55)
piętro, oś wejścia, stairs → okno klatki: szer. min(0.95, bw−0.4), od level+0.25, wys. min(1.35, rowH−0.55)  (rytm spoczników)
oś wykusza i piętro ≤ orielTo → bez okna w murze (okna na wykuszu)
ściana boczna: okno tylko gdy h(salt,i,floor,31) < sideWindows        // 0 = ślepa ściana szczytowa
balkon: okno && piętro ≥ 1 && balcBays[i] && floor ≥ balconyFrom && nie pent && nie „>55 m” → balcony(...)  (6.9)
inaczej okno: [cx − ww/2, cx + ww/2] × [level + sill, winTop], ościeże windowDepth
wallRegion(osi × wiersz, otwory)        (6.6);  dla otworów openingFill (6.7)
okno oświetlone nocą: h(salt,i,floor,41) < 0.33 → materiał glassLit
detal okien (6.8) z siłą k = orn · (1 − 0.12·(max(1, floor) − 1))     // „im wyżej, tym lżej”
  (bez detalu parteru, gdy boniowanie > 0.5 lub podcienia)
```

`salt = v.id·31 + indeks_ściany·7 + round(s0·10)`.

Podcienia (`recess > 0`): parter cofnięty o `recess` (rama przesunięta), strop podcieni na wys. `groundTop`, filary 0,6 m na granicach osi.

### 6.5 Wzory wyboru osi (`pick`)

```text
pick(n, density, salt):
  density ≤ 0.02 → żadna; density ≥ 0.97 → wszystkie
  m = max(1, round(1/density))
  asymmetry ≥ 0.5: off = ⌊h(salt,17)·m⌋; wybierz i: (i + off) mod m = 0          // wzór przesunięty
  density ≥ 0.4:   wybierz i: min(i, n−1−i) mod m = 0                           // symetrycznie od krańców
  inaczej:         c = (n−1)/2; wybierz i: round(|i − c|) mod m = 0             // symetrycznie od środka
  jeśli nic nie wybrano, a density·n ≥ 0.5 → oś środkowa
```

Lekcja z prototypu: wzór „od środka” przy dużej gęstości trafiał w oś klatki (zarezerwowaną) — bloki traciły balkony. Stąd wariant „od krańców” dla gęstości ≥ 0,4.

### 6.6 Ściana z otworami bez CSG (`wallRegion`)

```text
wallRegion(f, a0, a1, b0, b1, holes, accent, {rustic, joints, c}):
  A = posortowane {a0, a1} ∪ {krawędzie otworów w a, przycięte}
  B = posortowane {b0, b1} ∪ {krawędzie otworów w b} ∪ {plinth, jeśli wewnątrz}
  dla każdej komórki siatki A × B:
    jeśli środek komórki leży w otworze → pomiń
    materiał = (górna krawędź ≤ plinth) ? plinth : (accent ? accent : wall)
    prostokąt w płaszczyźnie c
    boniowanie: poziome fugi co 0,36 m (materiał joint, głęb. 0,008)
    styki płyt (panelJoints > 0.5): fuga przy dolnej krawędzi pasa (piętra ≥ 1) i lewej krawędzi osi (osie ≥ 1)
```

To proste, szczelne i szybkie (bez operacji boolowskich). Otwory dostają ościeża (`openingFill`), więc ściana nie ma „dziur do wnętrza”.

### 6.7 Otwory (`openingFill`)

- Ościeża: 4 prostokąty głębokości `depth` (materiał `trim` dla cegły/drewna, inaczej `wall`; spód bramy/drzwi — `slab`).
- `void` (wcięcie loggii) — tylko ościeża. `gate` — wrota (materiał `door`) z pionowym podziałem. `door` — drzwi z naświetlem (górne `min(0.5, 22%)` przeszklone).
- Okna/witryny/drzwi szklane/okienka piwnic: szyba (`glass` lub `glassLit`) + rama (szer. 0,055; witryny 0,07; piwnice 0,04).
- Podziały: witryny i drzwi szklane — słupki co ~1,3 m + ślemię, gdy wys. > 2,2 m; okna — słupki: przy szer. > 2,2 m co 0,9 m (mullions > 0,3) albo 1,4 m, inaczej 1 słupek, gdy `mullions ≥ 0.15`; ślemię na 70% wys. przy `mullions ≥ 0.45`; drugie na 36% przy `mullions ≥ 0.85`.

### 6.8 Detal okien (`windowDecor`)

```text
s = 0.09 + 0.08·orn; pd = 0.03 + 0.04·orn
parapet zewnętrzny (okna i okna klatek, nie drzwi balkonowe): [a0−0.06, a1+0.06] × [b0−0.06, b0], c ∈ [−0.02, 0.07]
orn > 0.25: opaska (boki, góra) szer. s, wysięg pd + podokiennik
orn > 0.5 i brak łuku: gzymsik nadokienny [a0−s−0.12, a1+s+0.12], wys. 0.15, wysięg pd+0.1
  orn > 0.72 i piętro 1: trójkątny przyczółek szer. w+2s+0.24, wys. 0.4
windowArch > 0.35: łuk nad oknem; > 0.75 półkolisty (kąt połówkowy 90°), inaczej odcinkowy (60°)
  cięciwa w+2s, R = cięciwa / (2 sin(kąt)), opaska grubości 1.1·s, 9 segmentów; klucz, gdy orn > 0.5
```

### 6.9 Balkony i loggie (`balcony`)

```text
bwid = balconyWidth ≥ 0.97 ? bw : clamp(balconyWidth·bw, min(1.0, bw), bw)
lg = loggia
above25 = regulations && row.level > 25 && year ≥ 1990
jeśli above25: lg = max(lg, 0.85)  (i notuj zamianę)              // WT § 303
depth = max(0.6, balconyDepth); rec = depth·lg (wcięcie); proj = depth − rec (wysięg)
płyta 0,16 m na poziomie posadzki: c ∈ [rec > 0.15 ? −rec : −0.02, max(proj, 0.02)]
rec > 0.15 (loggia): otwór „void” w murze (wcięcie 0,12 przy galerii, inaczej 0,02 od krawędzi), wys. do b1 − 0.22;
                     ściany boczne i strop wnęki; w tylnej ścianie drzwi/okno (szer. min(okno, wnęka − 0.3))
inaczej: drzwi balkonowe w murze (bez parapetu zewnętrznego)
balustrada (wys. 1,05): rail = railing; jeśli ornament > 0.5 i piętro 1: rail −= 0.35   // ciężej na piano nobile
  jeśli above25 i rail ∈ [0.33, 0.66): rail = 0.2                                      // § 303: tylko pełna
  rail < 0.33 → pełna (płyta 0,08); < 0.66 → prętowa (materiał railing, tekstura prętów); ≥ 0.66 → szklana (glassRail)
  boki balustrady, gdy proj > 0.15; pochwyt metalowy
>55 m (regulations, year ≥ 1990): balkon nie powstaje (okno zwykłe), notatka § 303
```

### 6.10 Wejścia (`entrance`, `portico`)

| typ | otwór | dodatki |
|---|---|---|
| `gate` | szer. clamp(0.9·bw, 2.2, 3.4), wys. clamp(b1 − 0.45, 2.6, 4.2), od y = 0 (przez cokół), głęb. 0,55 | nadproże i klucz przy ornament > 0.3 |
| `glass` | szer. clamp(0.8·bw, 1.8, 3.0), od posadzki do min(b1 − 0.35, level + 3.0) | daszek metalowy |
| `door` | szer. clamp(0.42·bw, 1.0, 1.5), wys. do level + 2.35 | daszek z płyty |
| `portico` | jak `door` + portyk | podest do poziomu posadzki, schody, 2 lub 4 kolumny (pw ≥ 4,2 m), belkowanie 0,45 m, fronton 24°; pw = clamp(min(3·bw, 0.45·front), 2.4, 9), głęb. clamp(0.35·pw, 1.8, 2.8) |

Schody przed wejściem (poza portykiem), gdy poziom parteru > 0,22 m: `ceil(level/0.17)` stopni (wys. ≤ 17 cm), szer. clamp(0.6·bw, 1.4, 3), każdy kolejny głębszy o 0,3 m. Daszek (`entranceCanopy > 0.1`): szer. clamp(0.9·bw, 1.6, 3.6), 0,16 m grubości, 0,18 m nad drzwiami, wysięg = `entranceCanopy` m.

### 6.11 Wykusze (`oriel`)

Na osiach z `orielBays` (tylko front, ≥ 2 osie): od piętra 1 do `orielTo`; rama elewacji przesunięta o `orielDepth`; w każdej kondygnacji okno z detalem (ornament × 0,6); ściany boczne; wspornik u dołu (0,25 m) i daszek u góry.

### 6.12 Gzymsy i lizeny (`decorStrip`)

- Gzymsy międzykondygnacyjne (nie na ścianach podwórza, ślepych bokach i penthouse): nad parterem, gdy `stringCourses > 0.15` lub `rustication > 0.5` (wys. 0,24); wyżej, gdy `stringCourses > 0.55` albo (`> 0.3` i co drugi pas) (wys. 0,14). Wysięg × (0,6 + 0,4·roleK), roleK: front 1, side 0,5, inne 0,4.
- Gzyms koronujący (`cornice > 0.05`): dwa elementy — pas wys. 0,12 + 0,25·cornice (wysięg c/2) i płyta 0,17 (wysięg c); na ścianach frontowej i tylnej wysunięty poza narożniki (pas o c/2, płyta o c); na podwórzu c × 0,4.
- Lizeny (`pilasters > 0.12`, nie podwórze/ślepe boki/pent): szer. 0,3 + 0,18·ornament; na granicach osi — skrajne zawsze, wewnętrzne co 1 (`> 0.7`) lub co 2; przy `≤ 0.35` tylko skrajne; od cokołu (albo od 1. piętra, gdy `rustication > 0.3`) do gzymsu.

### 6.13 Pasteloza (`accentPanels`)

Wzór wybierany per bryła: `⌊h(91, v.id)·4⌋`:
0 — losowe pionowe pasy osi (`h < a·0.55`); 1 — skrajne osie (a > 0,25) + losowe pola (`a·0.18`), także narożniki; 2 — dolny i górny pas (a > 0,2) + losowe osie (`a·0.2`); 3 — losowe bloki 2-kondygnacyjne (`a·0.45`). Pola akcentu dostają materiał `accent`.

### 6.14 Dachy (`roofs`)

Drzewo decyzji per bryła:

```text
hasAbove (pod penthouse)          → dach płaski-taras z balustradą (szkło / pręty wg railing)
roofPitch < 3                     → dach płaski
main/pent z kalenicą 'x'          → scal sąsiednie segmenty o tej samej głębokości, z i wysokości w jeden dach
inaczej                           → dach spadzisty dla bryły
```

**Płaski:** okap-płyta (0,24 m, wysięg `roofOverhang` na wszystkie strony), gdy brak tarasu, `roofOverhang > 0.12` i `parapetHeight < 0.15` (kostka, willa); attyka (`parapetHeight > 0.05`): mur 0,25 m + czapka, tylko na odsłoniętych odcinkach; nadbudówki na bryłach głównych: `rooftopBoxes ≥ 0.6` — maszynownia 2,6 m (≤ 3,2 × 3,8 m) z gzymsem, `≥ 0.2` — mała skrzynka; kominy: `min(12, round(chimneys·pole/70))` w miejscach z hasha.

**Spadzisty (`pitchedRoof`):**

```text
hl, hw = połowa długości i szerokości (wzdłuż i w poprzek kalenicy); θ = clamp(roofPitch, 3, 70)°
m = mansard
jeśli m > 0.05 (łamany): etap 1: w1 = hw·(0.12 + 0.2m), kąt stromy (60 + 18m)°
                          etap 2: hw − w1, kąt max(10°, θ·(1 − 0.55m))
inaczej: jeden etap (hw, θ)
hip = roofHip < 0.15 ? 0 : roofHip                     // 0 = dwuspadowy ze szczytami
ov = parapetHeight > 0.2 ? −0.2 : roofOverhang          // attyka → dach schowany za nią
jeśli mansarda: ov = min(ov, 0.25)
poziom okapu: L = hl + ov, Wd = hw + ov, y = H − ov·min(nachylenie etapu 1, 0.75)
kolejne poziomy: Wd −= w_etapu; L = max(0, L − hip·w_etapu) (przy hip > 0, inaczej hl + ov); y += h_etapu
połacie: czworoboki między poziomami (2 strony) + połacie szczytowe przy hip > 0
szczyty (hip = 0): ściana-profil (wielokąt), tylko gdy ta strona odsłonięta ≥ 50%;
  gableGlazing > 0.05: przeszklenie trapezowe w szczycie (stodoła)
kominy na kalenicy: max(1, round(chimneys·2hl/6)), rozłożone po długości kalenicy
lukarny (dormers > 0.05, bryły main/rear, hw > 2.4, roofPitch ≥ 22):
  na mansardzie (m > 0.4) na stromym etapie, 0,2 m od okapu; inaczej 0,8 m; wys. 1,35 (mansarda: min(1.6, 0.8·h1))
  liczba miejsc nB = max(1, round((2hl − 1)/max(2.2, bayWidth))), wybór pick(nB, dormers, v.id + 77)
  szer. clamp(0.5·bayWidth, 0.9, 1.6); pomiń lukarny wchodzące w połać szczytową przy dachu kopertowym
  bryła: front ze szkłem, boki, daszek dwuspadowy 38°
```

Lekcje: (1) bez scalania segmentów dach kopertowy socrealizmu był „piłą” (osobna koperta na każdy segment); (2) okap mansardy liczony pełnym nachyleniem dolnego etapu „wisiał” zbyt nisko — stąd ograniczenie `min(slope, 0.75)`; (3) lukarny na połaciach szczytowych dachu kopertowego „wisiały w powietrzu” — stąd test `|s| + dw > L_poziomu1`.

### 6.15 Wieżyczka (`turret ≥ 0.5`)

Narożnik frontowy (lewy/prawy wg `h(97)`), ośmioboczny graniastosłup r = clamp(0.55·bayWidth, 1.2, 2.1), osadzony 0,45·r w narożniku, od wierzchu parteru (lub od terenu dla ≤ 2 kondygnacji) do `top + max(1.2, wzniesienie dachu·0.6)`, gzyms, hełm-ostrosłup wys. 2,1·r; okna tylko na ścianach zwróconych na zewnątrz.

### 6.16 Metryka (`stats`)

```text
footprint = Σ pól brył bez penthouse; GFA (PC) = Σ pole bryły · liczba jej kondygnacji
heightWalls = max(top brył); heightTotal = max(heightWalls, najwyższy punkt dachu/nadbudówki/wieżyczki)
H (do klasy) = roofPitch < 3 ? heightWalls + parapetHeight : heightTotal
klasa: H ≤ 12 lub F ≤ 4 → N; H ≤ 25 lub F ≤ 9 → SW; H ≤ 55 lub F ≤ 18 → W; inaczej WW
single = F ≤ 2 && segments = 1 && footprint < 260
avgFlat = rok < 1918: 62 | < 1945: 58 | < 1959: 50 | < 1974: 42 | < 1990: 52 | < 2008: 58 | inaczej 64 [m²]
flatsEstimate = single ? 1 : max(1, round(GFA·0.72 / avgFlat))
staircases = single ? 1 : segments
uwagi: klasa wysokości; PRL >5 kond. bez maszynowni; § 54 (rok ≥ 2002, różnica posadzek > 9,5 m); § 303 (loggie / brak)
```

### 6.17 Materiały (`buildMaterials`)

| MatKey | Kolor | Szorstkość / metaliczność | Tekstura (kafel) | Uwagi |
|---|---|---|---|---|
| wall | wallColor, patyna: L − 0,05·w, chroma × (1 − 0,25·w) | plaster 0,92; brick 0,88; concrete 0,9; stone 0,7; wood 0,8 | plaster 2,0 m; brick 1,0; concrete 2,4; stone 1,8; wood 1,6 | w = weathering |
| accent | accentColor (patyna łagodniejsza) | 0,9 | brick lub plaster 2 m | pasteloza |
| trim | trimColor | 0,85 | plaster 3 m | detal, gzymsy |
| plinth | plinthColor | 0,9 | brick (ściany ceglane) / stone 1,6 m | cokół, kominy |
| roof | roofColor | tiles 0,75; sheet 0,55/0,35; tarpaper 0,95; seam 0,45/0,5; shingle 0,9 | tiles 1,2; sheet 1; tarpaper 3; seam 1; shingle 1 | dwustronny |
| glass / glassLit | glassTint | 0,06 / 0,4 | — | glassLit: emisja `#ffc27a` nocą |
| frame | frameColor | 0,6 | — | stolarka |
| slab | mieszanka koloru ściany (ciemniej, mniej nasycony) z `#9a9893` 60% | 0,92 | concrete 2 m | płyty balkonów, schody, pełne balustrady |
| metal | `#2e3133` | 0,45 / 0,6 | — | pochwyty, daszki |
| railing | `#2b2d2f` | 0,5 / 0,5 | pręty (alfa) 1 m | dwustronny |
| glassRail | `#a9c3cc`, krycie 0,32 | 0,05 / 0,1 | — | dwustronny, przezroczysty |
| door | frameColor ciemniej o 0,35 L | 0,6 | — | |
| joint | wallColor ciemniej o 0,28 L | 0,95 | — | fugi, styki płyt |
| ground / road / sidewalk | `#7d8a6a` / `#45484b` / `#a6a49c` | ~1 | — / asfalt 4 m / kostka 1 m | kontekst ulicy |

---

## 7. Ulica (`generateStreet`)

```text
profil = CITY_PROFILES[id] (wagi archetypów, liczba działek, domieszka) albo własne wagi
rnd = mulberry32(seed)
dla i = 0..count−1:                     // kolejność losowań ma znaczenie dla odtwarzalności
  a = pickWeighted(wagi, rnd())
  jeśli rnd() < mixing: b = pickWeighted(wagi, rnd()); jeśli b ≠ a: t = 0.15 + 0.3·rnd(); wagi budynku {a: 1−t, b: t}
  bseed = ⌊hash01(seed, i, 3)·1e9⌋
  model = generateFromSpec({weights, seed: bseed, variation})       // domyślnie variation 0,35
rozstaw wzdłuż X: ciągła pierzeja, gdy obaj sąsiedzi mają sideGap < 0.5, inaczej odstęp max(sideGap obu)
offset budynku = [x + szer/2, 0, −streetSetback]; na końcu wyśrodkowanie całej pierzei
kontekst: chodnik z ∈ [0, 3.6] (+0,12), jezdnia [3.6, 11.4], chodnik [11.4, 15], linia środkowa przerywana (3 m co 6 m), 20 m zapasu po bokach
```

Profile (załącznik D): Warszawa – Śródmieście Południowe, Łódź – Śródmieście, Wrocław – Przedmieście Oławskie, Gdynia – Śródmieście, Górny Śląsk – osiedle patronackie, blokowisko PRL, przedmieście domów jednorodzinnych, nowe osiedle deweloperskie. Wagi są orientacyjne (kalibracja projektowa na podstawie opisów i danych NSP 2021, nie pomiar konkretnej ulicy).

Ograniczenia: ulica prosta, bez narożników; sąsiednie budynki w ciągłej pierzei mają **pokrywające się ściany szczytowe** (każdy budynek generuje własną) — w prawdziwym generatorze usuwaj wspólne ściany na poziomie kwartału.

---

## 8. Eksport

- **GLB (glTF 2.0):** jeden węzeł na budynek (`translation` = offset), jeden prymityw na materiał; atrybuty POSITION (z min/max), NORMAL, TEXCOORD_0 (UV w metrach), indeksy `uint32`; materiał PBR metallic-roughness (`baseColorFactor` w przestrzeni liniowej), `doubleSided`, `alphaMode: BLEND` dla przezroczystych; bufor wyrównany do 4 B. Sprawdzone wczytaniem przez `GLTFLoader` (three.js).
- **OBJ + MTL:** obiekt na budynek, `usemtl` na materiał, `Kd` w sRGB, `Ns` z szorstkości, `d` = krycie.
- **ZIP (STORE, CRC32)** w przeglądarce — pakuje GLB/OBJ + `specyfikacja.json`.
- **PNG** — renderer programowy (MCP) albo zrzut WebGL (UI).
- Tekstury proceduralne istnieją tylko w podglądzie — **eksport ich nie zawiera** (sam kolor + UV w metrach).

---

## 9. Renderer programowy (podglądy MCP)

Cel: obrazy PNG bez GPU i bez zależności (Node). Plik `render/raster.ts`.

- Ujęcia (azymut / elewacja kamery): iso 34°/17°, front 0°/5°, street 20°/9°, aerial 32°/50°, back 214°/20°; FOV pionowy 30°; cel na 42% wysokości sceny; odległość dobierana tak, by wszystkie narożniki prostopadłościanu sceny mieściły się w kadrze (marginesy 0,92/0,9).
- Światło: kierunek dzienny (−0,5; 0,74; 0,46), nocny (0,3; 0,9; 0,35). Mapa cieni 1600² (rzut ortogonalny wzdłuż światła, maksimum głębokości, bias zależny od rozmiaru sceny).
- Cieniowanie: ambient (0,36 + 0,3·(n_y·0,5 + 0,5)) + słońce 0,95·max(0, n·L) z testem cienia; szkło: 0,5·kolor + odbicie nieba z Fresnelem 0,22 + 0,6·(1 − n·v)^2,2; noc: okna `glassLit` świecą.
- Przezroczyste (balustrady) rysowane po nieprzezroczystych, sortowane od tyłu.
- Krawędzie: przyciemnienie przy skoku głębokości > 1,8%; teren nie jest obrysowywany (maska terenu).
- Teren „do horyzontu”: siatka 40 × 40 pól wokół sceny + tło poniżej linii horyzontu w kolorze oświetlonego terenu; mgiełka rosnąca z odległością (od dystansu kamery, skala 1,6·dystans, maks. 0,97) zlewa teren z gradientem nieba, którego dół wypada dokładnie na horyzoncie.
- SSAA 1–3× (filtr pudełkowy), mapowanie tonów `c / (1 + 0.12c)·1.08`, sRGB. Opcja `transparent` (bez nieba i terenu) — miniatury.
- Czasy: kamienica 900 × 600 ≈ 0,3–0,8 s, ulica 1400 × 700 ≈ 1 s (Node, jeden wątek).

W nowej implementacji można go zastąpić prawdziwym rendererem (np. headless WebGL/Vulkan, Blender), ale zalety obecnego — brak zależności i deterministyczny obraz — są cenne dla MCP i testów obrazowych.

---

## 10. Aplikacja webowa (prototyp UI)

Stos: React 19, TypeScript, three.js r170, Vite 8; budowa jednoplikowa (artefakt claude.ai) przez `vite-plugin-singlefile` + `scripts/build-artifact.mjs`.

Architektura:

```text
App (stan: tryb, BuildingState, StreetState, MorphState)
 ├─ GenClient ── Web Worker (gen/worker.ts → runJob) ── rdzeń src/core
 │    kolejka „ostatnie wygrywa” (w locie ≤ 1 zadanie, nowsze nadpisuje oczekujące),
 │    tablice siatek przekazywane bez kopiowania (transfer), awaryjnie wątek główny (watchdog 6 s)
 ├─ Viewport → Viewer (three.js) — bez WebGL: obraz z renderera programowego
 └─ panele: Metryka · TypologyMap + VectorMix · MorphPanel · ArchetypeGrid · ParamPanel · SeedPanel · ExportPanel · McpPanel · Street*
```

Model stanu interpolacji (to jest realizacja „uzupełniania pomiędzy wektorami” w UI):

```ts
type Source = { kind: 'cursor'; cursor: MapPoint } | { kind: 'weights'; weights: WeightMap };
interface BuildingState {
  linked: boolean;                       // jeden kursor dla wszystkich wektorów
  active: GroupId;                       // wektor edytowany w trybie „osobno”
  sources: Record<GroupId, Source>;      // osobne źródło wag dla każdego wektora
  overrides: Partial<BuildingParams>;    // ręczne „przypięcia” parametrów
  seed, variation, sharpness, neighbors, regulations;
}
// spec = { groupCursors (źródła 'cursor'), groupWeights (źródła 'weights'), overrides, seed, ... }
```

- **Mapa typologii (SVG):** przeciąganie kursora (aktywnego wektora albo wszystkich), „magnes” 11 px do punktów archetypów (dokładne 100%), klawiatura (strzałki, Shift ×10), linie wpływu kursor → archetypy (grubość = waga), zachłanne rozmieszczenie podpisów bez kolizji, kursor źródła „weights” rysowany w średniej ważonej położeń.
- **Interpolacja A → B:** wagi {A: 1 − t, B: t} dla wszystkich lub aktywnego wektora; animacja „tam i z powrotem” (okres 8 s, ~20 aktualizacji/s).
- **Parametry:** wartości z mieszanki; zmiana suwaka = nadpisanie (przypięcie) z przyciskiem powrotu.
- **Specyfikacja JSON:** `compactSpec` — zaokrąglenia, zamiana kursora leżącego na archetypie (d < 0,012) na `archetype`, jednolite źródła → pola globalne; import przez `stateFromSpec` (te same reguły pierwszeństwa co resolver).
- **Viewer three.js:** `MeshStandardMaterial` z `MaterialSpec`; proceduralne tekstury canvas 256 px (repeat = 1/textureScale); balustrady prętowe — tekstura alfa z mipmapami i przezroczystością (alfa-test bez mipmap dawał mory, z mipmapami pręty znikały z odległości); szkło z `RoomEnvironment` (PMREM); `NeutralToneMapping`; cienie PCF z kamerą cieni dopasowaną do sceny (mapa 2048/4096); teren z siatką 5 m i mgłą; kadrowanie jak w rendererze programowym; **polityka kadrowania**: po zmianie modelu kamera wraca do kadru tylko, gdy użytkownik jej nie ruszał (lub skala zmieniła się drastycznie); dynamiczne near/far; wybór budynku w ulicy raycastem; tryb nocny; zrzut PNG w wyższej rozdzielczości.
- **Miniatury archetypów** generowane przy budowie (`scripts/thumbs.ts`) rendererem programowym (przezroczyste tło).
- Ograniczenia środowiska artefaktu: pobieranie plików tylko przez capability `downloads` (bez linków `<a download>`), dozwolone rozszerzenia nie obejmują `.glb/.obj` → pakowanie do `.zip`; brak zewnętrznych zasobów poza Google Fonts.

UI nie jest częścią rdzenia — w nowej implementacji potraktuj go jako referencję interakcji, nie kod do przenoszenia 1:1.

---

## 11. Serwer MCP

Plik `src/mcp/server.ts`, SDK `@modelcontextprotocol/sdk` 1.32 (`McpServer.registerTool`, `StdioServerTransport`), schematy zod 4. Zbudowany bundel: `dist-mcp/server.mjs` (bez zależności). Katalog wyjściowy: `PIERZEJA_OUT` (domyślnie `~/pierzeja-out`), nazwy plików bez nadpisywania (`-2`, `-3`, …).

| Narzędzie | Wejście (oprócz pól `BuildingSpec`) | Wynik |
|---|---|---|
| `list_archetypes` | — | JSON: id, nazwa, lata, mapa, opis |
| `describe_archetype` | `id` | archetyp + pełny wektor |
| `get_parameter_schema` | `group?` | definicje parametrów |
| `list_city_profiles` | — | profile ulic |
| `resolve_parameters` | spec | wagi per wektor, opis mieszanki, datowanie, dominujący, pełne parametry, ostrzeżenia |
| `generate_building` | spec + `format` (glb/obj/both), `preview`, `view`, `name` | metryka tekstem, ścieżki plików (.glb/.obj/.mtl/.json/.png), obraz PNG |
| `render_building` | spec + `view`, `width`, `height`, `night`, `azimuth`, `elevation` | metryka + PNG |
| `morph_sequence` | `from`, `to`, `steps` (2–9), `groups?`, `seed`, `view`, `save_glb` | szereg budynków A→B obok siebie na jednym PNG (+ GLB), opis kroków |
| `generate_street` | `profile` lub `weights`, `count`, `seed`, `mixing`, `variation`, `format`, `preview`, `view`, `night`, `name` | skład ulicy, pliki, PNG |

Walidacja: nieznane id archetypów → błąd z listą poprawnych; nieznane klucze `overrides` → ostrzeżenie (pominięte); wyjątki → `isError`. Test: `npm run build:mcp && npm run test:mcp` (prawdziwy klient SDK przez stdio, wszystkie narzędzia + przypadek błędu). Konfiguracja Claude Code: `claude mcp add pierzeja -- node …/dist-mcp/server.mjs`.

---

## 12. Weryfikacja

### 12.1 Co sprawdzono w prototypie

- `tsc --noEmit` (strict), budowa Vite i bundla MCP.
- Rendery kontrolne wszystkich 19 archetypów (`scripts/render-all.ts`) — przegląd wizualny; na tej podstawie poprawiono błędy z 6.5, 6.14 i 13.1.
- Playwright (Chromium, SwiftShader): komputer 1440 × 900 i telefon 390 × 844, motyw jasny i ciemny; interakcje: tryb „osobno” (bryła bloku + elewacja kamienicy), noc, ulica, wybór i edycja budynku z ulicy, animacja A→B, falowiec.
- Klient MCP SDK: 9 narzędzi + błąd; pliki GLB wczytane przez `GLTFLoader`.
- Wydajność (Node, generowanie): większość archetypów 1–20 ms, socrealizm ~22 ms, falowiec ~200 ms / 283 tys. trójkątów; ulica 9–10 budynków 60–90 ms / 100–150 tys. trójkątów.

### 12.2 Proponowany zestaw testów dla nowej implementacji

1. **Resolver (dokładne):** wagi IDW, mieszanie 50/50, kolor OKLab, wariacja z seedem — wartości z załącznika G (tolerancja 1e-6; kolory co do znaku hex).
2. **Determinizm:** ten sam spec → identyczny hash bufora siatki; różne seedy → różne siatki (gdy `variation > 0` lub elementy losowe).
3. **Własności:** wagi sumują się do 1; kursor na archetypie odtwarza jego pełny wektor; każdy parametr w zakresie; brak NaN/Inf w pozycjach; bounds zgodne z metryką (wys. ≈ heightTotal).
4. **Ciągłość:** mały ruch kursora → mała zmiana parametrów ciągłych (poza progami dyskretnymi z 4.1).
5. **Metryka:** dla każdego archetypu liczba kondygnacji, wysokości i klasa zgodne z G.4 (tolerancja geometryczna przy innej dokładności); budżet trójkątów.
6. **Reguły:** apartamentowiec → notatki § 54 i § 303 i brak balkonów wystających powyżej 25 m; blok 11k z lat 70. → brak reguły § 303 (rok < 1990).
7. **Obrazy:** render każdego archetypu w 3 ujęciach, porównanie percepcyjne z wzorcem (tolerancja), przegląd człowieka przy zmianach reguł.
8. **Eksport:** walidacja glTF (np. Khronos glTF-Validator), wczytanie OBJ, poprawne `min/max` akcesorów.

---

## 13. Znane problemy i lekcje

### 13.1 Błędy znalezione i naprawione w trakcie (nie powtarzaj)

| Objaw | Przyczyna | Rozwiązanie |
|---|---|---|
| Bloki z płyty bez balkonów na froncie | wzór „od środka” trafiał w oś klatki (zarezerwowaną) | wzór „od krańców” dla gęstości ≥ 0,4 (6.5) |
| „Piła” dachów kopertowych na socrealizmie | osobny dach na każdy segment | scalanie przyległych segmentów (6.14) |
| Okap mansardy za nisko | obniżenie okapu liczone pełnym nachyleniem stromego etapu | `min(slope, 0.75)` |
| Lizeny i gzymsy na ślepych ścianach szczytowych | brak warunku roli | pomijanie przy `side` i `sideWindows < 0.3` |
| Lukarny „w powietrzu” na końcach dachu kopertowego | brak testu zasięgu połaci | pomijanie lukarny, gdy abs(s) + dw > L (6.14) |
| Uwaga „PRL: winda” dla kamienicy z 1908 r. | brak zakresu lat | warunek 1950 ≤ rok < 1990 |
| Okna klatek wychodzące poza kondygnację | wysokość niezależna od wiersza | `min(1.35, rowH − 0.55)` |
| Podwójny parapet i detal na drzwiach balkonowych | drzwi traktowane jak okno | flaga `bd` |
| Detal okien w cofniętych podcieniach | detal liczony na ramie elewacji | pomijanie parteru przy podcieniach/boniowaniu |
| Schody portyku schowane pod podestem | wspólna logika schodów | portyk rysuje własny podest i schody |
| Falowiec z 10 segmentami: 471 tys. trójkątów | długi szereg segmentów, każde okno i balkon jako osobna geometria | domyślnie 6 segmentów (docelowo: instancjonowanie) |
| Balustrady prętowe znikały / migotały w WebGL | alfa-test na mipmapach | przezroczystość z mipmapami |
| Kamera uciekała przy edycji | kadrowanie po każdej zmianie | polityka „użytkownik ruszał kamerę” |
| `z.record(enum, …)` w zod 4 wymagał wszystkich kluczy | zmiana semantyki w zod 4 | `z.partialRecord` |

### 13.2 Ograniczenia pozostające w prototypie

- Rzuty wyłącznie prostokątne (segmenty, oficyny, uskoki); brak narożników ulic, ściętych narożników, rzutów L/U poza oficynami, łuków (obłe narożniki Gdyni).
- Brak grubości murów i wnętrz; ściany to powierzchnie (ościeża symulują grubość). Siatka nie ma gwarancji szczelności (manifold).
- Okna i balkony nie są instancjonowane — duże budynki są ciężkie.
- Brak wspólnych ścian i podwórzy w kwartale; tylko prosta ulica.
- Jeden zestaw progów i jitterów dla wszystkich archetypów (zamiast rozkładów per typ).
- Klasa wysokości liczona uproszczonym H (3.3); szacunek mieszkań heurystyczny.
- Tekstury proceduralne tylko w podglądzie; eksport bez tekstur.
- Lukarny zawsze z daszkiem dwuspadowym; brak wolich oczek, facjat i okien połaciowych; jeden kolor stolarki na cały budynek.

---

## 14. Rekomendacje dla „prawdziwego” generatora

### 14.1 Architektura

```text
crate/pakiet core
  schema/      definicje parametrów (jednostki, zakresy, progi), wersjonowane
  data/        archetypy jako dane (JSON/TOML/YAML) + źródła + pewność kalibracji
  resolve/     wagi (IDW, jawne, per wektor), mieszanie (liczby, OKLab, głosowanie), wariacja, nadpisania
  massing/     bryły z rzutu (wielokąty!), uskoki, oficyny, penthouse → lista Volume
  facade/      układ elewacji: osie × kondygnacje → komórki → instancje elementów (dane semantyczne, bez siatki)
  elements/    biblioteka elementów (okno, balkon, loggia, wejście, wykusz, lukarna, gzyms) z parametrami
  roof/        dachy (płaski, dwuspadowy, kopertowy, mansardowy) — docelowo straight skeleton
  rules/       przepisy (WT 2002 i starsze), walidacja i notatki
  mesh/        budowa siatek, instancjonowanie, LOD
  export/      glTF (EXT_mesh_gpu_instancing, extras z metadanymi), OBJ, CityJSON/CityGML
  render/      podgląd offline (opcjonalnie)
aplikacje: mcp-server, cli, web (WASM albo port TS)
```

Kluczowa zmiana względem prototypu: **oddziel układ elewacji (dane) od geometrii**. W prototypie `facade()` od razu buduje siatkę; w nowej wersji niech zwraca listę elementów (typ, pozycja w układzie elewacji, wymiary, piętro, oś, materiały), a dopiero osobny etap tworzy geometrię. To umożliwi instancjonowanie, LOD, eksport semantyczny i testy bez geometrii.

### 14.2 Zasady, które warto zachować

- `BuildingSpec` jako kontrakt (zgodność wsteczna; dodaj jawne pole wersji, np. `version`).
- Pierwszeństwo źródeł wag (4.4) i semantyka mieszania (5.2).
- Hash-RNG po ścieżce elementu (5.5) zamiast globalnego strumienia.
- Archetypy = dane z przypiętymi źródłami.
- Reguły przepisów jako osobny, testowalny moduł z notatkami w metryce.

### 14.3 Rozszerzenia typologii i geometrii (priorytety)

1. **Działki narożne i rzuty wielokątne** — kamienica narożna (ścięty narożnik, wieżyczka, dwie elewacje frontowe), modernistyczne obłe narożniki (Gdynia), rzuty L/U; dachy przez straight skeleton.
2. **Kwartał** — wspólne ściany, podwórza-studnie, oficyny sąsiadów, przejazdy bramne łączące ulicę z podwórzem.
3. **Instancjonowanie i LOD** — okno/balkon jako prototyp + transformacje; LOD1 (bryła), LOD2 (dachy), LOD3 (elewacje).
4. **Rozkłady zamiast punktów** — dla każdego archetypu średnia i rozrzut parametrów (np. z danych), wariacja per typ.
5. **Nowe archetypy:** kamienica narożna, punktowiec, galeriowiec, Superjednostka, osiedle WSM (lata 30.), blok Nikiszowca (zamknięty kwartał), dwór w „stylu dworkowym” (lata 20.), pawilon socmodernistyczny, kamienica po termomodernizacji.
6. **Modyfikatory epok** jako operatory na wektorze: termomodernizacja/pasteloza (głębsze ościeża, nowe kolory), nadbudowa, przebudowa parteru na sklepy, wymiana stolarki — pozwolą „starzeć” budynek w czasie.
7. **Wnętrza (opcjonalnie):** rzut z normatywów (M-1…M-5), klatki, układ mieszkań → rozmieszczenie okien.

### 14.4 Kalibracja i walidacja na danych

- **Modele 3D budynków z Geoportalu (GUGiK):** CityGML 2.0, **LoD1 dla całego kraju** (wysokości z LIDAR), **LoD2 dla 236 powiatów w 10 województwach**, bezpłatne do dowolnego wykorzystania — porównuj wysokości, dachy i rzuty z wygenerowanymi typami ([geoportal.gov.pl — Modele 3D budynków](https://www.geoportal.gov.pl/pl/dane/inne-dane/modele-3d-budynkow/)).
- BDOT10k (obrysy budynków) i dane NSP 2021 (zasób wg okresu budowy) — do profili ulic i dzielnic.
- OpenStreetMap (`building:levels`, `roof:shape`) — dodatkowa walidacja liczby kondygnacji i kształtu dachu.
- Literatura typologiczna (rozdz. 3) — do opisu rozkładów, nie tylko wartości średnich.

### 14.5 Wskazówki językowe

| Język | Matematyka / geometria | Eksport | MCP | Uwagi |
|---|---|---|---|---|
| Rust | `glam`, własne prymitywy; wielokąty: `geo`; straight skeleton — biblioteka lub własna implementacja | `gltf-json` + własny zapis GLB, `meshopt` | oficjalny SDK `rmcp` | `rayon` dla ulic; WASM (`wasm-bindgen`) do podglądu w przeglądarce — zwracaj `Float32Array` per materiał (jak `MeshData`) |
| Python | `numpy`; `shapely` dla rzutów; `scikit-geometry`/`polyskel` dla straight skeleton | `trimesh`, `pygltflib` | oficjalny SDK `mcp` (FastMCP) | opcjonalnie Blender (`bpy`) do renderów; pilnuj 32-bitowej arytmetyki w RNG |
| TypeScript | obecny `src/core` | obecne eksportery albo `@gltf-transform/core` | `@modelcontextprotocol/sdk` | podziel na pakiety (core, mcp, web), dodaj testy `vitest` z załącznika G |

### 14.6 Budżety i jakość

- Budynek typowy: < 20 ms, < 30 tys. trójkątów (bez instancjonowania); ulica 10 budynków: < 150 ms.
- Falowiec pełnej długości (~860 m) dopiero po instancjonowaniu.
- Każda zmiana reguł → rendery wszystkich archetypów + przegląd człowieka (tak wykryto większość błędów z 13.1).

---

# Załączniki

Załączniki A–E i G są generowane z kodu: `npx tsx scripts/handoff-data.ts` (zapisuje `docs/handoff/*.md` i `docs/pierzeja-dane.json`).

## Załącznik A — Parametry (71)

Kolumna *jitter*: amplituda wariacji jako ułamek zakresu przy `variation = 1`. Etykiety i opisy jak w UI.

#### Bryła (`massing`) — Rzut, segmenty, oficyny, uskoki i położenie na działce.

| klucz | typ | zakres | jedn. | domyślnie | jitter | znaczenie |
|---|---|---|---|---|---|---|
| `segments` | int | 1–20 |  | 1 | 0.04 | Segmenty / klatki — Moduły wzdłuż ulicy; w blokach odpowiadają klatkom schodowym. |
| `segmentWidth` | number | 5–60 | m | 14 | 0.03 | Szerokość segmentu |
| `depth` | number | 5–30 | m | 12 | 0.04 | Głębokość traktu |
| `wingLeft` | number | 0–1 |  | 0 | 0 | Oficyna lewa — 0 = brak, 1 = pełna długość. |
| `wingRight` | number | 0–1 |  | 0 | 0 | Oficyna prawa |
| `wingRear` | number | 0–1 |  | 0 | 0 | Oficyna tylna — Zamyka podwórze-studnię, gdy są obie oficyny boczne. |
| `wingLength` | number | 2–40 | m | 12 | 0.04 | Długość oficyn |
| `wingWidth` | number | 3–15 | m | 7 | 0.04 | Szerokość oficyn |
| `stagger` | number | 0–10 | m | 0 | 0.02 | Przesunięcie segmentów — Naprzemienne cofnięcie segmentów (falowiec). |
| `setbackFloors` | int | 0–4 |  | 0 | 0 | Cofnięte piętra |
| `setbackDepth` | number | 0–8 | m | 0 | 0.04 | Głębokość cofnięcia |
| `streetSetback` | number | 0–30 | m | 0 | 0.04 | Odsunięcie od ulicy — Używane w trybie ulicy. |
| `sideGap` | number | 0–30 | m | 0 | 0.04 | Odstęp od sąsiadów — 0 = zabudowa pierzejowa. |

#### Kondygnacje (`storeys`) — Liczba i wysokość kondygnacji, cokół.

| klucz | typ | zakres | jedn. | domyślnie | jitter | znaczenie |
|---|---|---|---|---|---|---|
| `floors` | int | 1–30 |  | 4 | 0.035 | Kondygnacje nadziemne |
| `groundFloorHeight` | number | 2.5–6 | m | 3.2 | 0.03 | Wysokość parteru |
| `floorHeight` | number | 2.4–4.8 | m | 2.9 | 0.02 | Wysokość piętra |
| `floorHeightDecay` | number | 0–0.35 |  | 0 | 0.02 | Malenie pięter ku górze — Piano nobile najwyższe, wyższe piętra niższe. |
| `plinthHeight` | number | 0–2 | m | 0.4 | 0.04 | Cokół |
| `basementWindows` | number | 0–1 |  | 0.5 | 0 | Okienka piwnic |

#### Dach (`roof`) — Kształt i spadek dachu, attyka, lukarny, nadbudówki.

| klucz | typ | zakres | jedn. | domyślnie | jitter | znaczenie |
|---|---|---|---|---|---|---|
| `roofPitch` | number | 0–60 | ° | 0 | 0.05 | Kąt dachu — Poniżej 3° dach jest płaski. |
| `roofHip` | number | 0–1 |  | 0 | 0.02 | Kopertowość — 0 = dwuspadowy ze szczytami, 1 = kopertowy. |
| `mansard` | number | 0–1 |  | 0 | 0.02 | Mansarda (dach łamany) |
| `ridgeAlongFront` | number | 0–1 |  | 1 | 0 | Kalenica wzdłuż ulicy — 0 = szczyt od ulicy. |
| `roofOverhang` | number | 0–1.5 | m | 0.3 | 0.03 | Okap |
| `parapetHeight` | number | 0–1.6 | m | 0 | 0.03 | Attyka |
| `dormers` | number | 0–1 |  | 0 | 0.05 | Lukarny |
| `chimneys` | number | 0–1 |  | 0.3 | 0.08 | Kominy |
| `rooftopBoxes` | number | 0–1 |  | 0 | 0.03 | Nadbudówki (maszynownie) |
| `gableGlazing` | number | 0–1 |  | 0 | 0.03 | Przeszklenie szczytu |
| `turret` | number | 0–1 |  | 0 | 0 | Wieżyczka narożna |

#### Elewacja (`facade`) — Rytm osi, okna, balkony, wykusze, gzymsy i detal.

| klucz | typ | zakres | jedn. | domyślnie | jitter | znaczenie |
|---|---|---|---|---|---|---|
| `bayWidth` | number | 1.2–8 | m | 3 | 0.03 | Rozstaw osi okiennych |
| `cornerMargin` | number | 0–3 | m | 0.6 | 0.03 | Pełny mur przy narożniku |
| `windowWidth` | number | 0.4–7 | m | 1.4 | 0.03 | Szerokość okna |
| `windowHeight` | number | 0.4–3.4 | m | 1.5 | 0.03 | Wysokość okna |
| `sillHeight` | number | 0–1.4 | m | 0.85 | 0.03 | Wysokość parapetu |
| `windowDepth` | number | 0.03–0.5 | m | 0.15 | 0.02 | Głębokość ościeża |
| `windowArch` | number | 0–1 |  | 0 | 0 | Łuk nad oknem |
| `mullions` | number | 0–1 |  | 0.4 | 0.02 | Podziały okien (szprosy) |
| `ribbon` | number | 0–1 |  | 0 | 0.02 | Okna pasmowe |
| `groundShop` | number | 0–1 |  | 0 | 0.08 | Witryny w parterze |
| `groundRecess` | number | 0–4 | m | 0 | 0 | Podcienia |
| `entranceType` | enum | `door`, `gate`, `portico`, `glass` |  | `door` | 0 | Wejście |
| `entranceCanopy` | number | 0–2.5 | m | 0.8 | 0.04 | Daszek nad wejściem |
| `stairWindows` | number | 0–1 |  | 0 | 0 | Okna klatek schodowych |
| `balconies` | number | 0–1 |  | 0 | 0.06 | Balkony (gęstość) |
| `balconyFrom` | int | 1–6 |  | 1 | 0 | Balkony od piętra |
| `balconyDepth` | number | 0–3 | m | 1.2 | 0.03 | Głębokość balkonu |
| `balconyWidth` | number | 0.3–1 |  | 0.7 | 0.03 | Szerokość balkonu — Ułamek rozstawu osi; 1 = ciągła galeria. |
| `loggia` | number | 0–1 |  | 0 | 0.03 | Loggia (wcięcie) |
| `railing` | number | 0–1 |  | 0.5 | 0 | Balustrada — 0 = pełna, 0,5 = pręty, 1 = szkło. |
| `oriels` | number | 0–1 |  | 0 | 0.04 | Wykusze |
| `orielDepth` | number | 0.3–1.5 | m | 0.8 | 0.03 | Wysięg wykusza |
| `cornice` | number | 0–1.2 | m | 0.1 | 0.04 | Gzyms koronujący |
| `stringCourses` | number | 0–1 |  | 0 | 0.04 | Gzymsy międzykondygnacyjne |
| `ornament` | number | 0–1 |  | 0 | 0.04 | Detal (opaski, naczółki) |
| `pilasters` | number | 0–1 |  | 0 | 0.04 | Lizeny / pilastry |
| `rustication` | number | 0–1 |  | 0 | 0.02 | Boniowanie parteru |
| `panelJoints` | number | 0–1 |  | 0 | 0 | Styki płyt |
| `sideWindows` | number | 0–1 |  | 1 | 0.04 | Okna w ścianach bocznych — 0 = ślepa ściana szczytowa. |
| `asymmetry` | number | 0–1 |  | 0 | 0.05 | Asymetria |

#### Materiał (`material`) — Tekstury, kolory, pokrycie dachu, patyna.

| klucz | typ | zakres | jedn. | domyślnie | jitter | znaczenie |
|---|---|---|---|---|---|---|
| `wallTexture` | enum | `plaster`, `brick`, `concrete`, `stone`, `wood` |  | `plaster` | 0 | Materiał ścian |
| `wallColor` | color | sRGB `#rrggbb` |  | `#d8d2c4` | 0.04 | Kolor ścian |
| `trimColor` | color | sRGB `#rrggbb` |  | `#e8e3d8` | 0.04 | Kolor detalu |
| `plinthColor` | color | sRGB `#rrggbb` |  | `#7c776d` | 0.04 | Kolor cokołu |
| `accentColor` | color | sRGB `#rrggbb` |  | `#9bb7c9` | 0.04 | Kolor akcentów |
| `accentPanels` | number | 0–1 |  | 0 | 0.06 | Pasteloza / panele akcentowe |
| `roofTexture` | enum | `tiles`, `sheet`, `tarpaper`, `seam`, `shingle` |  | `tarpaper` | 0 | Pokrycie dachu |
| `roofColor` | color | sRGB `#rrggbb` |  | `#4a4744` | 0.04 | Kolor dachu |
| `frameColor` | color | sRGB `#rrggbb` |  | `#efefea` | 0.04 | Kolor stolarki |
| `glassTint` | color | sRGB `#rrggbb` |  | `#3e5260` | 0.01 | Odcień szyb |
| `weathering` | number | 0–1 |  | 0.2 | 0.06 | Patyna |

## Załącznik B — Archetypy na mapie typologii

| id | nazwa | lata | mapa (rok; skala) |
|---|---|---|---|
| `dworek` | Dworek klasycystyczny (Dworek) | 1800–1860 | 1830; 0.06 |
| `kamienica_eklektyczna` | Kamienica czynszowa eklektyczna (Kamienica) | 1870–1905 | 1890; 0.62 |
| `kamienica_secesyjna` | Kamienica secesyjna (Secesja) | 1900–1914 | 1908; 0.68 |
| `familok` | Familok śląski (Familok) | 1870–1920 | 1893; 0.36 |
| `willa_drewniana` | Willa drewniana (świdermajer) (Świdermajer) | 1880–1930 | 1905; 0.1 |
| `willa_modernistyczna` | Willa modernistyczna (Willa 1930) | 1928–1939 | 1932; 0.12 |
| `kamienica_modernistyczna` | Kamienica modernistyczna (Modernizm) | 1926–1939 | 1934; 0.64 |
| `socrealizm` | Blok socrealistyczny (Socrealizm) | 1949–1956 | 1953; 0.72 |
| `blok_poodwilzowy` | Blok poodwilżowy (Blok 1960) | 1956–1968 | 1962; 0.5 |
| `blok_plyta_5` | Blok z wielkiej płyty, 5 kondygnacji (Płyta 5k) | 1970–1990 | 1976; 0.7 |
| `blok_plyta_11` | Blok z wielkiej płyty, 11 kondygnacji (Płyta 11k) | 1970–1990 | 1978; 0.88 |
| `falowiec` | Falowiec (Falowiec) | 1967–1973 | 1971; 1 |
| `kostka` | Kostka PRL (Kostka) | 1960–1985 | 1972; 0.14 |
| `dom_90` | Dom lat 90. (Dom 90.) | 1990–2005 | 1997; 0.1 |
| `blok_90` | Blok lat 90. (Blok 90.) | 1990–2002 | 1996; 0.55 |
| `deweloperski` | Blok deweloperski (Deweloper) | 2000–2012 | 2006; 0.6 |
| `plomba` | Plomba w pierzei (Plomba) | 1995–2015 | 2008; 0.7 |
| `apartamentowiec` | Apartamentowiec (Apartamenty) | 2008–2015 | 2012; 0.86 |
| `stodola` | Nowoczesna stodoła (Stodoła) | 2010–2015 | 2013; 0.06 |

## Załącznik C — Archetypy: opisy, cechy, źródła i nadpisania

Wektor archetypu = wartości domyślne z załącznika A nadpisane poniższymi. Pełne wektory: `docs/pierzeja-dane.json` → `archetypes[].vector`.

#### `dworek` — Dworek klasycystyczny (1800–1860)

Parterowy, symetryczny dom szlachecki z portykiem na osi i wysokim, łamanym dachem czterospadowym.

Cechy: portyk 2–4 kolumny z trójkątnym frontonem; dach polski łamany kryty gontem; bielone ściany na podmurówce.

Źródła: [Wikipedia – Dwór (manor house)](https://www.wikipedia.org/wiki/Dw%C3%B3r_(manor_house)) · [ZPE – Dwór szlachecki](https://zpe.gov.pl/a/miejsce-akcji---dwor-szlachecki/Dl3Ih2NOo)

```text
segments: 1, segmentWidth: 22, depth: 12, streetSetback: 14, sideGap: 14, floors: 1, groundFloorHeight: 3.6, floorHeight: 3.2, plinthHeight: 0.6, basementWindows: 0.6, roofPitch: 42, roofHip: 1, mansard: 0.65, ridgeAlongFront: 1, roofOverhang: 0.7, parapetHeight: 0, dormers: 0.25, chimneys: 0.7, bayWidth: 2.9, cornerMargin: 1.4, windowWidth: 1.15, windowHeight: 1.85, sillHeight: 0.75, windowDepth: 0.22, mullions: 0.95, entranceType: 'portico', entranceCanopy: 0, cornice: 0.35, ornament: 0.35, pilasters: 0.2, sideWindows: 1, wallTexture: 'plaster', wallColor: '#f1eee6', trimColor: '#fbf9f3', plinthColor: '#9a958a', accentColor: '#cfd9cf', roofTexture: 'shingle', roofColor: '#6a5a48', frameColor: '#f3f1ea', glassTint: '#3a4a52', weathering: 0.25
```

#### `kamienica_eklektyczna` — Kamienica czynszowa eklektyczna (1870–1905)

Budynek frontowy z oficynami wokół podwórza, wjazd bramą, ślepe ściany na granicy działki, bogaty detal malejący ku górze.

Cechy: parcele 22–29 m szerokości; piano nobile z najcięższym detalem; boniowany parter z witrynami; zasada „im wyżej, tym lżej”.

Źródła: [Kilanowski, Ochrona Zabytków 1/2015 (NID)](https://ochronazabytkow.nid.pl/wp-content/uploads/2019/08/OZ_1-2015_11_Kilanowski.pdf) · [Roguska, KAiU 4/2010 – balkony kamienic warszawskich](https://oldkaiu.pan.pl/images/stories/4.2010/J.Roguska2.pdf) · [Domińczak, Zaguła – Typologia łódzkiej kamienicy](https://architektmiasta.uml.lodz.pl/files/public/dla_mieszkanca/o-miescie/inne_publikacje/TRYPOL_SCALONA.pdf)

```text
segments: 1, segmentWidth: 22, depth: 14, wingLeft: 1, wingRight: 1, wingRear: 0, wingLength: 18, wingWidth: 7, streetSetback: 0, sideGap: 0, floors: 5, groundFloorHeight: 4.3, floorHeight: 3.7, floorHeightDecay: 0.16, plinthHeight: 0.6, basementWindows: 1, roofPitch: 18, roofHip: 0.2, mansard: 0, ridgeAlongFront: 1, roofOverhang: 0.1, parapetHeight: 0.5, chimneys: 0.95, bayWidth: 2.9, cornerMargin: 1.2, windowWidth: 1.25, windowHeight: 2.25, sillHeight: 0.8, windowDepth: 0.36, windowArch: 0.15, mullions: 0.8, groundShop: 0.65, entranceType: 'gate', entranceCanopy: 0, balconies: 0.16, balconyFrom: 1, balconyDepth: 0.95, balconyWidth: 0.6, railing: 0.45, cornice: 1, stringCourses: 0.75, ornament: 0.85, pilasters: 0.45, rustication: 0.95, sideWindows: 0, wallTexture: 'plaster', wallColor: '#d7c6a6', trimColor: '#e9dfc9', plinthColor: '#8a7d6b', accentColor: '#c9b48d', roofTexture: 'sheet', roofColor: '#5b5852', frameColor: '#5a4636', glassTint: '#33424c', weathering: 0.45
```

#### `kamienica_secesyjna` — Kamienica secesyjna (1900–1914)

Wyższa kamienica przełomu wieków: asymetria, wykusze, spłaszczone łuki okien, mansarda z lukarnami i narożna wieżyczka.

Cechy: wykusze i balkony słabo wysunięte; okna nieregularne, łukowe; po 1905 r. w Warszawie zwykle ≥ 5 pięter.

Źródła: [Janicka, Ochrona Zabytków 4/2001 – secesja w Krakowie](https://bazhum.muzhp.pl/media/texts/ochrona-zabytkow/2001-tom-54-numer-4-215/ochrona_zabytkow-r2001-t54-n4_215-s345-356.pdf) · [Roguska, KAiU 4/2010 – balkony kamienic warszawskich](https://oldkaiu.pan.pl/images/stories/4.2010/J.Roguska2.pdf) · [Kilanowski, Ochrona Zabytków 1/2015 (NID)](https://ochronazabytkow.nid.pl/wp-content/uploads/2019/08/OZ_1-2015_11_Kilanowski.pdf)

```text
segments: 1, segmentWidth: 20, depth: 15, wingLeft: 1, wingRight: 1, wingRear: 0.3, wingLength: 20, wingWidth: 7, streetSetback: 0, sideGap: 0, floors: 6, groundFloorHeight: 4.2, floorHeight: 3.4, floorHeightDecay: 0.1, plinthHeight: 0.6, basementWindows: 1, roofPitch: 50, roofHip: 0.4, mansard: 0.85, ridgeAlongFront: 1, roofOverhang: 0.2, dormers: 0.6, chimneys: 0.8, turret: 0.45, bayWidth: 2.8, cornerMargin: 1, windowWidth: 1.35, windowHeight: 2.2, sillHeight: 0.75, windowDepth: 0.32, windowArch: 0.75, mullions: 0.85, groundShop: 0.5, entranceType: 'gate', balconies: 0.3, balconyFrom: 1, balconyDepth: 0.8, balconyWidth: 0.6, railing: 0.48, oriels: 0.4, orielDepth: 0.85, cornice: 0.55, stringCourses: 0.3, ornament: 0.7, pilasters: 0.2, rustication: 0.4, sideWindows: 0, asymmetry: 0.6, wallTexture: 'plaster', wallColor: '#cfd1bb', trimColor: '#eceadb', plinthColor: '#7f8177', accentColor: '#a9b89a', roofTexture: 'sheet', roofColor: '#4b5655', frameColor: '#4e3b2c', weathering: 0.4
```

#### `familok` — Familok śląski (1870–1920)

Robotniczy dom wielorodzinny z nietynkowanej czerwonej cegły z ceglanym detalem i czerwonymi ramami okien.

Cechy: cegła licówka; łuki odcinkowe nad oknami; dach dwuspadowy; osiedla patronackie przy kopalniach.

Źródła: [Wikipedia – Familok](https://kaweah.freedombox.rocks/kiwix/wikipedia_en_all_maxi_2023-10/Familok) · [dzieje.pl – Nikiszowiec](https://dzieje.pl/node/46821)

```text
segments: 2, segmentWidth: 12.5, depth: 11, streetSetback: 3, sideGap: 6, floors: 3, groundFloorHeight: 3.1, floorHeight: 2.95, plinthHeight: 0.5, basementWindows: 0.8, roofPitch: 38, roofHip: 0, ridgeAlongFront: 1, roofOverhang: 0.35, dormers: 0.2, chimneys: 0.85, bayWidth: 2.7, cornerMargin: 1, windowWidth: 1, windowHeight: 1.55, sillHeight: 0.85, windowDepth: 0.22, windowArch: 0.6, mullions: 0.65, entranceType: 'door', entranceCanopy: 0, stairWindows: 0.6, cornice: 0.25, stringCourses: 0.55, ornament: 0.3, pilasters: 0.25, sideWindows: 0.5, wallTexture: 'brick', wallColor: '#9a4a34', trimColor: '#b05a3e', plinthColor: '#5c3a2d', accentColor: '#7a8b55', roofTexture: 'tiles', roofColor: '#7a3d2f', frameColor: '#b3261e', glassTint: '#384850', weathering: 0.5
```

#### `willa_drewniana` — Willa drewniana (świdermajer) (1880–1930)

Letniskowa willa z drewna z ażurowymi werandami, dużym okapem, szpiczastymi zwieńczeniami i szczytem od ulicy.

Cechy: werandy i ganki; okiennice, rzeźbione deski; linia otwocka.

Źródła: [dzieje.pl – świdermajer](https://dzieje.pl/node/169561) · [Notes from Poland – świdermajer](https://notesfrompoland.com/2023/08/31/polish-town-offers-free-house-designs-based-on-famous-19th-century-style/)

```text
segments: 1, segmentWidth: 12, depth: 10, streetSetback: 12, sideGap: 12, floors: 2, groundFloorHeight: 3, floorHeight: 2.7, plinthHeight: 0.7, basementWindows: 0.4, roofPitch: 45, roofHip: 0, ridgeAlongFront: 0.35, roofOverhang: 1, dormers: 0.3, chimneys: 0.5, turret: 0.25, gableGlazing: 0.12, bayWidth: 2.6, cornerMargin: 0.5, windowWidth: 1, windowHeight: 1.45, sillHeight: 0.8, windowDepth: 0.1, mullions: 0.9, entranceType: 'door', entranceCanopy: 1.6, balconies: 0.4, balconyFrom: 1, balconyDepth: 1.8, balconyWidth: 0.85, railing: 0.5, loggia: 0.15, cornice: 0.15, stringCourses: 0.4, ornament: 0.55, sideWindows: 1, wallTexture: 'wood', wallColor: '#9c7b58', trimColor: '#f0ead9', plinthColor: '#8d5a43', accentColor: '#5f7d5b', roofTexture: 'sheet', roofColor: '#6d5543', frameColor: '#f0ead9', weathering: 0.45
```

#### `willa_modernistyczna` — Willa modernistyczna (1928–1939)

Biała willa funkcjonalistyczna: geometryczna bryła, płaski dach z tarasem, okna pasmowe i balustrady z rur.

Cechy: płaski dach; okna pasmowe; taras na dachu i balkon narożny.

Źródła: [Muzeum Piłsudskiego – Architektura, inspiracje](https://muzeumpilsudski.pl/wp-content/uploads/2019/10/Architektura_inspiracje-.pdf)

```text
segments: 1, segmentWidth: 13, depth: 11, streetSetback: 8, sideGap: 10, floors: 2, groundFloorHeight: 3.1, floorHeight: 3, plinthHeight: 0.5, roofPitch: 0, parapetHeight: 0.45, roofOverhang: 0.25, chimneys: 0.3, bayWidth: 3.4, cornerMargin: 0.5, windowWidth: 2.5, windowHeight: 1.35, sillHeight: 0.9, windowDepth: 0.12, mullions: 0.4, ribbon: 0.65, entranceType: 'door', entranceCanopy: 1.4, balconies: 0.34, balconyFrom: 1, balconyDepth: 1.6, balconyWidth: 0.95, railing: 0.55, loggia: 0.2, cornice: 0.06, ornament: 0, sideWindows: 1, asymmetry: 0.45, wallTexture: 'plaster', wallColor: '#f0eee8', trimColor: '#f8f7f3', plinthColor: '#6d6a65', roofTexture: 'tarpaper', roofColor: '#3f3d3b', frameColor: '#2f3438', weathering: 0.15
```

#### `kamienica_modernistyczna` — Kamienica modernistyczna (1926–1939)

Pierzejowa kamienica funkcjonalizmu i „stylu okrętowego”: płaski dach z attyką, loggie, wykusze, okna pasmowe, jasny tynk z kamiennym cokołem.

Cechy: loggie ze skrzynkami na kwiaty; wykusze i obłe narożniki; okładziny z kamienia; brama na podwórze.

Źródła: [gdynia.pl – Gdyński modernizm](https://www.gdynia.pl/g2/2007_09/12512_fileot.pdf) · [Muzeum Piłsudskiego – Architektura, inspiracje](https://muzeumpilsudski.pl/wp-content/uploads/2019/10/Architektura_inspiracje-.pdf)

```text
segments: 1, segmentWidth: 24, depth: 14, wingLeft: 0.8, wingRight: 0.8, wingLength: 14, wingWidth: 6.5, streetSetback: 0, sideGap: 0, floors: 6, groundFloorHeight: 3.9, floorHeight: 3.05, plinthHeight: 0.5, basementWindows: 0.6, roofPitch: 0, parapetHeight: 0.8, roofOverhang: 0, chimneys: 0.4, rooftopBoxes: 0.15, bayWidth: 3.2, cornerMargin: 0.6, windowWidth: 1.9, windowHeight: 1.55, sillHeight: 0.85, windowDepth: 0.16, mullions: 0.45, ribbon: 0.5, groundShop: 0.55, entranceType: 'gate', entranceCanopy: 0, balconies: 0.4, balconyFrom: 1, balconyDepth: 1.1, balconyWidth: 0.85, loggia: 0.35, railing: 0.55, oriels: 0.25, orielDepth: 0.75, cornice: 0.12, stringCourses: 0.12, ornament: 0.05, sideWindows: 0, asymmetry: 0.25, wallTexture: 'plaster', wallColor: '#ebe6db', trimColor: '#f5f2ea', plinthColor: '#b8b0a0', accentColor: '#c9c0ad', roofTexture: 'tarpaper', roofColor: '#3d3b38', frameColor: '#3b3f42', weathering: 0.3
```

#### `socrealizm` — Blok socrealistyczny (1949–1956)

Monumentalna pierzeja „narodowa w formie”: podcienia, boniowany parter, attyka i wysunięty gzyms, wysokie kondygnacje, wysoki dach kopertowy.

Cechy: arkady i podcienia; attyki i gzymsy; zabudowa pierzejowa i osiowa; MDM, Nowa Huta, KDM.

Źródła: [dzieje.pl – Nowa Huta](https://dzieje.pl/node/101444) · [Przesmycka, Teka KAiU PAN – dzielnice socrealizmu](https://ph.pollub.pl/index.php/teka/issue/download/133/68) · [Sumorok – Między modernizmem a socrealizmem](https://gdynia.pl/modernizm/module/Files/controller/Default/action/downloadFile/hash/21db1ff53eedd0db225519f95c67e51c)

```text
segments: 3, segmentWidth: 20, depth: 13, streetSetback: 0, sideGap: 0, floors: 6, groundFloorHeight: 4.6, floorHeight: 3.3, floorHeightDecay: 0.04, plinthHeight: 0.7, basementWindows: 0.6, roofPitch: 28, roofHip: 0.85, ridgeAlongFront: 1, roofOverhang: 0.2, parapetHeight: 0.7, dormers: 0.1, chimneys: 0.75, bayWidth: 2.8, cornerMargin: 1, windowWidth: 1.3, windowHeight: 1.95, sillHeight: 0.85, windowDepth: 0.3, mullions: 0.75, groundShop: 0.75, groundRecess: 3.2, entranceType: 'door', entranceCanopy: 0, balconies: 0.12, balconyFrom: 1, balconyDepth: 0.8, balconyWidth: 0.6, railing: 0.2, cornice: 1.05, stringCourses: 0.6, ornament: 0.6, pilasters: 0.6, rustication: 1, sideWindows: 0.7, wallTexture: 'plaster', wallColor: '#d8c19c', trimColor: '#e9dcc0', plinthColor: '#9b917e', accentColor: '#c49a72', roofTexture: 'tiles', roofColor: '#7b4a39', frameColor: '#5b4535', weathering: 0.4
```

#### `blok_poodwilzowy` — Blok poodwilżowy (1956–1968)

Pięciokondygnacyjny blok wolnostojący wśród zieleni, murowany i tynkowany, z prostymi balkonami i oknami klatek schodowych.

Cechy: bloki różnej długości w zieleni; bez windy do 5 kondygnacji; małe mieszkania wg normatywu 1959.

Źródła: [Sumorok – Między modernizmem a socrealizmem](https://gdynia.pl/modernizm/module/Files/controller/Default/action/downloadFile/hash/21db1ff53eedd0db225519f95c67e51c) · [Kraków – poradnik budowy wind 2025](https://plikimpi.krakow.pl/zalacznik/541179)

```text
segments: 4, segmentWidth: 13, depth: 11, streetSetback: 10, sideGap: 14, floors: 5, groundFloorHeight: 2.9, floorHeight: 2.8, plinthHeight: 0.7, basementWindows: 0.8, roofPitch: 0, parapetHeight: 0.3, roofOverhang: 0.15, chimneys: 0.6, rooftopBoxes: 0.1, bayWidth: 3.1, cornerMargin: 0.9, windowWidth: 1.45, windowHeight: 1.45, sillHeight: 0.85, windowDepth: 0.14, mullions: 0.45, entranceType: 'door', entranceCanopy: 1.1, stairWindows: 1, balconies: 0.5, balconyFrom: 1, balconyDepth: 0.95, balconyWidth: 0.75, railing: 0.42, cornice: 0.15, sideWindows: 0.25, wallTexture: 'plaster', wallColor: '#cdc6b6', trimColor: '#d9d3c5', plinthColor: '#7f7a70', accentColor: '#b5c4b0', roofTexture: 'tarpaper', roofColor: '#3b3a38', frameColor: '#6b5440', weathering: 0.5
```

#### `blok_plyta_5` — Blok z wielkiej płyty, 5 kondygnacji (1970–1990)

Wielokrotny blok z prefabrykatów (W-70, OWT): pięć kondygnacji bez windy, widoczne styki płyt, pas okien klatki, loggie, ślepe szczyty.

Cechy: kondygnacja 2,7–2,8 m; siatka 1,2 / 2,4 / 4,8 m; klatka co ok. 14 m; szczyty bez okien.

Źródła: [Woźniczka 2021 – system OWT](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-c1dd141f-3a55-4fe3-a209-4bbfdb4c2b55/c/Wozniczka_M_Eksperymentalny_7_2021.pdf) · [Ostańska 2009 – OWT-67, trakty 4,80 m](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-article-BTB2-0054-0068/c/Ostanska_Stan_9_2009.pdf) · [Kraków – poradnik budowy wind 2025](https://plikimpi.krakow.pl/zalacznik/541179)

```text
segments: 5, segmentWidth: 14.4, depth: 11, streetSetback: 14, sideGap: 20, floors: 5, groundFloorHeight: 2.8, floorHeight: 2.75, plinthHeight: 0.9, basementWindows: 1, roofPitch: 0, parapetHeight: 0.25, roofOverhang: 0, chimneys: 0.35, rooftopBoxes: 0.25, bayWidth: 3.6, cornerMargin: 1.2, windowWidth: 1.5, windowHeight: 1.45, sillHeight: 0.85, windowDepth: 0.08, mullions: 0.35, entranceType: 'door', entranceCanopy: 1.3, stairWindows: 1, balconies: 0.5, balconyFrom: 1, balconyDepth: 1.2, balconyWidth: 0.85, loggia: 0.35, railing: 0.1, cornice: 0, panelJoints: 1, sideWindows: 0, wallTexture: 'concrete', wallColor: '#b9b6ad', trimColor: '#c7c4bb', plinthColor: '#87847c', accentColor: '#cf9f8d', roofTexture: 'tarpaper', roofColor: '#363636', frameColor: '#d8d6cf', weathering: 0.6
```

#### `blok_plyta_11` — Blok z wielkiej płyty, 11 kondygnacji (1970–1990)

Wysoki blok płytowy z windami i maszynowniami na dachu; rzut ok. 11,8 × 38,8 m, loggie i balkony w pionowych pasach.

Cechy: 11 kondygnacji; maszynownie dźwigów na dachu; styki płyt; ślepe ściany szczytowe.

Źródła: [Płachecki, Koziński – blok 11-kondygnacyjny](https://awarie.zut.edu.pl/files/ab2015/referaty/06_Konstrukcje_zelbetowe/12_P%c5%81ACHECKI_M,KOZI%c5%83SKI_K_Uszkodzenia%20jedenastokondygnacyjnego%20budynku%20mieszkalnego%20o%20konstrukcji%20wielkop%c5%82ytowej%20spowodowane%20wybuchem%20gazu.pdf) · [Woźniczka 2021 – system OWT](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-c1dd141f-3a55-4fe3-a209-4bbfdb4c2b55/c/Wozniczka_M_Eksperymentalny_7_2021.pdf)

```text
segments: 3, segmentWidth: 13, depth: 11.8, streetSetback: 18, sideGap: 30, floors: 11, groundFloorHeight: 2.8, floorHeight: 2.75, plinthHeight: 0.9, basementWindows: 1, roofPitch: 0, parapetHeight: 0.3, rooftopBoxes: 1, chimneys: 0.3, bayWidth: 3.6, cornerMargin: 1, windowWidth: 1.5, windowHeight: 1.45, sillHeight: 0.85, windowDepth: 0.08, mullions: 0.35, entranceType: 'door', entranceCanopy: 1.5, stairWindows: 0.4, balconies: 0.6, balconyFrom: 1, balconyDepth: 1.2, balconyWidth: 0.9, loggia: 0.55, railing: 0.1, panelJoints: 1, sideWindows: 0, wallTexture: 'concrete', wallColor: '#b4b2aa', trimColor: '#c3c1b9', plinthColor: '#84827b', accentColor: '#8fb0c4', roofTexture: 'tarpaper', roofColor: '#333333', frameColor: '#d8d6cf', weathering: 0.6
```

#### `falowiec` — Falowiec (1967–1973)

Bardzo długi, łamany blok galeriowy. Oryginał z Gdańska: ok. 860 m, 11 kondygnacji (32 m), 16 klatek, 1792 mieszkania. Domyślnie generowana jest krótsza wersja.

Cechy: przesunięte segmenty; galerie wzdłuż całej elewacji; głębokość ok. 13 m.

Źródła: [National Geographic – falowiec](https://www.national-geographic.pl/ludzie/gdansk-mikroklimaty-betonowej-fali/) · [kb.pl – falowiec w Gdańsku](https://kb.pl/aktualnosci/architektura-i-budownictwo/falowiec-w-gdansku-niedlugo-bedzie-wygladac-inaczej/)

```text
segments: 6, segmentWidth: 54, depth: 13, stagger: 6, streetSetback: 20, sideGap: 30, floors: 11, groundFloorHeight: 2.9, floorHeight: 2.8, plinthHeight: 0.6, basementWindows: 0.6, roofPitch: 0, parapetHeight: 0.35, rooftopBoxes: 0.6, bayWidth: 3.6, cornerMargin: 0.8, windowWidth: 1.6, windowHeight: 1.4, sillHeight: 0.9, windowDepth: 0.08, mullions: 0.3, entranceType: 'door', entranceCanopy: 1.2, stairWindows: 0.7, balconies: 1, balconyFrom: 1, balconyDepth: 1.4, balconyWidth: 1, railing: 0.08, panelJoints: 0.8, sideWindows: 0, wallTexture: 'concrete', wallColor: '#c9c5bb', trimColor: '#d5d1c7', plinthColor: '#8a867d', accentColor: '#e0b0a0', roofTexture: 'tarpaper', roofColor: '#353535', frameColor: '#dddad2', weathering: 0.55
```

#### `kostka` — Kostka PRL (1960–1985)

Piętrowy dom na planie zbliżonym do kwadratu, z płaskim dachem, wysokim parterem nad piwnicą, balkonem nad wejściem i tynkiem „baranek”.

Cechy: limit 110 m² powierzchni użytkowej; stropodach; loggie; szary lub beżowy tynk.

Źródła: [Murator – dom kostka](https://muratordom.pl/remont-domu/przebudowa/dom-kostka-do-remontu-lifting-elewacji-gruntowna-przebudowa-aa-TCXi-CSY6-y8zn.html) · [kb.pl – remont domu kostki](https://kb.pl/aktualnosci/remonty/generalny-remont-domu-typu-kostka/)

```text
segments: 1, segmentWidth: 10, depth: 10, streetSetback: 7, sideGap: 8, floors: 2, groundFloorHeight: 2.8, floorHeight: 2.8, plinthHeight: 1.1, basementWindows: 1, roofPitch: 0, parapetHeight: 0, roofOverhang: 0.35, chimneys: 0.6, bayWidth: 3.2, cornerMargin: 0.6, windowWidth: 1.5, windowHeight: 1.35, sillHeight: 0.9, windowDepth: 0.14, mullions: 0.4, entranceType: 'door', entranceCanopy: 1.1, balconies: 0.34, balconyFrom: 1, balconyDepth: 1.1, balconyWidth: 0.85, loggia: 0.3, railing: 0.45, cornice: 0, sideWindows: 1, wallTexture: 'plaster', wallColor: '#b6b0a3', trimColor: '#cfc9bc', plinthColor: '#7a7468', accentColor: '#a88b6e', roofTexture: 'tarpaper', roofColor: '#3a3836', frameColor: '#6b4d36', weathering: 0.45
```

#### `dom_90` — Dom lat 90. (1990–2005)

Podmiejski dom transformacji: wielospadowy dach z lukarnami, wieżyczka, ganek na kolumienkach, szprosy i pastelowy tynk.

Cechy: daszki, wieżyczki, kolumienki; czerwona dachówka; klinkierowy cokół.

Źródła: [PAP – rozmowa z Anną Cymer o latach 90.](https://dzieje.pl/node/156351) · [Tokajuk – postmodernizm w architekturze mieszkaniowej](https://repozytorium.biblos.pk.edu.pl/redo/resources/32441/file/suwFiles/TokajukA_TrwaniePrzemijanie.pdf) · [Wikipedia – Dwór (manor house)](https://www.wikipedia.org/wiki/Dw%C3%B3r_(manor_house))

```text
segments: 1, segmentWidth: 14, depth: 11, streetSetback: 8, sideGap: 9, floors: 2, groundFloorHeight: 2.9, floorHeight: 2.7, plinthHeight: 0.5, basementWindows: 0.2, roofPitch: 42, roofHip: 0.75, mansard: 0.3, ridgeAlongFront: 1, roofOverhang: 0.6, dormers: 0.7, chimneys: 0.6, turret: 0.6, bayWidth: 2.6, cornerMargin: 0.6, windowWidth: 1.2, windowHeight: 1.5, sillHeight: 0.85, windowDepth: 0.12, windowArch: 0.45, mullions: 0.85, entranceType: 'portico', entranceCanopy: 1.2, balconies: 0.22, balconyFrom: 1, balconyDepth: 1.2, balconyWidth: 0.8, railing: 0.25, cornice: 0.15, stringCourses: 0.2, ornament: 0.35, pilasters: 0.15, sideWindows: 1, asymmetry: 0.35, wallTexture: 'plaster', wallColor: '#eed8b4', trimColor: '#fbf8f2', plinthColor: '#8d4c38', accentColor: '#c86f55', roofTexture: 'tiles', roofColor: '#8b3224', frameColor: '#ffffff', weathering: 0.15
```

#### `blok_90` — Blok lat 90. (1990–2002)

Postmodernistyczny blok transformacji: mansardowy dach z lukarnami, narożna wieżyczka, wykusze, kolorowe pasy i daszki nad wejściami.

Cechy: dach jako znak „domu”; kolor i pasy na elewacji; wieżyczki i wykusze.

Źródła: [PAP – rozmowa z Anną Cymer o latach 90.](https://dzieje.pl/node/156351) · [Tokajuk – postmodernizm w architekturze mieszkaniowej](https://repozytorium.biblos.pk.edu.pl/redo/resources/32441/file/suwFiles/TokajukA_TrwaniePrzemijanie.pdf)

```text
segments: 3, segmentWidth: 15, depth: 13, streetSetback: 8, sideGap: 12, floors: 5, groundFloorHeight: 3, floorHeight: 2.8, plinthHeight: 0.6, basementWindows: 0.3, roofPitch: 38, roofHip: 0.5, mansard: 0.45, ridgeAlongFront: 1, roofOverhang: 0.45, dormers: 0.55, chimneys: 0.3, turret: 0.5, bayWidth: 3, cornerMargin: 0.8, windowWidth: 1.3, windowHeight: 1.5, sillHeight: 0.85, windowDepth: 0.14, windowArch: 0.25, mullions: 0.6, entranceType: 'door', entranceCanopy: 1.4, stairWindows: 0.6, balconies: 0.45, balconyFrom: 1, balconyDepth: 1.3, balconyWidth: 0.75, loggia: 0.2, railing: 0.45, oriels: 0.3, orielDepth: 0.7, cornice: 0.2, stringCourses: 0.45, ornament: 0.15, sideWindows: 0.6, asymmetry: 0.3, wallTexture: 'plaster', wallColor: '#e9d9b9', trimColor: '#f7f2e7', plinthColor: '#87503d', accentColor: '#c66a52', accentPanels: 0.5, roofTexture: 'tiles', roofColor: '#8a3a2c', frameColor: '#f7f7f2', weathering: 0.2
```

#### `deweloperski` — Blok deweloperski (2000–2012)

Osiedlowy blok lat 2000.: cofnięte ostatnie piętro, szklane balustrady, klinkierowy cokół, panele akcentowe, przeszklony hol.

Cechy: garaż podziemny; penthouse z tarasem; szklane balustrady; osiedle grodzone.

Źródła: [PKO BP – Puls Nieruchomości 2023](https://www.pkobp.pl/media_files/5ca4ece3-6b29-4e61-a3f4-daff2fdb784b.pdf) · [Owczarek – zamknięte osiedla](https://bibliotekanauki.pl/articles/413039.pdf)

```text
segments: 3, segmentWidth: 16, depth: 14, setbackFloors: 1, setbackDepth: 2.5, streetSetback: 6, sideGap: 10, floors: 6, groundFloorHeight: 3.1, floorHeight: 2.9, plinthHeight: 0.35, basementWindows: 0, roofPitch: 0, parapetHeight: 0.6, roofOverhang: 0, chimneys: 0.15, rooftopBoxes: 0.35, bayWidth: 3.4, cornerMargin: 0.6, windowWidth: 1.7, windowHeight: 1.8, sillHeight: 0.55, windowDepth: 0.12, mullions: 0.2, ribbon: 0.1, groundShop: 0.15, entranceType: 'glass', entranceCanopy: 1.6, stairWindows: 0.55, balconies: 0.6, balconyFrom: 1, balconyDepth: 1.6, balconyWidth: 0.75, loggia: 0.25, railing: 0.92, cornice: 0.05, accentPanels: 0.35, sideWindows: 0.7, asymmetry: 0.3, wallTexture: 'plaster', wallColor: '#efede8', trimColor: '#f7f6f3', plinthColor: '#7b4b3b', accentColor: '#8f6a4a', roofTexture: 'tarpaper', roofColor: '#3a3a3a', frameColor: '#4a4d52', glassTint: '#3c4f5c', weathering: 0.08
```

#### `plomba` — Plomba w pierzei (1995–2015)

Współczesne wypełnienie luki między kamienicami: gabaryt pierzei, ślepe ściany boczne, przeszklony parter, okna od podłogi, cofnięte piętro.

Cechy: bryła kamienicy, elewacja nowoczesna; okładzina kamienna; loggie ze szkłem.

Źródła: [Cymer – Długie lata 90. (plomby)](https://www.biblos.pk.edu.pl/ST/2024/05/100000346947/100000346947_Cymer_ArchitekturaWPolsce.pdf) · [Kilanowski, Ochrona Zabytków 1/2015 (NID)](https://ochronazabytkow.nid.pl/wp-content/uploads/2019/08/OZ_1-2015_11_Kilanowski.pdf)

```text
segments: 1, segmentWidth: 18, depth: 14, setbackFloors: 1, setbackDepth: 2.2, streetSetback: 0, sideGap: 0, floors: 6, groundFloorHeight: 4, floorHeight: 3.1, plinthHeight: 0.15, basementWindows: 0, roofPitch: 0, parapetHeight: 0.35, rooftopBoxes: 0.3, bayWidth: 3.6, cornerMargin: 0.4, windowWidth: 2.6, windowHeight: 2.45, sillHeight: 0.25, windowDepth: 0.2, mullions: 0.12, ribbon: 0.35, groundShop: 0.9, entranceType: 'glass', entranceCanopy: 0.6, balconies: 0.5, balconyFrom: 1, balconyDepth: 1.3, balconyWidth: 0.8, loggia: 0.6, railing: 1, cornice: 0.05, sideWindows: 0, asymmetry: 0.4, wallTexture: 'stone', wallColor: '#cfc9bd', trimColor: '#e2ded6', plinthColor: '#5f5d58', accentColor: '#4b4f55', accentPanels: 0.25, roofTexture: 'tarpaper', roofColor: '#3a3a3a', frameColor: '#3a3d41', weathering: 0.05
```

#### `apartamentowiec` — Apartamentowiec (2008–2015)

Wysoki budynek apartamentowy: uskoki ostatnich pięter, głębokie loggie (powyżej 25 m warunki techniczne nie dopuszczają balkonów), szkło od podłogi, wysoki przeszklony parter.

Cechy: uskoki i tarasy; loggie ze szklanymi balustradami; okładziny grafitowe.

Źródła: [Inżynier Budownictwa – Corbu na Mokotowie](https://inzynierbudownictwa.pl/corbu-na-mokotowie/) · [WT – balkony powyżej 25 m](https://arslege.pl/rozporzadzenie-ministra-infrastruktury-w-sprawie-warunkow-technicznych-jakim-powinny-odpowiadac-budynki-i-ich-usytuowanie/k218/a23452/) · [PKO BP – Puls Nieruchomości 2023](https://www.pkobp.pl/media_files/5ca4ece3-6b29-4e61-a3f4-daff2fdb784b.pdf)

```text
segments: 2, segmentWidth: 16, depth: 18, setbackFloors: 2, setbackDepth: 3, streetSetback: 8, sideGap: 14, floors: 12, groundFloorHeight: 4.6, floorHeight: 3.2, plinthHeight: 0.1, basementWindows: 0, roofPitch: 0, parapetHeight: 1, rooftopBoxes: 0.6, chimneys: 0, bayWidth: 3.6, cornerMargin: 0.3, windowWidth: 2.9, windowHeight: 2.65, sillHeight: 0.1, windowDepth: 0.18, mullions: 0.12, ribbon: 0.45, groundShop: 0.85, groundRecess: 1.6, entranceType: 'glass', entranceCanopy: 2.2, balconies: 0.8, balconyFrom: 1, balconyDepth: 2, balconyWidth: 0.92, loggia: 0.65, railing: 0.96, cornice: 0, sideWindows: 1, asymmetry: 0.25, wallTexture: 'stone', wallColor: '#4a4e54', trimColor: '#d8d4cc', plinthColor: '#2f3134', accentColor: '#c7c1b6', accentPanels: 0.3, roofTexture: 'tarpaper', roofColor: '#333333', frameColor: '#2a2c2f', glassTint: '#2f4250', weathering: 0.03
```

#### `stodola` — Nowoczesna stodoła (2010–2015)

Prostokątna bryła z dachem dwuspadowym bez okapu, szczytem od ulicy, przeszkloną ścianą szczytową i ciemnym deskowaniem; bez wykuszy i balkonów.

Cechy: dach bez okapu, blacha na rąbek; kąt 35–40° (często z planu miejscowego); duże przeszklenia.

Źródła: [kb.pl – nowoczesna stodoła](https://kb.pl/porady/moda-na-domy-w-stylu-modern-barn-czyli-dlaczego-coraz-wiecej-polakow-chce-mieszkac-w-nowoczesnej-stodole-_wp23/) · [Murator – dachy dwuspadowe 35–40°](https://muratordom.pl/budowa/dach/6-pieknych-nowoczesnych-domow-z-dachami-dwuspadowymi-aa-Mqg3-sPop-KqhU.html)

```text
segments: 1, segmentWidth: 8.5, depth: 16, streetSetback: 9, sideGap: 9, floors: 1, groundFloorHeight: 3, floorHeight: 2.8, plinthHeight: 0.15, basementWindows: 0, roofPitch: 42, roofHip: 0, mansard: 0, ridgeAlongFront: 0, roofOverhang: 0, dormers: 0, chimneys: 0.2, gableGlazing: 0.75, bayWidth: 3.4, cornerMargin: 0.5, windowWidth: 2.4, windowHeight: 2.55, sillHeight: 0, windowDepth: 0.1, mullions: 0.12, entranceType: 'glass', entranceCanopy: 0, balconies: 0, cornice: 0, ornament: 0, sideWindows: 0.6, asymmetry: 0.5, wallTexture: 'wood', wallColor: '#34312e', trimColor: '#3a3734', plinthColor: '#2c2b2a', accentColor: '#b08a5e', roofTexture: 'seam', roofColor: '#2e2f31', frameColor: '#222325', glassTint: '#33434d', weathering: 0.05
```

## Załącznik D — Profile ulic

| id | nazwa | działki | domieszka | wagi archetypów |
|---|---|---|---|---|
| `warszawa_srodmiescie` | Warszawa – Śródmieście Południowe | 9 | 0.25 | kamienica_eklektyczna 0.26, kamienica_secesyjna 0.12, kamienica_modernistyczna 0.16, socrealizm 0.14, plomba 0.16, blok_poodwilzowy 0.06, apartamentowiec 0.1 |
| `lodz_srodmiescie` | Łódź – Śródmieście | 10 | 0.2 | kamienica_eklektyczna 0.46, kamienica_secesyjna 0.16, kamienica_modernistyczna 0.1, familok 0.04, plomba 0.12, blok_poodwilzowy 0.08, deweloperski 0.04 |
| `wroclaw_przedmiescie` | Wrocław – Przedmieście Oławskie | 10 | 0.2 | kamienica_eklektyczna 0.44, kamienica_secesyjna 0.14, blok_plyta_5 0.12, plomba 0.16, deweloperski 0.1, socrealizm 0.04 |
| `gdynia_srodmiescie` | Gdynia – Śródmieście | 9 | 0.2 | kamienica_modernistyczna 0.56, willa_modernistyczna 0.1, blok_poodwilzowy 0.1, socrealizm 0.06, apartamentowiec 0.12, plomba 0.06 |
| `slask_osiedle` | Górny Śląsk – osiedle patronackie | 9 | 0.15 | familok 0.6, blok_poodwilzowy 0.12, blok_plyta_5 0.14, kostka 0.14 |
| `blokowisko` | Blokowisko PRL | 6 | 0.15 | blok_plyta_5 0.44, blok_plyta_11 0.3, falowiec 0.04, blok_poodwilzowy 0.1, deweloperski 0.12 |
| `przedmiescie` | Przedmieście domów jednorodzinnych | 10 | 0.2 | kostka 0.34, dom_90 0.3, stodola 0.16, willa_drewniana 0.06, willa_modernistyczna 0.08, dworek 0.06 |
| `nowe_osiedle` | Nowe osiedle deweloperskie | 7 | 0.2 | deweloperski 0.6, apartamentowiec 0.24, blok_90 0.16 |

## Załącznik E — Źródła

Źródła przypięte do archetypów w kodzie (`Archetype.sources`):

- [Wikipedia – Dwór (manor house)](https://www.wikipedia.org/wiki/Dw%C3%B3r_(manor_house))
- [ZPE – Dwór szlachecki](https://zpe.gov.pl/a/miejsce-akcji---dwor-szlachecki/Dl3Ih2NOo)
- [Kilanowski, Ochrona Zabytków 1/2015 (NID)](https://ochronazabytkow.nid.pl/wp-content/uploads/2019/08/OZ_1-2015_11_Kilanowski.pdf)
- [Roguska, KAiU 4/2010 – balkony kamienic warszawskich](https://oldkaiu.pan.pl/images/stories/4.2010/J.Roguska2.pdf)
- [Domińczak, Zaguła – Typologia łódzkiej kamienicy](https://architektmiasta.uml.lodz.pl/files/public/dla_mieszkanca/o-miescie/inne_publikacje/TRYPOL_SCALONA.pdf)
- [Janicka, Ochrona Zabytków 4/2001 – secesja w Krakowie](https://bazhum.muzhp.pl/media/texts/ochrona-zabytkow/2001-tom-54-numer-4-215/ochrona_zabytkow-r2001-t54-n4_215-s345-356.pdf)
- [Wikipedia – Familok](https://kaweah.freedombox.rocks/kiwix/wikipedia_en_all_maxi_2023-10/Familok)
- [dzieje.pl – Nikiszowiec](https://dzieje.pl/node/46821)
- [dzieje.pl – świdermajer](https://dzieje.pl/node/169561)
- [Notes from Poland – świdermajer](https://notesfrompoland.com/2023/08/31/polish-town-offers-free-house-designs-based-on-famous-19th-century-style/)
- [Muzeum Piłsudskiego – Architektura, inspiracje](https://muzeumpilsudski.pl/wp-content/uploads/2019/10/Architektura_inspiracje-.pdf)
- [gdynia.pl – Gdyński modernizm](https://www.gdynia.pl/g2/2007_09/12512_fileot.pdf)
- [dzieje.pl – Nowa Huta](https://dzieje.pl/node/101444)
- [Przesmycka, Teka KAiU PAN – dzielnice socrealizmu](https://ph.pollub.pl/index.php/teka/issue/download/133/68)
- [Sumorok – Między modernizmem a socrealizmem](https://gdynia.pl/modernizm/module/Files/controller/Default/action/downloadFile/hash/21db1ff53eedd0db225519f95c67e51c)
- [Kraków – poradnik budowy wind 2025](https://plikimpi.krakow.pl/zalacznik/541179)
- [Woźniczka 2021 – system OWT](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-c1dd141f-3a55-4fe3-a209-4bbfdb4c2b55/c/Wozniczka_M_Eksperymentalny_7_2021.pdf)
- [Ostańska 2009 – OWT-67, trakty 4,80 m](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-article-BTB2-0054-0068/c/Ostanska_Stan_9_2009.pdf)
- [Płachecki, Koziński – blok 11-kondygnacyjny](https://awarie.zut.edu.pl/files/ab2015/referaty/06_Konstrukcje_zelbetowe/12_P%c5%81ACHECKI_M,KOZI%c5%83SKI_K_Uszkodzenia%20jedenastokondygnacyjnego%20budynku%20mieszkalnego%20o%20konstrukcji%20wielkop%c5%82ytowej%20spowodowane%20wybuchem%20gazu.pdf)
- [National Geographic – falowiec](https://www.national-geographic.pl/ludzie/gdansk-mikroklimaty-betonowej-fali/)
- [kb.pl – falowiec w Gdańsku](https://kb.pl/aktualnosci/architektura-i-budownictwo/falowiec-w-gdansku-niedlugo-bedzie-wygladac-inaczej/)
- [Murator – dom kostka](https://muratordom.pl/remont-domu/przebudowa/dom-kostka-do-remontu-lifting-elewacji-gruntowna-przebudowa-aa-TCXi-CSY6-y8zn.html)
- [kb.pl – remont domu kostki](https://kb.pl/aktualnosci/remonty/generalny-remont-domu-typu-kostka/)
- [PAP – rozmowa z Anną Cymer o latach 90.](https://dzieje.pl/node/156351)
- [Tokajuk – postmodernizm w architekturze mieszkaniowej](https://repozytorium.biblos.pk.edu.pl/redo/resources/32441/file/suwFiles/TokajukA_TrwaniePrzemijanie.pdf)
- [PKO BP – Puls Nieruchomości 2023](https://www.pkobp.pl/media_files/5ca4ece3-6b29-4e61-a3f4-daff2fdb784b.pdf)
- [Owczarek – zamknięte osiedla](https://bibliotekanauki.pl/articles/413039.pdf)
- [Cymer – Długie lata 90. (plomby)](https://www.biblos.pk.edu.pl/ST/2024/05/100000346947/100000346947_Cymer_ArchitekturaWPolsce.pdf)
- [Inżynier Budownictwa – Corbu na Mokotowie](https://inzynierbudownictwa.pl/corbu-na-mokotowie/)
- [WT – balkony powyżej 25 m](https://arslege.pl/rozporzadzenie-ministra-infrastruktury-w-sprawie-warunkow-technicznych-jakim-powinny-odpowiadac-budynki-i-ich-usytuowanie/k218/a23452/)
- [kb.pl – nowoczesna stodoła](https://kb.pl/porady/moda-na-domy-w-stylu-modern-barn-czyli-dlaczego-coraz-wiecej-polakow-chce-mieszkac-w-nowoczesnej-stodole-_wp23/)
- [Murator – dachy dwuspadowe 35–40°](https://muratordom.pl/budowa/dach/6-pieknych-nowoczesnych-domow-z-dachami-dwuspadowymi-aa-Mqg3-sPop-KqhU.html)

Dodatkowe źródła z dokumentu z researchem (wybór — publikacje naukowe, raporty i przepisy):

- [Łupienko A., „Kamienice czynszowe Warszawy 1864–1914”, IH PAN 2015 (recenzja)](https://www.rcin.org.pl//Content/62896/PDF/WA308_82462_P331_Kamienice-czynszowe_I.pdf)
- [Zeszyty Naukowe PP, Architektura 9/2022 — wpływy berlińskie (Poznań, Bydgoszcz)](https://architecturejournal.put.poznan.pl/dl/i/pdf/472/df5f23b357aade2e6acc81c634fe6f90/)
- [IPN, Siwińska — odbudowa Warszawy (skala zniszczeń)](https://przystanekhistoria.pl/pa2/teksty/104971,Odbudowa-Warszawy.pdf)
- [Cetkowski — secesja na Pomorzu](https://kpbc.umk.pl//Content/219246/PDF/Biuletyn%20KMDG%202012%208(313).pdf)
- [Kudlicka, UŁ — świdermajer](https://czasopisma.uni.lodz.pl/kw/article/download/23282/23325/55114)
- [Koseski, KAiU 2/2016 — styl zakopiański, Gdynia](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-5667e772-e6ea-4418-91fc-0eff0862aae5/c/Koseski_GDYNIA_KAiU_2-2016.pdf)
- [Mazur — „Ulice osiedlowe…” (WSM, Praesens), RCIN](https://www.rcin.org.pl//Content/220437/PDF/WA308_241898_P330_Ulice-osiedlowe_I.pdf)
- [Łukaszewicz — luksusowy modernizm 1934–1939, repozytorium UW](https://repozytorium.uw.edu.pl//entities/publication/110fb838-cdca-422d-a2ba-2b40a3613b80)
- [Hirsch, PL Lublin — Gdynia jako Pomnik Historii](https://ph.pollub.pl/index.php/odk/article/download/35/660/2231)
- [Bieranowski 2013 — OWT-67](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-7f5b30a9-9e14-4794-be86-d344e6b5157d/c/Bieranowski_Redukcja_03_2013.pdf)
- [Politechnika Śląska 1988 — systemy W-70 i Szczecin](https://delibra.bg.polsl.pl//Content/38056/BCPS_41771_1988_Wplyw-konstrukcji-na.pdf)
- [Korzeniewski 2011 — systemy wielkopłytowe](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-article-BTB2-0075-0001/c/Korzeniewski_O_problemie_10_2011.pdf)
- [ABiD 4/2020 — udział systemów wielkopłytowych](https://yadda.icm.edu.pl/baztech/element/bwmeta1.element.baztech-6c8496ce-060d-459d-8237-222a270be472/c/abid_nr_4_2020_artykul_01.pdf)
- [Builder 02/2017 — normatywy powierzchni mieszkań 1959 i 1974](https://buildercorp.pl/wp-content/uploads/2017/02/B_02_2017_dobre.pdf)
- [Łątkowska, Rocznik Toruński — osiedla grodzone](https://cejsh.icm.edu.pl/cejsh/element/bwmeta1.element.ojs-doi-10_12775_RT_2018_011/c/RT.2018.011-23641.pdf)
- [Nikolić S., „Pasteloza – Refurbishing of the PPR heritage”, Urban Development Issues 2019](https://isidore.science/document/10670/1.zc28gl)
- [Rozporządzenie w sprawie warunków technicznych (WT 2002), tekst jednolity](https://bhp.uw.edu.pl/wp-content/uploads/sites/64/2021/06/Rozporzadzenie-Ministra-Infrastruktury-w-sprawie-warunkow-technicznych-jakim-powinny-odpowiadac-budynki-i-ich-usytuowanie.pdf)
- [WT 2002 § 8 — grupy wysokości (lexlege)](https://lexlege.pl/tech-war-budynkow/paragraf-8/)
- [WT 2002 § 54 — dźwig osobowy (arslege)](https://arslege.pl/dzwig-osobowy/k218/a23201/)
- [Wonka P. i in., „Instant Architecture”, ACM TOG 22(3), 2003](https://www.cg.tuwien.ac.at/research/vr/instantarchitecture/instant_architecture.pdf)
- [Müller P. i in., „Procedural Modeling of Buildings”, ACM TOG 25(3), 2006](https://course.ccs.neu.edu/cs5150f13/readings/muller_buildings.pdf)
- [Geoportal (GUGiK) — Modele 3D budynków LoD1/LoD2](https://www.geoportal.gov.pl/pl/dane/inne-dane/modele-3d-budynkow/)

Pełny dokument z researchem (wszystkie przypisy przy faktach): `https://claude.ai/artifact/HLripjtFagnkLnJV11SEzC`.

## Załącznik F — Pliki referencyjne

| Plik | Rola |
|---|---|
| `src/core/types.ts` | typy: parametry, spec, archetyp, siatka, metryka |
| `src/core/params.ts` | 71 definicji parametrów, `clampParam`, `normalizeParams` |
| `src/core/archetypes.ts` | 19 archetypów, źródła, mapa, epoki |
| `src/core/blend.ts` | IDW, wagi per wektor, mieszanie, wariacja, `resolveSpec` |
| `src/core/color.ts` | sRGB ↔ OKLab, mieszanie i korekty kolorów |
| `src/core/random.ts` | `mulberry32`, `hash01`, `hashString` |
| `src/core/generator.ts` | gramatyka budynku (bryła, elewacje, dachy, metryka) |
| `src/core/mesh.ts` | `Frame`, `MeshBuilder` (wielokąty, prostopadłościany, UV) |
| `src/core/materials.ts` | parametry → materiały PBR |
| `src/core/street.ts`, `cities.ts` | pierzeja i profile miast |
| `src/core/export/*` | GLB, OBJ, ZIP |
| `src/core/render/raster.ts` | renderer programowy |
| `src/mcp/server.ts`, `png.ts` | serwer MCP, koder PNG |
| `src/web/**` | aplikacja React + three.js |
| `scripts/render-all.ts`, `thumbs.ts`, `test-mcp.mjs`, `build-artifact.mjs`, `handoff-data.ts` | narzędzia: rendery kontrolne, miniatury, test MCP, artefakt, dane do tego dokumentu |
| `docs/pierzeja-dane.json` | dane maszynowe: parametry, archetypy (nadpisania + pełne wektory), profile, epoki |

## Załącznik G — Wektory testowe

Wartości policzone prototypem. Wagi i mieszanie (G.1–G.3) nie zależą od geometrii — nowa implementacja powinna je odtworzyć dokładnie. Metryka (G.4) zależy od reguł geometrii — traktuj ją jako punkt odniesienia (kondygnacje, wysokości, klasa — zgodne; liczby okien i trójkątów — orientacyjne).

#### G.1 Wagi IDW z mapy (sharpness = 3, neighbors = 3)

| kursor (rok; skala) | wagi |
|---|---|
| 1965; 0.6 | blok_poodwilzowy 0.460761, blok_plyta_5 0.334635, socrealizm 0.204604 |
| 1900; 0.5 | kamienica_eklektyczna 0.485175, familok 0.348276, kamienica_secesyjna 0.166549 |
| 2000; 0.3 | dom_90 0.408276, kostka 0.374711, stodola 0.217012 |
| 1890; 0.62 | kamienica_eklektyczna 1 |
| 1950; 0.95 | falowiec 0.659306, blok_plyta_11 0.269496, socrealizm 0.071198 |
| 1965; 0.6 (sharpness 1.5, neighbors 5) | blok_poodwilzowy 0.276928, blok_plyta_5 0.236001, socrealizm 0.184538, kamienica_modernistyczna 0.153502, blok_90 0.149031 |

#### G.2 Mieszanie 50/50: `kamienica_eklektyczna` + `blok_plyta_5` (bez wariacji)

| parametr | kamienica | blok 5k | wynik |
|---|---|---|---|
| `floors` | 5 | 5 | 5 |
| `segments` | 1 | 5 | 3 |
| `segmentWidth` | 22 | 14.4 | 18.2 |
| `depth` | 14 | 11 | 12.5 |
| `groundFloorHeight` | 4.3 | 2.8 | 3.55 |
| `floorHeight` | 3.7 | 2.75 | 3.225 |
| `roofPitch` | 18 | 0 | 9 |
| `mansard` | 0 | 0 | 0 |
| `balconies` | 0.16 | 0.5 | 0.33 |
| `railing` | 0.45 | 0.1 | 0.275 |
| `entranceType` | gate | door | gate |
| `wallTexture` | plaster | concrete | plaster |
| `roofTexture` | sheet | tarpaper | sheet |
| `wallColor` | #d7c6a6 | #b9b6ad | #c8beaa |
| `roofColor` | #5b5852 | #363636 | #484744 |
| `frameColor` | #5a4636 | #d8d6cf | #978b7f |

Remis w głosowaniu wyliczeń rozstrzyga kolejność wpisów (pierwsza opcja z maksymalną sumą wag) — przy 50/50 wygrywa archetyp podany pierwszy. Kontrola koloru: `blendHex([{#ff0000, 0.5}, {#0000ff, 0.5}])` = `#8c53a2`.

#### G.3 Wariacja: `{"archetype": "blok_plyta_5", "seed": 42, "variation": 1}`

floors 5, segments 5, segmentWidth 14.7731, depth 11.1129, floorHeight 2.7327, wallColor #bab6ad, balconies 0.5069, dormers 0 (zero zostaje zerem).

#### G.4 Archetypy (`{"archetype": id, "seed": 3}`): metryka referencyjna

| id | kond. | wys. ścian | wys. całk. | klasa | front × gł. | zabudowa m² | PC m² | klatki | okna | balkony | mieszk. | trójkąty |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dworek` | 1 | 4.2 | 11.03 | N | 22 × 12 | 264 | 264 | 1 | 21 | 0 | 3 | 2261 |
| `kamienica_eklektyczna` | 5 | 18.52 | 20.79 | SW | 22 × 14 | 560 | 2800 | 1 | 90 | 4 | 33 | 16017 |
| `kamienica_secesyjna` | 6 | 20.95 | 31.91 | SW | 20 × 15 | 580 | 3480 | 1 | 114 | 10 | 40 | 14432 |
| `familok` | 3 | 9.5 | 13.8 | N | 25 × 11 | 275 | 825 | 2 | 58 | 0 | 10 | 7120 |
| `willa_drewniana` | 2 | 6.4 | 12.4 | N | 12 × 10 | 120 | 240 | 1 | 31 | 4 | 1 | 2266 |
| `willa_modernistyczna` | 2 | 6.6 | 7.05 | N | 13 × 11 | 143 | 286 | 1 | 27 | 2 | 1 | 1952 |
| `kamienica_modernistyczna` | 6 | 19.65 | 20.45 | SW | 24 × 14 | 482 | 2890 | 1 | 91 | 21 | 36 | 7794 |
| `socrealizm` | 6 | 21.47 | 24.93 | SW | 60 × 13 | 780 | 4680 | 3 | 237 | 30 | 67 | 32756 |
| `blok_poodwilzowy` | 5 | 14.8 | 15.1 | SW | 52 × 11 | 572 | 2860 | 4 | 169 | 64 | 49 | 13598 |
| `blok_plyta_5` | 5 | 14.7 | 14.95 | SW | 72 × 11 | 792 | 3960 | 5 | 145 | 80 | 55 | 20530 |
| `blok_plyta_11` | 11 | 31.2 | 33.95 | W | 39 × 11.8 | 460 | 5062 | 3 | 195 | 120 | 70 | 28054 |
| `falowiec` | 11 | 31.5 | 34.25 | W | 324 × 13 | 4212 | 46332 | 6 | 1974 | 1740 | 794 | 283356 |
| `kostka` | 2 | 6.7 | 6.94 | N | 10 × 10 | 100 | 200 | 1 | 23 | 2 | 1 | 1962 |
| `dom_90` | 2 | 6.1 | 12.27 | N | 14 × 11 | 154 | 308 | 1 | 51 | 2 | 1 | 4245 |
| `blok_90` | 5 | 14.8 | 21.51 | SW | 45 × 13 | 585 | 2925 | 3 | 175 | 48 | 36 | 11400 |
| `deweloperski` | 6 | 17.95 | 18.55 | SW | 48 × 14 | 672 | 3912 | 3 | 167 | 48 | 49 | 11726 |
| `plomba` | 6 | 19.65 | 20 | SW | 18 × 14 | 252 | 1472 | 1 | 55 | 24 | 17 | 4624 |
| `apartamentowiec` | 12 | 39.9 | 42.65 | W | 32 × 18 | 576 | 6720 | 2 | 301 | 144 | 76 | 23608 |
| `stodola` | 1 | 3.15 | 6.98 | N | 8.5 × 16 | 136 | 136 | 1 | 9 | 0 | 1 | 442 |

Uwagi generowane dla wybranych budynków:

- `blok_plyta_11`: Grupa wysokości W wg WT 2002 (wysokość ok. 31,5 m).
- `apartamentowiec`: Grupa wysokości W wg WT 2002 (wysokość ok. 40,9 m). / WT 2002 § 54: różnica poziomów posadzek parteru i najwyższej kondygnacji (36,6 m) przekracza 9,5 m — budynek wielorodzinny wymaga windy. / WT 2002 § 303: powyżej 25 m nad terenem zakaz balkonów — zamieniono je na loggie z balustradą pełną.
- `kamienica_secesyjna`: Grupa wysokości SW wg WT 2002 (wysokość ok. 31,9 m).
- `stodola`: Budynek niski (N) wg WT 2002: do 12 m lub do 4 kondygnacji.

## Załącznik H — Słowniczek

| Polski | Angielski | Znaczenie w modelu |
|---|---|---|
| pierzeja | street frontage / building line | ciągła zabudowa wzdłuż ulicy (`sideGap = 0`) |
| kamienica czynszowa | tenement house | budynek frontowy + oficyny |
| oficyna | annex / rear wing | `wingLeft/Right/Rear` |
| podwórze-studnia | light well courtyard | oficyny zamknięte oficyną tylną |
| brama, przejazd bramny | carriage gateway | `entranceType: gate` |
| piano nobile | piano nobile | 1. piętro z najcięższym detalem |
| cokół | plinth | `plinthHeight` |
| boniowanie | rustication | `rustication` |
| gzyms (koronujący / międzykondygnacyjny) | cornice / string course | `cornice`, `stringCourses` |
| lizena, pilaster | pilaster strip | `pilasters` |
| opaska, naczółek, przyczółek | window surround, hood, pediment | `ornament` |
| wykusz | oriel / bay window | `oriels` |
| lukarna | dormer | `dormers` |
| attyka | parapet wall | `parapetHeight` |
| mansarda, dach łamany | mansard roof | `mansard` |
| dach kopertowy / dwuspadowy | hip / gable roof | `roofHip` |
| kalenica, okap, szczyt | ridge, eave, gable | `ridgeAlongFront`, `roofOverhang`, ściany szczytowe |
| loggia | loggia (recessed balcony) | `loggia` |
| podcienia | arcade | `groundRecess` |
| klatka schodowa | staircase core | segment bloku, `stairWindows` |
| wielka płyta, styk | large-panel system, panel joint | `panelJoints` |
| maszynownia windy | lift machine room | `rooftopBoxes` |
| pasteloza | pastel retrofit colour scheme | `accentPanels` |
| plomba | infill building | archetyp `plomba` |
| familok | Silesian workers' tenement | archetyp `familok` |
| świdermajer | Otwock wooden villa style | archetyp `willa_drewniana` |
| falowiec | wave-shaped gallery block | archetyp `falowiec` (`stagger`) |
| kostka | PRL cube house | archetyp `kostka` |
| WT | Polish building technical regulations | reguły § 8, § 54, § 303 |
