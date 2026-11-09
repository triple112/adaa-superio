# adaa-superio

Source of the three files in Adaa's sensor library that are covered by the
**Mozilla Public License 2.0** (MPL-2.0). They are published here to meet
MPL-2.0 §3.2 for every Adaa app that ships them (AdaaX, Adaa Care, and the
Adaa installer tool). The rest of those apps is proprietary and is not part of
this repository.

| File | What it does | Derived from |
|---|---|---|
| `src/nct.rs` | Read-only readout of Nuvoton NCT67xx / NCT5585 hardware monitors (temperatures, fan RPM) | LibreHardwareMonitor `Nct677X.cs` |
| `src/ite.rs` | Read-only readout of ITE IT86xx / IT87xx hardware monitors | LibreHardwareMonitor `IT87XX.cs` |
| `src/boards.rs` | Per-motherboard sensor labels (which channel is the VRM, CPU, chipset, fans…) | LibreHardwareMonitor `SuperIOHardware.cs` + `Identification.cs` |

The files are exactly as built into the apps. They refer to two small internal
items that are not MPL-covered and not published here: `crate::lpc::LpcPort`
(port I/O through the PawnIO driver) and `super::Family` (the chip-family enum).

Upstream: <https://github.com/LibreHardwareMonitor/LibreHardwareMonitor>  
Copyright (C) LibreHardwareMonitor and Contributors.  
Partial Copyright (C) Michael Möller and Contributors.

Licence: MPL-2.0 — see `LICENSE`.
