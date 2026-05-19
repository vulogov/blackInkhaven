extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use passwords::PasswordGenerator;
use rand::thread_rng;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum StringRandomAlgorithm {
    Name,
    LastName,
    FullName,
    Password,
    Phone,
    IPv4,
    Word,
}

#[time_graph::instrument]
fn string_password_generator() -> String {
    let pg = PasswordGenerator {
        length: 16,
        numbers: true,
        lowercase_letters: true,
        uppercase_letters: true,
        symbols: true,
        spaces: false,
        exclude_similar_characters: true,
        strict: true,
    };
    pg.generate_one().unwrap()
}

#[time_graph::instrument]
pub fn string_lipsum_base(vm: &mut VM, op: StackOps, err_prefix: String) -> Result<&mut VM, Error> {
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
    let n_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let num = match n_val {
        Some(num) => num,
        None => bail!("{} returns: NO DATA #1", &err_prefix),
    };
    let res = match num.cast_int() {
        Ok(n) => Value::from_string(lipsum::lipsum_words_with_rng(thread_rng(), n as usize)),
        Err(err) => bail!("Error casting in {}: {}", &err_prefix, err),
    };
    match op {
        StackOps::FromStack => vm.stack.push(res),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
    };

    Ok(vm)
}

#[time_graph::instrument]
pub fn string_random_base(
    vm: &mut VM,
    op: StackOps,
    rop: StringRandomAlgorithm,
    _err_prefix: String,
) -> Result<&mut VM, Error> {
    let res = match rop {
        StringRandomAlgorithm::Name => Value::from_string(fakeit::name::first()),
        StringRandomAlgorithm::LastName => Value::from_string(fakeit::name::last()),
        StringRandomAlgorithm::FullName => Value::from_string(fakeit::name::full()),
        StringRandomAlgorithm::Password => Value::from_string(string_password_generator()),
        StringRandomAlgorithm::Phone => Value::from_string(fakeit::contact::phone_formatted()),
        StringRandomAlgorithm::IPv4 => Value::from_string(fakeit::internet::ipv4_address()),
        StringRandomAlgorithm::Word => Value::from_string(fakeit::words::word()),
    };
    match op {
        StackOps::FromStack => vm.stack.push(res),
        StackOps::FromWorkBench => vm.stack.push_to_workbench(res),
    };

    Ok(vm)
}

pub fn stdlib_string_stack_random_name_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::Name,
        "STRING.RANDOM.NAME".to_string(),
    )
}
pub fn stdlib_string_workbench_random_name_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::Name,
        "STRING.RANDOM.NAME.".to_string(),
    )
}

pub fn stdlib_string_stack_random_last_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::LastName,
        "STRING.RANDOM.LAST".to_string(),
    )
}
pub fn stdlib_string_workbench_random_last_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::LastName,
        "STRING.RANDOM.LAST.".to_string(),
    )
}

pub fn stdlib_string_stack_random_fullname_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::FullName,
        "STRING.RANDOM.FULLNAME".to_string(),
    )
}
pub fn stdlib_string_workbench_random_fullname_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::FullName,
        "STRING.RANDOM.FULLNAME.".to_string(),
    )
}

pub fn stdlib_string_stack_random_password_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::Password,
        "STRING.RANDOM.PASSWORD".to_string(),
    )
}
pub fn stdlib_string_workbench_random_password_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::Password,
        "STRING.RANDOM.PASSWORD.".to_string(),
    )
}

pub fn stdlib_string_stack_random_phone_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::Phone,
        "STRING.RANDOM.PHONE".to_string(),
    )
}
pub fn stdlib_string_workbench_random_phone_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::Phone,
        "STRING.RANDOM.PHONE.".to_string(),
    )
}

pub fn stdlib_string_stack_random_ipv4_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::IPv4,
        "STRING.RANDOM.IPV4".to_string(),
    )
}
pub fn stdlib_string_workbench_random_ipv4_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::IPv4,
        "STRING.RANDOM.IPV4.".to_string(),
    )
}

pub fn stdlib_string_stack_random_word_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromStack,
        StringRandomAlgorithm::Word,
        "STRING.RANDOM.WORD".to_string(),
    )
}
pub fn stdlib_string_workbench_random_word_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_random_base(
        vm,
        StackOps::FromWorkBench,
        StringRandomAlgorithm::Word,
        "STRING.RANDOM.WORD.".to_string(),
    )
}

pub fn stdlib_string_stack_lipsum_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_lipsum_base(vm, StackOps::FromStack, "STRING.RANDOM.LOREM".to_string())
}
pub fn stdlib_string_workbench_lipsum_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    string_lipsum_base(
        vm,
        StackOps::FromWorkBench,
        "STRING.RANDOM.LOREM.".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "string.random.name".to_string(),
        stdlib_string_stack_random_name_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.name.".to_string(),
        stdlib_string_workbench_random_name_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.lastname".to_string(),
        stdlib_string_stack_random_last_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.lastname.".to_string(),
        stdlib_string_workbench_random_last_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.fullname".to_string(),
        stdlib_string_stack_random_fullname_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.fullname.".to_string(),
        stdlib_string_workbench_random_fullname_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.password".to_string(),
        stdlib_string_stack_random_password_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.password.".to_string(),
        stdlib_string_workbench_random_password_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.phone".to_string(),
        stdlib_string_stack_random_phone_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.phone.".to_string(),
        stdlib_string_workbench_random_phone_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.ipv4".to_string(),
        stdlib_string_stack_random_ipv4_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.ipv4.".to_string(),
        stdlib_string_workbench_random_ipv4_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.word".to_string(),
        stdlib_string_stack_random_word_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.word.".to_string(),
        stdlib_string_workbench_random_word_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.lorem".to_string(),
        stdlib_string_stack_lipsum_inline,
    )?;
    let _ = vm.vm.register_inline(
        "string.random.lorem.".to_string(),
        stdlib_string_workbench_lipsum_inline,
    )?;
    Ok(())
}
