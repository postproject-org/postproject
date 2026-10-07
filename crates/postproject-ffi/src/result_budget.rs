//! Shared payload bounds for enriched native result handles.

use postproject_core::{Error, ErrorKind, Result};

#[derive(Default)]
pub(crate) struct ResultBudget {
    items: usize,
    bytes: usize,
}

impl ResultBudget {
    pub(crate) fn record(&mut self, items: usize, bytes: usize) -> Result<()> {
        self.items = self.items.saturating_add(items);
        self.bytes = self.bytes.saturating_add(bytes);
        if self.items > 100_000 || self.bytes > 64 * 1024 * 1024 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "native result exceeds its materialization budget; request a smaller page",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_budget_rejects_enrichment_and_arithmetic_overflow() {
        let mut budget = ResultBudget::default();
        for _ in 0..7 {
            budget.record(1, 9 * 1024 * 1024).expect("bounded payload");
        }
        assert_eq!(
            budget.record(1, 9 * 1024 * 1024).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
        let mut budget = ResultBudget::default();
        budget.record(100_000, 0).expect("item boundary");
        assert_eq!(
            budget.record(usize::MAX, 0).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
        let mut budget = ResultBudget::default();
        budget.record(1, 1).expect("first byte");
        assert_eq!(
            budget.record(0, usize::MAX).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
    }
}
