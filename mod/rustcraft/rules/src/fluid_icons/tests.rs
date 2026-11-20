use super::*;
use crate::orders::FLUIDS;

fn repo_file(rel: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/../../../{rel}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn repo_text(rel: &str) -> String {
    String::from_utf8(repo_file(rel).unwrap_or_else(|| panic!("no {rel}"))).unwrap()
}

/// `rustcraft:solution_bucket` → `solution_bucket`.
fn name(item: &str) -> &str {
    item.strip_prefix("rustcraft:").unwrap()
}

#[test]
fn every_fluid_the_terminal_lists_shows_a_full_bucket() {
    // The owner: «питательный раствор — его иконка пустое ведро, исправь это».
    for fluid in FLUIDS {
        assert_ne!(icon_of(fluid), EMPTY, "{fluid}");
    }
    assert_eq!(icon_of("minecraft:water"), WATER_BUCKET);
    assert_eq!(icon_of("minecraft:lava"), LAVA_BUCKET);
    assert_eq!(icon_of("rustcraft:milk"), "minecraft:milk_bucket");
    assert_eq!(icon_of("rustcraft:solution"), "rustcraft:solution_bucket");
}

#[test]
fn every_mod_fluid_but_milk_has_its_own_bucket() {
    let mine: Vec<&str> = FLUIDS
        .iter()
        .copied()
        .filter(|f| f.starts_with("rustcraft:") && *f != "rustcraft:milk")
        .collect();
    let listed: Vec<&str> = ICONS.iter().map(|(f, _)| *f).collect();
    assert_eq!(listed, mine);
    for (fluid, icon) in ICONS {
        assert_eq!(
            icon,
            format!("{fluid}_bucket"),
            "a joint's filter names {fluid}'s bucket so"
        );
        assert!(is_icon_item(icon));
        assert_eq!(filter_icon(fluid), icon);
    }
    assert!(!is_icon_item(WATER_BUCKET));
}

#[test]
fn an_unknown_fluid_keeps_the_empty_bucket_and_a_filter_its_own() {
    assert_eq!(icon_of("othermod:oil"), EMPTY);
    assert_eq!(known_icon("othermod:oil"), None);
    assert_eq!(filter_icon("othermod:oil"), "othermod:oil_bucket");
    assert_eq!(filter_icon("minecraft:water"), WATER_BUCKET);
    assert_eq!(filter_icon("rustcraft:milk"), "minecraft:milk_bucket");
}

#[test]
fn every_display_bucket_has_its_item_model_texture_and_names() {
    let ru: serde_json::Value =
        serde_json::from_str(&repo_text("mod/content/assets/rustcraft/lang/ru_ru.json")).unwrap();
    let en: serde_json::Value =
        serde_json::from_str(&repo_text("mod/content/assets/rustcraft/lang/en_us.json")).unwrap();
    for (_, icon) in ICONS {
        let n = name(icon);
        let item = repo_text(&format!("mod/content/assets/rustcraft/items/{n}.json"));
        assert!(
            item.contains(&format!("\"rustcraft:item/{n}\"")),
            "{n}: {item}"
        );
        let model = repo_text(&format!(
            "mod/content/assets/rustcraft/models/item/{n}.json"
        ));
        assert!(
            model.contains(&format!("\"rustcraft:item/{n}\"")),
            "{n}: {model}"
        );
        let png = repo_file(&format!(
            "mod/content/assets/rustcraft/textures/item/{n}.png"
        ))
        .unwrap_or_else(|| panic!("no texture of {n}"));
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{n}");
        // IHDR: width, height, bit depth 8, colour type 6 (RGBA).
        assert_eq!(
            &png[16..26],
            &[0, 0, 0, 16, 0, 0, 0, 16, 8, 6],
            "{n} is no 16 x 16 RGBA"
        );
        for (code, lang) in [("ru_ru", &ru), ("en_us", &en)] {
            let key = format!("item.rustcraft.{n}");
            let words = lang[&key]
                .as_str()
                .unwrap_or_else(|| panic!("{code} has no {key}"));
            assert!(!words.trim().is_empty(), "{code}: {key}");
        }
    }
}

#[test]
fn every_display_bucket_is_painted_in_its_tank_colour() {
    let script = repo_text("assets/models/icons_mc.py");
    for (fluid, icon) in ICONS {
        let n = name(icon);
        let line = script
            .lines()
            .find(|l| l.starts_with(&format!("ICONS['{n}']")))
            .unwrap_or_else(|| panic!("icons_mc.py does not paint {n}"));
        let tint = format!("'#{:06x}'", crate::anim::fluid_tint(fluid));
        assert!(line.contains(&tint), "{n} is not painted {tint}: {line}");
    }
}

#[test]
fn the_display_buckets_have_no_recipe_and_no_place_in_the_creative_tab() {
    let tab = repo_text("mod/content/data/rustcraft/horizon/creative_tab/rustcraft.json");
    let dir = format!(
        "{}/../../../mod/content/data/rustcraft/recipe",
        env!("CARGO_MANIFEST_DIR")
    );
    let recipes: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|_| panic!("no {dir}"))
        .filter_map(|e| std::fs::read_to_string(e.ok()?.path()).ok())
        .collect();
    assert!(recipes.len() > 50, "{}", recipes.len());
    for (_, icon) in ICONS {
        assert!(
            !tab.contains(&format!("\"{icon}\"")),
            "{icon} is in the creative tab"
        );
        assert!(
            !recipes.iter().any(|r| r.contains(&format!("\"{icon}\""))),
            "a recipe names {icon}"
        );
    }
}
