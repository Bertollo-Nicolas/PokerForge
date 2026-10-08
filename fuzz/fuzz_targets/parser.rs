#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len() > 65536 {
        return;
    }
    let size = 1 + data.first().copied().unwrap_or(0) as usize;
    let expected = pokerforge::process(data);
    assert_eq!(pokerforge::process_reader(data, size).unwrap().0, expected);
    let mut a = Vec::new();
    let mut b = Vec::new();
    pokerforge::scan(data, |o, h| a.push((o as u64, h)));
    pokerforge::scan_reader(data, size, |o, h| b.push((o, h))).unwrap();
    assert_eq!(a, b);
});
