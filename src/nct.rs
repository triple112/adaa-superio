// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Nuvoton NCT67xx hardware-monitor readout. The temperature-channel tables and
// their order are ported from LibreHardwareMonitor's `Nct677X.cs`
// (Copyright (C) LibreHardwareMonitor and Contributors; Partial Copyright (C)
// Michael Möller and Contributors), so that LHM's per-board sensor indices
// (`boards.rs`) mean the same channel here. Read-only: no fan-control, PWM or
// I/O-space-lock register is ever written (LHM disables the lock; we report
// «locked» instead).

use std::collections::HashMap;

use crate::lpc::LpcPort;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NctChip {
    Nct6779D,
    Nct6791D,
    Nct6792D,
    Nct6792DA,
    Nct6793D,
    Nct6795D,
    Nct6796D,
    Nct6796DR,
    Nct6796DS,
    Nct6797D,
    Nct6798D,
    Nct6799D,
    Nct5585D,
    // MSI's EC-style interface (page / index / data).
    Nct6683D,
    Nct6686D,
    Nct6687D,
    Nct6687DR,
}

impl NctChip {
    pub(crate) fn name(self) -> &'static str {
        match self {
            NctChip::Nct6779D => "NCT6779D",
            NctChip::Nct6791D => "NCT6791D",
            NctChip::Nct6792D => "NCT6792D",
            NctChip::Nct6792DA => "NCT6792D-A",
            NctChip::Nct6793D => "NCT6793D",
            NctChip::Nct6795D => "NCT6795D",
            NctChip::Nct6796D => "NCT6796D",
            NctChip::Nct6796DR => "NCT6796D-R",
            NctChip::Nct6796DS => "NCT6796D-S",
            NctChip::Nct6797D => "NCT6797D",
            NctChip::Nct6798D => "NCT6798D",
            NctChip::Nct6799D => "NCT6799D",
            NctChip::Nct5585D => "NCT5585D",
            NctChip::Nct6683D => "NCT6683D",
            NctChip::Nct6686D => "NCT6686D",
            NctChip::Nct6687D => "NCT6687D",
            NctChip::Nct6687DR => "NCT6687D-R",
        }
    }

    pub(crate) fn is_ec(self) -> bool {
        matches!(self, NctChip::Nct6683D | NctChip::Nct6686D | NctChip::Nct6687D | NctChip::Nct6687DR)
    }

    /// Chip ID (0x20) + revision (0x21) as LHM's `LpcIO.DetectWinbondFintek`
    /// maps them. `board` disambiguates the IDs Nuvoton reused.
    pub(crate) fn identify(id: u8, rev: u8, board: &str) -> Option<Self> {
        let b = board.to_ascii_uppercase();
        Some(match (id, rev) {
            (0xC5, 0x60) => NctChip::Nct6779D,
            (0xC7, 0x32) => NctChip::Nct6683D,
            (0xC8, 0x03) => NctChip::Nct6791D,
            (0xC9, 0x11) => NctChip::Nct6792D,
            (0xC9, 0x13) => NctChip::Nct6792DA,
            (0xD1, 0x21) => NctChip::Nct6793D,
            (0xD3, 0x52) => NctChip::Nct6795D,
            (0xD4, 0x23) => NctChip::Nct6796D,
            (0xD4, 0x2A) if b.contains("X870E NOVA") => NctChip::Nct5585D,
            (0xD4, 0x2A) => NctChip::Nct6796DR,
            (0xD4, 0x51) => NctChip::Nct6797D,
            (0xD4, 0x2B) => NctChip::Nct6798D,
            (0xD4, 0x40 | 0x41) => NctChip::Nct6686D,
            (0xD5, 0x92) if ["B840", "B850", "B860", "X870", "Z890"].iter().any(|k| b.contains(k)) => NctChip::Nct6687DR,
            (0xD5, 0x92) => NctChip::Nct6687D,
            (0xD8, 0x02) if b.contains("X870E NOVA") || b.contains("B650M HDV/M.2") => NctChip::Nct6796DS,
            (0xD8, 0x02) => NctChip::Nct6799D,
            _ => return None,
        })
    }

    fn fan_count(self) -> usize {
        match self {
            NctChip::Nct6779D => 5,
            NctChip::Nct6796DR | NctChip::Nct6796DS | NctChip::Nct6797D | NctChip::Nct6798D | NctChip::Nct6799D | NctChip::Nct5585D => 7,
            NctChip::Nct6687DR => 16,
            NctChip::Nct6683D | NctChip::Nct6686D | NctChip::Nct6687D => 17,
            _ => 6,
        }
    }
}

