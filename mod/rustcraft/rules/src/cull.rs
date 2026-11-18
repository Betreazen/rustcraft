//! Blocks whose model does not fill its cell. The game hides the faces of a neighbour that touch an
//! occluding block, so a block that occludes but lets the eye through (a glass frame, open sides,
//! a plate) leaves holes in the walls next to it (0.18.2: the empty tank; 0.18.3: the station and
//! sixteen other machines). Every block named here is declared with `occludes: false`.

/// The ids of the blocks that do not occlude: their model leaves part of the cell open seen along
/// some axis, or its glass lets the eye through.
pub const SEE_THROUGH: [&str; 29] = [
    "rustcraft:anchor",
    "rustcraft:apiary",
    "rustcraft:archeo",
    "rustcraft:builder",
    "rustcraft:butcher",
    "rustcraft:charge_plate",
    "rustcraft:crystal",
    "rustcraft:disintegrator",
    "rustcraft:druse",
    "rustcraft:enchanter",
    "rustcraft:feeder",
    "rustcraft:fishery",
    "rustcraft:housing",
    "rustcraft:kitchen",
    "rustcraft:miner",
    "rustcraft:monorail",
    "rustcraft:projector",
    "rustcraft:repair",
    "rustcraft:seed",
    "rustcraft:solar",
    "rustcraft:station",
    "rustcraft:tank",
    "rustcraft:trash",
    "rustcraft:turbine",
    "rustcraft:turret",
    "rustcraft:vein",
    "rustcraft:windmill",
    "rustcraft:xp_tank",
    "rustcraft:zip_anchor",
];

/// Whether block `id` must not occlude its neighbours.
pub fn see_through(id: &str) -> bool {
    SEE_THROUGH.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content/assets/rustcraft");

    fn model(name: &str) -> Option<Value> {
        let path = name.strip_prefix("rustcraft:")?;
        let text = std::fs::read_to_string(format!("{ASSETS}/models/{path}.json")).ok()?;
        let mut json: Value = serde_json::from_str(&text).ok()?;
        if json.get("elements").is_none()
            && let Some(parent) = json["parent"].as_str().and_then(model)
        {
            json["elements"] = parent["elements"].clone();
        }
        Some(json)
    }

    /// The cells of a 16×16 view along `axis` (0 x, 1 y, 2 z) that no unrotated element covers.
    fn open_cells(elements: &[Value], axis: usize) -> usize {
        let mut seen = [[false; 16]; 16];
        let (u, v) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        for e in elements {
            if e["rotation"]["angle"].as_f64().is_some_and(|a| a != 0.0) || e["faces"].as_object().is_none_or(|f| f.is_empty()) {
                continue;
            }
            let at = |k: &str, i: usize| e[k][i].as_f64().unwrap_or(0.0);
            let span = |i: usize| (at("from", i).floor().max(0.0) as usize, at("to", i).ceil().min(16.0) as usize);
            let ((u0, u1), (v0, v1)) = (span(u), span(v));
            for row in seen.iter_mut().take(u1).skip(u0) {
                for cell in row.iter_mut().take(v1).skip(v0) {
                    *cell = true;
                }
            }
        }
        seen.iter().flatten().filter(|c| !**c).count()
    }

    /// The first model each block state file names: the block's body, before any tier badge.
    fn body_model(state: &Value) -> Option<String> {
        let first = state["variants"]
            .as_object()
            .and_then(|v| v.values().next().cloned())
            .or_else(|| state["multipart"][0]["apply"].clone().into());
        let first = if first.as_ref()?.is_array() { first?[0].clone() } else { first? };
        first["model"].as_str().map(str::to_owned)
    }

    #[test]
    fn every_block_with_an_open_model_is_listed_as_see_through() {
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(format!("{ASSETS}/blockstates")).unwrap().filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().trim_end_matches(".json").to_owned();
            let state: Value = serde_json::from_str(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
            let Some(body) = body_model(&state).and_then(|m| model(&m)) else { continue };
            let elements = body["elements"].as_array().cloned().unwrap_or_default();
            let open = elements.is_empty() || (0..3).any(|axis| open_cells(&elements, axis) > 0);
            let id = format!("rustcraft:{name}");
            if open && !see_through(&id) {
                missing.push(id);
            }
        }
        assert!(missing.is_empty(), "open models that still occlude: {missing:?}");
    }
    #[test]
    fn every_block_drops_when_mined_with_a_pickaxe() {
        // 0.18.4: the blocks copy the furnace, the iron block or the amethyst block, which drop only
        // to their right tool; a block left out of mineable/pickaxe had none and dropped nothing
        // (the owner's miner broke into air).
        let tag = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../content/data/minecraft/tags/block/mineable/pickaxe.json"
        ))
        .unwrap();
        let tag: Value = serde_json::from_str(&tag).unwrap();
        let listed: Vec<&str> = tag["values"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        let mut missing: Vec<String> = std::fs::read_dir(format!("{ASSETS}/blockstates"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| format!("rustcraft:{}", e.file_name().to_string_lossy().trim_end_matches(".json")))
            .filter(|id| !listed.contains(&id.as_str()))
            .collect();
        missing.sort();
        assert!(missing.is_empty(), "blocks no tool mines: {missing:?}");
    }
}
