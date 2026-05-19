extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use hyphenation::{Language, Load, Standard};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use textwrap::word_splitters::WordSplitter;
use textwrap::{Options, wrap};

#[time_graph::instrument]
pub fn string_wrap_base(
    vm: &mut VM,
    op: StackOps,
    lang: Language,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match op {
        StackOps::FromStack => {
            if vm.stack.current_stack_len() < 2 {
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
    let n_val_get = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull(),
    };

    let string1_val = match string1_val_get {
        Some(string1_val) => string1_val,
        None => {
            bail!("{} returns NO DATA #1", &err_prefix);
        }
    };
    let n_val = match n_val_get {
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
    let n = match n_val.cast_int() {
        Ok(string2_data) => string2_data,
        Err(err) => {
            bail!("{} returned for #2: {}", &err_prefix, err);
        }
    };
    let mut res = Value::list();
    let dictionary = match Standard::from_embedded(lang) {
        Ok(dictionary) => dictionary,
        Err(err) => bail!("{} error creating dictionary: {}", &err_prefix, err),
    };
    let options = Options::new(n as usize).word_splitter(WordSplitter::Hyphenation(dictionary));
    let data = wrap(&string1_data, &options);
    for l in data.iter() {
        let row = Value::from_string(l);
        res = res.push(row);
    }
    let _ = match op {
        StackOps::FromStack => vm.stack.push(res),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
    };
    Ok(vm)
}

#[time_graph::instrument]
pub fn stdlib_string_stack_wrap_english_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_wrap_base(
        vm,
        StackOps::FromStack,
        Language::EnglishUS,
        "STRING.WRAP.ENGLISH".to_string(),
    )
}

#[time_graph::instrument]
pub fn stdlib_string_workbench_wrap_english_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_wrap_base(
        vm,
        StackOps::FromWorkBench,
        Language::EnglishUS,
        "STRING.WRAP.ENGLISH.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.wrap.english".to_string(),
        stdlib_string_stack_wrap_english_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.wrap.english.".to_string(),
        stdlib_string_workbench_wrap_english_inline,
    )?;

    Ok(())
}
