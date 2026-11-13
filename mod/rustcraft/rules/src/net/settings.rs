//! A joint's settings (M7): for each channel on each face of a housing its own direction, item
//! filter (a white or black list) and priority. The housing keeps them; a face and slot without
//! settings works in the default of the block behind it. `Record` is the flat form the bundle
//! saves in the housing's block entity.

use std::collections::BTreeMap;

use crate::geom::Face;
use crate::net::items::{Filter, ItemKey, Mode};
use crate::net::topology::SLOTS;

/// How many items one joint's filter holds: the joint window shows that many cells a row.
pub const FILTER_LIMIT: usize = 3;
/// Priorities run from −PRIORITY_LIMIT to PRIORITY_LIMIT; higher fills first (M7, M8).
pub const PRIORITY_LIMIT: i32 = 9;

/// One channel's settings on one face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JointSetting {
    pub mode: Mode,
    pub filter: Filter,
    pub priority: i32,
}

impl JointSetting {
    /// A joint in `mode` that lets everything through at priority 0: what a block's default is.
    pub fn of_mode(mode: Mode) -> Self {
        JointSetting {
            mode,
            filter: Filter::default(),
            priority: 0,
        }
    }

    /// The setting after `edit`; an edit that cannot apply (a fourth filter item, a priority
    /// past the limit) leaves it as it is.
    pub fn apply(&self, edit: &Edit) -> JointSetting {
        let mut next = self.clone();
        match edit {
            Edit::NextMode => next.mode = self.mode.next(),
            Edit::PreviousMode => next.mode = self.mode.previous(),
            Edit::ToggleList => next.filter.allow_only = !self.filter.allow_only,
            Edit::AddItem(item) => {
                if next.filter.items.len() < FILTER_LIMIT {
                    next.filter.items.insert(item.clone());
                }
            }
            Edit::RemoveItem(item) => {
                next.filter.items.remove(item);
            }
            Edit::Priority(delta) => {
                next.priority = (self.priority + delta).clamp(-PRIORITY_LIMIT, PRIORITY_LIMIT);
            }
        }
        next
    }
}

/// What a click in the joint window changes in one setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    NextMode,
    PreviousMode,
    /// White list ↔ black list.
    ToggleList,
    AddItem(ItemKey),
    RemoveItem(ItemKey),
    Priority(i32),
}

/// The face and slot a setting is for.
pub type FaceSlot = (Face, usize);

/// Everything a housing keeps of its joints. Faces and slots it has nothing for are absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HousingJoints {
    pub settings: BTreeMap<FaceSlot, JointSetting>,
}

impl HousingJoints {
    pub fn get(&self, face: Face, slot: usize) -> Option<&JointSetting> {
        self.settings.get(&(face, slot))
    }

    /// The setting of `face` and `slot`, or the block's `default` mode when there is none.
    pub fn resolved(&self, face: Face, slot: usize, default: Mode) -> JointSetting {
        self.get(face, slot)
            .cloned()
            .unwrap_or_else(|| JointSetting::of_mode(default))
    }

    /// The housing with `setting` on `face` and `slot`.
    pub fn with(&self, face: Face, slot: usize, setting: JointSetting) -> HousingJoints {
        let mut next = self.clone();
        next.settings.insert((face, slot), setting);
        next
    }

    /// The housing without any setting of `slot`: its channel was taken out.
    pub fn without_slot(&self, slot: usize) -> HousingJoints {
        let mut next = self.clone();
        next.settings.retain(|(_, s), _| *s != slot);
        next
    }

    /// The four slots' settings of one face, for «Скопировать».
    pub fn face(&self, face: Face) -> [Option<JointSetting>; SLOTS] {
        std::array::from_fn(|slot| self.get(face, slot).cloned())
    }

    /// The housing with `copied` on `face` («Вставить»): a slot copied without settings
    /// goes back to the block's default.
    pub fn with_face(&self, face: Face, copied: &[Option<JointSetting>; SLOTS]) -> HousingJoints {
        let mut next = self.clone();
        for (slot, setting) in copied.iter().enumerate() {
            match setting {
                Some(s) => next.settings.insert((face, slot), s.clone()),
                None => next.settings.remove(&(face, slot)),
            };
        }
        next
    }

    /// The flat records the bundle saves, in a stable order.
    pub fn to_records(&self) -> Vec<Record> {
        self.settings
            .iter()
            .map(|(&(face, slot), s)| Record {
                face: face.index(),
                slot: slot as u8,
                mode: s.mode.code(),
                allow_only: s.filter.allow_only,
                priority: s.priority,
                items: s.filter.items.iter().cloned().collect(),
            })
            .collect()
    }

