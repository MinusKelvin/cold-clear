use libtetris::{ Board, LockResult, PlacementKind };
use serde::{ Serialize, Deserialize, ser::SerializeTuple, de::SeqAccess, de::Visitor };
use super::*;

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct PatternEvaluator {
    pub back_to_back: i32,
    pub thresholds: Array4096,
    pub below_values: Array4096,
    pub above_values: Array4096,
    pub rows: Array1024,

    pub b2b_clear: i32,
    pub clear1: i32,
    pub clear2: i32,
    pub clear3: i32,
    pub clear4: i32,
    pub tspin1: i32,
    pub tspin2: i32,
    pub tspin3: i32,
    pub mini_tspin1: i32,
    pub mini_tspin2: i32,
    pub perfect_clear: i32,
    pub combo_table: [i32; 12],
    pub soft_drop: i32
}

impl Evaluator for PatternEvaluator {
    fn info(&self) -> Info {
        vec![
            ("Pattern".to_owned(), None)
        ]
    }

    fn evaluate(
        &mut self,
        lock: &LockResult,
        board: &Board,
        soft_dropped: bool
    ) -> Evaluation {
        let mut transient_eval = 0;
        let mut acc_eval = 0;

        if lock.perfect_clear {
            acc_eval += self.perfect_clear;
        } else {
            if lock.b2b {
                acc_eval += self.b2b_clear;
            }
            if let Some(combo) = lock.combo {
                let combo = combo.min(11) as usize;
                acc_eval += self.combo_table[combo];
            }
            match lock.placement_kind {
                PlacementKind::Clear1 => {
                    acc_eval += self.clear1;
                }
                PlacementKind::Clear2 => {
                    acc_eval += self.clear2;
                }
                PlacementKind::Clear3 => {
                    acc_eval += self.clear3;
                }
                PlacementKind::Clear4 => {
                    acc_eval += self.clear4;
                }
                PlacementKind::Tspin1 => {
                    acc_eval += self.tspin1;
                }
                PlacementKind::Tspin2 => {
                    acc_eval += self.tspin2;
                }
                PlacementKind::Tspin3 => {
                    acc_eval += self.tspin3;
                }
                PlacementKind::MiniTspin1 => {
                    acc_eval += self.mini_tspin1;
                }
                PlacementKind::MiniTspin2 => {
                    acc_eval += self.mini_tspin2;
                }
                _ => {}
            }
        }

        if soft_dropped {
            acc_eval += self.soft_drop;
        }

        if board.b2b_bonus {
            transient_eval += self.back_to_back;
        }

        let mut patterns = [0; 4096];

        for y in -1..20 {
            for x in -1..11 {
                let i = board.occupied(x-1, y-1) as usize;
                let i = i << 1 | board.occupied(x,   y-1) as usize;
                let i = i << 1 | board.occupied(x+1, y-1) as usize;
                let i = i << 1 | board.occupied(x-1, y)   as usize;
                let i = i << 1 | board.occupied(x,   y)   as usize;
                let i = i << 1 | board.occupied(x+1, y)   as usize;
                let i = i << 1 | board.occupied(x-1, y+1) as usize;
                let i = i << 1 | board.occupied(x,   y+1) as usize;
                let i = i << 1 | board.occupied(x+1, y+1) as usize;
                let mut row_1_filled = true;
                let mut row_2_filled = true;
                let mut row_3_filled = true;
                for rx in 0..10 {
                    if rx < x-1 || rx > x+1 {
                        row_1_filled &= board.occupied(rx, y-1);
                        row_2_filled &= board.occupied(rx, y);
                        row_3_filled &= board.occupied(rx, y+1);
                    }
                }
                let i = i << 1 | row_1_filled as usize;
                let i = i << 1 | row_2_filled as usize;
                let i = i << 1 | row_3_filled as usize;
                if patterns[i] < self.thresholds[i] {
                    transient_eval += self.below_values[i];
                } else {
                    transient_eval += self.above_values[i];
                }
                patterns[i] += 1;
            }
        }

        for y in 0..20 {
            transient_eval += self.rows[*board.get_row(y) as usize];
        }

        Evaluation {
            accumulated: acc_eval,
            transient: transient_eval
        }
    }
}

#[derive(Copy, Clone)]
pub struct Array4096([i32; 4096]);

impl std::ops::Deref for Array4096 {
    type Target = [i32; 4096];
    fn deref(&self) -> &[i32; 4096] {
        &self.0
    }
}

impl std::ops::DerefMut for Array4096 {
    fn deref_mut(&mut self) -> &mut [i32; 4096] {
        &mut self.0
    }
}

impl From<[i32; 4096]> for Array4096 {
    fn from(v: [i32; 4096]) -> Self {
        Array4096(v)
    }
}

impl Serialize for Array4096 {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut seq = ser.serialize_tuple(4096)?;
        for v in self.0.iter() {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Array4096 {
    fn deserialize<D: serde::Deserializer<'de>>(deser: D) -> Result<Self, D::Error> {
        struct Array4096Deserializer;
        impl<'de> Visitor<'de> for Array4096Deserializer {
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "an array of length 4096")
            }
            type Value = Array4096;
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Array4096, A::Error> {
                let mut v = [0; 4096];
                for i in 0..4096 {
                    if let Some(n) = seq.next_element()? {
                        v[i] = n;
                    } else {
                        return Err(serde::de::Error::invalid_length(i, &self))
                    }
                }
                Ok(Array4096(v))
            }
        }
        deser.deserialize_tuple(4096, Array4096Deserializer)
    }
}

#[derive(Copy, Clone)]
pub struct Array1024([i32; 1024]);

impl std::ops::Deref for Array1024 {
    type Target = [i32; 1024];
    fn deref(&self) -> &[i32; 1024] {
        &self.0
    }
}

impl std::ops::DerefMut for Array1024 {
    fn deref_mut(&mut self) -> &mut [i32; 1024] {
        &mut self.0
    }
}

impl From<[i32; 1024]> for Array1024 {
    fn from(v: [i32; 1024]) -> Self {
        Array1024(v)
    }
}

impl Serialize for Array1024 {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut seq = ser.serialize_tuple(1024)?;
        for v in self.0.iter() {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Array1024 {
    fn deserialize<D: serde::Deserializer<'de>>(deser: D) -> Result<Self, D::Error> {
        struct Array1024Deserializer;
        impl<'de> Visitor<'de> for Array1024Deserializer {
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "an array of length 1024")
            }
            type Value = Array1024;
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Array1024, A::Error> {
                let mut v = [0; 1024];
                for i in 0..1024 {
                    if let Some(n) = seq.next_element()? {
                        v[i] = n;
                    } else {
                        return Err(serde::de::Error::invalid_length(i, &self))
                    }
                }
                Ok(Array1024(v))
            }
        }
        deser.deserialize_tuple(1024, Array1024Deserializer)
    }
}