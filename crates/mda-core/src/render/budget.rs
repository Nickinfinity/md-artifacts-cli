//! Output, step and iteration budgets plus warning dedupe (T3.1).

use std::collections::HashSet;

use super::{MAX_ITERATIONS, MAX_OUTPUT_BYTES, MAX_STEPS, Warning};
use crate::error::{LimitExceeded, RenderLimit};

/// The output buffer and the counters that bound the work done to fill it.
#[derive(Default)]
pub(super) struct Budget {
    pub out: String,
    steps: usize,
    iterations: usize,
}

impl Budget {
    /// Append `s`, refusing before the buffer would pass [`MAX_OUTPUT_BYTES`].
    pub fn push(&mut self, s: &str) -> Result<(), LimitExceeded> {
        if self.out.len().saturating_add(s.len()) > MAX_OUTPUT_BYTES {
            return Err(over(RenderLimit::OutputBytes, MAX_OUTPUT_BYTES));
        }
        self.out.push_str(s);
        Ok(())
    }

    /// Count one visited node (token, text gap, or value node).
    pub fn step(&mut self) -> Result<(), LimitExceeded> {
        if self.steps >= MAX_STEPS {
            return Err(over(RenderLimit::Steps, MAX_STEPS));
        }
        self.steps += 1;
        Ok(())
    }

    /// Count one loop iteration, before it runs.
    pub fn iteration(&mut self) -> Result<(), LimitExceeded> {
        if self.iterations >= MAX_ITERATIONS {
            return Err(over(RenderLimit::Iterations, MAX_ITERATIONS));
        }
        self.iterations += 1;
        Ok(())
    }
}

fn over(limit: RenderLimit, max: usize) -> LimitExceeded {
    LimitExceeded { limit, max }
}

/// Warnings deduped by `(code, name|path)`, first line kept.
#[derive(Default)]
pub(super) struct Warnings {
    seen: HashSet<(&'static str, String)>,
    list: Vec<Warning>,
}

impl Warnings {
    pub fn add(&mut self, w: Warning) {
        let key = w
            .params
            .get("name")
            .or_else(|| w.params.get("path"))
            .cloned()
            .unwrap_or_default();
        if self.seen.insert((w.code, key)) {
            self.list.push(w);
        }
    }

    /// Sorted by `(line as a number, code, name|path)`.
    pub fn into_sorted(mut self) -> Vec<Warning> {
        let key = |w: &Warning| {
            let line = w
                .params
                .get("line")
                .and_then(|l| l.parse().ok())
                .unwrap_or(0usize);
            let id = w.params.get("name").or_else(|| w.params.get("path"));
            (line, w.code, id.cloned().unwrap_or_default())
        };
        self.list.sort_by_cached_key(key);
        self.list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_stops_after_max() {
        let mut b = Budget::default();
        for _ in 0..MAX_STEPS {
            assert!(b.step().is_ok());
        }
        assert_eq!(b.step().unwrap_err(), over(RenderLimit::Steps, MAX_STEPS));
    }
}
