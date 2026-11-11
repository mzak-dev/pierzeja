use procedura_kit::data::ParamKind;
use procedura_kit::mix::{resolve_map, MixSpec};
use serde_json::json;
use tabor_core::data;

#[test]
fn loads_and_groups_are_fixed() {
    let ds = data();
    let ids: Vec<&str> = ds.groups.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, ["bryla", "sylwetka", "podwozie", "zabudowa", "sluzba", "material"]);
    assert_eq!(ds.archetypes.len(), 13);
    assert!(ds.archetype(&ds.default_archetype).is_some());
    assert!(ds.params.iter().all(|p| ids.contains(&p.group.as_str())));
}

#[test]
fn archetypes_have_sources_and_vectors_in_range() {
    let ds = data();
    for a in &ds.archetypes {
        assert!(!a.sources.is_empty(), "{}: brak źródeł", a.id);
        assert!(a.sources.iter().all(|s| s.verified.is_some()), "{}: źródło bez verified", a.id);
        assert_eq!(a.vector.len(), ds.params.len(), "{}: niepełny wektor", a.id);
        for p in &ds.params {
            let v = &a.vector[&p.key];
            match p.kind {
                ParamKind::Number | ParamKind::Int => {
                    let x = v.as_f64().unwrap_or_else(|| panic!("{}.{}: nie liczba", a.id, p.key));
                    assert!((p.min.unwrap()..=p.max.unwrap()).contains(&x), "{}.{} = {x} poza zakresem", a.id, p.key);
                }
                ParamKind::Enum => assert!(p.options.as_ref().unwrap().iter().any(|o| Some(o.value.as_str()) == v.as_str()), "{}.{} = {v}", a.id, p.key),
                ParamKind::Color => assert!(v.as_str().is_some_and(|s| s.len() == 7 && s.starts_with('#')), "{}.{} = {v}", a.id, p.key),
            }
        }
        // zwisy i rozstaw dają długość (zespół tylny tylko przy ≥ 3 osiach)
        let f = |k: &str| a.vector[k].as_f64().unwrap();
        let gap = if f("axles") > 2.0 { f("axleGap") } else { 0.0 };
        let sum = f("length") * (f("frontOverhang") + f("rearOverhang")) + f("wheelbase") + gap;
        assert!((sum - f("length")).abs() < 0.01, "{}: zwisy + rozstaw ≠ długość ({sum})", a.id);
    }
}

#[test]
fn m1_wheelbases_are_settable() {
    // Pojazdy M1 z RapidResponse/mods/core/data/pojazdy.toml: (archetyp bazowy, rozstaw osi)
    for (base, wb) in [("sedan_liftback", 2.8), ("kombi", 2.8), ("van", 3.7), ("podwozie_lekkie", 4.8), ("podwozie_srednie", 4.6)] {
        let spec = MixSpec { archetype: Some(base.into()), overrides: serde_json::from_value(json!({ "wheelbase": wb })).unwrap(), ..Default::default() };
        let (p, _, _) = resolve_map(data(), &spec);
        assert_eq!(p["wheelbase"].as_f64(), Some(wb), "{base}");
    }
}
