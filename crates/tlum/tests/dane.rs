use procedura_kit::data::ParamKind;
use serde_json::Value;
use tlum_core::{data, extra};

#[test]
fn loads_and_groups_are_fixed() {
    let ds = data();
    let ids: Vec<&str> = ds.groups.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, ["cialo", "glowa", "ubior", "akcesoria", "sezon", "material"]);
    assert!(ds.archetype(&ds.default_archetype).is_some());
    assert!(ds.params.iter().all(|p| ids.contains(&p.group.as_str())));
    for id in ["policjant_prewencji", "policjant_ruchu_drogowego", "ratownik_medyczny", "strazak_sluzbowy", "strazak_bojowy", "operator_ciezkiego_sprzetu"] {
        assert!(ds.archetype(id).is_some(), "brak archetypu {id}");
    }
}

#[test]
fn archetypes_have_sources_and_vectors_in_range() {
    let ds = data();
    for a in &ds.archetypes {
        assert!(!a.sources.is_empty(), "{}: brak źródeł", a.id);
        assert!(a.sources.iter().all(|s| s.verified.is_some()), "{}: źródło bez verified", a.id);
        assert_eq!(a.vector.len(), ds.params.len(), "{}: niepełny wektor", a.id);
        // overrides nie mogą być po cichu przycięte przez normalize_params (kolory: wielkość liter bez znaczenia)
        let norm = |v: &Value| v.as_f64().map_or_else(|| v.to_string().to_lowercase(), |x| x.to_string());
        for (k, v) in &a.overrides {
            assert_eq!(norm(&a.vector[k]), norm(v), "{}.{k}: override poza zakresem lub nieznany", a.id);
        }
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
    }
}

fn joints() -> Vec<(&'static str, Option<&'static str>, [f64; 3], [f64; 3])> {
    let v3 = |v: &'static Value| -> [f64; 3] { std::array::from_fn(|i| v[i].as_f64().unwrap()) };
    extra()["szkielet"]["jointy"].as_array().unwrap().iter()
        .map(|j| (j["name"].as_str().unwrap(), j["parent"].as_str(), v3(&j["head"]), v3(&j["tail"]))).collect()
}

#[test]
fn skeleton_matches_rr_human() {
    let j = joints();
    let mut names: Vec<&str> = j.iter().map(|x| x.0).collect();
    let mut want: Vec<String> = ["Root", "Hips", "Spine", "Chest", "UpperChest", "Neck", "Head"].map(String::from).into();
    for s in ["Left", "Right"] {
        for n in ["Shoulder", "UpperArm", "LowerArm", "Hand", "HandGrip", "UpperLeg", "LowerLeg", "Foot", "Toes", "HandIK", "FootIK"] {
            want.push(format!("{s}{n}"));
        }
    }
    assert_eq!(names.len(), 29);
    names.sort();
    want.sort();
    assert_eq!(names, want);
    // każdy rodzic istnieje, Root jest jedyny bez rodzica
    assert!(j.iter().all(|x| x.1.map_or(x.0 == "Root", |p| names.contains(&p))));

    let get = |n: &str| j.iter().find(|x| x.0 == n).unwrap();
    assert!((get("Hips").2[1] - 0.98).abs() <= 0.02, "Hips na {}", get("Hips").2[1]);
    assert!(get("LeftFoot").3[1] < 0.05 && get("LeftToes").3[1] < 0.05, "stopy nad ziemią");
    assert!(get("Head").2[1] > get("Hips").2[1]);
    assert!(get("LeftUpperLeg").2[0] > 0.0 && get("RightUpperLeg").2[0] < 0.0, "lewa = +X");
    assert!(get("LeftToes").3[2] > get("LeftFoot").2[2], "przód = +Z");
}
