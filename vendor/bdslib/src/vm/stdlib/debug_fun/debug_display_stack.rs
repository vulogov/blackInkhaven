extern crate log;

use bundcore::bundcore::Bund;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use easy_error::{Error, bail};
use rust_multistackvm::multistackvm::VM;

pub fn stdlib_debug_display_stack(vm: &mut VM) -> Result<&mut VM, Error> {
    let current_stack = match vm.stack.current() {
        Some(current_stack) => current_stack,
        None => {
            bail!("Error getting current stack");
        }
    };
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    for n in &current_stack.stack {
        table.add_row(vec![format!("{:?}", &n)]);
    }
    println!("{table}");
    Ok(vm)
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "debug.display_stack".to_string(),
        stdlib_debug_display_stack,
    );
    Ok(())
}