// Temperature sources (LHM's `SourceNct67Xxd`).
const SYSTIN: u8 = 1;
const CPUTIN: u8 = 2;
const AUXTIN0: u8 = 3;
const AUXTIN1: u8 = 4;
const AUXTIN2: u8 = 5;
const AUXTIN3: u8 = 6;
const AUXTIN4: u8 = 7;
const SMBUSMASTER0: u8 = 8;
const SMBUSMASTER1: u8 = 9;
const TSENSOR: u8 = 10;
const PECI_0: u8 = 16;
const PECI_1: u8 = 17;
const PCH_CHIP_CPU_MAX_TEMP: u8 = 18;
const PCH_CHIP_TEMP: u8 = 19;
const PCH_CPU_TEMP: u8 = 20;
const PCH_MCH_TEMP: u8 = 21;
const AGENT0_DIMM0: u8 = 22;
const AGENT0_DIMM1: u8 = 23;
const AGENT1_DIMM0: u8 = 24;
const AGENT1_DIMM1: u8 = 25;
const BYTE_TEMP0: u8 = 26;
const BYTE_TEMP1: u8 = 27;
const PECI_0_CAL: u8 = 28;
const PECI_1_CAL: u8 = 29;
const VIRTUAL_TEMP: u8 = 31;
const SPARE_TEMP: u8 = 32;
const SPARE_TEMP2: u8 = 33;
const AUXTIN5: u8 = 34;

/// One temperature channel: (source, value reg, half reg, half bit, source-select reg, direct reg).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ch {
    pub(crate) source: u8,
    reg: u16,
    half_reg: u16,
    half_bit: i8,
    src_reg: u16,
    alt: Option<u16>,
}

const fn ch(source: u8, reg: u16, half_reg: u16, half_bit: i8, src_reg: u16, alt: Option<u16>) -> Ch {
    Ch { source, reg, half_reg, half_bit, src_reg, alt }
}
const fn c(source: u8, reg: u16) -> Ch {
    ch(source, reg, 0, -1, 0, None)
}

const PCH_DIMM_TAIL: [Ch; 13] = [
    ch(PECI_1, 0x672, 0, -1, 0xC27, None),
    ch(PCH_CHIP_CPU_MAX_TEMP, 0x674, 0, -1, 0xC28, Some(0x400)),
    ch(PCH_CHIP_TEMP, 0x676, 0, -1, 0xC29, Some(0x401)),
    ch(PCH_CPU_TEMP, 0x678, 0, -1, 0xC2A, Some(0x402)),
    ch(PCH_MCH_TEMP, 0x67A, 0, -1, 0xC2B, Some(0x404)),
    c(AGENT0_DIMM0, 0x405),
    c(AGENT0_DIMM1, 0x406),
    c(AGENT1_DIMM0, 0x407),
    c(AGENT1_DIMM1, 0x408),
    c(BYTE_TEMP0, 0x419),
    c(BYTE_TEMP1, 0x41A),
    c(PECI_0_CAL, 0x4F4),
    c(PECI_1_CAL, 0x4F5),
];

