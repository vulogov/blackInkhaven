extern crate log;
use std::collections::HashSet;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use natural::tokenize;
use rnltk::token;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum TokenizeAlgorithm {
    Simple,
    SimpleUnique,
    SimpleStemmed,
    Lines,
}

#[time_graph::instrument]
fn string_tokenize_base(
    vm: &mut VM,
    op: StackOps,
    ta: TokenizeAlgorithm,
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
        }
    }
    let string_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    match string_val {
        Some(string_val) => match string_val.cast_string() {
            Ok(string_data) => {
                let mut res = Value::list();
                match ta {
                    TokenizeAlgorithm::Simple => {
                        for t in tokenize::tokenize(&string_data) {
                            res = res.push(Value::from_string(t.trim()));
                        }
                    }
                    TokenizeAlgorithm::SimpleUnique => {
                        let mut s: HashSet<String> = HashSet::new();
                        for t in tokenize::tokenize(&string_data.to_lowercase()) {
                            s.insert(t.trim().to_string());
                        }
                        for t in s.iter() {
                            res = res.push(Value::from_string(t));
                        }
                    }
                    TokenizeAlgorithm::SimpleStemmed => {
                        let mut s: HashSet<String> = HashSet::new();
                        for t in token::tokenize_stemmed_sentence(&string_data) {
                            s.insert(t.trim().to_string());
                        }
                        for t in s.iter() {
                            res = res.push(Value::from_string(t));
                        }
                    }
                    TokenizeAlgorithm::Lines => {
                        for t in string_data.lines() {
                            res = res.push(Value::from_string(t.trim()));
                        }
                    }
                }
                vm.stack.push(res);
            }
            Err(err) => {
                bail!("{} returns: {}", &err_prefix, err);
            }
        },
        None => {
            bail!("{} returns: NO DATA #1", &err_prefix);
        }
    }
    Ok(vm)
}

pub fn stdlib_tokenize_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromWorkBench,
        TokenizeAlgorithm::Simple,
        "STRING.TOKENIZE.".to_string(),
    )
}
pub fn stdlib_tokenize_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromStack,
        TokenizeAlgorithm::Simple,
        "STRING.TOKENIZE".to_string(),
    )
}

pub fn stdlib_tokenize_simpleunique_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromWorkBench,
        TokenizeAlgorithm::SimpleUnique,
        "STRING.TOKENIZE.UNIQUE.".to_string(),
    )
}
pub fn stdlib_tokenize_simpleunique_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromStack,
        TokenizeAlgorithm::SimpleUnique,
        "STRING.TOKENIZE.UNIQUE".to_string(),
    )
}

pub fn stdlib_tokenize_simplestemmed_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromWorkBench,
        TokenizeAlgorithm::SimpleStemmed,
        "STRING.TOKENIZE.STEMMED.".to_string(),
    )
}
pub fn stdlib_tokenize_simplestemmed_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromStack,
        TokenizeAlgorithm::SimpleStemmed,
        "STRING.TOKENIZE.STEMMED".to_string(),
    )
}

pub fn stdlib_tokenize_lines_wb_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromWorkBench,
        TokenizeAlgorithm::Lines,
        "STRING.TOKENIZE.LINES.".to_string(),
    )
}
pub fn stdlib_tokenize_lines_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_tokenize_base(
        vm,
        StackOps::FromStack,
        TokenizeAlgorithm::Lines,
        "STRING.TOKENIZE.LINES".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("string.tokenize".to_string(), stdlib_tokenize_stack_inline)?;
    let _ = vm
        .vm
        .register_inline("string.tokenize.".to_string(), stdlib_tokenize_wb_inline)?;
    let _ = vm.vm.register_inline(
        "string.tokenize.unique".to_string(),
        stdlib_tokenize_simpleunique_stack_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.tokenize.unique.".to_string(),
        stdlib_tokenize_simpleunique_wb_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.tokenize.stemmed".to_string(),
        stdlib_tokenize_simplestemmed_stack_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.tokenize.stemmed.".to_string(),
        stdlib_tokenize_simplestemmed_wb_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.tokenize.lines".to_string(),
        stdlib_tokenize_lines_stack_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.tokenize.lines.".to_string(),
        stdlib_tokenize_lines_wb_inline,
    )?;

    Ok(())
}
