use gtk::prelude::*;
use crate::gui::freezeable::Freezeable;

pub struct DecIntervalsWidget {
    /// Contains as many elements as `combo`.
    intervals: Vec<(f64, f64)>,
    /// Contains as many items as `intervals`.
    combo: Freezeable<gtk::ComboBoxText>
}

impl DecIntervalsWidget {
    pub fn new(min_val: f64, max_val: f64, current: f64, num_decimals: usize) -> DecIntervalsWidget {
        assert!(max_val >= min_val);

        let mut intervals: Vec<(f64, f64)> = vec![];
        let combo = gtk::ComboBoxText::new();

        let min_exp = find_smallest_higher_dec_power(min_val);
        let max_exp = find_highest_lower_dec_power(max_val);

        let min_dec = 10f64.powi(min_exp);
        let max_dec = 10f64.powi(max_exp);

        if min_val == max_val {
            intervals.push((min_dec, max_dec));
            combo.append_text(&format!("{}–{}", dec_pow_as_string(min_exp - 1), dec_pow_as_string(max_exp + 1)));
        } else if min_exp == max_exp + 1 || min_exp == max_exp { // there is 0 or 1 decimal power between `min_val` and `max_val`
            intervals.push((min_val, max_val));
            combo.append_text(&format!("{:.*}–{:.*}", num_decimals, min_val, num_decimals, max_val));
        } else { // there are at least 2 decimal powers between `min_val` and `max_val`
            // add first interval
            if !values_are_closer_than(min_val, min_dec, 0.1) {
                intervals.push((min_val, 10f64.powi(min_exp)));
                combo.append_text(&format!("{:.*}–{}", num_decimals, min_val, dec_pow_as_string(min_exp)));
            }

            // add intermediate intervals
            let mut exponent = min_exp;
            while exponent + 1 <= max_exp {
                intervals.push((10f64.powi(exponent), 10f64.powi(exponent + 1)));
                combo.append_text(&format!("{}–{}", dec_pow_as_string(exponent), dec_pow_as_string(exponent + 1)));
                exponent += 1;
            }

            // add last interval
            if !values_are_closer_than(max_val, max_dec, 0.1) {
                intervals.push((10f64.powi(max_exp), max_val));
                combo.append_text(&format!("{}-{:.*}", dec_pow_as_string(max_exp), num_decimals, max_val));
            }
        }

        let dec_intervals_widget = DecIntervalsWidget{
            intervals,
            combo: Freezeable::new(combo, None)
        };

        dec_intervals_widget.set_value(current);

        dec_intervals_widget
    }

    pub fn set_value(&self, value: f64) {
        let prev_idx = self.combo().active();

        for (idx, interval) in self.intervals.iter().enumerate() {
            if idx > 0 && value < interval.0 || idx < self.intervals.len() - 1 &&  value > interval.1 {
                continue;
            }

            let dows = |wrapper: &Freezeable<gtk::ComboBoxText>, idx: u32| {
                wrapper.freeze();
                wrapper.set_active(Some(idx));
                wrapper.thaw();
            };

            match prev_idx {
                None => dows(&self.combo, idx as u32),
                Some(prev_idx) => {
                    if value > interval.0 && value < interval.1 {
                        dows(&self.combo, idx as u32);
                    } else if value == interval.0 {
                        if idx == 0 {
                            dows(&self.combo, idx as u32);
                        } else {
                            if prev_idx != idx as u32 - 1 {
                                dows(&self.combo, idx as u32);
                            }
                        }
                    } else if value == interval.1 {
                        if idx == self.intervals.len() - 1 {
                            dows(&self.combo, idx as u32);
                        } else {
                            if prev_idx != idx as u32 + 1 {
                                dows(&self.combo, idx as u32);
                            }
                        }
                    }
                }
            }

            break;
        }
    }

    pub fn combo(&self) -> &gtk::ComboBoxText {
        &self.combo
    }

    pub fn set_signal(&mut self, signal: glib::SignalHandlerId) {
        self.combo.set_signal(signal);
    }

    pub fn interval(&self) -> (f64, f64) {
        let idx = self.combo.active().unwrap();
        self.intervals[idx as usize]
    }
}

fn values_are_closer_than(v1: f64, v2: f64, max_relative_difference: f64) -> bool {
    assert!(v1 > 0.0 && v2 > 0.0);

    let ratio = v1 / v2;
    (1.0 - ratio).abs() <= max_relative_difference
}

/// Formats 10^exponent as string.
fn dec_pow_as_string(exponent: i32) -> String {
    let mut result = "1".to_string();

    if exponent > 0 {
        result += &"0".repeat(exponent as usize);
    } else if exponent < 0 {
        result = "0.".to_string() + &"0".repeat((-exponent - 1) as usize) + &result;
    }

    result
}

fn find_smallest_higher_dec_power(val: f64) -> i32 {
    assert!(val > 0.0);

    let mut exp = 0;
    if 10f64.powi(exp) > val {
        while 10f64.powi(exp - 1) > val {
            exp -= 1;
        }
    } else {
        while 10f64.powi(exp) <= val {
            exp += 1;
        }
    }

    exp
}

fn find_highest_lower_dec_power(val: f64) -> i32 {
    assert!(val > 0.0);

    let mut exp = 0;
    if 10f64.powi(exp) < val {
        while 10f64.powi(exp + 1) < val {
            exp += 1;
        }
    } else {
        while 10f64.powi(exp) >= val {
            exp -= 1;
        }
    }

    exp
}

mod tests {
    use super::*;

    #[test]
    fn exponents() {
        assert_eq!(-1, find_highest_lower_dec_power(1.0));
        assert_eq!(1, find_smallest_higher_dec_power(1.0));

        assert_eq!(0, find_highest_lower_dec_power(10.0));
        assert_eq!(2, find_smallest_higher_dec_power(10.0));

        assert_eq!(-2, find_highest_lower_dec_power(0.1));
        assert_eq!(0, find_smallest_higher_dec_power(0.1));

        assert_eq!(-2, find_highest_lower_dec_power(0.02));
        assert_eq!(1, find_highest_lower_dec_power(20.0));

        assert_eq!(-1, find_smallest_higher_dec_power(0.02));
        assert_eq!(2, find_smallest_higher_dec_power(20.0));
    }

    #[test]
    fn format_dec_power() {
        assert_eq!("1", dec_pow_as_string(0));
        assert_eq!("0.01", dec_pow_as_string(-2));
        assert_eq!("100", dec_pow_as_string(2));
    }
}
