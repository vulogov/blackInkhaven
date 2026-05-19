use rust_dynamic::types::*;
use rust_multistackvm::multistackvm::{VM};
use bund_language_parser::bund_parse;
use easy_error::{Error, bail};

#[time_graph::instrument]
pub fn bund_compile_and_eval(vm: &mut VM, code: String) -> Result<&mut VM, Error>  {
    let source = format!("{}\n", code.clone());
    match bund_parse(&source) {
        Ok(words) => {
            for word in words {
                match word.dt {
                    NONE => {
                        continue;
                    }
                    EXIT => {
                        break;
                    }
                    ERROR => {
                        match word.cast_error() {
                            Ok(error) => {
                                bail!("Detected an Error posted on the stack: {:?}", error);
                            }
                            Err(err) => {
                                bail!("Detected an Error posted on the stack, but extraction of error is failed: {}", err);
                            }
                        }
                    }
                    _ => {
                        match vm.apply(word.clone()) {
                            Ok(_) => {}
                            Err(err) => {
                                bail!("Attempt to evaluate value {:?} returned error: {}", &word, err);
                            }
                        }
                    }
                }
            }
        }
        Err(err) => {
            bail!("{}", err);
        }
    }
    Ok(vm)
}
