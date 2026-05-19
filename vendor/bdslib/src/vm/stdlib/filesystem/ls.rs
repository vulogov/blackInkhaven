extern crate log;

use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};
use scan_dir::ScanDir;

#[derive(Debug, Clone)]
pub enum LsOperations {
    Files,
    Directories,
    Both,
}

pub fn stdlib_bund_fs_ls_base(
    vm: &mut VM,
    op: StackOps,
    lop: LsOperations,
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
    let dir_val = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };

    let dir_value = match dir_val {
        Some(dir_value) => dir_value,
        None => {
            bail!("{} returns: NO DATA #1", &err_prefix);
        }
    };

    let dir_val_name = match dir_value.cast_string() {
        Ok(dir_val_name) => dir_val_name,
        Err(err) => {
            bail!("Error casting string for target {}: {}", &err_prefix, err);
        }
    };
    let mut scan_all = ScanDir::all();
    let mut scan_files = ScanDir::files();
    let mut scan_dirs = ScanDir::dirs();

    let scan = match lop {
        LsOperations::Files => scan_files.skip_hidden(true).skip_backup(true),
        LsOperations::Directories => scan_dirs.skip_hidden(true),
        LsOperations::Both => scan_all.skip_backup(true),
    };
    let mut res = Value::list();
    let _ = scan.walk(dir_val_name, |iter| {
        for (entry, _) in iter {
            res = res.push(Value::from_string(format!("{}", entry.path().display())));
        }
    });
    vm.stack.push(res);
    Ok(vm)
}

pub fn stdlib_bund_fs_ls_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromStack,
        LsOperations::Both,
        "FS.LS".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_workbench_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromWorkBench,
        LsOperations::Both,
        "FS.LS.".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_dir_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromStack,
        LsOperations::Directories,
        "FS.LS.DIR".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_dir_workbench_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromWorkBench,
        LsOperations::Directories,
        "FS.LS.DIR.".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_files_stack_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromStack,
        LsOperations::Files,
        "FS.LS.FILES".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_files_workbench_inline(vm: &mut VM) -> Result<&mut VM, Error> {
    stdlib_bund_fs_ls_base(
        vm,
        StackOps::FromWorkBench,
        LsOperations::Files,
        "FS.LS.FILES.".to_string(),
    )
}

pub fn stdlib_bund_fs_ls_disabled(_vm: &mut VM) -> Result<&mut VM, Error> {
    bail!("bund FS.LS functions disabled with --noio");
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm
        .vm
        .register_inline("fs.ls".to_string(), stdlib_bund_fs_ls_stack_inline);
    let _ = vm
        .vm
        .register_inline("fs.ls.".to_string(), stdlib_bund_fs_ls_workbench_inline);
    let _ = vm
        .vm
        .register_inline("fs.ls.dir".to_string(), stdlib_bund_fs_ls_dir_stack_inline);
    let _ = vm.vm.register_inline(
        "fs.ls.dir.".to_string(),
        stdlib_bund_fs_ls_dir_workbench_inline,
    );
    let _ = vm.vm.register_inline(
        "fs.ls.files".to_string(),
        stdlib_bund_fs_ls_files_stack_inline,
    );
    let _ = vm.vm.register_inline(
        "fs.ls.files.".to_string(),
        stdlib_bund_fs_ls_files_workbench_inline,
    );
    Ok(())
}
