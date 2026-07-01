//! Data Source Chaining
//!
//! A data source chain lets multiple data sources play one after another as if
//! they were a single continuous source.
//!
//! # How a `ChainSource` works internally
//!
//! Lets image a data source chain like this:
//! decoder1 -> decoder2 -> decoder3 -> decoder4
//! In maudio it is structures as:
//! head: decoder1 (always the first data source in the chain)
//! tail: decoder2, decoder3, decoder4
//!
//! All of the data source related operations are done on the head.
//! For example, when we do: `chain.read_pcm_frames` it is actually `head.read_pcm_frames`.
//! Then internally, miniaudio resolves which data source is currenly active.
//!
//! Each chained data source stores a pointer to the next source in the chain.
//! The head also stores a pointer to the current source being read.
//!
//! For example, while the chain is playing decoder3, the links look like this:
//! head.current = decoder3 (called as chain.current)
//! decoder3.next = decoder4 (called as chain.next)
//!
//! Calling chain.get_current() would therefore return a [DataSourceRef] to decoder3.
//!
//! When decoder3 reaches the end, miniaudio advances the chain by looking at
//! the current source's next pointer:
//! head.current = head.current.next
//!
//! After advancing, the current source becomes decoder4.
//!
//! # How it works in practice
//!
//!
//!
//!
//!
//!
//!
//!

use crate::{
    data_source::{data_source_ffi, private_data_source, AsSourcePtr, DataSourceRef, SharedSource},
    pcm_frames::PcmFormat,
    ErrorKinds, MaResult, MaudioError,
};

pub struct ChainSource<'a, F: PcmFormat> {
    head: DataSourceRef<'a, F>,
    tail: Vec<DataSourceRef<'a, F>>,
    looping: bool,
}

impl<'a, F: PcmFormat> ChainSource<'a, F> {
    /// Creates a new chain with `source` as the head.
    ///
    /// The head is always present and is the source used as the chain entry point.
    pub fn new(source: DataSourceRef<'a, F>, looping: bool) -> Self {
        Self {
            head: source,
            tail: Vec::new(),
            looping,
        }
    }

    /// Returns `true` if the chain loops back to the head after the final source.
    pub fn is_looping(&self) -> bool {
        self.looping
    }

    /// Sets whether the chain loops back to the head after the final source.
    ///
    /// This does not enable looping on individual data sources.
    pub fn set_looping(&mut self, yes: bool) -> MaResult<()> {
        self.looping = yes;
        self.relink()
    }

    /// Returns the number of sources after the head.
    pub fn tail_len(&self) -> usize {
        self.tail.len()
    }

    /// Returns `true` if the chain only contains the head.
    pub fn tail_is_empty(&self) -> bool {
        self.tail.is_empty()
    }

    /// Removes all sources after the head.
    pub fn clear_tail(&mut self) -> MaResult<()> {
        self.tail.clear();
        self.relink()
    }

