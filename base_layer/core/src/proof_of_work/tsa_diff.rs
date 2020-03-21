// LWMA-1 for BTC & Zcash clones
// Copyright (c) 2017-2019 The Bitcoin Gold developers, Zawy, iamstenman (Microbitcoin)
// MIT License
// Algorithm by Zawy, a modification of WT-144 by Tom Harding
// References:
// https://github.com/zawy12/difficulty-algorithms/issues/3#issuecomment-442129791
// https://github.com/zcash/zcash/issues/4021

use crate::proof_of_work::{
    difficulty::{Difficulty, DifficultyAdjustment},
    error::DifficultyAdjustmentError,
    lwma_diff::LinearWeightedMovingAverage,
};
use log::*;
use std::{cmp, collections::VecDeque};
use tari_crypto::tari_utilities::epoch_time::EpochTime;

pub const LOG_TARGET: &str = "c::pow::lwma_diff";

pub struct TimeStampAdjustment {
    lwma_diff: LinearWeightedMovingAverage,
}

impl TimeStampAdjustment {
    pub fn new(block_window: usize, target_time: u64, initial_difficulty: u64) -> TimeStampAdjustment {
        let lwma_diff = LinearWeightedMovingAverage {
            timestamps: VecDeque::with_capacity(block_window + 1),
            accumulated_difficulties: VecDeque::with_capacity(block_window + 1),
            block_window,
            target_time,
            initial_difficulty,
        };
        TimeStampAdjustment { lwma_diff }
    }

    fn calculate(&self) -> Difficulty {
        let timestamps = &self.lwma_diff.timestamps;
        if timestamps.len() <= 2 {
            // return INITIAL_DIFFICULTY;
            return self.lwma_diff.initial_difficulty.into();
        }

        let mut lwma_diff = self.lwma_diff.get_difficulty().as_u64() as f64;

        // R is the "softness" of the per-block TSA adjustment to the DA. R<6 is aggressive.
        let R = 2;
        // "m" is a factor to help get e^x from integer math. 1E5 was not as precise
        let m = 1E6;
        let mut exm = m as f64; // This will become m*e^x. Initial value is m*e^(mod(<1)) = m.

        let n = timestamps.len() as u64 - 1;
        let prev_timestamp = timestamps[n as usize - 1];
        let this_timestamp = if timestamps[n as usize] > prev_timestamp {
            timestamps[n as usize]
        } else {
            prev_timestamp.increase(1)
        };
        let mut solve_time = cmp::min(
            (this_timestamp - prev_timestamp).as_u64(),
            6 * self.lwma_diff.target_time,
        );

        // #########  Begin Unwanted Modification to TSA logic
        //----------Xbuffer------------------------------
        let mut asc = (timestamps[n as usize] - timestamps[0]).as_u64(); // accumulated solve time
        if (asc / n + 1 <= self.lwma_diff.target_time / R) {
            asc = (asc / (n + 1) / self.lwma_diff.target_time) * asc;
        };
        solve_time = (solve_time * ((asc / (n + 1) * 1000) / self.lwma_diff.target_time)) / 1000;
        if (solve_time < 0) {
            solve_time = 0;
        }
        if ((prev_timestamp - timestamps[n as usize - 1]) <= (self.lwma_diff.target_time / R).into() &&
            solve_time < (self.lwma_diff.target_time - (self.lwma_diff.target_time / 5)))
        {
            lwma_diff = lwma_diff * (1.0 / 5.0);
        } else if (solve_time <= self.lwma_diff.target_time / 5) {
            lwma_diff = lwma_diff * (1.0 / 5.0);
        }
        // ########### Begin Actual TSA   ##########
        else {
            // It would be good to turn the for statement into a look-up table;
            let mut i = 1;
            while (i <= solve_time / self.lwma_diff.target_time / R) {
                exm = (exm * (2.71828 * m)) / m;
                i += 1;
            }
            let f = (solve_time % (self.lwma_diff.target_time * R)) as f64;
            exm = (exm *
                (m + (f *
                    (m + (f *
                        (m + (f * (m + (f * m) / (4 * self.lwma_diff.target_time * R) as f64)) /
                            (3 * self.lwma_diff.target_time * R) as f64)) /
                        (2 * self.lwma_diff.target_time * R) as f64)) /
                    (self.lwma_diff.target_time * R) as f64)) /
                m;
            // 1000 below is to prevent overflow on testnet
            lwma_diff = (lwma_diff *
                ((1000.0 *
                    (m * self.lwma_diff.target_time as f64 +
                        (solve_time - self.lwma_diff.target_time) as f64 * exm)) /
                    (m * solve_time as f64))) /
                1000.0;
        }
        // if (lwma_diff > powLimit) {
        //     lwma_diff = powLimit;
        // }
        let target = lwma_diff.ceil() as u64;
        target.into()
    }
}

impl DifficultyAdjustment for TimeStampAdjustment {
    fn add(
        &mut self,
        timestamp: EpochTime,
        accumulated_difficulty: Difficulty,
    ) -> Result<(), DifficultyAdjustmentError>
    {
        trace!(
            target: LOG_TARGET,
            "Adding new timestamp and difficulty requested: {:?}, {:?}",
            timestamp,
            accumulated_difficulty
        );
        self.lwma_diff.add(timestamp, accumulated_difficulty)
    }