const HEAD: [Ch; 8] = [
    ch(PECI_0, 0x073, 0x074, 7, 0x100, None),
    ch(CPUTIN, 0x075, 0x076, 7, 0x200, Some(0x491)),
    ch(SYSTIN, 0x077, 0x078, 7, 0x300, Some(0x490)),
    ch(AUXTIN0, 0x079, 0x07A, 7, 0x800, Some(0x492)),
    ch(AUXTIN1, 0x07B, 0x07C, 7, 0x900, Some(0x493)),
    ch(AUXTIN2, 0x07D, 0x07E, 7, 0xA00, Some(0x494)),
    ch(AUXTIN3, 0x4A0, 0x49E, 6, 0xB00, Some(0x495)),
    ch(AUXTIN4, 0x027, 0, -1, 0x621, None),
];
const SMBUS: [Ch; 2] = [ch(SMBUSMASTER0, 0x150, 0x151, 7, 0x622, None), ch(SMBUSMASTER1, 0x670, 0, -1, 0xC26, None)];
const VIRTUALS: [Ch; 3] = [c(VIRTUAL_TEMP, 0), c(SPARE_TEMP, 0), c(SPARE_TEMP2, 0)];

/// LHM's channel table for each banked chip, in LHM's order (that order is
/// what the board map's indices refer to).
pub(crate) fn channels(chip: NctChip) -> Vec<Ch> {
    let mut v = Vec::new();
    match chip {
        // Group A: 6791/6792/6793/6795 — head, PCH/DIMM tail with SMBus after the DIMMs.
        NctChip::Nct6791D | NctChip::Nct6792D | NctChip::Nct6793D | NctChip::Nct6795D => {
            v.extend(HEAD);
            v.extend(PCH_DIMM_TAIL[..9].iter().copied());
            v.extend(SMBUS);
            v.extend(PCH_DIMM_TAIL[9..].iter().copied());
            v.extend(VIRTUALS);
        }
        // Group B: 6796D/-R and 6797D — SMBus right after AUXTIN4 (index 8).
        NctChip::Nct6796D | NctChip::Nct6796DR | NctChip::Nct6797D => {
            v.extend(HEAD);
            v.extend(SMBUS);
            v.extend(PCH_DIMM_TAIL);
            v.extend(VIRTUALS);
        }
        // Group D: 6798D/6799D — TSENSOR after AUXTIN4.
        NctChip::Nct6798D | NctChip::Nct6799D => {
            v.extend(HEAD);
            v.push(ch(TSENSOR, 0x4A2, 0x4A1, 7, 0xC00, Some(0x496)));
            v.extend(SMBUS);
            v.extend(PCH_DIMM_TAIL);
            v.extend(VIRTUALS);
        }
        NctChip::Nct6796DS => v.extend([
            ch(CPUTIN, 0x073, 0x074, 7, 0x100, Some(0x491)),
            ch(SYSTIN, 0x075, 0x076, 7, 0x200, Some(0x490)),
            ch(AUXTIN0, 0x077, 0x078, 7, 0x300, Some(0x492)),
            ch(AUXTIN1, 0x079, 0x07A, 7, 0x800, Some(0x493)),
            ch(AUXTIN2, 0x07B, 0x07C, 7, 0x900, Some(0x494)),
            ch(AUXTIN3, 0x07D, 0x07E, 7, 0xA00, Some(0x495)),
            ch(AUXTIN4, 0x027, 0, 4, 0xC16, Some(0x496)),
            ch(AUXTIN5, 0x449, 0, 4, 0x100, Some(0x4A2)),
            ch(SMBUSMASTER0, 0x150, 0x151, 7, 0x622, None),
            c(PECI_0, 0x0720),
            c(VIRTUAL_TEMP, 0),
        ]),
        NctChip::Nct5585D => v.extend([
            ch(PECI_0, 0x0720, 0, -1, 0x100, None),
            ch(CPUTIN, 0x075, 0x076, 7, 0x000, Some(0x073)),
            ch(AUXTIN1, 0x07B, 0x07C, 7, 0x900, Some(0x493)),
            ch(AUXTIN3, 0x4A0, 0x49E, 6, 0xB00, Some(0x495)),
        ]),
        // 6779D and 6792D-A use LHM's default table.
        _ => v.extend([
            ch(PECI_0, 0x027, 0, -1, 0x621, None),
            ch(CPUTIN, 0x073, 0x074, 7, 0x100, Some(0x491)),
            ch(SYSTIN, 0x075, 0x076, 7, 0x200, Some(0x490)),
            ch(AUXTIN0, 0x077, 0x078, 7, 0x300, Some(0x492)),
            ch(AUXTIN1, 0x079, 0x07A, 7, 0x800, Some(0x493)),
            ch(AUXTIN2, 0x07B, 0x07C, 7, 0x900, Some(0x494)),
            ch(AUXTIN3, 0x150, 0x151, 7, 0x622, Some(0x495)),
        ]),
    }
    v
}