    /// What `to_records` wrote. A record with an unknown face, slot or mode is dropped, so a
    /// damaged save loses one joint's settings, not the housing's; numbers past the limits are
    /// brought back inside them.
    pub fn from_records(records: &[Record]) -> HousingJoints {
        let settings = records
            .iter()
            .filter_map(|r| {
                let face = Face::from_index(r.face)?;
                let slot = usize::from(r.slot);
                if slot >= SLOTS {
                    return None;
                }
                let mode = Mode::from_code(r.mode)?;
                let filter = Filter {
                    allow_only: r.allow_only,
                    items: r.items.iter().take(FILTER_LIMIT).cloned().collect(),
                };
                let priority = r.priority.clamp(-PRIORITY_LIMIT, PRIORITY_LIMIT);
                Some((
                    (face, slot),
                    JointSetting {
                        mode,
                        filter,
                        priority,
                    },
                ))
            })
            .collect();
        HousingJoints { settings }
    }
}

/// One saved setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub face: u8,
    pub slot: u8,
    pub mode: u8,
    pub allow_only: bool,
    pub priority: i32,
    pub items: Vec<ItemKey>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn into() -> JointSetting {
        JointSetting::of_mode(Mode::Into)
    }

    #[test]
    fn the_direction_steps_both_ways() {
        let s = into().apply(&Edit::NextMode);
        assert_eq!(s.mode, Mode::From);
        assert_eq!(s.apply(&Edit::PreviousMode).mode, Mode::Into);
        assert_eq!(into().apply(&Edit::PreviousMode).mode, Mode::Off);
    }

    #[test]
    fn a_filter_takes_three_items_and_no_duplicates() {
        let mut s = into();
        for item in ["a", "b", "b", "c", "d"] {
            s = s.apply(&Edit::AddItem(item.into()));
        }
        assert_eq!(
            s.filter
                .items
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        let s = s.apply(&Edit::RemoveItem("b".into()));
        assert_eq!(s.filter.items.len(), 2);
        let s = s.apply(&Edit::ToggleList);
        assert!(s.filter.allow_only);
        assert!(s.filter.passes("a") && !s.filter.passes("z"));
    }

    #[test]
    fn the_priority_stays_inside_its_limits() {
        let mut s = into();
        for _ in 0..20 {
            s = s.apply(&Edit::Priority(1));
        }
        assert_eq!(s.priority, PRIORITY_LIMIT);
        for _ in 0..40 {
            s = s.apply(&Edit::Priority(-1));
        }
        assert_eq!(s.priority, -PRIORITY_LIMIT);
    }

    #[test]
    fn a_face_and_slot_without_settings_take_the_blocks_default() {
        let h = HousingJoints::default().with(Face::YPos, 1, into().apply(&Edit::Priority(3)));
        assert_eq!(h.resolved(Face::YPos, 1, Mode::Both).priority, 3);
        assert_eq!(h.resolved(Face::YPos, 1, Mode::Both).mode, Mode::Into);
        // Another slot of the same face and the same slot of another face are apart (M7).
        assert_eq!(
            h.resolved(Face::YPos, 0, Mode::Both),
            JointSetting::of_mode(Mode::Both)
        );
        assert_eq!(h.resolved(Face::XPos, 1, Mode::Into), into());
    }

    #[test]
    fn copy_and_paste_move_one_faces_four_slots() {
        let h = HousingJoints::default()
            .with(Face::XNeg, 0, into().apply(&Edit::ToggleList))
            .with(Face::XPos, 2, into());
        let copied = h.face(Face::XNeg);
        let pasted = h.with_face(Face::XPos, &copied);
        assert_eq!(pasted.get(Face::XPos, 0), h.get(Face::XNeg, 0));
        // Slot 2 had a setting there and none in the copy: back to the default.
        assert_eq!(pasted.get(Face::XPos, 2), None);
        assert_eq!(pasted.get(Face::XNeg, 0), h.get(Face::XNeg, 0));
    }

    #[test]
    fn taking_a_channel_out_forgets_its_slot_on_every_face() {
        let h = HousingJoints::default()
            .with(Face::XNeg, 1, into())
            .with(Face::YPos, 1, into())
            .with(Face::YPos, 0, into());
        let left = h.without_slot(1);
        assert_eq!(left.settings.len(), 1);
        assert!(left.get(Face::YPos, 0).is_some());
    }

    #[test]
    fn settings_survive_a_save_and_bad_records_are_dropped() {
        let h = HousingJoints::default()
            .with(
                Face::ZPos,
                3,
                into()
                    .apply(&Edit::AddItem("minecraft:raw_iron".into()))
                    .apply(&Edit::ToggleList)
                    .apply(&Edit::Priority(-2)),
            )
            .with(Face::YNeg, 0, JointSetting::of_mode(Mode::Off));
        let records = h.to_records();
        assert_eq!(HousingJoints::from_records(&records), h);
        let mut bad = records.clone();
        bad.push(Record {
            face: 9,
            ..records[0].clone()
        });
        bad.push(Record {
            slot: 4,
            ..records[0].clone()
        });
        bad.push(Record {
            mode: 0,
            ..records[0].clone()
        });
        bad.push(Record {
            face: Face::XNeg.index(),
            priority: 1000,
            ..records[0].clone()
        });
        let read = HousingJoints::from_records(&bad);
        assert_eq!(read.settings.len(), 3);
        let slot = usize::from(records[0].slot);
        assert_eq!(
            read.get(Face::XNeg, slot).map(|s| s.priority),
            Some(PRIORITY_LIMIT)
        );
    }
}
