//! Slice 18: a bucket on a tank (guide `tank`): a water or lava bucket pours into the tank, an
//! empty bucket fills from it. The nutrient solution and the catalysts live only in tanks and
//! channels: no bucket takes them.

use crate::machines::pump::{LAVA, WATER};
use crate::net::fluids::BUCKET;
use crate::net::tanks::TankGroup;

pub const EMPTY: &str = "minecraft:bucket";
pub const WATER_BUCKET: &str = "minecraft:water_bucket";
pub const LAVA_BUCKET: &str = "minecraft:lava_bucket";

/// What a click with `held` on a tank of `group` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Use {
    /// A bucket of `fluid` pours in; the player keeps an empty bucket.
    Pour { fluid: &'static str },
    /// An empty bucket fills with `fluid`, the player gets `filled`.
    Fill { fluid: &'static str, filled: &'static str },
    /// The tank has no room for a bucket of this fluid.
    NoRoom,
    /// The tank holds less than a bucket.
    Empty,
    /// The tank holds a fluid no bucket takes (the solution, a catalyst).
    NoBucket(String),
    /// Not a bucket.
    Other,
}

pub fn use_on(held: &str, group: &TankGroup) -> Use {
    let poured = match held {
        WATER_BUCKET => Some(WATER),
        LAVA_BUCKET => Some(LAVA),
        _ => None,
    };
    if let Some(fluid) = poured {
        return if group.room_for(fluid) >= BUCKET {
            Use::Pour { fluid }
        } else {
            Use::NoRoom
        };
    }
    if held != EMPTY {
        return Use::Other;
    }
    match group.fluid.as_deref() {
        Some(f) if group.amount >= BUCKET && f == WATER => Use::Fill { fluid: WATER, filled: WATER_BUCKET },
        Some(f) if group.amount >= BUCKET && f == LAVA => Use::Fill { fluid: LAVA, filled: LAVA_BUCKET },
        Some(f) if group.amount > 0 && f != WATER && f != LAVA => Use::NoBucket(f.to_owned()),
        _ => Use::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tank(fluid: Option<&str>, amount: u64) -> TankGroup {
        TankGroup {
            blocks: vec![[0, 0, 0]],
            fluid: fluid.map(str::to_owned),
            amount,
            capacity: 16 * BUCKET,
            mixed: false,
            pin: fluid.map(str::to_owned),
        }
    }

    #[test]
    fn a_bucket_pours_into_a_tank_with_room() {
        assert_eq!(use_on(WATER_BUCKET, &tank(None, 0)), Use::Pour { fluid: WATER });
        assert_eq!(use_on(LAVA_BUCKET, &tank(Some(LAVA), 1000)), Use::Pour { fluid: LAVA });
        assert_eq!(use_on(WATER_BUCKET, &tank(Some(LAVA), 1000)), Use::NoRoom);
        assert_eq!(use_on(WATER_BUCKET, &tank(Some(WATER), 15_500)), Use::NoRoom);
    }

    #[test]
    fn an_empty_bucket_takes_water_or_lava_only() {
        assert_eq!(
            use_on(EMPTY, &tank(Some(WATER), 2000)),
            Use::Fill { fluid: WATER, filled: WATER_BUCKET }
        );
        assert_eq!(use_on(EMPTY, &tank(Some(WATER), 500)), Use::Empty);
        assert_eq!(use_on(EMPTY, &tank(None, 0)), Use::Empty);
        assert_eq!(
            use_on(EMPTY, &tank(Some("rustcraft:solution"), 4000)),
            Use::NoBucket("rustcraft:solution".into())
        );
        assert_eq!(use_on("minecraft:stone", &tank(None, 0)), Use::Other);
    }
}
