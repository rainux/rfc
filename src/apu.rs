use std::sync::Arc;

use crate::audio::AudioBuffer;

const CPU_FREQUENCY: f64 = 1_789_773.0;
const SAMPLE_RATE: f64 = 44100.0;

const LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const NOISE_PERIODS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068,
];

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0], // 12.5%
    [0, 1, 1, 0, 0, 0, 0, 0], // 25%
    [0, 1, 1, 1, 1, 0, 0, 0], // 50%
    [1, 0, 0, 1, 1, 1, 1, 1], // 75% (inverted 25%)
];

const TRIANGLE_SEQUENCE: [u8; 32] = [
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
];

// Frame counter divider: ~7457.5 CPU cycles per quarter frame
const FRAME_COUNTER_RATE: u16 = 7457;

struct PulseChannel {
    enabled: bool,
    channel: u8, // 1 or 2, affects sweep negate behavior
    duty: u8,
    length_halt: bool,
    constant_volume: bool,
    volume: u8,

    sweep_enabled: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_reload: bool,
    sweep_divider: u8,

    timer: u16,
    timer_counter: u16,
    sequencer_step: u8,

    length_counter: u8,

    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
}

impl PulseChannel {
    fn new(channel: u8) -> Self {
        Self {
            enabled: false,
            channel,
            duty: 0,
            length_halt: false,
            constant_volume: false,
            volume: 0,
            sweep_enabled: false,
            sweep_period: 0,
            sweep_negate: false,
            sweep_shift: 0,
            sweep_reload: false,
            sweep_divider: 0,
            timer: 0,
            timer_counter: 0,
            sequencer_step: 0,
            length_counter: 0,
            envelope_start: false,
            envelope_divider: 0,
            envelope_decay: 0,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer;
            self.sequencer_step = (self.sequencer_step + 1) % 8;
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if self.length_halt {
                self.envelope_decay = 15; // loop
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn sweep_target_period(&self) -> i32 {
        let shift = (self.timer >> self.sweep_shift) as i32;
        let delta = if self.sweep_negate {
            if self.channel == 1 {
                -shift - 1 // Pulse 1 uses one's complement
            } else {
                -shift // Pulse 2 uses two's complement
            }
        } else {
            shift
        };
        self.timer as i32 + delta
    }

    fn clock_sweep(&mut self) {
        let target = self.sweep_target_period();
        if self.sweep_divider == 0 && self.sweep_enabled && self.sweep_shift > 0 {
            if target >= 0 && target <= 0x7FF {
                self.timer = target as u16;
            }
        }
        if self.sweep_divider == 0 || self.sweep_reload {
            self.sweep_divider = self.sweep_period;
            self.sweep_reload = false;
        } else {
            self.sweep_divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled
            || self.length_counter == 0
            || DUTY_TABLE[self.duty as usize][self.sequencer_step as usize] == 0
            || self.timer < 8
            || self.sweep_target_period() > 0x7FF
        {
            return 0;
        }
        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

struct TriangleChannel {
    enabled: bool,
    control: bool,
    linear_load: u8,
    timer: u16,
    timer_counter: u16,
    sequencer_step: u8,
    length_counter: u8,
    linear_counter: u8,
    linear_reload: bool,
}

impl TriangleChannel {
    fn new() -> Self {
        Self {
            enabled: false,
            control: false,
            linear_load: 0,
            timer: 0,
            timer_counter: 0,
            sequencer_step: 0,
            length_counter: 0,
            linear_counter: 0,
            linear_reload: false,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer;
            if self.length_counter > 0 && self.linear_counter > 0 {
                self.sequencer_step = (self.sequencer_step + 1) % 32;
            }
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_linear(&mut self) {
        if self.linear_reload {
            self.linear_counter = self.linear_load;
        } else if self.linear_counter > 0 {
            self.linear_counter -= 1;
        }
        if !self.control {
            self.linear_reload = false;
        }
    }

    fn clock_length(&mut self) {
        if !self.control && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || self.linear_counter == 0 || self.timer < 2 {
            return 0;
        }
        TRIANGLE_SEQUENCE[self.sequencer_step as usize]
    }
}

struct NoiseChannel {
    enabled: bool,
    length_halt: bool,
    constant_volume: bool,
    volume: u8,
    mode: bool,
    timer_period: u16,
    timer_counter: u16,
    shift_register: u16,
    length_counter: u8,
    envelope_start: bool,
    envelope_divider: u8,
    envelope_decay: u8,
}

impl NoiseChannel {
    fn new() -> Self {
        Self {
            enabled: false,
            length_halt: false,
            constant_volume: false,
            volume: 0,
            mode: false,
            timer_period: 4,
            timer_counter: 4,
            shift_register: 1,
            length_counter: 0,
            envelope_start: false,
            envelope_divider: 0,
            envelope_decay: 0,
        }
    }

    fn clock_timer(&mut self) {
        if self.timer_counter == 0 {
            self.timer_counter = self.timer_period;
            let feedback_bit = if self.mode { 6 } else { 1 };
            let feedback = (self.shift_register & 1) ^ ((self.shift_register >> feedback_bit) & 1);
            self.shift_register >>= 1;
            self.shift_register |= feedback << 14;
        } else {
            self.timer_counter -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if self.length_halt {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    fn clock_length(&mut self) {
        if !self.length_halt && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled || self.length_counter == 0 || (self.shift_register & 1) != 0 {
            return 0;
        }
        if self.constant_volume {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

struct FrameCounter {
    mode: u8,
    interrupt_inhibit: bool,
    step: u16,
    divider: u16,
}

impl FrameCounter {
    fn new() -> Self {
        Self {
            mode: 0,
            interrupt_inhibit: true,
            step: 0,
            divider: 0,
        }
    }
}

pub struct Apu {
    pulse1: PulseChannel,
    pulse2: PulseChannel,
    triangle: TriangleChannel,
    noise: NoiseChannel,

    frame_counter: FrameCounter,

    pub audio_buffer: Arc<AudioBuffer>,

    cycles: u64,
    sample_period: f64,
    sample_accumulator: f64,

    filter_prev: f32,
}

impl Apu {
    pub fn new() -> Self {
        Self {
            pulse1: PulseChannel::new(1),
            pulse2: PulseChannel::new(2),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            frame_counter: FrameCounter::new(),
            audio_buffer: AudioBuffer::new(),
            cycles: 0,
            sample_period: CPU_FREQUENCY / SAMPLE_RATE,
            sample_accumulator: 0.0,
            filter_prev: 0.0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr {
            // Pulse 1
            0x4000 => {
                self.pulse1.duty = (data >> 6) & 0x03;
                self.pulse1.length_halt = data & 0x20 != 0;
                self.pulse1.constant_volume = data & 0x10 != 0;
                self.pulse1.volume = data & 0x0F;
            }
            0x4001 => {
                self.pulse1.sweep_enabled = data & 0x80 != 0;
                self.pulse1.sweep_period = (data >> 4) & 0x07;
                self.pulse1.sweep_negate = data & 0x08 != 0;
                self.pulse1.sweep_shift = data & 0x07;
                self.pulse1.sweep_reload = true;
            }
            0x4002 => {
                self.pulse1.timer = (self.pulse1.timer & 0x0700) | data as u16;
            }
            0x4003 => {
                self.pulse1.timer = (self.pulse1.timer & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.pulse1.enabled {
                    self.pulse1.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse1.sequencer_step = 0;
                self.pulse1.envelope_start = true;
            }
            // Pulse 2
            0x4004 => {
                self.pulse2.duty = (data >> 6) & 0x03;
                self.pulse2.length_halt = data & 0x20 != 0;
                self.pulse2.constant_volume = data & 0x10 != 0;
                self.pulse2.volume = data & 0x0F;
            }
            0x4005 => {
                self.pulse2.sweep_enabled = data & 0x80 != 0;
                self.pulse2.sweep_period = (data >> 4) & 0x07;
                self.pulse2.sweep_negate = data & 0x08 != 0;
                self.pulse2.sweep_shift = data & 0x07;
                self.pulse2.sweep_reload = true;
            }
            0x4006 => {
                self.pulse2.timer = (self.pulse2.timer & 0x0700) | data as u16;
            }
            0x4007 => {
                self.pulse2.timer = (self.pulse2.timer & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.pulse2.enabled {
                    self.pulse2.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.pulse2.sequencer_step = 0;
                self.pulse2.envelope_start = true;
            }
            // Triangle
            0x4008 => {
                self.triangle.control = data & 0x80 != 0;
                self.triangle.linear_load = data & 0x7F;
            }
            0x4009 => {} // unused
            0x400A => {
                self.triangle.timer = (self.triangle.timer & 0x0700) | data as u16;
            }
            0x400B => {
                self.triangle.timer =
                    (self.triangle.timer & 0x00FF) | (((data & 0x07) as u16) << 8);
                if self.triangle.enabled {
                    self.triangle.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.triangle.linear_reload = true;
            }
            // Noise
            0x400C => {
                self.noise.length_halt = data & 0x20 != 0;
                self.noise.constant_volume = data & 0x10 != 0;
                self.noise.volume = data & 0x0F;
            }
            0x400D => {} // unused
            0x400E => {
                self.noise.mode = data & 0x80 != 0;
                self.noise.timer_period = NOISE_PERIODS[(data & 0x0F) as usize];
            }
            0x400F => {
                if self.noise.enabled {
                    self.noise.length_counter = LENGTH_TABLE[(data >> 3) as usize];
                }
                self.noise.envelope_start = true;
            }
            // Status
            0x4015 => {
                self.pulse1.enabled = data & 0x01 != 0;
                self.pulse2.enabled = data & 0x02 != 0;
                self.triangle.enabled = data & 0x04 != 0;
                self.noise.enabled = data & 0x08 != 0;
                if !self.pulse1.enabled {
                    self.pulse1.length_counter = 0;
                }
                if !self.pulse2.enabled {
                    self.pulse2.length_counter = 0;
                }
                if !self.triangle.enabled {
                    self.triangle.length_counter = 0;
                }
                if !self.noise.enabled {
                    self.noise.length_counter = 0;
                }
            }
            // Frame counter
            0x4017 => {
                self.frame_counter.mode = (data >> 7) & 1;
                self.frame_counter.interrupt_inhibit = data & 0x40 != 0;
                self.frame_counter.step = 0;
                self.frame_counter.divider = 0;
                // Mode 1: immediately clock all units
                if self.frame_counter.mode == 1 {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
            }
            _ => {}
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x4015 => {
                let mut status = 0u8;
                if self.pulse1.length_counter > 0 {
                    status |= 0x01;
                }
                if self.pulse2.length_counter > 0 {
                    status |= 0x02;
                }
                if self.triangle.length_counter > 0 {
                    status |= 0x04;
                }
                if self.noise.length_counter > 0 {
                    status |= 0x08;
                }
                status
            }
            _ => 0,
        }
    }

    pub fn step(&mut self) {
        self.cycles += 1;

        // Clock triangle timer every CPU cycle
        self.triangle.clock_timer();

        // Clock pulse and noise timers every other CPU cycle
        if self.cycles % 2 == 0 {
            self.pulse1.clock_timer();
            self.pulse2.clock_timer();
            self.noise.clock_timer();
        }

        // Frame counter
        self.clock_frame_counter();

        // Dynamic sample rate adjustment based on buffer fill level
        let fill = self.audio_buffer.len();
        let adjustment = if fill > 3000 {
            0.98 // Speed up slightly (produce fewer samples)
        } else if fill < 1000 {
            1.02 // Slow down slightly (produce more samples)
        } else {
            1.0
        };

        // Generate audio sample at sample rate
        self.sample_accumulator += 1.0;
        if self.sample_accumulator >= self.sample_period * adjustment {
            self.sample_accumulator -= self.sample_period * adjustment;
            let raw = self.mix_output();

            // First-order low-pass filter to reduce aliasing
            const ALPHA: f32 = 0.65;
            let filtered = ALPHA * self.filter_prev + (1.0 - ALPHA) * raw;
            self.filter_prev = filtered;

            let _ = self.audio_buffer.push(filtered);
        }
    }

    fn clock_frame_counter(&mut self) {
        self.frame_counter.divider += 1;
        if self.frame_counter.divider < FRAME_COUNTER_RATE {
            return;
        }
        self.frame_counter.divider = 0;
        self.frame_counter.step += 1;

        if self.frame_counter.mode == 0 {
            // 4-step mode
            match self.frame_counter.step {
                1 => self.clock_quarter_frame(),
                2 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
                3 => self.clock_quarter_frame(),
                4 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                    self.frame_counter.step = 0;
                }
                _ => self.frame_counter.step = 0,
            }
        } else {
            // 5-step mode
            match self.frame_counter.step {
                1 => self.clock_quarter_frame(),
                2 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                }
                3 => self.clock_quarter_frame(),
                4 => {} // do nothing
                5 => {
                    self.clock_quarter_frame();
                    self.clock_half_frame();
                    self.frame_counter.step = 0;
                }
                _ => self.frame_counter.step = 0,
            }
        }
    }

    fn clock_quarter_frame(&mut self) {
        self.pulse1.clock_envelope();
        self.pulse2.clock_envelope();
        self.triangle.clock_linear();
        self.noise.clock_envelope();
    }

    fn clock_half_frame(&mut self) {
        self.pulse1.clock_length();
        self.pulse2.clock_length();
        self.triangle.clock_length();
        self.noise.clock_length();
        self.pulse1.clock_sweep();
        self.pulse2.clock_sweep();
    }

    fn mix_output(&self) -> f32 {
        let p1 = self.pulse1.output() as f32;
        let p2 = self.pulse2.output() as f32;
        let tri = self.triangle.output() as f32;
        let noise = self.noise.output() as f32;

        let pulse_out = 0.00752 * (p1 + p2);
        let tnd_out = 0.00851 * tri + 0.00494 * noise;

        pulse_out + tnd_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apu_new() {
        let apu = Apu::new();
        assert_eq!(apu.cycles, 0);
        assert_eq!(apu.pulse1.enabled, false);
        assert_eq!(apu.triangle.enabled, false);
        assert_eq!(apu.noise.shift_register, 1);
    }

    #[test]
    fn test_status_register() {
        let mut apu = Apu::new();
        // Enable all channels
        apu.write(0x4015, 0x0F);
        assert!(apu.pulse1.enabled);
        assert!(apu.pulse2.enabled);
        assert!(apu.triangle.enabled);
        assert!(apu.noise.enabled);

        // Read status with no length counters loaded
        assert_eq!(apu.read(0x4015), 0x00);

        // Load length counters by writing to $4003, $4007, $400B, $400F
        apu.write(0x4003, 0x08); // length table index 1 = 254
        apu.write(0x4007, 0x08);
        apu.write(0x400B, 0x08);
        apu.write(0x400F, 0x08);
        assert_eq!(apu.read(0x4015), 0x0F);

        // Disable pulse1
        apu.write(0x4015, 0x0E);
        assert_eq!(apu.pulse1.length_counter, 0);
        assert_eq!(apu.read(0x4015) & 0x01, 0);
    }

    #[test]
    fn test_pulse_register_writes() {
        let mut apu = Apu::new();
        apu.write(0x4015, 0x01); // Enable pulse 1

        apu.write(0x4000, 0xBF); // duty=2, halt=1, const=1, vol=15
        assert_eq!(apu.pulse1.duty, 2);
        assert!(apu.pulse1.length_halt);
        assert!(apu.pulse1.constant_volume);
        assert_eq!(apu.pulse1.volume, 15);

        apu.write(0x4002, 0xFD); // timer low
        apu.write(0x4003, 0x00); // timer high + length
        assert_eq!(apu.pulse1.timer & 0xFF, 0xFD);
    }

    #[test]
    fn test_mix_output_silent() {
        let apu = Apu::new();
        assert_eq!(apu.mix_output(), 0.0);
    }

    #[test]
    fn test_sample_generation() {
        let mut apu = Apu::new();
        // Step enough cycles to generate at least one sample
        for _ in 0..50 {
            apu.step();
        }
        // With ~40.6 cycles per sample, 50 cycles should produce at least 1 sample
        assert!(apu.audio_buffer.len() >= 1);
    }
}
