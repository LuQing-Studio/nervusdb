use super::{GraphSnapshot, PlanIterator, Result, Row};

pub(super) fn wrap_plan_iterator<'a, S: GraphSnapshot + 'a>(
    iter: PlanIterator<'a, S>,
    params: &'a crate::query_api::Params,
    stage: &'static str,
) -> PlanIterator<'a, S> {
    PlanIterator::Dynamic(Box::new(RuntimeGuardIter {
        inner: Box::new(iter),
        params,
        stage,
    }))
}

struct RuntimeGuardIter<'a> {
    inner: Box<dyn Iterator<Item = Result<Row>> + 'a>,
    params: &'a crate::query_api::Params,
    stage: &'static str,
}

impl<'a> Iterator for RuntimeGuardIter<'a> {
    type Item = Result<Row>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(err) = self.params.check_timeout(self.stage) {
            return Some(Err(err));
        }

        match self.inner.next() {
            Some(Ok(row)) => {
                if let Err(err) = self.params.note_emitted_row(self.stage) {
                    return Some(Err(err));
                }
                Some(Ok(row))
            }
            Some(Err(err)) => Some(Err(err)),
            None => None,
        }
    }
}
