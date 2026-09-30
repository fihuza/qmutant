#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| qmutant::fuzzing::directive(text));
