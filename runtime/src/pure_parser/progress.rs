//! Bound work without confusing long input or finite stack unwinding with a stall.

use crate::errors::ParserWorkLimit;

// Retain the old guard's threshold; apply it to work without admitted progress.
const MAX_ITERATIONS_WITHOUT_PROGRESS: usize = 10_000;

pub(super) struct ProgressBudget {
    input_frontier: usize,
    minimum_stack_depth: usize,
    completed_iterations: usize,
}

impl ProgressBudget {
    pub(super) fn new(stack_depth: usize) -> Self {
        Self {
            input_frontier: 0,
            minimum_stack_depth: stack_depth,
            completed_iterations: 0,
        }
    }

    /// Observe progress before charging the next LR loop body.
    ///
    /// The frontier only increases. Within one frontier phase, the stack
    /// minimum only decreases at that same frontier; rewinds and temporary
    /// growth/shrink cannot refill credit.
    /// A phase starting at depth d therefore admits at most limit * (d + 1)
    /// bodies. Finite source length bounds the number of frontier advances.
    pub(super) fn admit_iteration(
        &mut self,
        position: usize,
        stack_depth: usize,
    ) -> Result<(), ParserWorkLimit> {
        if position > self.input_frontier {
            self.input_frontier = position;
            self.minimum_stack_depth = stack_depth;
            self.completed_iterations = 0;
        } else if position == self.input_frontier && stack_depth < self.minimum_stack_depth {
            self.minimum_stack_depth = stack_depth;
            self.completed_iterations = 0;
        }

        if self.completed_iterations == MAX_ITERATIONS_WITHOUT_PROGRESS {
            return Err(ParserWorkLimit {
                limit: MAX_ITERATIONS_WITHOUT_PROGRESS,
                completed_iterations: self.completed_iterations,
            });
        }
        self.completed_iterations += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refuses_the_next_body_without_counting_it_as_completed() {
        let mut budget = ProgressBudget::new(2);
        for _ in 0..10_000 {
            assert_eq!(budget.admit_iteration(0, 2), Ok(()));
        }
        assert_eq!(
            budget.admit_iteration(0, 2),
            Err(ParserWorkLimit {
                limit: 10_000,
                completed_iterations: 10_000,
            })
        );
        // The last admitted body may have made progress. Observe that before
        // refusing a subsequent body, even when all old credit was consumed.
        assert_eq!(budget.admit_iteration(1, 3), Ok(()));
    }

    #[test]
    fn test_cursor_rewind_and_revisit_do_not_replenish_credit() {
        let mut budget = ProgressBudget::new(2);
        assert_eq!(budget.admit_iteration(2, 2), Ok(()));
        for step in 1..10_000 {
            let position = step % 3;
            let depth = if position < 2 { 1 } else { 2 };
            assert_eq!(budget.admit_iteration(position, depth), Ok(()));
        }
        assert!(budget.admit_iteration(2, 2).is_err());
        assert!(budget.admit_iteration(1, 0).is_err());
        assert_eq!(budget.admit_iteration(2, 1), Ok(()));
        assert_eq!(budget.admit_iteration(3, 2), Ok(()));
    }

    #[test]
    fn test_temporary_stack_growth_and_shrink_do_not_replenish_credit() {
        let mut budget = ProgressBudget::new(2);
        for step in 0..10_000 {
            assert_eq!(budget.admit_iteration(0, 2 + step % 2), Ok(()));
        }
        assert!(budget.admit_iteration(0, 2).is_err());
        assert_eq!(budget.admit_iteration(0, 1), Ok(()));
    }

    #[test]
    fn test_long_eof_unwinding_can_make_finite_stack_progress() {
        let mut budget = ProgressBudget::new(20_002);
        for depth in (1..=20_002).rev() {
            assert_eq!(budget.admit_iteration(0, depth), Ok(()));
        }
    }
}
