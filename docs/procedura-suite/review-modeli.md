# Review modeli 3D (Tabor, Tłum) — realizm i pojazdy europejskie

Wymóg użytkownika (handoff §5): modele mają wyglądać realistycznie w granicach low-poly i budżetów,
pojazdy europejskie. Review robi się przed wypiekaniem modeli do gry (issue 09) i powtarza przy
każdym issue, które tworzy geometrię (09–11, 14, 16, 18).

## Jak

- Arkusze widoków: `cargo run -p pierzeja-render --example review --release -- <tabor|tlum> crates/render/examples/review/<tabor|tlum>.json <katalog>`.
  Pojazd: 12 widoków (przód, boki, tył, 3/4, z góry, zbliżenia przodu i tyłu, kabina z załogą bez szyb,
  skala z człowiekiem 1,78 m, przód nisko). Człowiek: 15 widoków (stoi, idzie, siedzi, prowadzi,
  zbliżenia głowy, dłonie, stopy, T-pose). Każdy kafelek ma podpis, `stats.json` obok.
- Runda 1 (2026-11-09): 5 równoległych recenzentów (osobówki, vany, osobówki służb, ciężkie, ludzie),
  ocena wyłącznie z obrazów, porównanie z wymiarami wzorców i przepisami. Raporty źródłowe są w notatkach
  sesji; poniżej skrót z decyzjami.
- Ograniczenie: sieć sesji nie miała dostępu do isap.sejm.gov.pl, eli.gov.pl, wikipedia.org i stron
  producentów (scania.com, lectura-specs). Przepisy i wymiary z wyszukiwarki lub z wiedzy recenzentów
  oznaczone są jako *niezweryfikowane*.

Priorytety: **P1** psuje realizm albo przepis i widać to z kamery gry; **P2** wyraźne, drugorzędne;
**P3** kosmetyka. Status: ✅ naprawione, ⏳ w toku, ⏸ odłożone (z powodem), ❓ decyzja użytkownika.

## Wspólne dla pojazdów

| Usterka | Jak jest naprawdę | Źródło | P | Status |
|---|---|---|---|---|
| Brak tablic rejestracyjnych z przodu i z tyłu | Tablica UE 520 × 114 mm, niebieski pasek „PL” po lewej patrzącego, z przodu i z tyłu | rozporządzenie w sprawie tablic rejestracyjnych (*niezweryfikowane online*) | P1 | ✅ `tablice.rs`: tło, pasek UE, 7 znaków (prostokąty); osobówki i van na zderzakach, ciężkie na zderzakach (z wyciągarką — po prawej) |
| Brak pomarańczowych kierunkowskazów i powtarzaczy | Przód i tył pomarańczowe, boczne powtarzacze (Regulamin EKG ONZ nr 48) | UN ECE R48 (*niezweryfikowane online*) | P1 | ✅ `MatKey::Indicator`: w oprawie reflektora, w lampie tylnej, na obudowie lusterka; ciężkie — na czole kabiny, za nadkolem, boczne obrysówki co 2,5 m |
| Lusterka 6 × 9 cm, nieczytelne | Osobówka ok. 0,14 m poza burtę, van ok. 0,2 m, na ramieniu | vehiclesizes.com (Octavia IV, szer. z lusterkami ~2,0 m) | P2 | ✅ ramię + obudowa; `stats.width` = szerokość bez lusterek, `widthMirrors` = z lusterkami |
| Brak wycieraczek | Dwa ramiona na dole szyby | — | P3 | ✅ osobówki, van, ciężkie |

## Osobówki (hatchback, liftback, sedan, kombi, SUV)

| Usterka | Jak jest naprawdę | Źródło | P | Status |
|---|---|---|---|---|
| Sylwetka „lat 80.”: pionowa płyta czoła, duże kliny reflektorów, płaskie boki | Pochylone czoło, wąskie lampy zawinięte na błotnik, próg schowany pod bok | Octavia IV, Golf 8 (zdjęcia) | P2 | ✅ czoło pochylone ok. 30° od pionu (`zn`, `yf` w `body.rs`), lampy i grill na skosie, zawinięcie lamp przednich i tylnych, próg 0,93 szerokości |
| Sedan nie różni się od liftbacka (dach jak w coupé) | Trzy bryły: bagażnik ok. 0,8–0,9 m, tylna szyba stromsza | Corolla sedan | P2 | ✅ bagażnik 0,17·L, tylna szyba ≥ 40°, szyba przednia 29° w archetypie |
| Kombi: pionowy słupek D jak Volvo 240 | Octavia Combi: pochylony słupek D | cararac.com | P2 | ✅ `rearGlassAngle` 62° |
| SUV: czarna płyta grilla, niewidoczne paski lamp | Duży grill, ale widoczne lampy | Tucson IV / Sportage | P2 | ✅ `grill` 0,75, wyższy pasek LED z zawinięciem |
| Czerwona „warga” lakieru pod zderzakiem | Zderzak do spodu nadwozia | — | P2 | ✅ |
| Załoga: stopy przebijają podłogę, poza „krzesło” | Punkt H ok. 0,5 m, pięty ok. 0,28 m pod punktem H, nogi wyciągnięte | CIOP, SAE J826 (*niezweryfikowane online*) | P1 | ⏳ nowa poza `drive`/`ride` w Tłumie |
| Nadkola trapezowe | Łuk | — | P3 | ⏸ limit 12 stacji lofta (ADR 0023) |

