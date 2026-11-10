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

Decyzja użytkownika (2026-11-09): **urealnić** (głowa ok. 1/7,5 wzrostu, kończyny grubsze, sylwetka wg
BMI, płci i wieku) i dodać **palce z rigiem**. Uwagi użytkownika: nogi dzieci prawie z boku ciała, twarze
sztuczne. Trzy rundy review (po 3 recenzentów: cywile i warianty sylwetek, mundury i służby, anatomia
z rigiem i pozami).

| Usterka (runda 1–2) | P | Status |
|---|---|---|
| Nogi dzieci z boku ciała (`shoulderWidth`, `hipWidth`, `crotchHeight` to ułamki wzrostu — skel.rs ich nie mnożył) | P1 | ✅ test `child_legs_start_under_the_pelvis` |
| Jedno ciało dla wszystkich; głowa 1/6 | P1 | ✅ tułów 8-boczny w 9 przekrojach wg płci, BMI i wieku; głowa dorosłego 1/7,1 (pomiar siatki), dziecka 8 l. 1/6,3, 5 l. 1/5,8 |
| Twarze sztuczne | P1 | ✅ rzeźbiona głowa 8×10, wzorce dorosły / kobieta / dziecko, białka i tęczówki, nos, wargi, uszy, drobne różnice z ziarna; ocena recenzenta 3/5 z bliska, 3,5/5 z odległości kamery |
| Poza `drive` = krzesło, stopy przez podłogę auta | P1 | ✅ oparcie 27°, udo 11°, kolano 130°, pięty 0,3 m pod punktem H, dłonie 0,34 m nad i 0,40 m przed H; nowa poza `ride` (pasażer); punkty H Miejsc w Taborze dopasowane (osobówki 0,56 m, kabiny ciężkie +0,1 m i 0,13 m w tył) — stopy ok. 0,10 m przed czołem kabiny, głowy ≥ 0,5 m pod dachem ciężkich |
| Palce z rigiem | — | ✅ 30 kości Unity Humanoid dopisane po `RightHandGrip` (pierwsze 29 kości rr_human bez zmian, test 2 cm), człony przypięte do kości, chwyt kierownicy |
| Zarost jako maska | P1 | ✅ cienka warstwa na żuchwie i brodzie (materiał `Stubble`) |
| Brak odblasków i napisów | P1 | ✅ kamizelki z taśmami, POLICJA na plecach, RATOWNIK MEDYCZNY, PSP, taśmy żółto-srebrne strażaka |
| Hełm PSP jak bejsbolówka, kask budowlany jak czapka | P1 | ✅ kopuła z grzebieniem i osłoną karku; kask z rondem |
| Drogówka vs prewencja, biznes vs policjant | P1 | ✅ biała czapka garnizonowa, biały pas, białe rękawice; czarna kamizelka prewencji; marynarka z klapami, koszula, krawat |
| Płaszcz seniora, spódnica jako sztywne pudła | P1 | ✅ poły przypięte do ud, leżą na udach w pozach siedzących; ⏸ P2: sztywna krawędź nad kolanem |
| Sylwetka kobieca, otyłość widoczna tylko z profilu | P1 | ✅ |
| Strój zimowy | P1 | ✅ grubsza puchówka, szalik, rękawiczki; ⏸ P3: przeszycia puchówki |
| Torba ratownika wisi w powietrzu | P1 | ✅ na pasku przez ramię; ⏸ P2: przecina lewe ramię przy biodrze |
| Kolor ubioru służbowego PSP | P2 | ✅ piaskowy potwierdzony źródłami wtórnymi (załączniki do rozporządzenia niedostępne z sesji) |

Runda 3: **0 usterek P1** (służby 0, anatomia 0). Recenzent cywili zgłosił 2 P1, oba zweryfikowane:
proporcje głów dzieci są w celu (pomiar siatki powyżej, recenzent szacował z obrazu), płaszcz w pozach
siedzących leży na udach (ciemne fragmenty to spodnie między połami) — obniżone do P2. Budżet 1124–1406
trójkątów (≤ 1500, test także dla najcięższej głowy).

Zostało (P2–P3): szyja dorosłych nieco długa, żuchwa w profilu lekko ścięta, ciało dziecka smukłe,
aparat oddechowy bez pasów, kołnierze robotnika/operatora z drobnym zębatym stykiem, plecak dziecka
i butla strażaka wchodzą w oparcie w pozach siedzących, chód bez uniesienia czubka stopy.
**GLB człowieka nie ma jeszcze skinu ani wag** — to zaplanowane issue 16.

## Runda 2 — pojazdy (2026-11-09)

3 równoległych recenzentów (osobówki z wariantami służb, vany, ciężkie), arkusze przed/po. Wynik:
**0 usterek P1** we wszystkich 19 modelach, werdykt „akceptowalny z uwagami”. Poprawione od razu (P2):

- pasy oklejenia kończą się równo (jeden zakres dla całego zestawu pasów), van PSP ma numer operacyjny na ścianie ładowni i na tylnych drzwiach, osobówki PSP — numer na klapie;
- ramka (uchwyt) tablicy — tablica nie ginie na białym zderzaku; zderzaki osobówek ze ściętymi narożnikami i nie szersze od nadwozia; cieńsza czarna okładzina w kombi i liftbacku; ciemna oprawa lamp tylnych;
- lampy belek jako osobny materiał `Beacon` (emisyjny, nie szkło) — żółte nie wychodzą już brązowe;
- przedział przelotowy zabudowy pożarniczej to zamknięta skrytka z żaluzjami (zamiast „wiszącego” mostu dachowego);
- szyba lekkiej kabiny (PD) zaczyna się nad kołem (ok. 1,4 m), numer na drzwiach kabiny PSP pod klamką, wyższy nos vana.

Zostały P2–P3 do kolejnych issue: nos vana bliżej Sprintera W907, okno w tylnych drzwiach furgonu Policji,
STRAŻ POŻARNA na vanie (do sprawdzenia w zarządzeniu KG PSP), żuraw HDS bez podpór i haka, lampy obrysowe
na dachu kabiny, trzecie okno kabiny brygadowej wygląda jak drzwi, numer na dachu SUV-a PSP czytany od przodu.

## Do decyzji użytkownika

1. Budżet aut służb: 1800 trójkątów zamiast 1500 (napisy z przodu, z boków i z tyłu); najcięższy wariant 1744. Cywilne ok. 1140.
2. Budżet człowieka z palcami: do 1500 (było 800–1200); najcięższy archetyp 1406.
3. Oznakowanie karetki wg Dz.U. 2025 poz. 1768: Gwiazda Życia i wzór pasów — wymaga tekstu rozporządzenia.
4. Akceptacja modeli przed wypiekaniem do gry (issue 09).
