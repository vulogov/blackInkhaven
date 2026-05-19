extern crate log;

use bundcore::bundcore::Bund;
use distance;
use easy_error::{Error, bail};
use natural::distance::jaro_winkler_distance;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum DistanceAlgorithm {
    Levenshtein,
    DamerauLevenshtein,
    Hamming,
    Sift3,
    JaroWinkler,
}

#[time_graph::instrument]
pub fn string_distance_base(
    vm: &mut VM,
    op: StackOps,
    aop: DistanceAlgorithm,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
        StackOps::FromWorkBench => {
            if vm.stack.workbench.len() < 1 {
                bail!("Workbench is too shallow for inline {}", &err_prefix);
            }
            if vm.stack.current_stack_len() < 1 {
                bail!("Stack is too shallow for inline {}", &err_prefix);
            }
        }
    }
    let string1_val_get = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let string2_val_get = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull(),
    };

    let string1_val = match string1_val_get {
        Some(string1_val) => string1_val,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let string2_val = match string2_val_get {
        Some(string2_val) => string2_val,
        None => {
            bail!("{} returns NO DATA #2", &err_prefix);
        }
    };

    let string1_data = match string1_val.cast_string() {
        Ok(string1_data) => string1_data,
        Err(err) => {
            bail!("{} returned for #1: {}", &err_prefix, err);
        }
    };
    let string2_data = match string2_val.cast_string() {
        Ok(string2_data) => string2_data,
        Err(err) => {
            bail!("{} returned for #2: {}", &err_prefix, err);
        }
    };

    let res: Value = match aop {
        DistanceAlgorithm::Levenshtein => {
            Value::from_int(distance::levenshtein(&string1_data, &string2_data) as i64)
        }
        DistanceAlgorithm::DamerauLevenshtein => {
            Value::from_int(distance::damerau_levenshtein(&string1_data, &string2_data) as i64)
        }
        DistanceAlgorithm::Hamming => {
            Value::from_int(match distance::hamming(&string1_data, &string2_data) {
                Ok(dist) => dist,
                Err(err) => {
                    bail!("{} returned hamming error: {:?}", &err_prefix, err);
                }
            } as i64)
        }
        DistanceAlgorithm::Sift3 => {
            Value::from_float(distance::sift3(&string1_data, &string2_data) as f64)
        }
        DistanceAlgorithm::JaroWinkler => {
            Value::from_float(jaro_winkler_distance(&string1_data, &string2_data) as f64)
        }
    };

    let _ = match op {
        StackOps::FromStack => vm.stack.push(res),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
    };
    Ok(vm)
}

pub fn stdlib_string_stack_distance_lev_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromStack,
        DistanceAlgorithm::Levenshtein,
        "STRING.DISTANCE.LEVENSHTEIN".to_string(),
    )
}
pub fn stdlib_string_wb_distance_lev_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromWorkBench,
        DistanceAlgorithm::Levenshtein,
        "STRING.DISTANCE.LEVENSHTEIN.".to_string(),
    )
}

pub fn stdlib_string_stack_distance_dlev_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromStack,
        DistanceAlgorithm::DamerauLevenshtein,
        "STRING.DISTANCE.DAMERAULEVENSHTEIN".to_string(),
    )
}
pub fn stdlib_string_wb_distance_dlev_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromWorkBench,
        DistanceAlgorithm::DamerauLevenshtein,
        "STRING.DISTANCE.DAMERAULEVENSHTEIN.".to_string(),
    )
}

pub fn stdlib_string_stack_distance_ham_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromStack,
        DistanceAlgorithm::Hamming,
        "STRING.DISTANCE.HAMMING".to_string(),
    )
}
pub fn stdlib_string_wb_distance_ham_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromWorkBench,
        DistanceAlgorithm::Hamming,
        "STRING.DISTANCE.HAMMING.".to_string(),
    )
}

pub fn stdlib_string_stack_distance_sift_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromStack,
        DistanceAlgorithm::Sift3,
        "STRING.DISTANCE.SIFT3".to_string(),
    )
}
pub fn stdlib_string_wb_distance_sift_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromWorkBench,
        DistanceAlgorithm::Sift3,
        "STRING.DISTANCE.SIFT3.".to_string(),
    )
}
pub fn stdlib_string_wb_distance_jw_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromWorkBench,
        DistanceAlgorithm::JaroWinkler,
        "STRING.DISTANCE.JAROWINKLER.".to_string(),
    )
}
pub fn stdlib_string_stack_distance_jw_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_distance_base(
        vm,
        StackOps::FromStack,
        DistanceAlgorithm::JaroWinkler,
        "STRING.DISTANCE.JAROWINKLER".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.distance.levenshtein".to_string(),
        stdlib_string_stack_distance_lev_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.levenshtein.".to_string(),
        stdlib_string_wb_distance_lev_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.dameraulevenshtein".to_string(),
        stdlib_string_stack_distance_dlev_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.dameraulevenshtein.".to_string(),
        stdlib_string_wb_distance_dlev_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.hamming".to_string(),
        stdlib_string_stack_distance_ham_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.hamming.".to_string(),
        stdlib_string_wb_distance_ham_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.sift3".to_string(),
        stdlib_string_stack_distance_sift_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.sift3.".to_string(),
        stdlib_string_wb_distance_sift_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.jarowinkler".to_string(),
        stdlib_string_stack_distance_jw_inline,
    );
    let _ = vm.vm.register_inline(
        "string.distance.jarowinkler.".to_string(),
        stdlib_string_wb_distance_jw_inline,
    );
    Ok(())
}
