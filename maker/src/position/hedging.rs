use std::cmp::Ordering;
use std::num::NonZeroU32;
use std::ops::Add;

#[derive(Debug, PartialEq)]
pub enum Action {
    StandPat,
    Buy { hundreds_of_contracts: NonZeroU32 },
    Sell { hundreds_of_contracts: NonZeroU32 },
}

pub fn derive_hedging_action(tentenone: i32, bitmex: i32) -> Action {
    fn derive_hedging_action_rec(tentenone: i32, bitmex: i32, action_acc: Action) -> Action {
        let diff = tentenone - bitmex;
        let diff_hundreds = diff / 100;

        if 1 <= diff_hundreds {
            derive_hedging_action_rec(
                tentenone,
                bitmex + 100,
                action_acc + Action::buy_one_hundred(),
            )
        } else if diff_hundreds <= -1 {
            derive_hedging_action_rec(
                tentenone,
                bitmex - 100,
                action_acc + Action::sell_one_hundred(),
            )
        } else {
            action_acc
        }
    }

    derive_hedging_action_rec(tentenone, bitmex, Action::StandPat)
}

impl Action {
    pub fn contracts(&self) -> i32 {
        self.to_int() * 100
    }

    fn buy_hundreds(n: u32) -> Self {
        Self::new(n as i32)
    }

    fn sell_hundreds(n: u32) -> Self {
        Self::new(-(n as i32))
    }

    fn new(n: i32) -> Self {
        let hundreds_of_contracts = NonZeroU32::new(n.unsigned_abs()).expect("not zero");
        match n.cmp(&0) {
            Ordering::Greater => Self::Buy {
                hundreds_of_contracts,
            },
            Ordering::Less => Self::Sell {
                hundreds_of_contracts,
            },
            Ordering::Equal => Self::StandPat,
        }
    }

    fn to_int(&self) -> i32 {
        match self {
            Action::StandPat => 0,
            Action::Buy {
                hundreds_of_contracts,
            } => hundreds_of_contracts.get() as i32,
            Action::Sell {
                hundreds_of_contracts,
            } => -(hundreds_of_contracts.get() as i32),
        }
    }

    fn buy_one_hundred() -> Action {
        Action::buy_hundreds(1)
    }

    fn sell_one_hundred() -> Action {
        Action::sell_hundreds(1)
    }
}

impl Add for Action {
    type Output = Action;

    fn add(self, rhs: Self) -> Self::Output {
        let lhs = self.to_int();
        let rhs = rhs.to_int();

        Action::new(lhs + rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_hedging_action_singleton() {
        check(0, 0, Action::StandPat);

        check(99, 0, Action::StandPat);
        check(100, 0, Action::buy_one_hundred());
        check(101, 0, Action::buy_one_hundred());

        check(-99, 0, Action::StandPat);
        check(-100, 0, Action::sell_one_hundred());
        check(-101, 0, Action::sell_one_hundred());

        check(0, -99, Action::StandPat);
        check(0, -100, Action::buy_one_hundred());
        check(0, -101, Action::buy_one_hundred());

        check(0, 99, Action::StandPat);
        check(0, 100, Action::sell_one_hundred());
        check(0, 101, Action::sell_one_hundred());

        check(550, 300, Action::buy_hundreds(2));
        check(-330, 200, Action::sell_hundreds(5));
    }

    // TODO(lucas): Verify that executing a single `Action` and then calling
    // `derive_hedging_action_singleton` again always leads to `Action::StandPat`.

    #[track_caller]
    fn check(tentenone: i32, bitmex: i32, expected: Action) {
        let actual = derive_hedging_action(tentenone, bitmex);
        assert_eq!(expected, actual);
    }
}
