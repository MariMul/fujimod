//! Immutable page versions with conservative engine/DMA dirty tracking.
//! Keyframes never alias mutable guest backing. Only flagged pages are read
//! after an initial capture; equal or zero candidates retain shared pages.
use crate::BackingObserver;
mod checkpoint;
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, OnceLock},
};

pub const PAGE_BYTES: usize = 4096;
type Page = [u8; PAGE_BYTES];
static NEXT_TRACKER_ID: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct Image {
    len: usize,
    pages: Arc<[Arc<Page>]>,
}
/// Stable bitmap allocation retained until every observing processor is gone.
/// Native processor threads and the owner may flag pages concurrently.
pub struct Tracker {
    id: u64,
    len: usize,
    dirty: Box<[AtomicU64]>,
    versions: Box<[AtomicU64]>,
    watched: Box<[AtomicU64]>,
    pending_rearm: Box<[AtomicU64]>,
    base: RefCell<Option<Image>>,
}
fn zero() -> &'static Arc<Page> {
    static ZERO: OnceLock<Arc<Page>> = OnceLock::new();
    ZERO.get_or_init(|| Arc::new([0; PAGE_BYTES]))
}
impl Image {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn pages(&self) -> usize {
        self.pages.len()
    }
    pub fn shared_pages(&self, other: &Self) -> usize {
        self.pages
            .iter()
            .zip(other.pages.iter())
            .filter(|(a, b)| Arc::ptr_eq(a, b))
            .count()
    }
}
impl Tracker {
    pub fn new(len: usize) -> Self {
        let page_count = len.div_ceil(PAGE_BYTES);
        Self {
            id: NEXT_TRACKER_ID.fetch_add(1, Ordering::Relaxed),
            len,
            dirty: (0..len.div_ceil(PAGE_BYTES * 64))
                .map(|_| AtomicU64::new(0))
                .collect(),
            versions: (0..page_count).map(|_| AtomicU64::new(0)).collect(),
            watched: (0..len.div_ceil(PAGE_BYTES * 64))
                .map(|_| AtomicU64::new(0))
                .collect(),
            pending_rearm: (0..len.div_ceil(PAGE_BYTES * 64))
                .map(|_| AtomicU64::new(0))
                .collect(),
            base: RefCell::new(None),
        }
    }
    pub fn observer(&self, backing: NonNull<u8>) -> Option<BackingObserver> {
        (self.len != 0).then(|| BackingObserver {
            backing,
            bytes: self.len,
            bitmap: NonNull::new(self.dirty.as_ptr() as *mut AtomicU64).unwrap(),
            versions: NonNull::new(self.versions.as_ptr() as *mut AtomicU64).unwrap(),
            watched: NonNull::new(self.watched.as_ptr() as *mut AtomicU64).unwrap(),
            pending_rearm: NonNull::new(self.pending_rearm.as_ptr() as *mut AtomicU64).unwrap(),
            page_shift: 12,
        })
    }
    pub fn mark(&self, offset: usize, bytes: usize) -> Result<(), String> {
        self.mark_inner(offset, bytes, true)
    }
    pub fn mark_coherent(&self, offset: usize, bytes: usize) -> Result<(), String> {
        self.mark_inner(offset, bytes, false)
    }
    fn mark_inner(&self, offset: usize, bytes: usize, invalidate: bool) -> Result<(), String> {
        if offset.checked_add(bytes).is_none_or(|end| end > self.len) {
            return Err("dirty range escapes backing".into());
        }
        if bytes != 0 {
            let first = offset / PAGE_BYTES;
            let last = (offset + bytes - 1) / PAGE_BYTES;
            for page in first..=last {
                self.dirty[page / 64].fetch_or(1 << (page % 64), Ordering::Relaxed);
                if invalidate {
                    self.versions[page].fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        Ok(())
    }
    pub fn page_versions(&self, offset: usize, bytes: usize) -> Result<(Vec<u64>, bool), String> {
        if offset.checked_add(bytes).is_none_or(|end| end > self.len) {
            return Err("page-version range escapes backing".into());
        }
        if bytes == 0 {
            return Ok((Vec::new(), false));
        }
        let first = offset / PAGE_BYTES;
        let last = (offset + bytes - 1) / PAGE_BYTES;
        let mut versions = Vec::with_capacity((last - first + 1) * 3);
        let mut needs_rearm = false;
        for page in first..=last {
            let bit = 1 << (page % 64);
            let word = page / 64;
            let previous = self.watched[word].fetch_or(bit, Ordering::Relaxed);
            if previous & bit == 0 {
                self.pending_rearm[word].fetch_or(bit, Ordering::Relaxed);
                needs_rearm = true;
            } else {
                needs_rearm |= self.pending_rearm[word].load(Ordering::Relaxed) & bit != 0;
            }
            versions.extend([
                self.id,
                page as u64,
                self.versions[page].load(Ordering::Relaxed),
            ]);
        }
        Ok((versions, needs_rearm))
    }
    pub fn clear_pending_rearm(&self) {
        for pending in &self.pending_rearm {
            pending.store(0, Ordering::Relaxed);
        }
    }
    fn dirty(&self, page: usize) -> bool {
        self.dirty[page / 64].load(Ordering::Relaxed) & (1 << (page % 64)) != 0
    }
    fn set_base(&self, image: &Image) {
        for flag in self.dirty.iter() {
            flag.store(0, Ordering::Relaxed);
        }
        *self.base.borrow_mut() = Some(image.clone());
    }
    /// # Safety
    /// Every candidate (dirty or initial) page is readable at `pointer`;
    /// clean pages are never accessed. All CPUs are stopped. Before any
    /// processor resumes, re-arm its backing observers to flush cached TLBs.
    /// Every host/DMA write must call `mark` in the same physical domain.
    pub unsafe fn capture(&self, pointer: *const u8) -> Image {
        let previous = self.base.borrow();
        let mut pages = Vec::with_capacity(self.len.div_ceil(PAGE_BYTES));
        let zero_page = zero();
        for offset in (0..self.len).step_by(PAGE_BYTES) {
            let index = offset / PAGE_BYTES;
            let size = PAGE_BYTES.min(self.len - offset);
            let old = previous.as_ref().and_then(|p| p.pages.get(index));
            if let Some(old) = old.filter(|_| !self.dirty(index)) {
                pages.push(old.clone());
                continue;
            }
            let bytes = unsafe { std::slice::from_raw_parts(pointer.add(offset), size) };
            let page = if let Some(old) = old.filter(|p| p[..size] == *bytes) {
                old.clone()
            } else if bytes == &zero_page[..size] {
                zero_page.clone()
            } else {
                let mut page = [0; PAGE_BYTES];
                page[..size].copy_from_slice(bytes);
                Arc::new(page)
            };
            pages.push(page);
        }
        let image = Image {
            len: self.len,
            pages: pages.into(),
        };
        drop(previous);
        self.set_base(&image);
        image
    }
    /// # Safety
    /// Exclusive paused access to writable backing. Notify code caches and
    /// monitors of the returned ranges, then re-arm all engine observers.
    pub unsafe fn restore(
        &self,
        image: &Image,
        pointer: *mut u8,
    ) -> Result<Vec<(usize, usize)>, String> {
        if self.len != image.len {
            return Err("keyframe memory extent differs".into());
        }
        let previous = self.base.borrow();
        let mut changed: Vec<(usize, usize)> = Vec::new();
        for (index, page) in image.pages.iter().enumerate() {
            let old = previous.as_ref().and_then(|p| p.pages.get(index));
            if !self.dirty(index) && old.is_some_and(|old| Arc::ptr_eq(old, page)) {
                continue;
            }
            let offset = index * PAGE_BYTES;
            let size = PAGE_BYTES.min(self.len - offset);
            let current = unsafe { std::slice::from_raw_parts(pointer.add(offset), size) };
            if current == &page[..size] {
                continue;
            }
            unsafe { std::ptr::copy_nonoverlapping(page.as_ptr(), pointer.add(offset), size) };
            if let Some((start, bytes)) = changed.last_mut()
                && *start + *bytes == offset
            {
                *bytes += size;
            } else {
                changed.push((offset, size));
            }
            let first = offset / PAGE_BYTES;
            let last = (offset + size - 1) / PAGE_BYTES;
            for page in first..=last {
                self.versions[page].fetch_add(1, Ordering::Relaxed);
            }
        }
        drop(previous);
        self.set_base(image);
        Ok(changed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dirty_versions_share_pages_restore_branches_and_track_partial_pages() {
        let mut ram = vec![0; PAGE_BYTES * 3 + 7];
        ram[PAGE_BYTES] = 42;
        let tracker = Tracker::new(ram.len());
        let first = unsafe { tracker.capture(ram.as_ptr()) };
        let same = unsafe { tracker.capture(ram.as_ptr()) };
        assert_eq!(first.shared_pages(&same), 4);
        assert!(Arc::ptr_eq(&first.pages[0], &first.pages[3]));
        ram[PAGE_BYTES * 2] = 99;
        ram[PAGE_BYTES * 3 + 6] = 11;
        tracker.mark(PAGE_BYTES * 2, PAGE_BYTES + 7).unwrap();
        let next = unsafe { tracker.capture(ram.as_ptr()) };
        assert_eq!(first.shared_pages(&next), 2);
        assert_eq!(
            unsafe { tracker.restore(&first, ram.as_mut_ptr()) }.unwrap(),
            [(PAGE_BYTES * 2, PAGE_BYTES + 7)]
        );
        ram[PAGE_BYTES] = 8;
        tracker.mark(PAGE_BYTES, 1).unwrap();
        assert_eq!(
            unsafe { tracker.restore(&next, ram.as_mut_ptr()) }.unwrap(),
            [(PAGE_BYTES, PAGE_BYTES * 2 + 7)]
        );
        assert_eq!(ram[PAGE_BYTES], 42);
        assert_eq!(ram[PAGE_BYTES * 2], 99);
        assert_eq!(ram.last(), Some(&11));
        assert!(
            unsafe { tracker.restore(&next, ram.as_mut_ptr()) }
                .unwrap()
                .is_empty()
        );
        assert!(tracker.mark(ram.len(), 1).is_err());
        let empty = Tracker::new(0);
        let image = unsafe { empty.capture(std::ptr::null()) };
        assert!(
            unsafe { empty.restore(&image, std::ptr::null_mut()) }
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn clean_pages_are_not_read_when_capturing_the_next_version() {
        let mut ram = vec![0; PAGE_BYTES * 2];
        ram[0] = 1;
        let tracker = Tracker::new(ram.len());
        let first = unsafe { tracker.capture(ram.as_ptr()) };
        // A sentinel pointer proves a completely clean capture never reads
        // backing: no candidate page requires a readable pointer in this case.
        let same = unsafe { tracker.capture(std::ptr::NonNull::<u8>::dangling().as_ptr()) };
        assert_eq!(first.shared_pages(&same), 2);
    }

    #[test]
    fn watched_page_generations_ignore_coherent_writeback() {
        let tracker = Tracker::new(PAGE_BYTES * 2);
        let (first, newly_watched) = tracker.page_versions(PAGE_BYTES - 1, 2).unwrap();
        assert!(newly_watched);
        assert_eq!(first.len(), 6);
        assert_eq!(tracker.pending_rearm[0].load(Ordering::Relaxed) & 0b11, 0b11);
        tracker.clear_pending_rearm();
        let (same, newly_watched) = tracker.page_versions(PAGE_BYTES - 1, 2).unwrap();
        assert!(!newly_watched);
        assert_eq!(same, first);

        tracker.mark_coherent(PAGE_BYTES, 1).unwrap();
        assert_eq!(tracker.page_versions(PAGE_BYTES - 1, 2).unwrap().0, first);
        tracker.pending_rearm[0].fetch_or(1 << 1, Ordering::Relaxed);
        assert!(tracker.page_versions(PAGE_BYTES - 1, 2).unwrap().1);
        tracker.clear_pending_rearm();
        assert!(!tracker.page_versions(PAGE_BYTES - 1, 2).unwrap().1);
        tracker.mark(PAGE_BYTES, 1).unwrap();
        let (changed, newly_watched) = tracker.page_versions(PAGE_BYTES - 1, 2).unwrap();
        assert!(!newly_watched);
        assert_eq!(changed[5], first[5] + 1);
    }
}