## Vany (van, ZRM P, furgon Policji, van PSP)

| Usterka | Jak jest naprawdę | Źródło | P | Status |
|---|---|---|---|---|
| Drzwi przesuwne na obu burtach | Seryjnie tylko po prawej (ruch prawostronny) | parkers.co.uk (Sprinter) | P1 | ✅ szwy, klamka, szyny tylko na −X; lewa burta gładka z listwą |
| Kabina jak osobówka z doklejonym pudłem (długa maska, szyba 0,45 m za osią) | Sprinter/Crafter: maska ~1,0 m, podstawa szyby prawie nad osią, nad szybą daszek | carexpert.com.au, vandimensions.com | P1 | ✅ podstawa szyby 0,08 m za osią, szyba 52° do 0,3 m pod dachem, pas nad szybą, okna kabiny do górnej krawędzi szyby, słupek B 1,45 m za podstawą szyby |
| 8 żeber dachu jak rolety | Gładki dach, 2–3 przetłoczenia | — | P3 | ✅ 3 przetłoczenia w kolorze lakieru |
| Mały grill, białe zderzaki | Duży grill, czarne zderzaki | — | P3 | ✅ `grill` 0,85, `bumperColor` grafit (służby nadpisują) |
| Koła 0,72 m | 225/75 R16 ≈ 0,74–0,76 m | vandimensions.com | P3 | ✅ 0,74 m |
| ZRM: AMBULANS tylko lustrzany na masce; brak napisu z tyłu, pasów na tylnych drzwiach | Lustrzany z przodu, zwykły z tyłu, pasy dookoła | mp.pl (PTRM) | P1 | ✅ AMBULANS z tyłu, pasy na tylnych drzwiach; oznaczenie zespołu „P” nad pasami |
| ZRM: Gwiazda Życia | Do weryfikacji w Dz.U. 2025 poz. 1768 (niedostępne z sesji) | — | P2 | ❓ niepewne, nie dodane |
| Policja: POLICJA tylko na burtach | Napis także z przodu i z tyłu | Dz.U. 2022 poz. 122 (§ 31, fragmenty) | P1 | ✅ |

## Osobówki służb (RO, RD, Straż Miejska, techniczne, PSP)

| Usterka | Jak jest naprawdę | Źródło | P | Status |
|---|---|---|---|---|
| Straż Miejska bez napisu | Szachownica + żółty napis STRAŻ MIEJSKA na drzwiach i z tyłu | Dz.U. 2012 poz. 525 (omówienia prasowe) | P1 | ✅ 2 rzędy pól 7 × 7 cm, napis na przednich drzwiach i na klapie |
| Radiowóz bez oznakowania z przodu i z tyłu | POLICJA na klapie, z przodu niebieskie pole z białym napisem | Dz.U. 2022 poz. 122 | P1 | ✅ |
| Pas zwęża się klinem na masce i tyle | Pas stałej szerokości od błotnika do tyłu | § 31 („z dostosowaniem do linii nadwozia”) | P2 | ✅ `band_range`: pas kończy się prosto, górna krawędź pod podstawą szyby |
| Ż rysowane jako Ƶ | Z z kropką | — | P2 | ✅ |
| Belki za krótkie i ciemne | Ok. 1,2–1,4 m, jasne klosze | Elektrometal/Haztec (katalogi) | P2–P3 | ✅ 0,9–1,4 m, 11 cm, jaśniejsze niebieskie i żółte |
| Pojazd techniczny bez nazwy służby | Nazwa służby/organizacji na drzwiach | — | P2 | ⏸ nie wiadomo, jaka służba (pomoc drogowa, ZDM) — do danych służb |
| Gwiazda policyjna, hasło | Opcjonalne wg § 31 | Dz.U. 2022 poz. 122 | P3 | ⏸ |
| Budżet trójkątów | Napisy z fontu 5×7 z przodu, z boków i z tyłu | — | — | ❓ limit dla aut służb podniesiony z 1500 do 1800 (cywilne bez zmian ≤ 1500) |

