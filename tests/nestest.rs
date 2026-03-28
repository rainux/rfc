use std::fs;

#[test]
fn test_load_nestest() {
    let data = fs::read("roms/nestest.nes").expect("nestest.nes not found in roms/");
    let cart = rfc::cartridge::Cartridge::from_ines(&data).unwrap();
    assert_eq!(cart.mirroring, rfc::cartridge::Mirroring::Horizontal);
}
