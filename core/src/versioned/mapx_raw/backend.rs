//!
//! Core logic of the version managements.
//!

use crate::{
    basic::mapx_raw::{MapxRaw, MapxRawIter},
    common::{
        BranchID, BranchIDBase, BranchName, BranchNameOwned, RawKey, RawValue,
        VersionID, VersionIDBase, VersionName, VersionNameOwned, INITIAL_BRANCH_ID,
        INITIAL_BRANCH_NAME, NULL, RESERVED_VERSION_NUM_DEFAULT, VSDB,
    },
};
use ruc::*;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow, cmp::Ordering, collections::HashSet, mem::size_of, ops::RangeBounds,
};

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct MapxRawVs {
    default_branch: BranchID,

    branch_name_to_branch_id: MapxRaw, // MapxOrdRawKey<BranchID>,
    version_name_to_version_id: MapxRaw, // MapxOrdRawKey<VersionID>,

    branch_id_to_branch_name: MapxRaw, // MapxOrdRawValue<BranchID>,
    version_id_to_version_name: MapxRaw, // MapxOrdRawValue<VersionID>,

    // versions on this branch,
    // created dirctly by it or inherited from its ancestors
    branch_to_its_versions: MapxRaw, // MapxOrd<BranchID, MapxOrd<VersionID, ()>>,

    // globally ever changed keys(no value is stored here!) within each version
    version_to_change_set: MapxRaw, // MapxOrd<VersionID, MapxRaw>,

    // key -> multi-version(globally unique) -> multi-value
    layered_kv: MapxRaw, // MapxOrdRawKey<MapxOrd<VersionID, Option<RawValue>>>,
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

impl MapxRawVs {
    #[inline(always)]
    pub(super) unsafe fn shadow(&self) -> Self {
        Self {
            default_branch: self.default_branch,
            branch_name_to_branch_id: self.branch_name_to_branch_id.shadow(),
            version_name_to_version_id: self.version_name_to_version_id.shadow(),
            branch_id_to_branch_name: self.branch_id_to_branch_name.shadow(),
            version_id_to_version_name: self.version_id_to_version_name.shadow(),
            branch_to_its_versions: self.branch_to_its_versions.shadow(),
            version_to_change_set: self.version_to_change_set.shadow(),
            layered_kv: self.layered_kv.shadow(),
        }
    }

    #[inline(always)]
    pub(super) fn new() -> Self {
        let mut ret = Self {
            default_branch: BranchID::default(),
            branch_name_to_branch_id: MapxRaw::new(),
            version_name_to_version_id: MapxRaw::new(),
            branch_id_to_branch_name: MapxRaw::new(),
            version_id_to_version_name: MapxRaw::new(),
            branch_to_its_versions: MapxRaw::new(),
            version_to_change_set: MapxRaw::new(),
            layered_kv: MapxRaw::new(),
        };
        ret.init();
        ret
    }

    #[inline(always)]
    fn init(&mut self) {
        let initial_brid = INITIAL_BRANCH_ID.to_be_bytes();
        self.default_branch = initial_brid;
        self.branch_name_to_branch_id
            .insert(INITIAL_BRANCH_NAME.0, &initial_brid[..]);
        self.branch_id_to_branch_name
            .insert(&initial_brid[..], INITIAL_BRANCH_NAME.0);
        self.branch_to_its_versions
            .insert(&initial_brid[..], &encode(&MapxRaw::new()));
    }

    #[inline(always)]
    pub(super) fn insert(
        &mut self,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<RawValue>> {
        self.insert_by_branch(key, value, self.branch_get_default())
            .c(d!())
    }

    #[inline(always)]
    pub(super) fn insert_by_branch(
        &mut self,
        key: &[u8],
        value: &[u8],
        branch_id: BranchID,
    ) -> Result<Option<RawValue>> {
        decode_map(
            &self
                .branch_to_its_versions
                .get(&branch_id[..])
                .c(d!("branch not found"))?,
        )
        .iter()
        .last()
        .c(d!("no version on this branch, create a version first"))
        .and_then(|(version_id, _)| {
            self.insert_by_branch_version(key, value, branch_id, to_verid(&version_id))
                .c(d!())
        })
    }

    // This function should **NOT** be public,
    // `write`-like operations should only be applied
    // on the latest version of every branch,
    // historical data version should be immutable in the user view.
    #[inline(always)]
    fn insert_by_branch_version(
        &mut self,
        key: &[u8],
        value: &[u8],
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Result<Option<RawValue>> {
        self.write_by_branch_version(key, Some(value), branch_id, version_id)
            .c(d!())
    }

    #[inline(always)]
    pub(super) fn remove(&mut self, key: &[u8]) -> Result<Option<RawValue>> {
        self.remove_by_branch(key, self.branch_get_default())
            .c(d!())
    }

    #[inline(always)]
    pub(super) fn remove_by_branch(
        &mut self,
        key: &[u8],
        branch_id: BranchID,
    ) -> Result<Option<RawValue>> {
        decode_map(
            &self
                .branch_to_its_versions
                .get(&branch_id)
                .c(d!("branch not found"))?,
        )
        .iter()
        .last()
        .c(d!("no version on this branch, create a version first"))
        .and_then(|(version_id, _)| {
            self.remove_by_branch_version(key, branch_id, to_verid(&version_id))
                .c(d!())
        })
    }

    // This function should **NOT** be public,
    // `write`-like operations should only be applied
    // on the latest version of every branch,
    // historical data version should be immutable in the user view.
    //
    // The `remove` is essentially assign a `None` value to the key.
    fn remove_by_branch_version(
        &mut self,
        key: &[u8],
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Result<Option<RawValue>> {
        self.write_by_branch_version(key, None, branch_id, version_id)
            .c(d!())
    }

    // This function should **NOT** be public,
    // `write`-like operations should only be applied
    // on the latest version of every branch,
    // historical data version should be immutable in the user view.
    fn write_by_branch_version(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Result<Option<RawValue>> {
        let ret = self.get_by_branch_version(key, branch_id, version_id);

        // remove a non-existing value
        if value.is_none() && ret.is_none() {
            return Ok(None);
        }

        // NOTE: the value needs not to be stored here
        decode_map(&self.version_to_change_set.get_mut(&version_id).c(d!())?)
            .insert(key, &[]);

        decode_map(
            &self
                .layered_kv
                .entry(key)
                .or_insert(&encode(&MapxRaw::new())),
        )
        .insert(&version_id[..], value.unwrap_or_default());

        Ok(ret)
    }

    #[inline(always)]
    pub(super) fn get(&self, key: &[u8]) -> Option<RawValue> {
        self.get_by_branch(key, self.branch_get_default())
    }

    #[inline(always)]
    pub(super) fn get_by_branch(
        &self,
        key: &[u8],
        branch_id: BranchID,
    ) -> Option<RawValue> {
        if let Some(vers) = self.branch_to_its_versions.get(&branch_id) {
            if let Some(version_id) = decode_map(&vers).iter().last().map(|(id, _)| id) {
                return self.get_by_branch_version(
                    key,
                    branch_id,
                    to_verid(&version_id),
                );
            }
        }
        None
    }

    #[inline(always)]
    pub(super) fn get_by_branch_version(
        &self,
        key: &[u8],
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Option<RawValue> {
        let vers = decode_map(&self.branch_to_its_versions.get(&branch_id)?);

        decode_map(&self.layered_kv.get(key)?)
            .range(..=Cow::Borrowed(&version_id[..]))
            .rev()
            .find(|(ver, _)| vers.contains_key(ver))
            .and_then(|(_, value)| alt!(value.is_empty(), None, Some(value)))
    }

    #[inline(always)]
    pub(super) fn get_ge(&self, key: &[u8]) -> Option<(RawKey, RawValue)> {
        self.range(Cow::Borrowed(key)..).next()
    }

    #[inline(always)]
    pub(super) fn get_ge_by_branch(
        &self,
        key: &[u8],
        branch_id: BranchID,
    ) -> Option<(RawKey, RawValue)> {
        self.range_by_branch(branch_id, Cow::Borrowed(key)..).next()
    }

    #[inline(always)]
    pub(super) fn get_ge_by_branch_version(
        &self,
        key: &[u8],
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Option<(RawKey, RawValue)> {
        self.range_by_branch_version(branch_id, version_id, Cow::Borrowed(key)..)
            .next()
    }

    #[inline(always)]
    pub(super) fn get_le(&self, key: &[u8]) -> Option<(RawKey, RawValue)> {
        self.range(..=Cow::Borrowed(key)).next_back()
    }

    #[inline(always)]
    pub(super) fn get_le_by_branch(
        &self,
        key: &[u8],
        branch_id: BranchID,
    ) -> Option<(RawKey, RawValue)> {
        self.range_by_branch(branch_id, ..=Cow::Borrowed(key))
            .next_back()
    }

    #[inline(always)]
    pub(super) fn get_le_by_branch_version(
        &self,
        key: &[u8],
        branch_id: BranchID,
        version_id: VersionID,
    ) -> Option<(RawKey, RawValue)> {
        self.range_by_branch_version(branch_id, version_id, ..=Cow::Borrowed(key))
            .next_back()
    }

    #[inline(always)]
    pub(super) fn iter(&self) -> MapxRawVsIter {
        self.iter_by_branch(self.branch_get_default())
    }

    #[inline(always)]
    pub(super) fn iter_by_branch(&self, branch_id: BranchID) -> MapxRawVsIter {
        if let Some(vers) = self.branch_to_its_versions.get(&branch_id) {
            if let Some((version_id, _)) = decode_map(&vers).iter().last() {
                return self
                    .iter_by_branch_version(to_brid(&branch_id), to_verid(&version_id));
            }
        }

        MapxRawVsIter {
            hdr: self,
            iter: self.layered_kv.iter(),
            branch_id: NULL,
            version_id: NULL,
        }
    }

    #[inline(always)]
    pub(super) fn iter_by_branch_version(
        &self,
        branch_id: BranchID,
        version_id: VersionID,
    ) -> MapxRawVsIter {
        MapxRawVsIter {
            hdr: self,
            iter: self.layered_kv.iter(),
            branch_id,
            version_id,
        }
    }

    #[inline(always)]
    pub(super) fn range<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        self.range_by_branch(self.branch_get_default(), bounds)
    }

    #[inline(always)]
    pub(super) fn range_by_branch<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        branch_id: BranchID,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        if let Some(vers) = self.branch_to_its_versions.get(&branch_id) {
            if let Some((version_id, _)) = decode_map(&vers).iter().last() {
                return self.range_by_branch_version(
                    branch_id,
                    to_verid(&version_id),
                    bounds,
                );
            }
        }

        // An empty `Iter`
        MapxRawVsIter {
            hdr: self,
            iter: self.layered_kv.iter(),
            branch_id,
            version_id: NULL,
        }
    }

    #[inline(always)]
    pub(super) fn range_by_branch_version<'a, R: RangeBounds<Cow<'a, [u8]>>>(
        &'a self,
        branch_id: BranchID,
        version_id: VersionID,
        bounds: R,
    ) -> MapxRawVsIter<'a> {
        MapxRawVsIter {
            hdr: self,
            iter: self.layered_kv.range(bounds),
            branch_id,
            version_id,
        }
    }

    // NOTE: just a stupid O(n) counter, very slow!
    #[inline(always)]
    pub(super) fn len(&self) -> usize {
        self.iter().count()
    }

    // NOTE: just a stupid O(n) counter, very slow!
    #[inline(always)]
    pub(super) fn len_by_branch(&self, branch_id: BranchID) -> usize {
        self.iter_by_branch(branch_id).count()
    }

    // NOTE: just a stupid O(n) counter, very slow!
    #[inline(always)]
    pub(super) fn len_by_branch_version(
        &self,
        branch_id: BranchID,
        version_id: VersionID,
    ) -> usize {
        self.iter_by_branch_version(branch_id, version_id).count()
    }

    // Clear all data, for testing purpose.
    #[inline(always)]
    pub(super) fn clear(&mut self) {
        self.branch_name_to_branch_id.clear();
        self.version_name_to_version_id.clear();
        self.branch_id_to_branch_name.clear();
        self.version_id_to_version_name.clear();
        self.branch_to_its_versions.clear();
        self.version_to_change_set.clear();
        self.layered_kv.clear();

        self.init();
    }

    #[inline(always)]
    pub(super) fn version_create(&mut self, version_name: &[u8]) -> Result<()> {
        self.version_create_by_branch(version_name, self.branch_get_default())
            .c(d!())
    }

    pub(super) fn version_create_by_branch(
        &mut self,
        version_name: &[u8],
        branch_id: BranchID,
    ) -> Result<()> {
        if self.version_name_to_version_id.get(version_name).is_some() {
            return Err(eg!("version already exists"));
        }

        let mut vers = decode_map(
            &self
                .branch_to_its_versions
                .get_mut(&branch_id)
                .c(d!("branch not found"))?,
        );

        let version_id = VSDB.alloc_version_id().to_be_bytes();
        vers.insert(&version_id, &[]);

        self.version_name_to_version_id
            .insert(version_name, &version_id);
        self.version_id_to_version_name
            .insert(&version_id, version_name);
        self.version_to_change_set
            .insert(&version_id, &encode(&MapxRaw::new()));

        Ok(())
    }

    // Check if a verison exists on the default branch
    #[inline(always)]
    pub(super) fn version_exists(&self, version_id: BranchID) -> bool {
        self.version_exists_on_branch(version_id, self.branch_get_default())
    }

    // Check if a verison exists in the global scope
    #[inline(always)]
    pub(super) fn version_exists_globally(&self, version_id: BranchID) -> bool {
        self.version_to_change_set.contains_key(&version_id)
    }

    // Check if a version exists on a specified branch
    #[inline(always)]
    pub(super) fn version_exists_on_branch(
        &self,
        version_id: VersionID,
        branch_id: BranchID,
    ) -> bool {
        self.branch_to_its_versions
            .get(&branch_id)
            .map(|vers| decode_map(&vers).contains_key(&version_id))
            .unwrap_or(false)
    }

    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    #[inline(always)]
    pub(super) fn version_pop(&mut self) -> Result<()> {
        self.version_pop_by_branch(self.branch_get_default())
            .c(d!())
    }

    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    #[inline(always)]
    pub(super) fn version_pop_by_branch(&mut self, branch_id: BranchID) -> Result<()> {
        let mut vers = decode_map(
            &self
                .branch_to_its_versions
                .get(&branch_id)
                .c(d!("branch not found"))?,
        );

        if let Some((version_id, _)) = vers.iter().next_back() {
            vers.remove(&version_id)
                .c(d!("BUG: version is not on this branch"))?;
        }

        Ok(())
    }

    // # Safety
    //
    // It's the caller's duty to ensure that
    // the `base_version` was created directly by the `branch_id`,
    // or the data records of other branches may be corrupted.
    #[inline(always)]
    pub(super) unsafe fn version_rebase(
        &mut self,
        base_version: VersionID,
    ) -> Result<()> {
        self.version_rebase_by_branch(base_version, self.branch_get_default())
            .c(d!())
    }

    // # Safety
    //
    // It's the caller's duty to ensure that
    // the `base_version` was created directly by the `branch_id`,
    // or the data records of other branches may be corrupted.
    pub(super) unsafe fn version_rebase_by_branch(
        &mut self,
        base_version: VersionID,
        branch_id: BranchID,
    ) -> Result<()> {
        let mut vers_hdr = decode_map(
            &self
                .branch_to_its_versions
                .get(&branch_id)
                .c(d!("branch not found"))?,
        );
        let mut vers = vers_hdr
            .range(Cow::Borrowed(&base_version[..])..)
            .map(|(ver, _)| ver);

        if let Some(ver) = vers.next() {
            if base_version[..] != ver[..] {
                return Err(eg!("base version is not on this branch"));
            }
        } else {
            return Err(eg!("base version is not on this branch"));
        };

        let mut base_ver_chg_set =
            decode_map(&self.version_to_change_set.get(&base_version).c(d!())?);
        let vers_to_be_merged = vers.collect::<Vec<_>>();

        for verid in vers_to_be_merged.iter() {
            // we do not call `clear()` on the discarded instance for performance reason.
            let chgset = decode_map(&self.version_to_change_set.remove(verid).c(d!())?);
            for (k, _) in chgset.iter() {
                base_ver_chg_set.insert(&k, &[]);
                self.layered_kv.get(&k).c(d!()).and_then(|hdr| {
                    let mut hdr = decode_map(&hdr);
                    hdr.remove(verid)
                        .c(d!())
                        .map(|v| hdr.insert(&base_version, &v))
                })?;
            }

            self.version_id_to_version_name
                .remove(verid)
                .c(d!())
                .and_then(|vername| {
                    self.version_name_to_version_id.remove(&vername).c(d!())
                })
                .and_then(|_| vers_hdr.remove(verid).c(d!()))?;
        }

        Ok(())
    }

    #[inline(always)]
    pub(super) fn version_get_id_by_name(
        &self,
        version_name: VersionName,
    ) -> Option<VersionID> {
        self.version_name_to_version_id
            .get(version_name.0)
            .map(|bytes| to_verid(&bytes))
    }

    #[inline(always)]
    pub(super) fn version_list(&self) -> Result<Vec<VersionNameOwned>> {
        self.version_list_by_branch(self.branch_get_default())
    }

    #[inline(always)]
    pub(super) fn version_list_by_branch(
        &self,
        branch_id: BranchID,
    ) -> Result<Vec<VersionNameOwned>> {
        self.branch_to_its_versions
            .get(&branch_id)
            .c(d!())
            .map(|vers| {
                decode_map(&vers)
                    .iter()
                    .map(|(ver, _)| {
                        self.version_id_to_version_name.get(&ver).unwrap().to_vec()
                    })
                    .map(VersionNameOwned)
                    .collect()
            })
    }

    #[inline(always)]
    pub(super) fn version_list_globally(&self) -> Vec<VersionNameOwned> {
        self.version_to_change_set
            .iter()
            .map(|(ver, _)| self.version_id_to_version_name.get(&ver).unwrap().to_vec())
            .map(VersionNameOwned)
            .collect()
    }

    #[inline(always)]
    pub(super) fn version_has_change_set(&self, version_id: VersionID) -> Result<bool> {
        self.version_to_change_set
            .get(&version_id)
            .c(d!())
            .map(|chgset| !chgset.is_empty())
    }

    // # Safety
    //
    // Version itself and its corresponding changes will be completely purged from all branches
    pub(super) unsafe fn version_revert_globally(
        &mut self,
        version_id: VersionID,
    ) -> Result<()> {
        let chgset = self.version_to_change_set.remove(&version_id).c(d!())?;
        for (key, _) in decode_map(&chgset).iter() {
            decode_map(&self.layered_kv.get(&key).c(d!())?)
                .remove(&version_id)
                .c(d!())?;
        }

        self.branch_to_its_versions.iter().for_each(|(_, vers)| {
            decode_map(&vers).remove(&version_id);
        });

        self.version_id_to_version_name
            .remove(&version_id)
            .c(d!())
            .and_then(|vername| self.version_name_to_version_id.remove(&vername).c(d!()))
            .map(|_| ())
    }

    // clean up all orphaned versions in the global scope
    pub(super) fn version_clean_up_globally(&mut self) -> Result<()> {
        let mut valid_vers = HashSet::new();
        self.branch_to_its_versions.iter().for_each(|(_, vers)| {
            decode_map(&vers).iter().for_each(|(ver, _)| {
                valid_vers.insert(ver);
            })
        });

        for (ver, chgset) in unsafe { self.version_to_change_set.shadow() }
            .iter()
            .filter(|(ver, _)| !valid_vers.contains(ver))
        {
            for (k, _) in decode_map(&chgset).iter() {
                decode_map(&self.layered_kv.get(&k).c(d!())?)
                    .remove(&ver)
                    .c(d!())?;
            }
            self.version_id_to_version_name
                .remove(&ver)
                .c(d!())
                .and_then(|vername| {
                    self.version_name_to_version_id.remove(&vername).c(d!())
                })
                .and_then(|_| self.version_to_change_set.remove(&ver).c(d!()))?;
        }

        Ok(())
    }

    #[inline(always)]
    pub(super) fn branch_create(
        &mut self,
        branch_name: &[u8],
        version_name: &[u8],
        force: bool,
    ) -> Result<()> {
        self.branch_create_by_base_branch(
            branch_name,
            version_name,
            self.branch_get_default(),
            force,
        )
        .c(d!())
    }

    #[inline(always)]
    pub(super) fn branch_create_by_base_branch(
        &mut self,
        branch_name: &[u8],
        version_name: &[u8],
        base_branch_id: BranchID,
        force: bool,
    ) -> Result<()> {
        if self.version_name_to_version_id.contains_key(version_name) {
            return Err(eg!("this version already exists"));
        }

        let base_version_id = decode_map(
            &self
                .branch_to_its_versions
                .get(&base_branch_id)
                .c(d!("base branch not found"))?,
        )
        .iter()
        .last()
        .map(|(version_id, _)| version_id);

        unsafe {
            self.do_branch_create_by_base_branch_version(
                branch_name,
                Some(version_name),
                base_branch_id,
                base_version_id.map(|bytes| to_verid(&bytes)),
                force,
            )
            .c(d!())
        }
    }

    #[inline(always)]
    pub(super) fn branch_create_by_base_branch_version(
        &mut self,
        branch_name: &[u8],
        version_name: &[u8],
        base_branch_id: BranchID,
        base_version_id: VersionID,
        force: bool,
    ) -> Result<()> {
        if self.version_name_to_version_id.contains_key(version_name) {
            return Err(eg!("this version already exists"));
        }

        unsafe {
            self.do_branch_create_by_base_branch_version(
                branch_name,
                Some(version_name),
                base_branch_id,
                Some(base_version_id),
                force,
            )
            .c(d!())
        }
    }

    #[inline(always)]
    pub(super) unsafe fn branch_create_without_new_version(
        &mut self,
        branch_name: &[u8],
        force: bool,
    ) -> Result<()> {
        self.branch_create_by_base_branch_without_new_version(
            branch_name,
            self.branch_get_default(),
            force,
        )
        .c(d!())
    }

    #[inline(always)]
    pub(super) unsafe fn branch_create_by_base_branch_without_new_version(
        &mut self,
        branch_name: &[u8],
        base_branch_id: BranchID,
        force: bool,
    ) -> Result<()> {
        let base_version_id = decode_map(
            &self
                .branch_to_its_versions
                .get(&base_branch_id)
                .c(d!("base branch not found"))?,
        )
        .iter()
        .last()
        .map(|(version_id, _)| version_id);

        self.do_branch_create_by_base_branch_version(
            branch_name,
            None,
            base_branch_id,
            base_version_id.map(|bytes| to_verid(&bytes)),
            force,
        )
        .c(d!())
    }

    #[inline(always)]
    pub(super) unsafe fn branch_create_by_base_branch_version_without_new_version(
        &mut self,
        branch_name: &[u8],
        base_branch_id: BranchID,
        base_version_id: VersionID,
        force: bool,
    ) -> Result<()> {
        self.do_branch_create_by_base_branch_version(
            branch_name,
            None,
            base_branch_id,
            Some(base_version_id),
            force,
        )
        .c(d!())
    }

    // param 'force':
    // remove the target new branch if it exists
    unsafe fn do_branch_create_by_base_branch_version(
        &mut self,
        branch_name: &[u8],
        version_name: Option<&[u8]>,
        base_branch_id: BranchID,
        base_version_id: Option<VersionID>,
        force: bool,
    ) -> Result<()> {
        if force {
            if let Some(brid) = self.branch_name_to_branch_id.get(branch_name) {
                self.branch_remove(to_brid(&brid)).c(d!())?;
            }
        }

        if self.branch_name_to_branch_id.contains_key(branch_name) {
            return Err(eg!("branch already exists"));
        }

        let vers = decode_map(
            &self
                .branch_to_its_versions
                .get(&base_branch_id)
                .c(d!("base branch not exist"))?,
        );

        let vers_copied = if let Some(bv) = base_version_id {
            if !vers.contains_key(&bv) {
                return Err(eg!("version is not on the base branch"));
            }
            vers.range(..=Cow::Borrowed(&bv[..])).fold(
                MapxRaw::new(),
                |mut acc, (k, v)| {
                    acc.insert(&k, &v);
                    acc
                },
            )
        } else {
            MapxRaw::new()
        };

        let branch_id = VSDB.alloc_branch_id().to_be_bytes();

        self.branch_name_to_branch_id
            .insert(branch_name, &branch_id);
        self.branch_id_to_branch_name
            .insert(&branch_id, branch_name);
        self.branch_to_its_versions
            .insert(&branch_id, &encode(&vers_copied));

        if let Some(vername) = version_name {
            // create the first version of the new branch
            self.version_create_by_branch(vername, branch_id).c(d!())?;
        }

        Ok(())
    }

    // Check if a branch exists or not.
    #[inline(always)]
    pub(super) fn branch_exists(&self, branch_id: BranchID) -> bool {
        self.branch_id_to_branch_name.contains_key(&branch_id)
    }

    // Check if a branch exists and has versions on it.
    #[inline(always)]
    pub(super) fn branch_has_versions(&self, branch_id: BranchID) -> bool {
        self.branch_exists(branch_id)
            && self
                .branch_to_its_versions
                .get(&branch_id)
                .map(|vers| !decode_map(&vers).is_empty())
                .unwrap_or(false)
    }

    // Remove all changes directly made by this branch, and delete the branch itself.
    //
    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    #[inline(always)]
    pub(super) fn branch_remove(&mut self, branch_id: BranchID) -> Result<()> {
        // if self.branch_get_default() == branch_id {
        //     return Err(eg!("the default branch can NOT be removed"));
        // }

        self.branch_truncate(branch_id).c(d!())?;

        self.branch_id_to_branch_name
            .remove(&branch_id)
            .c(d!())
            .and_then(|brname| self.branch_name_to_branch_id.remove(&brname).c(d!()))?;

        self.branch_to_its_versions
            .remove(&branch_id)
            .c(d!())
            .map(|_| ())
    }

    #[inline(always)]
    pub(super) fn branch_keep_only(&mut self, branch_ids: &[BranchID]) -> Result<()> {
        for brid in unsafe { self.branch_id_to_branch_name.shadow() }
            .iter()
            .map(|(brid, _)| brid)
            .filter(|brid| !branch_ids.contains(&to_brid(brid)))
        {
            self.branch_remove(to_brid(&brid)).c(d!())?;
        }
        self.version_clean_up_globally().c(d!())
    }

    // Remove all changes directly made by this branch, but keep its meta infomation.
    //
    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    #[inline(always)]
    pub(super) fn branch_truncate(&mut self, branch_id: BranchID) -> Result<()> {
        if let Some(vers) = self.branch_to_its_versions.get(&branch_id) {
            decode_map(&vers).clear();
            Ok(())
        } else {
            Err(eg!(
                "branch not found: {}",
                BranchIDBase::from_be_bytes(branch_id)
            ))
        }
    }

    // Remove all changes directly made by versions(bigger than `last_version_id`) of this branch.
    //
    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    pub(super) fn branch_truncate_to(
        &mut self,
        branch_id: BranchID,
        last_version_id: VersionID,
    ) -> Result<()> {
        if let Some(vers) = self.branch_to_its_versions.get(&branch_id) {
            // version id must be in descending order
            let mut vers = decode_map(&vers);
            let vers_shadow = unsafe { vers.shadow() };
            for (version_id, _) in vers_shadow
                .range(
                    Cow::Borrowed(
                        &(VersionIDBase::from_be_bytes(last_version_id) + 1)
                            .to_be_bytes()[..],
                    )..,
                )
                .rev()
            {
                vers.remove(&version_id)
                    .c(d!("version is not on this branch"))?;
            }
            Ok(())
        } else {
            Err(eg!(
                "branch not found: {}",
                BranchIDBase::from_be_bytes(branch_id)
            ))
        }
    }

    // 'Write'-like operations on branches and versions are different from operations on data.
    //
    // 'Write'-like operations on data require recursive tracing of all parent nodes,
    // while operations on branches and versions are limited to their own perspective,
    // and should not do any tracing.
    #[inline(always)]
    pub(super) fn branch_pop_version(&mut self, branch_id: BranchID) -> Result<()> {
        self.version_pop_by_branch(branch_id).c(d!())
    }

    #[inline(always)]
    pub(super) fn branch_merge_to(
        &mut self,
        branch_id: BranchID,
        target_branch_id: BranchID,
    ) -> Result<()> {
        unsafe { self.do_branch_merge_to(branch_id, target_branch_id, false) }
    }

    // # Safety
    //
    // If new different versions have been created on the target branch,
    // the data records referenced by other branches may be corrupted.
    #[inline(always)]
    pub(super) unsafe fn branch_merge_to_force(
        &mut self,
        branch_id: BranchID,
        target_branch_id: BranchID,
    ) -> Result<()> {
        self.do_branch_merge_to(branch_id, target_branch_id, true)
    }

    // Merge a branch into another,
    // even if new different versions have been created on the target branch.
    //
    // # Safety
    //
    // If new different versions have been created on the target branch,
    // the data records referenced by other branches may be corrupted.
    unsafe fn do_branch_merge_to(
        &mut self,
        branch_id: BranchID,
        target_branch_id: BranchID,
        force: bool,
    ) -> Result<()> {
        let vers = decode_map(
            &self
                .branch_to_its_versions
                .get(&branch_id)
                .c(d!("branch not found"))?,
        );
        let mut target_vers = decode_map(
            &self
                .branch_to_its_versions
                .get(&target_branch_id)
                .c(d!("target branch not found"))?,
        );

        if !force {
            if let Some((ver, _)) = target_vers.iter().last() {
                if !vers.contains_key(&ver) {
                    // Some new versions have been generated on the target branch
                    return Err(eg!("unable to merge safely"));
                }
            }
        }

        if let Some(fork_point) = vers
            .iter()
            .zip(target_vers.iter())
            .find(|(a, b)| a.0 != b.0)
        {
            vers.range(Cow::Borrowed(&fork_point.0.0[..])..)
                .for_each(|(ver, _)| {
                    target_vers.insert(&ver, &[]);
                });
        } else if let Some((latest_ver, _)) = vers.iter().last() {
            if let Some((target_latest_ver, _)) = target_vers.iter().last() {
                match latest_ver.cmp(&target_latest_ver) {
                    Ordering::Equal => {
                        // no differences between the two branches
                        return Ok(());
                    }
                    Ordering::Greater => {
                        vers.range(
                            Cow::Borrowed(
                                &(VersionIDBase::from_be_bytes(to_verid(
                                    &target_latest_ver,
                                )) + 1)
                                    .to_be_bytes()[..],
                            )..,
                        )
                        .map(|(ver, _)| ver)
                        .for_each(|ver| {
                            target_vers.insert(&ver, &[]);
                        });
                    }
                    _ => {}
                }
            } else {
                // target branch is empty, copy all versions to it
                vers.iter().for_each(|(ver, _)| {
                    target_vers.insert(&ver, &[]);
                });
            }
        } else {
            // nothing to be merges
            return Ok(());
        };

        Ok(())
    }

    #[inline(always)]
    pub(super) fn branch_set_default(&mut self, branch_id: BranchID) -> Result<()> {
        if !self.branch_exists(branch_id) {
            return Err(eg!("branch not found"));
        }
        self.default_branch = branch_id;
        Ok(())
    }

    #[inline(always)]
    pub(super) fn branch_get_default(&self) -> BranchID {
        self.default_branch
    }

    #[inline(always)]
    pub(super) fn branch_get_default_name(&self) -> BranchNameOwned {
        self.branch_id_to_branch_name
            .get(&self.default_branch)
            .map(|br| BranchNameOwned(br.to_vec()))
            .unwrap()
    }

    #[inline(always)]
    pub(super) fn branch_is_empty(&self, branch_id: BranchID) -> Result<bool> {
        self.branch_to_its_versions
            .get(&branch_id)
            .c(d!())
            .map(|vers| {
                decode_map(&vers).iter().all(|(ver, _)| {
                    !self.version_has_change_set(to_verid(&ver)).unwrap()
                })
            })
    }

    #[inline(always)]
    pub(super) fn branch_list(&self) -> Vec<BranchNameOwned> {
        self.branch_name_to_branch_id
            .iter()
            .map(|(brname, _)| brname.to_vec())
            .map(BranchNameOwned)
            .collect()
    }

    // Logically similar to `std::ptr::swap`
    //
    // For example: If you have a master branch and a test branch, the data is always trial-run on the test branch, and then periodically merged back into the master branch. Rather than merging the test branch into the master branch, and then recreating the new test branch, it is more efficient to just swap the two branches, and then recreating the new test branch.
    //
    // # Safety
    //
    // - Non-'thread safe'
    // - Must ensure that there are no reads and writes to these two branches during the execution
    pub(super) unsafe fn branch_swap(
        &mut self,
        branch_1: &[u8],
        branch_2: &[u8],
    ) -> Result<()> {
        let brid_1 = to_brid(&self.branch_name_to_branch_id.get(branch_1).c(d!())?);
        let brid_2 = to_brid(&self.branch_name_to_branch_id.get(branch_2).c(d!())?);

        self.branch_name_to_branch_id
            .insert(branch_1, &brid_2)
            .c(d!())?;
        self.branch_name_to_branch_id
            .insert(branch_2, &brid_1)
            .c(d!())?;

        self.branch_id_to_branch_name
            .insert(&brid_1, branch_2)
            .c(d!())?;
        self.branch_id_to_branch_name
            .insert(&brid_2, branch_1)
            .c(d!())?;

        if self.default_branch == brid_1 {
            self.default_branch = brid_2;
        } else if self.default_branch == brid_2 {
            self.default_branch = brid_1;
        }

        Ok(())
    }

    #[inline(always)]
    pub(super) fn branch_get_id_by_name(
        &self,
        branch_name: BranchName,
    ) -> Option<BranchID> {
        self.branch_name_to_branch_id
            .get(branch_name.0)
            .map(|bytes| to_brid(&bytes))
    }

    #[inline(always)]
    pub(super) fn prune(&mut self, reserved_ver_num: Option<usize>) -> Result<()> {
        self.version_clean_up_globally().c(d!())?;

        let reserved_ver_num = reserved_ver_num.unwrap_or(RESERVED_VERSION_NUM_DEFAULT);
        if 0 == reserved_ver_num {
            return Err(eg!("reserved version number should NOT be zero"));
        }

        let br_vers = self
            .branch_to_its_versions
            .iter()
            .map(|(_, vers)| decode_map(&vers))
            .filter(|vers| !vers.is_empty())
            .collect::<Vec<_>>();
        alt!(br_vers.is_empty(), return Ok(()));
        let mut br_vers = (0..br_vers.len())
            .map(|i| (&br_vers[i]).iter())
            .collect::<Vec<_>>();

        // filter out the longest common prefix
        let mut guard = Default::default();
        let mut vers_to_be_merged: Vec<VersionID> = vec![];
        'x: loop {
            for (idx, vers) in br_vers.iter_mut().enumerate() {
                if let Some((ver, _)) = vers.next() {
                    alt!(0 == idx, guard = to_verid(&ver));
                    alt!(guard[..] != ver[..], break 'x);
                } else {
                    break 'x;
                }
            }
            vers_to_be_merged.push(to_verid(&guard));
        }

        let (vers_to_be_merged, rewrite_ver) = {
            let l = vers_to_be_merged.len();
            if l > reserved_ver_num {
                let guard_idx = l - reserved_ver_num;
                (
                    &vers_to_be_merged[..guard_idx],
                    &vers_to_be_merged[guard_idx],
                )
            } else {
                return Ok(());
            }
        };

        let mut rewrite_ver_chgset =
            decode_map(&self.version_to_change_set.get(rewrite_ver).c(d!())?);

        for (_, vers) in self
            .branch_to_its_versions
            .iter()
            .filter(|(_, vers)| !decode_map(vers).is_empty())
        {
            for ver in vers_to_be_merged.iter() {
                decode_map(&vers).remove(ver).c(d!())?;
            }
        }

        for ver in vers_to_be_merged.iter() {
            self.version_id_to_version_name
                .remove(ver)
                .c(d!())
                .and_then(|vername| {
                    self.version_name_to_version_id.remove(&vername).c(d!())
                })?;
            let chgset = decode_map(&self.version_to_change_set.remove(ver).c(d!())?);
            for (k, _) in chgset.iter() {
                let mut k_vers = decode_map(&self.layered_kv.get(&k).c(d!())?);
                let value = k_vers.remove(ver).c(d!())?;

                // keep at least one version
                if k_vers
                    .range(..=Cow::Borrowed(&rewrite_ver[..]))
                    .next()
                    .is_none()
                {
                    assert!(rewrite_ver_chgset.insert(&k, &[]).is_none());
                    assert!(k_vers.insert(&rewrite_ver[..], &value).is_none());
                }
            }
        }

        Ok(())
    }
}

impl Default for MapxRawVs {
    fn default() -> Self {
        Self::new()
    }
}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

pub struct MapxRawVsIter<'a> {
    hdr: &'a MapxRawVs,
    iter: MapxRawIter<'a>, // <MapxOrd<VersionID, Option<RawValue>>>,
    branch_id: BranchID,
    version_id: VersionID,
}

impl<'a> Iterator for MapxRawVsIter<'a> {
    type Item = (RawKey, RawValue);

    #[allow(clippy::while_let_on_iterator)]
    fn next(&mut self) -> Option<Self::Item> {
        if NULL == self.branch_id || NULL == self.version_id {
            return None;
        }

        while let Some((k, _)) = self.iter.next() {
            if let Some(v) =
                self.hdr
                    .get_by_branch_version(&k, self.branch_id, self.version_id)
            {
                return Some((k, v));
            }
        }

        None
    }
}

impl DoubleEndedIterator for MapxRawVsIter<'_> {
    #[allow(clippy::while_let_on_iterator)]
    fn next_back(&mut self) -> Option<Self::Item> {
        if NULL == self.branch_id || NULL == self.version_id {
            return None;
        }

        while let Some((k, _)) = self.iter.next_back() {
            if let Some(v) =
                self.hdr
                    .get_by_branch_version(&k, self.branch_id, self.version_id)
            {
                return Some((k, v));
            }
        }

        None
    }
}

impl ExactSizeIterator for MapxRawVsIter<'_> {}

////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////

#[inline(always)]
fn encode(t: &impl Serialize) -> Vec<u8> {
    pnk!(msgpack::to_vec(t))
}

#[inline(always)]
fn decode<'a, T: Deserialize<'a>>(t: &'a [u8]) -> T {
    pnk!(msgpack::from_slice(t))
}

#[inline(always)]
fn decode_map(t: &[u8]) -> MapxRaw {
    decode(t)
}

#[inline(always)]
fn to_brid(bytes: &[u8]) -> BranchID {
    <[u8; size_of::<BranchID>()]>::try_from(bytes).unwrap()
}

#[inline(always)]
fn to_verid(bytes: &[u8]) -> VersionID {
    <[u8; size_of::<VersionID>()]>::try_from(bytes).unwrap()
}