    fn get_difficulty(&self) -> Difficulty {
        self.calculate()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tsa_zero_len() {
        let dif = TimeStampAdjustment::new(90, 120, 1);
        assert_eq!(dif.get_difficulty(), Difficulty::min());
    }

    #[test]
    fn tsa_add_non_increasing_diff() {
        let mut dif = TimeStampAdjustment::new(90, 120, 1);
        assert!(dif.add(100.into(), 100.into()).is_ok());
        assert!(dif.add(100.into(), 100.into()).is_err());
        assert!(dif.add(100.into(), 50.into()).is_err());
    }

    #[test]
    fn tsa_negative_solve_times() {
        let mut dif = TimeStampAdjustment::new(90, 120, 1);
        let mut timestamp = 60.into();
        let mut cum_diff = Difficulty::from(100);
        let _ = dif.add(timestamp, cum_diff);
        timestamp = timestamp.increase(60);
        cum_diff += Difficulty::from(100);
        let _ = dif.add(timestamp, cum_diff);
        // Lets create a history and populate the vecs
        for _i in 0..150 {
            cum_diff += Difficulty::from(100);
            timestamp = timestamp.increase(60);
            let _ = dif.add(timestamp, cum_diff);
        }
        // lets create chaos by having 60 blocks as negative solve times. This should never be allowed in practice by
        // having checks on the block times.
        for _i in 0..60 {
            cum_diff += Difficulty::from(100);
            timestamp = (timestamp.as_u64() - 1).into(); // Only choosing -1 here since we are testing negative solve times and we cannot have 0 time
            let diff_before = dif.get_difficulty();
            let _ = dif.add(timestamp, cum_diff);
            let diff_after = dif.get_difficulty();
            // Algo should handle this as 1sec solve time thus increase the difficulty constantly
            assert!(diff_after > diff_before);
        }
    }

    #[test]
    fn tsa_limit_difficulty_change() {
        let mut dif = LinearWeightedMovingAverage::new(5, 60, 1);
        let _ = dif.add(60.into(), 100.into());
        let _ = dif.add(10_000_000.into(), 200.into());
        assert_eq!(dif.get_difficulty(), 17.into());
        let _ = dif.add(20_000_000.into(), 216.into());
        assert_eq!(dif.get_difficulty(), 10.into());
    }

    #[test]
    // Data for 5-period moving average
    // Timestamp: 60, 120, 180, 240, 300, 350, 380, 445, 515, 615, 975, 976, 977, 978, 979
    // Intervals: 60,  60,  60,  60,  60,  50,  30,  65,  70, 100, 360,   1,   1,   1,   1
    // Diff:     100, 100, 100, 100, 100, 105, 128, 123, 116,  94,  39,  46,  55,  75, 148
    // Acum dif: 100, 200, 300, 400, 500, 605, 733, 856, 972,1066,1105,1151,1206,1281,1429
    // Target:     1, 100, 100, 100, 100, 107, 136, 130, 120,  94,  36,  39,  47,  67, 175
    fn tsa_calculate() {
        let mut dif = LinearWeightedMovingAverage::new(5, 60, 1);
        let _ = dif.add(60.into(), 100.into());
        assert_eq!(dif.get_difficulty(), 1.into());
        let _ = dif.add(120.into(), 200.into());
        assert_eq!(dif.get_difficulty(), 100.into());
        let _ = dif.add(180.into(), 300.into());
        assert_eq!(dif.get_difficulty(), 100.into());
        let _ = dif.add(240.into(), 400.into());
        assert_eq!(dif.get_difficulty(), 100.into());
        let _ = dif.add(300.into(), 500.into());
        assert_eq!(dif.get_difficulty(), 100.into());
        let _ = dif.add(350.into(), 605.into());
        assert_eq!(dif.get_difficulty(), 107.into());
        let _ = dif.add(380.into(), 733.into());
        assert_eq!(dif.get_difficulty(), 136.into());
        let _ = dif.add(445.into(), 856.into());
        assert_eq!(dif.get_difficulty(), 130.into());
        let _ = dif.add(515.into(), 972.into());
        assert_eq!(dif.get_difficulty(), 120.into());
        let _ = dif.add(615.into(), 1066.into());
        assert_eq!(dif.get_difficulty(), 94.into());
        let _ = dif.add(975.into(), 1105.into());
        assert_eq!(dif.get_difficulty(), 36.into());
        let _ = dif.add(976.into(), 1151.into());
        assert_eq!(dif.get_difficulty(), 39.into());
        let _ = dif.add(977.into(), 1206.into());
        assert_eq!(dif.get_difficulty(), 47.into());
        let _ = dif.add(978.into(), 1281.into());
        assert_eq!(dif.get_difficulty(), 67.into());
        let _ = dif.add(979.into(), 1429.into());
        assert_eq!(dif.get_difficulty(), 175.into());
    }
}
