use ruc::*;
use vsdb::{basic_multi_key::mapx_raw::MapxRawMk, vsdb_set_base_dir};

const NIL: &[u8] = &[];

#[test]
fn basic_cases() {
    info_omit!(vsdb_set_base_dir(&format!(
        "/tmp/vsdb_testing/{}",
        rand::random::<u64>()
    )));

    let mut map = MapxRawMk::new(4);

    // key size mismatch
    assert!(map.insert(&[&[1]], &[]).is_err());

    assert!(map.insert(&[&[1], &[2], &[3], &[4]], &[9]).is_ok());
    assert!(map.insert(&[&[1], &[2], &[3], &[40]], &[8]).is_ok());
    assert!(map.insert(&[&[1], &[2], &[30], &[40]], &[7]).is_ok());
    assert!(map.insert(&[&[1], &[2], &[30], &[41]], &[6]).is_ok());

    assert_eq!(&map.get(&[&[1], &[2], &[3], &[4]]).unwrap(), &[9]);
    assert_eq!(&map.get(&[&[1], &[2], &[3], &[40]]).unwrap(), &[8]);
    assert_eq!(&map.get(&[&[1], &[2], &[30], &[40]]).unwrap(), &[7]);
    assert_eq!(&map.get(&[&[1], &[2], &[30], &[41]]).unwrap(), &[6]);

    // key size mismatch
    assert!(map.get(&[&[1], &[2], &[3]]).is_none());
    assert!(map.get(&[&[1], &[2]]).is_none());
    assert!(map.get(&[&[1]]).is_none());
    assert!(map.get(&[]).is_none());

    // does not exist
    assert!(map.remove(&[&[1], &[2], &[3], &[200]]).unwrap().is_none());

    assert!(map.remove(&[&[1], &[2], &[3], &[40]]).unwrap().is_some());
    assert!(map.get(&[&[1], &[2], &[3], &[40]]).is_none());

    // partial-path remove
    assert!(map.remove(&[&[1], &[2], &[30]]).unwrap().is_none()); // yes, is none
    assert!(map.get(&[&[1], &[2], &[30], &[40]]).is_none());
    assert!(map.get(&[&[1], &[2], &[30], &[41]]).is_none());

    // nothing will be removed by an empty key
    assert!(map.remove(&[]).unwrap().is_none());

    assert!(map.get(&[&[1], &[2], &[3], &[4]]).is_some());
    assert!(map.remove(&[&[1]]).unwrap().is_none()); // yes, is none
    assert!(map.get(&[&[1], &[2], &[3], &[4]]).is_none());

    assert!(map.entry(&[]).or_insert(&[]).is_err());
    assert!(
        map.entry(&[&[11], &[12], &[13], &[14]])
            .or_insert(&[])
            .is_ok()
    );
    assert_eq!(&map.get(&[&[11], &[12], &[13], &[14]]).unwrap(), NIL);

    let mut cnt = 0;
    let mut op = |k: &[&[u8]], v: &[u8]| {
        cnt += 1;
        assert_eq!(k, &[&[11], &[12], &[13], &[14]]);
        assert_eq!(v, NIL);
        Ok(())
    };

    pnk!(map.iter_op(&mut op));
    assert_eq!(cnt, 1);

    assert!(
        map.entry(&[&[11], &[12], &[13], &[15]])
            .or_insert(&[0])
            .is_ok()
    );
    assert_eq!(&map.get(&[&[11], &[12], &[13], &[15]]).unwrap(), &[0]);

    let mut cnt = 0;
    let mut op = |k: &[&[u8]], v: &[u8]| {
        cnt += 1;
        if v == NIL {
            assert_eq!(k, &[&[11], &[12], &[13], &[14]]);
        } else {
            assert_eq!(k, &[&[11], &[12], &[13], &[15]]);
        }
        Ok(())
    };

    // cnt += 2
    pnk!(map.iter_op(&mut op));
    // cnt += 2
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[11]]));
    // cnt += 2
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[11], &[12]]));
    // cnt += 2
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[11], &[12], &[13]]));
    // cnt += 1
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[11], &[12], &[13], &[14]]));
    // cnt += 1
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[11], &[12], &[13], &[15]]));

    // cnt += 0
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[111]]));
    // cnt += 0
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[111], &[12]]));
    // cnt += 0
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[111], &[12], &[13]]));
    // cnt += 0
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[111], &[12], &[13], &[14]]));
    // cnt += 0
    pnk!(map.iter_op_with_key_prefix(&mut op, &[&[111], &[12], &[13], &[15]]));

    drop(op);
    assert_eq!(cnt, 10);
}
