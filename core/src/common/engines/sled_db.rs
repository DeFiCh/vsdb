use crate::common::{
    vsdb_get_base_dir, vsdb_set_base_dir, BranchIDBase as BranchID, Engine, Pre,
    PreBytes, RawKey, RawValue, VersionIDBase as VersionID, GB, INITIAL_BRANCH_ID,
    PREFIX_SIZE, RESERVED_ID_CNT,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use ruc::*;
use sled::{Config, Db, IVec, Iter, Mode, Tree};
use std::{
    borrow::Cow,
    ops::{Bound, RangeBounds},
    thread::available_parallelism,
};

// the 'prefix search' in sled is just a global scaning,
// use a relative larger number to sharding the `Tree` pressure.
//
// NOTE:
// do NOT make the number of areas bigger than `u8::MAX`
const DATA_SET_NUM: usize = u8::MAX as usize;

const META_KEY_BRANCH_ID: [u8; 1] = [u8::MAX - 1];
const META_KEY_VERSION_ID: [u8; 1] = [u8::MAX - 2];
const META_KEY_PREFIX_ALLOCATOR: [u8; 1] = [u8::MIN];

pub struct SledEngine {
    meta: Db,
    areas: Vec<Tree>,
    prefix_allocator: PreAllocator,
}

impl Engine for SledEngine {
    fn new() -> Result<Self> {
        let meta = sled_open().c(d!())?;

        let areas = (0..DATA_SET_NUM)
            .map(|idx| meta.open_tree(idx.to_be_bytes()).c(d!()))
            .collect::<Result<Vec<_>>>()?;

        let (prefix_allocator, initial_value) = PreAllocator::init();

        if meta.get(&META_KEY_BRANCH_ID).c(d!())?.is_none() {
            meta.insert(
                META_KEY_BRANCH_ID,
                (1 + INITIAL_BRANCH_ID as usize).to_be_bytes(),
            )
            .c(d!())?;
        }

        if meta.get(&META_KEY_VERSION_ID).c(d!())?.is_none() {
            meta.insert(META_KEY_VERSION_ID, 0_usize.to_be_bytes())
                .c(d!())?;
        }

        if meta.get(prefix_allocator.key).c(d!())?.is_none() {
            meta.insert(prefix_allocator.key, initial_value).c(d!())?;
        }

        Ok(SledEngine {
            meta,
            areas,
            prefix_allocator,
        })
    }

    // 'step 1' and 'step 2' is not atomic in multi-threads scene,
    // so we use a `Mutex` lock for thread safe.
    #[allow(unused_variables)]
    fn alloc_prefix(&self) -> Pre {
        static LK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
        let x = LK.lock();

        // step 1
        let ret = crate::parse_prefix!(
            self.meta
                .get(self.prefix_allocator.key)
                .unwrap()
                .unwrap()
                .as_ref()
        );

        // step 2
        self.meta
            .insert(self.prefix_allocator.key, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    // 'step 1' and 'step 2' is not atomic in multi-threads scene,
    // so we use a `Mutex` lock for thread safe.
    #[allow(unused_variables)]
    fn alloc_br_id(&self) -> BranchID {
        static LK: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
        let x = LK.lock();

        // step 1
        let ret = crate::parse_int!(
            self.meta.get(META_KEY_BRANCH_ID).unwrap().unwrap().as_ref(),
            BranchID
        );

        // step 2
        self.meta
            .insert(META_KEY_BRANCH_ID, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    // 'step 1' and 'step 2' is not atomic in multi-threads scene,
    // so we use a `Mutex` lock for thread safe.
    #[allow(unused_variables)]
    fn alloc_ver_id(&self) -> VersionID {
        static LK: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
        let x = LK.lock();

        // step 1
        let ret = crate::parse_int!(
            self.meta
                .get(META_KEY_VERSION_ID)
                .unwrap()
                .unwrap()
                .as_ref(),
            VersionID
        );

        // step 2
        self.meta
            .insert(META_KEY_VERSION_ID, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    fn area_count(&self) -> usize {
        self.areas.len()
    }

    fn flush(&self) {
        (0..self.areas.len()).for_each(|i| {
            self.areas[i].flush().unwrap();
        });
    }

    fn iter(&self, meta_prefix: PreBytes) -> SledIter {
        let area_idx = self.area_idx(meta_prefix);

        SledIter {
            inner: self.areas[area_idx].scan_prefix(meta_prefix.as_slice()),
            bounds: (Bound::Unbounded, Bound::Unbounded),
        }
    }

    fn range<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        meta_prefix: PreBytes,
        bounds: R,
    ) -> SledIter {
        let area_idx = self.area_idx(meta_prefix);

        let mut b_lo = meta_prefix.to_vec();
        let l = match bounds.start_bound() {
            Bound::Included(lo) => {
                b_lo.extend_from_slice(lo);
                Bound::Included(IVec::from(b_lo))
            }
            Bound::Excluded(lo) => {
                b_lo.extend_from_slice(lo);
                Bound::Excluded(IVec::from(b_lo))
            }
            Bound::Unbounded => Bound::Unbounded,
        };

        let mut b_hi = meta_prefix.to_vec();
        let h = match bounds.end_bound() {
            Bound::Included(hi) => {
                b_hi.extend_from_slice(hi);
                Bound::Included(IVec::from(b_hi))
            }
            Bound::Excluded(hi) => {
                b_hi.extend_from_slice(hi);
                Bound::Excluded(IVec::from(b_hi))
            }
            Bound::Unbounded => Bound::Unbounded,
        };

        SledIter {
            inner: self.areas[area_idx].scan_prefix(meta_prefix.as_slice()),
            bounds: (l, h),
        }
    }

    fn get(&self, meta_prefix: PreBytes, key: &[u8]) -> Option<RawValue> {
        let area_idx = self.area_idx(meta_prefix);

        let mut k = meta_prefix.to_vec();
        k.extend_from_slice(key);
        let k = k.into_boxed_slice();

        self.areas[area_idx]
            .get(k)
            .unwrap()
            .map(|iv| iv.to_vec().into_boxed_slice())
    }

    fn insert(
        &self,
        meta_prefix: PreBytes,
        key: &[u8],
        value: &[u8],
    ) -> Option<RawValue> {
        let area_idx = self.area_idx(meta_prefix);

        let mut k = meta_prefix.to_vec();
        k.extend_from_slice(key);
        let k = k.into_boxed_slice();

        self.areas[area_idx]
            .insert(k, value)
            .unwrap()
            .map(|iv| iv.to_vec().into_boxed_slice())
    }

    fn remove(&self, meta_prefix: PreBytes, key: &[u8]) -> Option<RawValue> {
        let area_idx = self.area_idx(meta_prefix);

        let mut k = meta_prefix.to_vec();
        k.extend_from_slice(key);
        let k = k.into_boxed_slice();

        self.areas[area_idx]
            .remove(k)
            .unwrap()
            .map(|iv| iv.to_vec().into_boxed_slice())
    }

    fn get_instance_len(&self, instance_prefix: PreBytes) -> u64 {
        crate::parse_int!(self.meta.get(instance_prefix).unwrap().unwrap(), u64)
    }

    fn set_instance_len(&self, instance_prefix: PreBytes, new_len: u64) {
        self.meta
            .insert(instance_prefix, new_len.to_be_bytes())
            .unwrap();
    }
}

pub struct SledIter {
    inner: Iter,
    bounds: (Bound<IVec>, Bound<IVec>),
}

impl Iterator for SledIter {
    type Item = (RawKey, RawValue);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((k, v)) = self.inner.next().map(|i| i.unwrap()) {
            if self.bounds.contains(&k) {
                return Some((
                    k[PREFIX_SIZE..].to_vec().into_boxed_slice(),
                    v.to_vec().into_boxed_slice(),
                ));
            }
        }
        None
    }
}

impl DoubleEndedIterator for SledIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some((k, v)) = self.inner.next_back().map(|i| i.unwrap()) {
            if self.bounds.contains(&k) {
                return Some((
                    k[PREFIX_SIZE..].to_vec().into_boxed_slice(),
                    v.to_vec().into_boxed_slice(),
                ));
            }
        }
        None
    }
}

// key of the prefix allocator in the 'meta'
struct PreAllocator {
    key: [u8; 1],
}

impl PreAllocator {
    const fn init() -> (Self, PreBytes) {
        (
            Self {
                key: META_KEY_PREFIX_ALLOCATOR,
            },
            (RESERVED_ID_CNT + Pre::MIN).to_be_bytes(),
        )
    }
}

fn sled_open() -> Result<Db> {
    let dir = vsdb_get_base_dir();

    // avoid setting again on an opened DB
    info_omit!(vsdb_set_base_dir(&dir));

    let parallelism = available_parallelism().c(d!())?.get() as u64;
    let cache_cap = max!(GB, min!((parallelism * 2 / 10) * GB, 12 * GB));

    let mut cfg = Config::new()
        .path(&dir)
        .mode(Mode::HighThroughput)
        .cache_capacity(cache_cap);

    #[cfg(feature = "compress")]
    {
        cfg = cfg.use_compression(true).compression_factor(1);
    }

    #[cfg(not(feature = "compress"))]
    {
        cfg = cfg.use_compression(false);
    }

    cfg.open().c(d!())
}
