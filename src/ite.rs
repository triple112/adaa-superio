// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// ITE IT86xx / IT87xx environment-controller readout (mostly Gigabyte boards).
// Register layout and per-chip channel counts ported from LibreHardwareMonitor's
// `IT87XX.cs` (Copyright (C) LibreHardwareMonitor and Contributors; Partial
// Copyright (C) Michael Möller and Contributors). Read-only: no bank, PWM or
// fan-control register is written, so the chips LHM must bank-switch
// (IT8655E, IT8665E) are not read.

use crate::lpc::LpcPort;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IteChip {
    It8613E,
    It8620E,
    It8625E,
    It8628E,
    It8631E,
    It8638E,
    It8686E,
    It8688E,
    It8689E,
    It8696E,
    It8721F,
    It8728F,
    It8771E,
    It8772E,
    It8790E,
    It8792E,
    It87952E,
}

impl IteChip {
    pub(crate) fn identify(id: u16) -> Option<Self> {
        Some(match id {
            0x8613 => IteChip::It8613E,
            0x8620 => IteChip::It8620E,
            0x8625 => IteChip::It8625E,
            0x8628 => IteChip::It8628E,
            0x8631 => IteChip::It8631E,
            0x8638 => IteChip::It8638E,
            0x8686 => IteChip::It8686E,
            0x8688 => IteChip::It8688E,
            0x8689 => IteChip::It8689E,
            0x8696 => IteChip::It8696E,
            0x8721 => IteChip::It8721F,
            0x8728 => IteChip::It8728F,
            0x8771 => IteChip::It8771E,
            0x8772 => IteChip::It8772E,
            0x8790 => IteChip::It8790E,
            0x8733 => IteChip::It8792E,
            0x8695 => IteChip::It87952E,
            _ => return None,
        })
    }

    pub(crate) fn name(self) -> String {
        format!("{self:?}").replace("It", "IT")
    }

    /// The secondary chips Gigabyte puts at 0x4E (LHM's «configuration C»).
    pub(crate) fn is_secondary(self) -> bool {
        matches!(self, IteChip::It8790E | IteChip::It8792E | IteChip::It87952E)
    }

    fn counts(self) -> (usize, usize) {
        match self {
            IteChip::It8613E => (4, 5),
            IteChip::It8625E => (3, 6),
            IteChip::It8628E => (6, 6),
            IteChip::It8631E | IteChip::It8638E => (2, 2),
            IteChip::It8686E => (7, 6),
            IteChip::It8688E | IteChip::It8689E | IteChip::It8696E => (6, 6),
            IteChip::It87952E | IteChip::It8792E | IteChip::It8790E => (3, 3),
            IteChip::It8620E => (3, 5),
            _ => (3, 5),
        }
    }
}

const TEMP_BASE: u8 = 0x29;
const FAN_16BIT_MODES: u8 = 0x0C;
const FAN_LO: [u8; 6] = [0x0D, 0x0E, 0x0F, 0x80, 0x82, 0x4C];
const FAN_HI: [u8; 6] = [0x18, 0x19, 0x1A, 0x81, 0x83, 0x4D];
const FAN_LO_ALT: [u8; 6] = [0x0D, 0x0E, 0x0F, 0x80, 0x82, 0x93];
const FAN_HI_ALT: [u8; 6] = [0x18, 0x19, 0x1A, 0x81, 0x83, 0x94];

/// 16-bit tachometer → RPM (LHM: `1.35e6 / (count * 2)`; tiny counts are noise).
pub(crate) fn fan_rpm_16bit(count: u16) -> Option<f32> {
    if count <= 0x3F {
        None
    } else if count == 0xFFFF {
        Some(0.0)
    } else {
        Some(1.35e6 / (count as f32 * 2.0))
    }
}

pub(crate) struct Ite {
    pub(crate) chip: IteChip,
    pub(crate) port: LpcPort,
    base: u16,
}

impl Ite {
    pub(crate) fn new(chip: IteChip, port: LpcPort, base: u16) -> Self {
        Self { chip, port, base }
    }

    /// Index on base+5, data on base+6; the index reads back as written on a
    /// healthy chip (IT8688E excepted, as in LHM).
    fn rd(&self, reg: u8) -> Option<u8> {
        self.port.outb(self.base + 5, reg);
        let v = self.port.inb(self.base + 6)?;
        let ok = self.chip == IteChip::It8688E || self.port.inb(self.base + 5) == Some(reg);
        ok.then_some(v)
    }

    pub(crate) fn accessible(&self) -> bool {
        matches!(self.rd(0x58), Some(0x90 | 0x7F))
    }

    pub(crate) fn update(&self) -> (Vec<Option<f32>>, Vec<Option<f32>>) {
        let (nt, nf) = self.chip.counts();
        let temps = (0..nt)
            .map(|i| self.rd(TEMP_BASE + i as u8).map(|v| v as i8).filter(|v| *v > 0 && *v < i8::MAX).map(|v| v as f32))
            .collect();
        let modes = self.rd(FAN_16BIT_MODES).unwrap_or(0xFF);
        let alt = matches!(self.chip, IteChip::It8625E);
        let fans = (0..nf)
            .map(|i| {
                let disabled = (i == 3 && modes & (1 << 4) == 0) || (i == 4 && modes & (1 << 5) == 0) || (i == 5 && modes & (1 << 2) == 0);
                if disabled && nf >= 5 {
                    return None;
                }
                let (lo, hi) = if alt { (FAN_LO_ALT[i], FAN_HI_ALT[i]) } else { (FAN_LO[i], FAN_HI[i]) };
                let count = (self.rd(lo)? as u16) | ((self.rd(hi)? as u16) << 8);
                fan_rpm_16bit(count)
            })
            .collect();
        (temps, fans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_counts() {
        assert_eq!(IteChip::identify(0x8689), Some(IteChip::It8689E));
        assert_eq!(IteChip::identify(0x8733), Some(IteChip::It8792E));
        assert!(IteChip::It8792E.is_secondary());
        assert_eq!(IteChip::It8686E.counts(), (7, 6));
        assert_eq!(IteChip::It8689E.name(), "IT8689E");
        assert_eq!(IteChip::identify(0x8665), None); // needs a bank write — not read
    }

    #[test]
    fn tach() {
        assert_eq!(fan_rpm_16bit(0x20), None);
        assert_eq!(fan_rpm_16bit(0xFFFF), Some(0.0));
        assert!((fan_rpm_16bit(405).unwrap() - 1666.7).abs() < 0.5);
    }
}