/// The generic name of a source, for boards without a map.
pub(crate) fn source_name(source: u8) -> &'static str {
    match source {
        SYSTIN => "SYSTIN",
        CPUTIN => "CPUTIN",
        AUXTIN0 => "AUXTIN0",
        AUXTIN1 => "AUXTIN1",
        AUXTIN2 => "AUXTIN2",
        AUXTIN3 => "AUXTIN3",
        AUXTIN4 => "AUXTIN4",
        AUXTIN5 => "AUXTIN5",
        TSENSOR => "TSENSOR",
        SMBUSMASTER0 => "SMBUS0",
        SMBUSMASTER1 => "SMBUS1",
        PECI_0 => "PECI0",
        PECI_1 => "PECI1",
        _ => "",
    }
}

const FAN_COUNT_REGS: [u16; 7] = [0x4B0, 0x4B2, 0x4B4, 0x4B6, 0x4B8, 0x4BA, 0x4CC];
const EC_FAN_REGS: [u16; 17] = [0x140, 0x142, 0x144, 0x146, 0x148, 0x14A, 0x14C, 0x14E, 0x150, 0x152, 0x154, 0x156, 0x158, 0x15A, 0x15C, 0x15E, 0x852];
const EC_DR_FAN_REGS: [u16; 16] = [0x140, 0x142, 0x144, 0x146, 0x148, 0x14A, 0x14C, 0x14E, 0x150, 0x152, 0x15E, 0x15C, 0x15A, 0x158, 0x156, 0x154];

/// 13-bit fan count → RPM (LHM: `1.35e6 / count`, 0 at the counter's max = stopped).
pub(crate) fn fan_rpm_13bit(high: u8, low: u8) -> Option<f32> {
    let count = ((high as u32) << 5) | (low as u32 & 0x1F);
    if count >= 0x1FFF {
        Some(0.0)
    } else if count >= 0x15 {
        Some(1.35e6 / count as f32)
    } else {
        None
    }
}

/// One read of a banked NCT67xx channel table (LHM's `Update`, temperatures
/// part): each monitoring channel reads its currently selected source; the
/// first channel that sees a source wins; table entries whose source no
/// channel selects fall back to the chip's direct readout register.
pub(crate) fn read_temps(chip: NctChip, table: &[Ch], rd: &mut dyn FnMut(u16) -> Option<u8>) -> Vec<Option<f32>> {
    let masked = !matches!(chip, NctChip::Nct6779D | NctChip::Nct6792DA | NctChip::Nct6791D | NctChip::Nct6792D | NctChip::Nct6793D | NctChip::Nct6795D);
    let mut seen: HashMap<u8, Option<f32>> = HashMap::new();
    for t in table {
        if t.reg == 0 {
            continue;
        }
        let Some(raw) = rd(t.reg) else { continue };
        let mut value = ((raw as i8) as i32) << 1;
        if let Some(h) = (t.half_bit > 0).then(|| rd(t.half_reg)).flatten() {
            value |= ((h >> t.half_bit) & 1) as i32;
        }
        let source = if t.src_reg > 0 {
            match rd(t.src_reg) {
                Some(s) if masked => s & 0x1F,
                Some(s) => s,
                None => continue,
            }
        } else {
            t.source
        };
        if seen.contains_key(&source) {
            continue;
        }
        let temp = 0.5 * value as f32;
        if (-55.0..=125.0).contains(&temp) {
            seen.insert(source, Some(temp));
        }
    }
    table
        .iter()
        .map(|t| match seen.get(&t.source) {
            Some(v) => *v,
            None => t.alt.and_then(&mut *rd).map(|r| (r as i8) as f32).filter(|v| *v > 0.0 && *v <= 125.0),
        })
        .collect()
}

