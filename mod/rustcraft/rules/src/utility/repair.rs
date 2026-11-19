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
        // Slice 18 (guide `repair`): the rod, the bow and the crossbow with string.
        "fishing_rod" | "bow" | "crossbow" => Some(&["minecraft:string"]),
        // 0.20 (round 31): the archaeology table's brush goes to the stand too; its copper.
        "brush" => Some(&["minecraft:copper_ingot"]),
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
        "minecraft:bow",
    ]
    .iter()
    .filter_map(|i| material_of(i))
    .any(|m| m.contains(&item))
}

/// The damage left after one unit of material: a quarter of the item's life back.
pub fn repaired(damage: u32, max_damage: u32) -> u32 {
    damage.saturating_sub((max_damage / 4).max(1))
}

/// 0.19.3 (the owner: «ремонтный стенд всё ещё забирает вещи, это останавливает выполнение
/// заказов»): what the magistral or a hopper may put into the stand. Into a slot of what is
/// repaired, only an item the stand repairs (its wear the caller checks); into a material slot,
/// only the material of an item that lies there now (`tools`). Before, the net filled the item
/// slots with anything that was no material (an order's chests, hoppers and sticks) and the
/// material slots with any material, and the orders waited for them.
pub fn net_takes(tool_slot: bool, tools: &[&str], item: &str) -> bool {
    if tool_slot {
        return material_of(item).is_some();
    }
    tools.iter().filter_map(|t| material_of(t)).any(|m| m.contains(&item))
}

// ---- 0.20 (round 31): a machine's worn tool to the stand and back ----
//
// The owner: «рыболовный стенд, у него портится удочка, у нас там есть ремонтный стенд, чтобы он
// корректно забирал, отдавал удочку… забирал для её починки необходимые ресурсы… она целая, потом
// возвращалась». A machine that works with a tool (the fishing station's rod, the butcher's
// sword, the archaeology table's brush) hands it, one point before it breaks, to a repair stand
// on its item net; the storage on the stand's net gives the stand the material it needs; the
// mended tool goes straight back into its machine. One tool of a machine is away at a time.

/// The custom data key a tool away for repair carries: the cell of its machine, `[x, y, z]`.
pub const HOME_KEY: &str = "rustcraft:tool_home";
/// The machine's saved key while its tool is away: the cell of the stand, `[x, y, z]`.
pub const TRIP_KEY: &str = "tool_trip";

/// Whether a tool with `damage` of `max_damage` goes to the stand: one point before it breaks
/// (the fishing station stops there, «Удочка изношена»; the butcher and the archaeology table would
/// spend the last point and lose the tool). A tool that does not wear stays.
pub fn wants_repair(damage: u32, max_damage: u32) -> bool {
    max_damage > 0 && damage + 1 >= max_damage
}

/// How many units of its material mend a tool with `damage` of `max_damage` whole: a quarter of
/// its life a unit (`repaired`).
pub fn units_for(damage: u32, max_damage: u32) -> u32 {
    damage.div_ceil((max_damage / 4).max(1))
}

/// How many units the storage gives the stand: what its tools away for repair still `need`, less
/// what its material cells `have`, no more than their `room`.
pub fn top_up(need: u32, have: u32, room: u32) -> u32 {
    need.saturating_sub(have).min(room)
}

/// The stand a machine at `from` sends its tool to: of the stands on its item net with a free
/// cell for it (`stands`: where each is and whether it has one), the nearest.
pub fn pick_stand(from: [i32; 3], stands: &[([i32; 3], bool)]) -> Option<[i32; 3]> {
    let far = |p: &[i32; 3]| (0..3).map(|i| i64::from((p[i] - from[i]).abs())).sum::<i64>();
    stands.iter().filter(|(_, free)| *free).map(|(p, _)| *p).min_by_key(far)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_goes_to_the_stand_one_point_before_it_breaks() {
        assert!(wants_repair(63, 64), "the rod's last point");
        assert!(!wants_repair(62, 64));
        assert!(wants_repair(249, 250), "an iron sword");
        assert!(!wants_repair(0, 0), "what does not wear stays");
        assert_eq!(material_of("minecraft:brush"), Some(&["minecraft:copper_ingot"][..]));
    }

    #[test]
    fn the_stand_gets_as_much_material_as_mends_the_tool_whole() {
        // A rod of 64 points: 16 a string.
        assert_eq!(units_for(63, 64), 4);
        assert_eq!(units_for(16, 64), 1);
        assert_eq!(units_for(0, 64), 0);
        assert_eq!(units_for(249, 250), 5, "62 points a unit");
        assert_eq!(top_up(4, 1, 8), 3);
        assert_eq!(top_up(4, 5, 8), 0);
        assert_eq!(top_up(9, 0, 2), 2, "no more than the cells hold");
    }

    #[test]
    fn the_nearest_stand_with_room_takes_the_tool() {
        let stands = [([10, 0, 0], true), ([3, 0, 0], false), ([5, 0, 1], true)];
        assert_eq!(pick_stand([0, 0, 0], &stands), Some([5, 0, 1]));
        assert_eq!(pick_stand([0, 0, 0], &[([3, 0, 0], false)]), None);
    }

    #[test]
    fn the_net_brings_only_what_the_stand_mends_and_its_material() {
        // The owner's stand held sticks, gold and iron beside two rods.
        for item in ["minecraft:stick", "minecraft:chest", "minecraft:hopper", "rustcraft:coppersteel"] {
            assert!(!net_takes(true, &[], item), "{item} is nothing it repairs");
        }
        assert!(net_takes(true, &[], "minecraft:iron_pickaxe"));
        assert!(net_takes(true, &[], "minecraft:fishing_rod"));
        let rods = ["minecraft:fishing_rod", "minecraft:fishing_rod"];
        assert!(net_takes(false, &rods, "minecraft:string"));
        for item in ["minecraft:gold_ingot", "minecraft:iron_ingot", "minecraft:stick"] {
            assert!(!net_takes(false, &rods, item), "{item} mends no rod");
        }
        assert!(!net_takes(false, &[], "minecraft:iron_ingot"), "nothing to mend, no material");
        assert!(net_takes(false, &["minecraft:fishing_rod", "minecraft:iron_pickaxe"], "minecraft:iron_ingot"));
    }

    #[test]
    fn tools_take_their_own_material() {
        assert_eq!(material_of("minecraft:iron_pickaxe"), Some(&["minecraft:iron_ingot"][..]));
        assert!(material_of("minecraft:wooden_axe").unwrap().contains(&"minecraft:birch_planks"));
        assert_eq!(material_of("minecraft:stick"), None);
        assert!(is_material("minecraft:diamond"));
        assert!(!is_material("minecraft:dirt"));
        // Slice 18: string for the rod, the bow and the crossbow.
        for i in ["minecraft:fishing_rod", "minecraft:bow", "minecraft:crossbow"] {
            assert_eq!(material_of(i), Some(&["minecraft:string"][..]));
        }
        assert!(is_material("minecraft:string"));
    }

    #[test]
    fn a_unit_gives_back_a_quarter() {
        assert_eq!(repaired(200, 250), 138);
        assert_eq!(repaired(10, 250), 0);
    }
}
