use std::fs;

/// Parse one line of nestest.log to extract PC, A, X, Y, P, SP, CYC
fn parse_log_line(line: &str) -> (u16, u8, u8, u8, u8, u8, u64) {
    let pc = u16::from_str_radix(&line[0..4], 16).unwrap();
    let a_pos = line.find("A:").unwrap();
    let a = u8::from_str_radix(&line[a_pos + 2..a_pos + 4], 16).unwrap();
    let x_pos = line.find("X:").unwrap();
    let x = u8::from_str_radix(&line[x_pos + 2..x_pos + 4], 16).unwrap();
    let y_pos = line.find("Y:").unwrap();
    let y = u8::from_str_radix(&line[y_pos + 2..y_pos + 4], 16).unwrap();
    let p_pos = line.find("P:").unwrap();
    let p = u8::from_str_radix(&line[p_pos + 2..p_pos + 4], 16).unwrap();
    let sp_pos = line.find("SP:").unwrap();
    let sp = u8::from_str_radix(&line[sp_pos + 3..sp_pos + 5], 16).unwrap();
    let cyc_pos = line.find("CYC:").unwrap();
    let cyc_str = line[cyc_pos + 4..].trim();
    let cyc = cyc_str.parse::<u64>().unwrap();
    (pc, a, x, y, p, sp, cyc)
}

#[test]
fn test_nestest_cpu() {
    let rom_data = fs::read("roms/nestest.nes").expect("nestest.nes not found");
    let log_data = fs::read_to_string("roms/nestest.log").expect("nestest.log not found");

    let cart = rfc::cartridge::Cartridge::from_ines(&rom_data).unwrap();
    let mut bus = rfc::bus::Bus::new();
    bus.load_cartridge(cart);

    let mut cpu = rfc::cpu::Cpu::new();
    cpu.pc = 0xC000; // nestest automation mode starts at $C000
    cpu.status = 0x24; // nestest expects this initial status
    cpu.cycles = 7;

    let log_lines: Vec<&str> = log_data.lines().collect();

    for (i, expected_line) in log_lines.iter().enumerate() {
        // Stop at unofficial opcodes (marked with * before the mnemonic)
        // In nestest.log, the disassembly area is columns ~15-47, before "A:"
        if expected_line.len() > 15 && expected_line[..48.min(expected_line.len())].contains('*') {
            println!(
                "Stopped at line {} (unofficial opcode territory) - all {} official opcode tests passed!",
                i + 1,
                i
            );
            return;
        }

        let (exp_pc, exp_a, exp_x, exp_y, exp_p, exp_sp, exp_cyc) = parse_log_line(expected_line);

        // Compare state BEFORE executing the instruction
        assert_eq!(
            cpu.pc,
            exp_pc,
            "Line {}: PC mismatch: got 0x{:04X}, expected 0x{:04X}",
            i + 1,
            cpu.pc,
            exp_pc
        );
        assert_eq!(
            cpu.a,
            exp_a,
            "Line {}: A mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1,
            cpu.a,
            exp_a
        );
        assert_eq!(
            cpu.x,
            exp_x,
            "Line {}: X mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1,
            cpu.x,
            exp_x
        );
        assert_eq!(
            cpu.y,
            exp_y,
            "Line {}: Y mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1,
            cpu.y,
            exp_y
        );
        assert_eq!(
            cpu.status,
            exp_p,
            "Line {}: P mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1,
            cpu.status,
            exp_p
        );
        assert_eq!(
            cpu.sp,
            exp_sp,
            "Line {}: SP mismatch: got 0x{:02X}, expected 0x{:02X}",
            i + 1,
            cpu.sp,
            exp_sp
        );
        assert_eq!(
            cpu.cycles,
            exp_cyc,
            "Line {}: CYC mismatch: got {}, expected {}",
            i + 1,
            cpu.cycles,
            exp_cyc
        );

        cpu.step(&mut bus);
    }

    println!("nestest passed all {} lines!", log_lines.len());
}
