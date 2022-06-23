use super::*;
use crate::ValueEnDe;
use ruc::*;

fn gen_sample(idx: usize) -> Box<[u8]> {
    idx.to_be_bytes().to_vec().into_boxed_slice()
}

#[test]
fn test_insert() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i, <usize as ValueEnDe>::encode(&(max + i))))
        .for_each(|(key, value)| {
            assert!(hdr.get(key).is_none());
            hdr.insert(key, &value);
            let val = pnk!(<usize as ValueEnDe>::decode(&pnk!(hdr.get(key))));
            assert_eq!(max + key, val);
        });
    hdr.clear();
    (0..max).map(|i: usize| i).for_each(|key| {
        assert!(hdr.get(key).is_none());
    });
    assert!(hdr.is_empty());
}
#[test]
fn test_len() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i, (max + i).to_be_bytes()))
        .for_each(|(key, value)| {
            hdr.insert(key, &value[..]);
        });
    assert_eq!(100, hdr.len());
    hdr.clear();
    assert_eq!(0, hdr.len());
}

#[test]
fn test_valueende() {
    let cnt = 100;
    let dehdr = {
        let mut hdr = VecxRaw::new();
        (0..cnt)
            .map(|i: usize| (i, <usize as ValueEnDe>::encode(&i)))
            .for_each(|(key, value)| {
                hdr.insert(key, &value);
            });
        <VecxRaw as ValueEnDe>::encode(&hdr)
    };
    let mut reloaded = pnk!(<VecxRaw as ValueEnDe>::decode(&dehdr));
    assert_eq!(cnt, reloaded.len());
    (0..cnt).map(|i: usize| i).for_each(|i| {
        let val = pnk!(<usize as ValueEnDe>::decode(&pnk!(reloaded.get(i))));
        assert_eq!(i, val);
    });
}

#[test]
fn test_remove() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i, <usize as ValueEnDe>::encode(&(max + i))))
        .for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
    assert_eq!(max, hdr.len());

    let idx = 50;
    let val = pnk!(<usize as ValueEnDe>::decode(&hdr.remove(idx)));
    assert_eq!(max + idx, val);
    hdr.clear();
    assert_eq!(0, hdr.len());
}

#[test]
fn test_iter_next() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i, <usize as ValueEnDe>::encode(&i)))
        .for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
    let value = pnk!(hdr.iter().next());
    let val = pnk!(<usize as ValueEnDe>::decode(&value));
    assert_eq!(0, val);

    let value = pnk!(hdr.iter().next_back());
    let val = pnk!(<usize as ValueEnDe>::decode(&value));
    assert_eq!(max - 1, val);
}

#[test]
fn test_push_pop() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| <usize as ValueEnDe>::encode(&i))
        .for_each(|value| {
            hdr.push(&value);
        });
    for idx in (0..max).rev() {
        let val = pnk!(<usize as ValueEnDe>::decode(&pnk!(hdr.pop())));
        assert_eq!(idx, val);
    }
}

#[test]
fn test_swap_remove() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| <usize as ValueEnDe>::encode(&i))
        .for_each(|value| {
            hdr.push(&value);
        });
    for idx in (0..max - 1).rev() {
        let val = pnk!(<usize as ValueEnDe>::decode(&hdr.swap_remove(idx)));
        assert_eq!(val, idx);
    }
    assert_eq!(1, hdr.len());
    let value = pnk!(hdr.last());
    let val = pnk!(<usize as ValueEnDe>::decode(&value));
    assert_eq!(max - 1, val);
}

#[test]
fn test_last() {
    let mut hdr = VecxRaw::new();
    let max = 100;
    (0..max)
        .map(|i: usize| (i, <usize as ValueEnDe>::encode(&i)))
        .for_each(|(key, value)| {
            hdr.insert(key, &value);
        });
    let value = pnk!(hdr.last());
    let val = pnk!(<usize as ValueEnDe>::decode(&value));
    assert_eq!(max - 1, val);
}

#[test]
#[should_panic]
fn write_out_of_index_0() {
    let mut hdr = VecxRaw::new();
    hdr.insert(100, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_1() {
    let mut hdr = VecxRaw::new();
    hdr.insert(0, &gen_sample(0));
    hdr.insert(100, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_2() {
    let mut hdr = VecxRaw::new();
    hdr.update(100, &gen_sample(0));
    hdr.insert(0, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_3() {
    let mut hdr = VecxRaw::new();
    hdr.insert(0, &gen_sample(0));
    hdr.update(100, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_4() {
    let mut hdr = VecxRaw::new();
    hdr.remove(100);
    hdr.insert(0, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_5() {
    let mut hdr = VecxRaw::new();
    hdr.insert(0, &gen_sample(0));
    hdr.remove(100);
}

#[test]
#[should_panic]
fn write_out_of_index_6() {
    let mut hdr = VecxRaw::new();
    hdr.swap_remove(100);
    hdr.insert(0, &gen_sample(0));
}

#[test]
#[should_panic]
fn write_out_of_index_7() {
    let mut hdr = VecxRaw::new();
    hdr.insert(0, &gen_sample(0));
    hdr.swap_remove(100);
}