pub(crate) struct Nct {
    pub(crate) chip: NctChip,
    pub(crate) port: LpcPort,
    base: u16,
    table: Vec<Ch>,
}

impl Nct {
    pub(crate) fn new(chip: NctChip, port: LpcPort, base: u16) -> Self {
        let table = if chip.is_ec() { Vec::new() } else { channels(chip) };
        Self { chip, port, base, table }
    }

    /// Banked access: bank select (0x4E) then register, on base+5/base+6.
    fn rd_banked(&self, addr: u16) -> Option<u8> {
        let (a, d) = (self.base + 5, self.base + 6);
        self.port.outb(a, 0x4E);
        self.port.outb(d, (addr >> 8) as u8);
        self.port.outb(a, (addr & 0xFF) as u8);
        self.port.inb(d)
    }

    /// MSI EC-style access: page register base+4 (0xFF = free), index base+5,
    /// data base+6 — the handshake LHM uses, released back to 0xFF.
    fn rd_ec(&self, addr: u16) -> Option<u8> {
        let page = self.base + 4;
        for _ in 0..50 {
            if self.port.inb(page) == Some(0xFF) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.port.outb(page, 0xFF);
        self.port.outb(page, (addr >> 8) as u8);
        self.port.outb(self.base + 5, (addr & 0xFF) as u8);
        let v = self.port.inb(self.base + 6);
        self.port.outb(page, 0xFF);
        v
    }

    /// Readable = vendor ID visible. When the board's firmware set the
    /// I/O-space lock, the ID reads wrong and we stop (no unlock write).
    pub(crate) fn accessible(&self) -> bool {
        if self.chip.is_ec() {
            return true;
        }
        let hi = self.rd_banked(0x804F);
        let lo = self.rd_banked(0x004F);
        matches!((hi, lo), (Some(h), Some(l)) if ((h as u16) << 8 | l as u16) == 0x5CA3)
    }

    /// (temperatures in the board-map index order, fans in RPM).
    pub(crate) fn update(&self) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
        if self.chip.is_ec() {
            return self.update_ec();
        }
        // Keep the bank the firmware left selected.
        let orig_bank = {
            self.port.outb(self.base + 5, 0x4E);
            self.port.inb(self.base + 6)
        };
        let mut rd = |a: u16| self.rd_banked(a);
        let temps = read_temps(self.chip, &self.table, &mut rd);
        let fans = (0..self.chip.fan_count())
            .map(|i| {
                let r = FAN_COUNT_REGS[i];
                match (self.rd_banked(r), self.rd_banked(r + 1)) {
                    (Some(h), Some(l)) => fan_rpm_13bit(h, l),
                    _ => None,
                }
            })
            .collect();
        if let Some(b) = orig_bank {
            self.port.outb(self.base + 5, 0x4E);
            self.port.outb(self.base + 6, b);
        }
        (temps, fans)
    }

