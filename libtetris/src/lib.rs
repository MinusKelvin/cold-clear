mod board;
mod lock_data;
mod moves;
mod piece;

#[cfg(feature = "fumen")]
mod fumen_conv;

#[cfg(feature = "pcf")]
mod pcf_conv;

pub use board::*;
pub use lock_data::*;
pub use moves::*;
pub use piece::*;

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
pub struct Controller {
    pub left: bool,
    pub right: bool,
    pub rotate_right: bool,
    pub rotate_left: bool,
    pub soft_drop: bool,
    pub hard_drop: bool,
    pub hold: bool,
}

impl serde::Serialize for Controller {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(
            (self.left as u8) << 1
                | (self.right as u8) << 2
                | (self.rotate_left as u8) << 3
                | (self.rotate_right as u8) << 4
                | (self.hold as u8) << 5
                | (self.soft_drop as u8) << 6
                | (self.hard_drop as u8) << 7,
        )
    }
}

impl<'de> serde::Deserialize<'de> for Controller {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ControllerDeserializer;
        impl serde::de::Visitor<'_> for ControllerDeserializer {
            type Value = Controller;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a byte-sized bit vector")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Controller, E> {
                Ok(Controller {
                    left: (v >> 1) & 1 != 0,
                    right: (v >> 2) & 1 != 0,
                    rotate_left: (v >> 3) & 1 != 0,
                    rotate_right: (v >> 4) & 1 != 0,
                    hold: (v >> 5) & 1 != 0,
                    soft_drop: (v >> 6) & 1 != 0,
                    hard_drop: (v >> 7) & 1 != 0,
                })
            }
        }
        deserializer.deserialize_u8(ControllerDeserializer)
    }
}