    /// Adds a data source to the end of the chain.
    ///
    /// Returns `InvalidOperation` if the source is already in the chain.
    pub fn insert(&mut self, src: DataSourceRef<'a, F>) -> MaResult<()> {
        if self.exists(src) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Data Source already exists",
            )));
        }
        self.tail.push(src);
        self.relink()
    }

    /// Removes a data source from the tail.
    ///
    /// Returns `Ok(false)` if the source is not in the tail. The head cannot be
    /// removed.
    pub fn remove(&mut self, src: DataSourceRef<'a, F>) -> MaResult<bool> {
        if let Some(index) = self.tail.iter().position(|c| c == &src) {
            self.tail.remove(index);
            self.relink()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Inserts `next` after `curr`.
    ///
    /// Returns `InvalidOperation` if `next` is already in the chain. If `curr`
    /// is the head, `next` becomes the first tail source.
    pub fn insert_after(
        &mut self,
        curr: DataSourceRef<'a, F>,
        next: DataSourceRef<'a, F>,
    ) -> MaResult<()> {
        if !self.exists(curr) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Current data source does not exist",
            )));
        }
        if self.exists(next) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Next data source already exists",
            )));
        }
        if self.head == curr {
            self.tail.insert(0, next);
        }
        if let Some(index) = self.tail.iter().position(|c| c == &curr) {
            self.tail.insert(index + 1, next);
        }
        self.relink()
    }

    /// Inserts a data source immediately after the head.
    ///
    /// Returns `InvalidOperation` if the source is already in the chain.
    pub fn insert_after_head(&mut self, src: DataSourceRef<'a, F>) -> MaResult<()> {
        if self.exists(src) {
            return Err(MaudioError::new_ma_error(ErrorKinds::InvalidOperation(
                "Data Source already exists",
            )));
        }
        self.tail.insert(0, src);
        self.relink()
    }

    pub(crate) fn set_curr(&mut self, curr: DataSourceRef<'a, F>) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_current(self.head, curr)
    }

    pub(crate) fn get_curr(&self) -> DataSourceRef<'_, F> {
        data_source_ffi::ma_data_source_get_current(self.head)
    }

    pub(crate) fn get_next(&self) -> Option<DataSourceRef<'a, F>> {
        data_source_ffi::ma_data_source_get_next(self.head)
    }

    pub(crate) fn get_next_at(&self, curr: DataSourceRef<'a, F>) -> Option<DataSourceRef<'a, F>> {
        data_source_ffi::ma_data_source_get_next(curr)
    }

    pub(crate) fn set_next(&mut self, next: Option<DataSourceRef<'a, F>>) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_next(self.head, next)
    }

    pub(crate) fn set_next_at(
        &mut self,
        curr: DataSourceRef<'a, F>,
        next: Option<DataSourceRef<'a, F>>,
    ) -> MaResult<()> {
        data_source_ffi::ma_data_source_set_next(curr, next)
    }

    fn exists(&self, src: DataSourceRef<'_, F>) -> bool {
        self.tail.contains(&src) || self.head == src
    }

    fn relink(&mut self) -> MaResult<()> {
        self.set_next_at(self.head, self.tail.first().copied())?;

        let mut prev = self.head;

        for i in 0..self.tail.len() {
            let next = self.tail.get(i + 1).copied();
            self.set_next_at(self.tail[i], next)?;
            prev = self.tail[i];
        }

        if self.looping {
            self.set_next_at(prev, Some(self.head))?;
        } else {
            self.set_next_at(prev, None)?;
        }

        Ok(())
    }

    pub(crate) fn as_source(&'a self) -> DataSourceRef<'a, F> {
        self.head
    }
}

impl<F: PcmFormat> SharedSource for ChainSource<'_, F> {}

impl<'a, F: PcmFormat> AsSourcePtr for ChainSource<'a, F> {
    type Format = F;
    type __PtrProvider = private_data_source::ChainSourceProvider;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        audio::sample_rate::SampleRate,
        data_source::{data_source_builder::DataSourceBuilder, DataSource},
    };

    fn buffer_source(data: &[f32]) -> DataSource<f32, Vec<f32>> {
        DataSourceBuilder::new(2, SampleRate::Sr44100)
            .build_f32(data.to_vec())
            .unwrap()
    }

    #[test]
    fn test_data_chain_new_non_looping_chain_has_empty_tail() {
        let data = vec![0.0f32; 200];
        let src = buffer_source(&data);

        let chain = ChainSource::new(src.as_source(), false);

        assert!(!chain.is_looping());
        assert_eq!(chain.tail_len(), 0);
        assert!(chain.tail_is_empty());
    }

    #[test]
    fn test_data_chain_new_looping_chain_reports_looping() {
        let data = vec![0.0f32; 200];
        let src = buffer_source(&data);

        let chain = ChainSource::new(src.as_source(), true);

        assert!(chain.is_looping());
        assert_eq!(chain.tail_len(), 0);
    }

    #[test]
    fn test_data_chain_set_looping_can_toggle_looping() {
        let data = vec![0.0f32; 200];
        let src = buffer_source(&data);

        let mut chain = ChainSource::new(src.as_source(), false);

        chain.set_looping(true).unwrap();
        assert!(chain.is_looping());

        chain.set_looping(false).unwrap();
        assert!(!chain.is_looping());
    }

    #[test]
    fn test_data_chain_insert_adds_source_to_tail() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();

        assert_eq!(chain.tail_len(), 1);
        assert!(!chain.tail_is_empty());
    }

    #[test]
    fn test_data_chain_insert_rejects_head_as_duplicate() {
        let data = vec![0.0f32; 200];
        let src = buffer_source(&data);

        let mut chain = ChainSource::new(src.as_source(), false);

        let result = chain.insert(src.as_source());

        assert!(result.is_err());
        assert_eq!(chain.tail_len(), 0);
    }

    #[test]
    fn test_data_chain_insert_rejects_existing_tail_source() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();

        let result = chain.insert(src2.as_source());

        assert!(result.is_err());
        assert_eq!(chain.tail_len(), 1);
    }

    #[test]
    fn test_data_chain_remove_existing_tail_source_returns_true() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();

        let removed = chain.remove(src2.as_source()).unwrap();

        assert!(removed);
        assert_eq!(chain.tail_len(), 0);
        assert!(chain.tail_is_empty());
    }

    #[test]
    fn test_data_chain_remove_missing_source_returns_false() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        let removed = chain.remove(src2.as_source()).unwrap();

        assert!(!removed);
        assert_eq!(chain.tail_len(), 0);
    }

    #[test]
    fn test_data_chain_remove_head_returns_false_and_keeps_chain() {
        let data = vec![0.0f32; 200];
        let src = buffer_source(&data);

        let mut chain = ChainSource::new(src.as_source(), false);

        let removed = chain.remove(src.as_source()).unwrap();

        assert!(!removed);
        assert_eq!(chain.tail_len(), 0);
    }

    #[test]
    fn test_data_chain_clear_tail_removes_all_tail_sources() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];
        let data3 = vec![2.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);
        let src3 = buffer_source(&data3);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();
        chain.insert(src3.as_source()).unwrap();

        assert_eq!(chain.tail_len(), 2);

        chain.clear_tail().unwrap();

        assert_eq!(chain.tail_len(), 0);
        assert!(chain.tail_is_empty());
    }

    #[test]
    fn test_data_chain_insert_after_head_places_source_at_front_of_tail() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];
        let data3 = vec![2.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);
        let src3 = buffer_source(&data3);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();
        chain.insert_after_head(src3.as_source()).unwrap();

        assert_eq!(chain.tail_len(), 2);

        assert_eq!(chain.tail[0], src3.as_source());
        assert_eq!(chain.tail[1], src2.as_source());
    }

    #[test]
    fn test_data_chain_insert_after_tail_source_places_source_after_current() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];
        let data3 = vec![2.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);
        let src3 = buffer_source(&data3);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();
        chain
            .insert_after(src2.as_source(), src3.as_source())
            .unwrap();

        assert_eq!(chain.tail_len(), 2);

        assert_eq!(chain.tail[0], src2.as_source());
        assert_eq!(chain.tail[1], src3.as_source());
    }

    #[test]
    fn test_data_chain_insert_after_head_matches_insert_after_with_head() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain
            .insert_after(src1.as_source(), src2.as_source())
            .unwrap();

        assert_eq!(chain.tail_len(), 1);
        assert_eq!(chain.tail[0], src2.as_source());
    }

    #[test]
    fn test_data_chain_insert_after_rejects_duplicate_next_source() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();

        let result = chain.insert_after(src1.as_source(), src2.as_source());

        assert!(result.is_err());
        assert_eq!(chain.tail_len(), 1);
    }

    #[test]
    fn test_data_chain_relink_non_looping_sets_expected_native_next_links() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];
        let data3 = vec![2.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);
        let src3 = buffer_source(&data3);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();
        chain.insert(src3.as_source()).unwrap();

        assert_eq!(chain.get_next(), Some(src2.as_source()));
        assert_eq!(chain.get_next_at(src2.as_source()), Some(src3.as_source()));
        assert_eq!(chain.get_next_at(src3.as_source()), None);
    }

    #[test]
    fn test_data_chain_relink_looping_sets_last_source_next_to_head() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];
        let data3 = vec![2.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);
        let src3 = buffer_source(&data3);

        let mut chain = ChainSource::new(src1.as_source(), true);

        chain.insert(src2.as_source()).unwrap();
        chain.insert(src3.as_source()).unwrap();

        assert_eq!(chain.get_next(), Some(src2.as_source()));
        assert_eq!(chain.get_next_at(src2.as_source()), Some(src3.as_source()));
        assert_eq!(chain.get_next_at(src3.as_source()), Some(src1.as_source()));
    }

    #[test]
    fn test_data_chain_set_looping_updates_native_links() {
        let data1 = vec![0.0f32; 200];
        let data2 = vec![1.0f32; 200];

        let src1 = buffer_source(&data1);
        let src2 = buffer_source(&data2);

        let mut chain = ChainSource::new(src1.as_source(), false);

        chain.insert(src2.as_source()).unwrap();

        assert_eq!(chain.get_next_at(src2.as_source()), None);

        chain.set_looping(true).unwrap();

        assert_eq!(chain.get_next_at(src2.as_source()), Some(src1.as_source()));

        chain.set_looping(false).unwrap();

        assert_eq!(chain.get_next_at(src2.as_source()), None);
    }
}
