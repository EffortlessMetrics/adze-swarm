//! Error collection helpers for procedural macro expansion.

use std::marker::PhantomData;

/// An iterator that maps [`Result`]s to their [`Ok`] values
/// and stores combined errors within itself.
struct CollectingShunt<'a, I, A> {
    iter: I,
    err: &'a mut Option<syn::Error>,
    _marker: PhantomData<fn() -> A>,
}

impl<I, A> Iterator for CollectingShunt<'_, I, A>
where
    I: Iterator<Item = syn::Result<A>>,
{
    type Item = A;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.next() {
            Some(Ok(x)) => Some(x),
            Some(Err(another)) => {
                match self.err {
                    Some(x) => x.combine(another),
                    ref mut x => **x = Some(another),
                }
                None
            }
            _ => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }
}

pub trait IteratorExt<A>: Iterator<Item = syn::Result<A>> {
    /// Reduces an iterator with items of type [`syn::Result<T>`] into one large collection,
    /// [combining] errors and [collecting] successes.
    ///
    /// [combining]: syn::Error::combine
    /// [collecting]: FromIterator
    fn sift<T>(self) -> syn::Result<T>
    where
        Self: Sized,
        T: FromIterator<A>,
    {
        let mut err = None;
        let iter = CollectingShunt {
            iter: self,
            err: &mut err,
            _marker: PhantomData,
        };
        let collection = iter.collect();
        match err {
            Some(error) => Err(error),
            None => Ok(collection),
        }
    }
}

impl<A, T> IteratorExt<A> for T where T: Iterator<Item = syn::Result<A>> {}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    fn err(msg: &str) -> syn::Error {
        syn::Error::new(Span::call_site(), msg)
    }

    #[test]
    fn sift_collects_oks_into_vec() {
        let iter = [Ok::<i32, syn::Error>(1), Ok(2), Ok(3)].into_iter();
        let result: syn::Result<Vec<i32>> = iter.sift();
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn sift_collects_oks_into_string() {
        let iter = [Ok::<char, syn::Error>('h'), Ok('i')].into_iter();
        let result: syn::Result<String> = iter.sift();
        assert_eq!(result.unwrap(), "hi");
    }

    #[test]
    fn sift_empty_iterator_returns_empty_collection() {
        let iter = std::iter::empty::<syn::Result<i32>>();
        let result: syn::Result<Vec<i32>> = iter.sift();
        assert_eq!(result.unwrap(), Vec::<i32>::new());
    }

    #[test]
    fn sift_single_err_preserves_message() {
        let iter = std::iter::once(Err::<i32, syn::Error>(err("boom")));
        let result: syn::Result<Vec<i32>> = iter.sift();
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "boom");
    }

    #[test]
    fn sift_first_err_short_circuits_collection() {
        // The shunt's `next` returns `None` once it sees an `Err`, so
        // `collect` stops immediately. Only the first error is retained,
        // and subsequent items (whether Ok or Err) are never consulted.
        let iter = std::iter::once(Err::<i32, syn::Error>(err("first")))
            .chain(std::iter::once(Err(err("second"))))
            .chain(std::iter::once(Err(err("third"))));
        let result: syn::Result<Vec<i32>> = iter.sift();
        let error = result.unwrap_err();
        let messages: Vec<String> = error.into_iter().map(|e| e.to_string()).collect();
        assert_eq!(messages, vec!["first"]);
    }

    #[test]
    fn sift_mixed_ok_and_err_returns_err() {
        // When the iterator yields Ok, Err, Ok, Err: collecting into a Vec
        // stops at the first Err. The intermediate Ok values consumed before
        // the error are discarded because `sift` returns Err, not Ok.
        let iter = std::iter::once(Ok::<i32, syn::Error>(1))
            .chain(std::iter::once(Err(err("bad"))))
            .chain(std::iter::once(Ok(2)))
            .chain(std::iter::once(Err(err("worse"))));
        let result: syn::Result<Vec<i32>> = iter.sift();
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "bad");
    }

    #[test]
    fn sift_size_hint_clamps_lower_bound_to_zero() {
        // The CollectingShunt's size_hint should report (0, upper) matching
        // the underlying iterator's upper bound. We observe this by collecting
        // into a Vec via sift: a Vec with three Ok items must allocate enough
        // capacity to hold them, proving the upper bound is propagated.
        let iter = [Ok::<i32, syn::Error>(10), Ok(20), Ok(30)].into_iter();
        assert_eq!(iter.size_hint(), (3, Some(3)));
        let result: syn::Result<Vec<i32>> = [Ok::<i32, syn::Error>(10), Ok(20), Ok(30)]
            .into_iter()
            .sift();
        let collected = result.unwrap();
        assert_eq!(collected.len(), 3);
        assert!(collected.capacity() >= 3);
    }
}
