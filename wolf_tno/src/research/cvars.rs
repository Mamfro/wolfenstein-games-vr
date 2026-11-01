//! Probe (`probe_cvars=<name>@<rva>=<value>,...`): integer cvars set for the run (put back at the
//! stop), each named and placed as the game's cvar list has it (the rva of its name pointer), to try
//! a renderer switch against a fault seen in the headset. A cvar whose name is not at its rva in
//! this build is left alone.

use eng_idt::Cvar;
use monaka_core::sync::Locked;
use monaka_hook::module::Module;
use monaka_producer::ledger::Ledger;
use monaka_producer::log;
use std::sync::Mutex;

/// The cvars asked for so far: the ledger keeps a cvar for the run, so each lives for good, made
/// once whatever the number of runs.
static KNOWN: Mutex<Vec<&'static Cvar>> = Mutex::new(Vec::new());

/// Sets the cvars `probe_cvars` asks for, on `ledger`.
pub fn apply(ledger: &mut Ledger, exe: &Module) {
    let asked = super::options().cvars.clone();
    for entry in asked.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let Some((name, rva, value)) = parse(entry) else {
            log!("probe_cvars: {entry:?} is not name@rva=value");
            continue;
        };
        eng_idt::set_int(ledger, exe, known(name, rva), value, "probe_cvars leaves it as it is");
    }
}

/// The cvar `name` at `rva`, made the first time it is asked for.
fn known(name: &str, rva: usize) -> &'static Cvar {
    let mut known = KNOWN.locked();
    if let Some(cvar) = known.iter().find(|c| c.name == name && c.name_pointer == rva) {
        return cvar;
    }
    let cvar: &'static Cvar = Box::leak(Box::new(Cvar::id5(rva, Box::leak(name.to_owned().into_boxed_str()))));
    known.push(cvar);
    cvar
}

/// `name@rva=value` (rva in hex, `0x` optional) as the name, the rva and the value.
pub fn parse(entry: &str) -> Option<(&str, usize, i32)> {
    let (name, rest) = entry.split_once('@')?;
    let (rva, value) = rest.split_once('=')?;
    let rva = usize::from_str_radix(rva.trim().trim_start_matches("0x"), 16).ok()?;
    Some((name.trim(), rva, value.trim().parse().ok()?))
}

#[cfg(test)]
mod tests;
