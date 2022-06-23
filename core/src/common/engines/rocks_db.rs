use crate::common::{
    vsdb_get_base_dir, vsdb_set_base_dir, BranchIDBase as BranchID, Engine, Pre,
    PreBytes, RawBytes, RawKey, RawValue, VersionIDBase as VersionID, GB,
    INITIAL_BRANCH_ID, MB, PREFIX_SIZE, RESERVED_ID_CNT,
};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rocksdb::{
    Cache, ColumnFamily, ColumnFamilyDescriptor, DBCompressionType, DBIterator,
    Direction, IteratorMode, Options, ReadOptions, SliceTransform, DB,
};
use ruc::*;
use std::{
    borrow::Cow,
    mem::size_of,
    ops::{Bound, RangeBounds},
    sync::atomic::{AtomicUsize, Ordering},
    thread::available_parallelism,
};

// NOTE:
// do NOT make the number of areas bigger than `u8::MAX`
const DATA_SET_NUM: usize = 8;

const META_KEY_MAX_KEYLEN: [u8; 1] = [u8::MAX];
const META_KEY_BRANCH_ID: [u8; 1] = [u8::MAX - 1];
const META_KEY_VERSION_ID: [u8; 1] = [u8::MAX - 2];
const META_KEY_PREFIX_ALLOCATOR: [u8; 1] = [u8::MIN];

static HDR: Lazy<(DB, Vec<String>, Cache)> = Lazy::new(|| rocksdb_open().unwrap());

pub struct RocksEngine {
    meta: &'static DB,
    areas: Vec<&'static str>,
    prefix_allocator: PreAllocator,
    max_keylen: AtomicUsize,
}

impl RocksEngine {
    #[inline(always)]
    fn cf_hdr(&self, area_idx: usize) -> &ColumnFamily {
        self.meta.cf_handle(self.areas[area_idx]).unwrap()
    }

