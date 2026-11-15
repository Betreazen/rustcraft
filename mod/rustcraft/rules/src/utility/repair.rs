//! The repair stand (spec 22): one unit of an item's own material gives back a quarter of its
//! durability, as the anvil's repair with material does (no level cost: the stand pays in energy).

/// The material that repairs `item`, by the item's family; none for an item it does not repair.
pub fn material_of(item: &str) -> Option<&'static [&'static str]> {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    const PLANKS: &[&str] = &[
        "minecraft:oak_planks",
        "minecraft:spruce_planks",
        "minecraft:birch_planks",
        "minecraft:jungle_planks",
        "minecraft:acacia_planks",
        "minecraft:dark_oak_planks",
        "minecraft:mangrove_planks",
        "minecraft:cherry_planks",
        "minecraft:pale_oak_planks",
        "minecraft:bamboo_planks",
        "minecraft:crimson_planks",
        "minecraft:warped_planks",
    ];
    const STONE: &[&str] = &["minecraft:cobblestone", "minecraft:cobbled_deepslate", "minecraft:blackstone"];
    let prefixes: [(&str, &'static [&'static str]); 8] = [
        ("wooden_", PLANKS),
        ("stone_", STONE),
        ("iron_", &["minecraft:iron_ingot"]),
        ("chainmail_", &["minecraft:iron_ingot"]),
        ("golden_", &["minecraft:gold_ingot"]),
        ("diamond_", &["minecraft:diamond"]),
        ("netherite_", &["minecraft:netherite_ingot"]),
        ("copper_", &["minecraft:copper_ingot"]),
    ];
    if let Some((_, m)) = prefixes.iter().find(|(p, _)| name.starts_with(p)) {
        return Some(m);
    }
    match name {
        n if n.starts_with("leather_") => Some(&["minecraft:leather"]),
        "turtle_helmet" => Some(&["minecraft:turtle_scute"]),
        "elytra" => Some(&["minecraft:phantom_membrane"]),
        "mace" => Some(&["minecraft:breeze_rod"]),
        "shield" => Some(PLANKS),
        "wolf_armor" => Some(&["minecraft:armadillo_scute"]),
        _ => None,
    }
}

/// Whether `item` is a material any item is repaired with.
pub fn is_material(item: &str) -> bool {
    [
        "minecraft:wooden_sword",
        "minecraft:stone_sword",
        "minecraft:iron_sword",
        "minecraft:golden_sword",
        "minecraft:diamond_sword",
        "minecraft:netherite_sword",
        "minecraft:copper_sword",
        "minecraft:leather_helmet",
        "minecraft:turtle_helmet",
        "minecraft:elytra",
        "minecraft:mace",
        "minecraft:wolf_armor",
    ]
    .iter()
    .filter_map(|i| material_of(i))
    .any(|m| m.contains(&item))
}

/// The damage left after one unit of material: a quarter of the item's life back.
pub fn repaired(damage: u32, max_damage: u32) -> u32 {
    damage.saturating_sub((max_damage / 4).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_take_their_own_material() {
        assert_eq!(material_of("minecraft:iron_pickaxe"), Some(&["minecraft:iron_ingot"][..]));
        assert!(material_of("minecraft:wooden_axe").unwrap().contains(&"minecraft:birch_planks"));
        assert_eq!(material_of("minecraft:stick"), None);
        assert!(is_material("minecraft:diamond"));
        assert!(!is_material("minecraft:dirt"));
    }

    #[test]
    fn a_unit_gives_back_a_quarter() {
        assert_eq!(repaired(200, 250), 138);
        assert_eq!(repaired(10, 250), 0);
    }
}