## Ciężkie (PD, SCRt, GBA, podwozia)

| Usterka | Jak jest naprawdę | Źródło | P | Status |
|---|---|---|---|---|
| Szyba przednia 1,48 m do samego dachu („smukła”, autobusowa kabina) | MAN TGM / Scania P: szyba ok. 1,0–1,2 m, nad nią pas blachy 0,25–0,35 m | truckscout24 (TGM 4x4 3200 mm), Rosenbauer AT na MAN | P1 | ✅ szyba do `top − 0,32`, pas blachy zaokrąglony do dachu, osłona przeciwsłoneczna; decyzja użytkownika: wymiary kabiny zostają |
| Okna boczne do dachu, dach jak pudło | Okno ~0,8–0,9 m, zaokrąglona krawędź dachu | j.w. | P2 | ✅ okna do górnej krawędzi szyby, fazka 0,2 m, `tumblehome` 0,05 |
| GCBA: 2 skrytki po 2,9 m | Żaluzje ≤ 1,6 m | OPZ PSP (platformazakupowa.pl) | P1 | ✅ liczba kolumn rośnie z długością zabudowy |
| Brak oznakowania PSP i belek | Belka na kabinie, STRAŻ z przodu, numer operacyjny | zarządzenie KG PSP nr 1/2020 (format numeru) | P1 | ✅ belka na dachu kabiny (PSP niebieska, PD żółta), STRAŻ na pasie nad szybą, numer na drzwiach; ⏸ pasy konturowe (R104), numer na dachu zabudowy |
| Felgi „osobowe” 5-ramienne | Stalowa tarcza z 10 nakrętkami | — | P2 | ✅ |
| Jasny zbiornik „pływa” w szczelinie za kabiną | Ciemny, pod ramą | — | P2 | ✅ |
| PD: za krótki zwis tylny (7,0 m przy rozstawie 4,8 m) | Laweta 7,8–8,3 m | oferty lawet | P2 | ✅ spec PD: 7,9 m |
| PD: płaski pokład bez burt i rolek | Platforma z szynami bocznymi, rolkami | — | P2 | ⏸ issue 11 (zabudowy) |
| 3 osie / 6x6, koła bliźniacze | — | — | P2 | ⏸ issue 10 (znane) |
| Pionowe zespoły lamp tylnych, chlapacze, belka podjazdowa | R48/R58 | — | P2 | ⏸ issue 10–11 |
| Siedzenia: biodra 1,55 m, 0,6 m nad głową | Biodra ok. 1,6–1,7 m | spec MAN TGM (*niezweryfikowane*) | P3 | ⏳ razem z pozą `drive` |

## Ludzie (Tłum)

Decyzja użytkownika (2026-11-09): **urealnić** (głowa ok. 1/7,5 wzrostu, kończyny o 10–15% grubsze,
sylwetka wg BMI, płci i wieku) oraz dodać **palce z rigiem**. Uwagi użytkownika: nogi dzieci prawie
z boku ciała, twarze sztuczne.

| Usterka | P | Status |
|---|---|---|
| Poza `drive` = krzesło: stopy przebijają podłogę auta | P1 | ⏳ |
| Zarost jako brązowa maska na pół twarzy | P1 | ⏳ |
| Brak odblasków i napisów (kamizelki, ratownik, strażak) | P1 | ⏳ |
| Hełm PSP jak bejsbolówka; kask budowlany jak czapka | P1 | ⏳ |
| Drogówka różni się od prewencji tylko kamizelką; prewencja bez widocznej kamizelki | P1 | ⏳ |
| Biznes nie do odróżnienia od policjanta | P1 | ⏳ |
| Senior: jasne szczeliny w płaszczu | P1 | ⏳ |
| Nogi dzieci z boku ciała, jedno ciało dla wszystkich, twarze sztuczne | P1 | ⏳ |
| Palce (rig Unity Humanoid, dopisane do rr_human) | — | ⏳ |
| Kolor ubioru służbowego PSP | P2 | ❓ do sprawdzenia w Dz.U. 2021 poz. 1795 |

## Do decyzji użytkownika

1. Budżet aut służb: 1800 trójkątów zamiast 1500 (napisy z przodu, z boków i z tyłu).
2. Budżet człowieka z palcami: do 1500 (było 800–1200).
3. Oznakowanie karetki wg Dz.U. 2025 poz. 1768: Gwiazda Życia i wzór pasów — wymaga tekstu rozporządzenia.