    #[inline(always)]
    fn get_max_keylen(&self) -> usize {
        self.max_keylen.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn set_max_key_len(&self, len: usize) {
        self.max_keylen.store(len, Ordering::Relaxed);
        self.meta
            .put(META_KEY_MAX_KEYLEN, len.to_be_bytes())
            .unwrap();
    }

    #[inline(always)]
    fn get_upper_bound_value(&self, meta_prefix: PreBytes) -> Vec<u8> {
        static BUF: Lazy<RawBytes> = Lazy::new(|| vec![u8::MAX; 512]);

        let mut max_guard = meta_prefix.to_vec();

        let l = self.get_max_keylen();
        if l < 513 {
            max_guard.extend_from_slice(&BUF[..l]);
        } else {
            max_guard.extend_from_slice(&vec![u8::MAX; l]);
        }

        max_guard
    }
}

impl Engine for RocksEngine {
    fn new() -> Result<Self> {
        let (meta, areas) =
            (&HDR.0, HDR.1.iter().map(|i| i.as_str()).collect::<Vec<_>>());

        let (prefix_allocator, initial_value) = PreAllocator::init();

        if meta.get(&META_KEY_MAX_KEYLEN).c(d!())?.is_none() {
            meta.put(META_KEY_MAX_KEYLEN, 0_usize.to_be_bytes())
                .c(d!())?;
        }

        if meta.get(&META_KEY_BRANCH_ID).c(d!())?.is_none() {
            meta.put(
                META_KEY_BRANCH_ID,
                (1 + INITIAL_BRANCH_ID as usize).to_be_bytes(),
            )
            .c(d!())?;
        }

        if meta.get(&META_KEY_VERSION_ID).c(d!())?.is_none() {
            meta.put(META_KEY_VERSION_ID, 0_usize.to_be_bytes())
                .c(d!())?;
        }

        if meta.get(prefix_allocator.key).c(d!())?.is_none() {
            meta.put(prefix_allocator.key, initial_value).c(d!())?;
        }

        let max_keylen = AtomicUsize::new(crate::parse_int!(
            meta.get(&META_KEY_MAX_KEYLEN).unwrap().unwrap(),
            usize
        ));

        Ok(RocksEngine {
            meta,
            areas,
            prefix_allocator,
            // length of the raw key, exclude the meta prefix
            max_keylen,
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
            self.meta.get(self.prefix_allocator.key).unwrap().unwrap()
        );

        // step 2
        self.meta
            .put(self.prefix_allocator.key, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    // 'step 1' and 'step 2' is not atomic in multi-threads scene,
    // so we use a `Mutex` lock for thread safe.
    #[allow(unused_variables)]
    fn alloc_br_id(&self) -> BranchID {
        static LK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
        let x = LK.lock();

        // step 1
        let ret = crate::parse_int!(
            self.meta.get(META_KEY_BRANCH_ID).unwrap().unwrap(),
            BranchID
        );

        // step 2
        self.meta
            .put(META_KEY_BRANCH_ID, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    // 'step 1' and 'step 2' is not atomic in multi-threads scene,
    // so we use a `Mutex` lock for thread safe.
    #[allow(unused_variables)]
    fn alloc_ver_id(&self) -> VersionID {
        static LK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
        let x = LK.lock();

        // step 1
        let ret = crate::parse_int!(
            self.meta.get(META_KEY_VERSION_ID).unwrap().unwrap(),
            VersionID
        );

        // step 2
        self.meta
            .put(META_KEY_VERSION_ID, (1 + ret).to_be_bytes())
            .unwrap();

        ret
    }

    fn area_count(&self) -> usize {
        DATA_SET_NUM
    }

    fn flush(&self) {
        self.meta.flush().unwrap();
        (0..DATA_SET_NUM).for_each(|i| {
            self.meta.flush_cf(self.cf_hdr(i)).unwrap();
        });
    }

    fn iter(&self, meta_prefix: PreBytes) -> RocksIter {
        let area_idx = self.area_idx(meta_prefix);

        let inner = self
            .meta
            .prefix_iterator_cf(self.cf_hdr(area_idx), meta_prefix);

        let mut opt = ReadOptions::default();
        opt.set_prefix_same_as_start(true);

        let inner_rev = self.meta.iterator_cf_opt(
            self.cf_hdr(area_idx),
            opt,
            IteratorMode::From(
                &self.get_upper_bound_value(meta_prefix),
                Direction::Reverse,
            ),
        );

        RocksIter { inner, inner_rev }
    }

    fn range<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        meta_prefix: PreBytes,
        bounds: R,
    ) -> RocksIter {
        let area_idx = self.area_idx(meta_prefix);

        let mut opt = ReadOptions::default();
        let mut opt_rev = ReadOptions::default();

        let mut b_lo = meta_prefix.to_vec();
        let l = match bounds.start_bound() {
            Bound::Included(lo) => {
                b_lo.extend_from_slice(lo);
                opt.set_iterate_lower_bound(b_lo.as_slice());
                opt_rev.set_iterate_lower_bound(b_lo.as_slice());
                b_lo.as_slice()
            }
            Bound::Excluded(lo) => {
                b_lo.extend_from_slice(lo);
                b_lo.push(0u8);
                opt.set_iterate_lower_bound(b_lo.as_slice());
                opt_rev.set_iterate_lower_bound(b_lo.as_slice());
                b_lo.as_slice()
            }
            _ => meta_prefix.as_slice(),
        };

        let mut b_hi = meta_prefix.to_vec();
        let h = match bounds.end_bound() {
            Bound::Included(hi) => {
                b_hi.extend_from_slice(hi);
                b_hi.push(0u8);
                opt.set_iterate_upper_bound(b_hi.as_slice());
                opt_rev.set_iterate_upper_bound(b_hi.as_slice());
                b_hi
            }
            Bound::Excluded(hi) => {
                b_hi.extend_from_slice(hi);
                opt.set_iterate_upper_bound(b_hi.as_slice());
                opt_rev.set_iterate_upper_bound(b_hi.as_slice());
                b_hi
            }
            _ => self.get_upper_bound_value(meta_prefix),
        };

        opt.set_prefix_same_as_start(true);
        opt_rev.set_prefix_same_as_start(true);

        let inner = self.meta.iterator_cf_opt(
            self.cf_hdr(area_idx),
            opt,
            IteratorMode::From(l, Direction::Forward),
        );

        let inner_rev = self.meta.iterator_cf_opt(
            self.cf_hdr(area_idx),
            opt_rev,
            IteratorMode::From(&h, Direction::Reverse),
        );

        RocksIter { inner, inner_rev }
    }

    fn get(&self, meta_prefix: PreBytes, key: &[u8]) -> Option<RawValue> {
        let area_idx = self.area_idx(meta_prefix);

        let mut k = meta_prefix.to_vec();
        k.extend_from_slice(key);
        self.meta.get_cf(self.cf_hdr(area_idx), k).unwrap()
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

        if key.len() > self.get_max_keylen() {
            self.set_max_key_len(key.len());
        }

        let old_v = self.meta.get_cf(self.cf_hdr(area_idx), &k).unwrap();
        self.meta.put_cf(self.cf_hdr(area_idx), k, value).unwrap();
        old_v
    }

    fn remove(&self, meta_prefix: PreBytes, key: &[u8]) -> Option<RawValue> {
        let area_idx = self.area_idx(meta_prefix);

        let mut k = meta_prefix.to_vec();
        k.extend_from_slice(key);
        let old_v = self.meta.get_cf(self.cf_hdr(area_idx), &k).unwrap();
        self.meta.delete_cf(self.cf_hdr(area_idx), k).unwrap();
        old_v
    }

    fn get_instance_len(&self, instance_prefix: PreBytes) -> u64 {
        crate::parse_int!(self.meta.get(instance_prefix).unwrap().unwrap(), u64)
    }

    fn set_instance_len(&self, instance_prefix: PreBytes, new_len: u64) {
        self.meta
            .put(instance_prefix, new_len.to_be_bytes())
            .unwrap();
    }
}

pub struct RocksIter {
    inner: DBIterator<'static>,
    inner_rev: DBIterator<'static>,
}

impl Iterator for RocksIter {
    type Item = (RawKey, RawValue);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(ik, iv)| (ik[PREFIX_SIZE..].to_vec(), iv.into_vec()))
    }
}

impl DoubleEndedIterator for RocksIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner_rev
            .next()
            .map(|(ik, iv)| (ik[PREFIX_SIZE..].to_vec(), iv.into_vec()))
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

    // fn next(base: &[u8]) -> [u8; PREFIX_SIZE] {
    //     (crate::parse_prefix!(base) + 1).to_be_bytes()
    // }
}

fn rocksdb_open() -> Result<(DB, Vec<String>, Cache)> {
    let dir = vsdb_get_base_dir();

    // avoid setting again on an opened DB
    info_omit!(vsdb_set_base_dir(&dir));

    let parallelism = available_parallelism().c(d!())?.get() as i32;
    let cache_cap =
        max!(GB, min!(((parallelism as u64) * 2 / 10) * GB, 12 * GB)) as usize;

    let mut cfg = Options::default();

    cfg.create_if_missing(true);
    cfg.create_missing_column_families(true);
    cfg.set_prefix_extractor(SliceTransform::create_fixed_prefix(size_of::<Pre>()));
    cfg.increase_parallelism(parallelism);
    cfg.set_num_levels(7);
    cfg.set_max_open_files(8192);
    cfg.set_allow_mmap_writes(true);
    cfg.set_allow_mmap_reads(true);
    // cfg.set_use_direct_reads(true);
    // cfg.set_use_direct_io_for_flush_and_compaction(true);
    cfg.set_write_buffer_size(512 * MB as usize);
    cfg.set_max_write_buffer_number(3);

    let lru = Cache::new_lru_cache(cache_cap).c(d!())?;
    cfg.set_row_cache(&lru);

    #[cfg(feature = "compress")]
    {
        cfg.set_compression_type(DBCompressionType::Lz4);
    }

    #[cfg(not(feature = "compress"))]
    {
        cfg.set_compression_type(DBCompressionType::None);
    }

    let cfhdrs = (0..DATA_SET_NUM).map(|i| i.to_string()).collect::<Vec<_>>();

    let cfs = cfhdrs
        .iter()
        .map(|i| ColumnFamilyDescriptor::new(i, cfg.clone()))
        .collect::<Vec<_>>();

    let db = DB::open_cf_descriptors(&cfg, &dir, cfs).c(d!())?;

    Ok((db, cfhdrs, lru))
}
