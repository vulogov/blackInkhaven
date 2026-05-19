extern crate log;

use crate::vm::stdlib::statistics;
use easy_error::{Error, bail};
use rust_dynamic::types::*;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[time_graph::instrument]
fn get_data_for_stat_from_stack_or_workbench(
    vm: &mut VM,
    op: StackOps,
    err_prefix: String,
) -> Result<Vec<f64>, Error> {
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
    let mut res: Vec<f64> = Vec::new();
    match op {
        StackOps::FromStack => loop {
            let value = match vm.stack.pull() {
                Some(value) => value,
                None => {
                    break;
                }
            };
            if value.type_of() == NODATA {
                break;
            }
            let f_value = match value.conv(FLOAT) {
                Ok(f_value) => f_value,
                Err(err) => {
                    bail!("{} returns during conversion: {}", &err_prefix, err);
                }
            };
            res.push(f_value.cast_float().unwrap());
        },
        StackOps::FromWorkBench => loop {
            let value = match vm.stack.pull_from_workbench() {
                Some(value) => value,
                None => {
                    break;
                }
            };
            if value.type_of() == NODATA {
                break;
            }
            let f_value = match value.conv(FLOAT) {
                Ok(f_value) => f_value,
                Err(err) => {
                    bail!("{} returns during conversion: {}", &err_prefix, err);
                }
            };
            res.push(f_value.cast_float().unwrap());
        },
    }
    Ok(res)
}

#[time_graph::instrument]
fn get_data_from_metrics(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<Vec<f64>, Error> {
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
    let value = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let metric_value = match value {
        Some(metric_value) => metric_value,
        None => {
            bail!("{} returns NO DATA", &err_prefix);
        }
    };
    if metric_value.type_of() != METRICS {
        bail!("{} did not find a metric type on the stack", &err_prefix);
    }
    let mut res: Vec<f64> = Vec::new();
    match metric_value.cast_metrics() {
        Ok(metrics) => {
            for v in metrics {
                res.push(v.data);
            }
        }
        Err(err) => {
            bail!("{} returns NO METRICS: {}", &err_prefix, err);
        }
    }
    match smode {
        statistics::SourceMode::Keep => {
            let _ = match op {
                StackOps::FromStack => vm.stack.push(metric_value),
                StackOps::FromWorkBench => vm.stack.push_to_workbench(metric_value),
            };
        }
        _ => {}
    }
    Ok(res)
}

#[time_graph::instrument]
pub fn get_data_from_list(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<Vec<f64>, Error> {
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
    log::debug!("Taking data from stack.");
    let value = match op {
        StackOps::FromStack => vm.stack.pull(),
        StackOps::FromWorkBench => vm.stack.pull_from_workbench(),
    };
    let list_value = match value {
        Some(list_value) => list_value,
        None => {
            bail!("{} returns NO DATA", &err_prefix);
        }
    };
    if list_value.type_of() != LIST {
        bail!("{} did not find a list type on the stack", &err_prefix);
    }
    let mut res: Vec<f64> = Vec::new();
    match list_value.cast_list() {
        Ok(lvalue) => {
            for v in lvalue {
                match v.conv(FLOAT) {
                    Ok(float_value) => match float_value.cast_float() {
                        Ok(fdata) => {
                            res.push(fdata);
                        }
                        Err(err) => {
                            bail!("{} error FLOAT casting: {}", &err_prefix, err);
                        }
                    },
                    Err(err) => {
                        bail!("{} error FLOAT conversion: {}", &err_prefix, err);
                    }
                }
            }
        }
        Err(err) => {
            bail!("{} returns NO METRICS: {}", &err_prefix, err);
        }
    }
    match smode {
        statistics::SourceMode::Keep => {
            log::debug!("Keeping value in the stack by reinserting.");
            let _ = match op {
                StackOps::FromStack => vm.stack.push(list_value),
                StackOps::FromWorkBench => vm.stack.push_to_workbench(list_value),
            };
        }
        _ => {}
    }
    Ok(res)
}

#[time_graph::instrument]
pub fn get_data(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    err_prefix: String,
) -> Result<Vec<f64>, Error> {
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
    let value = match op {
        StackOps::FromStack => vm.stack.peek(),
        StackOps::FromWorkBench => vm.stack.workbench.peek().cloned(),
    };
    match value {
        Some(data_value) => match data_value.type_of() {
            LIST => {
                log::debug!("Processing data from list.");
                return get_data_from_list(vm, op, smode, err_prefix);
            }
            METRICS => {
                log::debug!("Processing data from metrics.");
                return get_data_from_metrics(vm, op, smode, err_prefix);
            }
            NODATA => bail!("{} END OF DATA", &err_prefix),
            _ => {
                log::debug!("Processing data from stack.");
                return get_data_for_stat_from_stack_or_workbench(vm, op, err_prefix);
            }
        },
        None => {
            bail!("{} returns NO DATA", &err_prefix);
        }
    }
}
