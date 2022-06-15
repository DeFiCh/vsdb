use super::*;
use ruc::*;

#[test]
fn test_insert() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max)
        .map(|i: usize| (i, (max + i)))
        .for_each(|(key, value)| {
            assert!(hdr.get(key).is_none());
            hdr.insert(key, &value);
            assert_eq!(pnk!(hdr.get(key)), value);
        });
    hdr.clear();
    (0..max).map(|i: usize| i).for_each(|key| {
        assert!(hdr.get(key).is_none());
    });
    assert!(hdr.is_empty());
}
#[test]
fn test_len() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max)
        .map(|i: usize| (i, (max + i)))
        .for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
    assert_eq!(500, hdr.len());
    hdr.clear();
    assert_eq!(0, hdr.len());
}

#[test]
fn test_valueende() {
    let cnt = 500;
    let dehdr = {
        let mut hdr = Vecx::new();
        (0..cnt).map(|i: usize| (i, i)).for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
        <Vecx<usize> as ValueEnDe>::encode(&hdr)
    };
    let mut reloaded = pnk!(<Vecx<usize> as ValueEnDe>::decode(&dehdr));
    assert_eq!(cnt, reloaded.len());
    (0..cnt).map(|i: usize| i).for_each(|i| {
        assert_eq!(i, reloaded.get(i).unwrap());
    });
}

#[test]
fn test_remove() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max)
        .map(|i: usize| (i, (max + i)))
        .for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
    assert_eq!(max, hdr.len());

    let idx = 400;
    assert_eq!(max + idx, hdr.remove(idx));
    hdr.clear();
    assert_eq!(0, hdr.len());
}

#[test]
fn test_iter_next() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max).map(|i: usize| (i, i)).for_each(|(key, value)| {
        hdr.insert(key, &value);
    });
    let value = pnk!(hdr.iter().next());
    assert_eq!(0, value);

    let value = pnk!(hdr.iter().next_back());
    assert_eq!(max - 1, value);
}

#[test]
fn test_push_pop() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max).map(|i: usize| i).for_each(|value| {
        hdr.push(&value);
    });
    for val in (0..max).rev() {
        assert_eq!(val, pnk!(hdr.pop()));
    }
}

#[test]
fn test_swap_remove() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max).map(|i: usize| i).for_each(|value| {
        hdr.push(&value);
    });
    for idx in (0..max - 1).rev() {
        assert_eq!(idx, hdr.swap_remove(idx));
    }
    assert_eq!(1, hdr.len());
    let value = pnk!(hdr.last());
    assert_eq!(max - 1, value);
}

#[test]
fn test_last() {
    let mut hdr = Vecx::new();
    let max = 500;
    (0..max).map(|i: usize| (i, i)).for_each(|(key, value)| {
        hdr.insert(key, &value);
    });
    let value = pnk!(hdr.last());
    assert_eq!(max - 1, value);
}

#[test]
#[should_panic]
fn write_out_of_index_0() {
    let mut hdr = Vecx::new();
    hdr.insert(100, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_1() {
    let mut hdr = Vecx::new();
    hdr.insert(0, &0);
    hdr.insert(100, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_2() {
    let mut hdr = Vecx::new();
    hdr.update(100, &0);
    hdr.insert(0, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_3() {
    let mut hdr = Vecx::new();
    hdr.insert(0, &0);
    hdr.update(100, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_4() {
    let mut hdr = Vecx::new();
    hdr.remove(100);
    hdr.insert(0, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_5() {
    let mut hdr = Vecx::new();
    hdr.insert(0, &0);
    hdr.remove(100);
}

#[test]
#[should_panic]
fn write_out_of_index_6() {
    let mut hdr = Vecx::new();
    hdr.swap_remove(100);
    hdr.insert(0, &0);
}

#[test]
#[should_panic]
fn write_out_of_index_7() {
    let mut hdr = Vecx::new();
    hdr.insert(0, &0);
    hdr.swap_remove(100);
}