    fn update_ec(&self) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
        let n_temps = if self.chip == NctChip::Nct6687DR { 7 } else { 11 };
        let temps = (0..n_temps)
            .map(|i| {
                let r = 0x100 + 2 * i as u16;
                let v = self.rd_ec(r)? as i8 as f32;
                let half = (self.rd_ec(r + 1)? >> 7) & 1;
                let t = v + 0.5 * half as f32;
                (t > 0.0 && t <= 125.0).then_some(t)
            })
            .collect();
        let regs: &[u16] = if self.chip == NctChip::Nct6687DR { &EC_DR_FAN_REGS } else { &EC_FAN_REGS };
        let fans = regs
            .iter()
            .take(self.chip.fan_count())
            .map(|&r| {
                let (h, l) = (self.rd_ec(r)?, self.rd_ec(r + 1)?);
                if self.chip == NctChip::Nct6687D && h == 0xFF && l == 0xF8 {
                    return Some(0.0);
                }
                Some(((h as u16) << 8 | l as u16) as f32)
            })
            .collect();
        (temps, fans)
    }

    /// The source of each table slot (for unmapped boards' generic labels).
    pub(crate) fn slot_sources(&self) -> Vec<u8> {
        self.table.iter().map(|t| t.source).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_follow_lhm() {
        assert_eq!(NctChip::identify(0xD4, 0x2A, "B550M Steel Legend"), Some(NctChip::Nct6796DR));
        assert_eq!(NctChip::identify(0xD4, 0x2A, "X870E Nova WiFi"), Some(NctChip::Nct5585D));
        assert_eq!(NctChip::identify(0xD5, 0x92, "MAG B850 TOMAHAWK WIFI"), Some(NctChip::Nct6687DR));
        assert_eq!(NctChip::identify(0xD5, 0x92, "MAG B550 TOMAHAWK"), Some(NctChip::Nct6687D));
        assert_eq!(NctChip::identify(0xD8, 0x02, "ROG STRIX X670E-E"), Some(NctChip::Nct6799D));
        assert_eq!(NctChip::identify(0x12, 0x34, ""), None);
    }

    #[test]
    fn table_indices_match_lhm() {
        // Group B: index 8 = SMBUSMASTER0 (B550M Pro4's «CPU» is index 8 in LHM).
        let b = channels(NctChip::Nct6796DR);
        assert_eq!(b[1].source, CPUTIN);
        assert_eq!(b[2].source, SYSTIN);
        assert_eq!(b[8].source, SMBUSMASTER0);
        assert_eq!(b.len(), 26);
        // Group D: TSENSOR at 8, SMBUS at 9.
        let d = channels(NctChip::Nct6798D);
        assert_eq!(d[8].source, TSENSOR);
        assert_eq!(d[9].source, SMBUSMASTER0);
        // Group A: SMBUS after the DIMMs.
        let a = channels(NctChip::Nct6795D);
        assert_eq!(a[17].source, SMBUSMASTER0);
        assert_eq!(channels(NctChip::Nct6779D).len(), 7);
    }

    #[test]
    fn source_selection_first_channel_wins_then_direct_fallback() {
        // Fixture: CPUTIN channel (0x075) selects source SYSTIN; SYSTIN channel (0x077) also SYSTIN.
        let regs: HashMap<u16, u8> = [
            (0x073, 0x00), (0x100, 0x00), // PECI_0 channel: source 0 (none), value 0
            (0x075, 40), (0x076, 0x80), (0x200, SYSTIN), // 40.5 °C from SYSTIN
            (0x077, 99), (0x078, 0x00), (0x300, SYSTIN), // ignored: SYSTIN seen
            (0x079, 0), (0x800, 0), (0x07B, 0), (0x900, 0), (0x07D, 0), (0xA00, 0),
            (0x4A0, 0), (0xB00, 0), (0x027, 0), (0x621, 0), (0x150, 0), (0x622, 0), (0x670, 0), (0xC26, 0),
            (0x491, 55), // CPUTIN direct
        ]
        .into_iter()
        .collect();
        let table = channels(NctChip::Nct6796DR);
        let mut rd = |a: u16| Some(*regs.get(&a).unwrap_or(&0));
        let t = read_temps(NctChip::Nct6796DR, &table, &mut rd);
        assert_eq!(t[2], Some(40.5)); // SYSTIN slot
        assert_eq!(t[1], Some(55.0)); // CPUTIN from its direct register
    }

    #[test]
    fn fan_counts() {
        assert_eq!(fan_rpm_13bit(0xFF, 0x1F), Some(0.0));
        let rpm = fan_rpm_13bit(0x19, 0x0C).unwrap(); // count 812 → ~1663 rpm
        assert!((rpm - 1662.6).abs() < 1.0);
        assert_eq!(fan_rpm_13bit(0, 3), None);
    }
}
