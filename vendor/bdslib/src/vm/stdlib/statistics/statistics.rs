extern crate log;

use crate::vm::stdlib::statistics;
use bundcore::bundcore::Bund;
use easy_error::{Error, bail};
use rstats::*;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::{StackOps, VM};

#[derive(Debug, Clone)]
pub enum StatisticsAlgo {
    ArithmeticMean,
    GeometricMean,
    ArithmeticWeightedMean,
    GeometricWeightedMean,
    HarmonicMean,
    HarmonicWeightedMean,
    HarmonicSpread,
}

#[time_graph::instrument]
fn stats_statistics_base(
    vm: &mut VM,
    op: StackOps,
    smode: statistics::SourceMode,
    salgo: StatisticsAlgo,
    err_prefix: String,
) -> Result<&mut VM, Error> {
    match statistics::get_data::get_data(vm, op.clone(), smode, err_prefix.clone()) {
        Ok(source) => {
            let res_val = match salgo {
                StatisticsAlgo::ArithmeticMean => source.amean(),
                StatisticsAlgo::GeometricMean => source.gmean(),
                StatisticsAlgo::ArithmeticWeightedMean => source.awmean(),
                StatisticsAlgo::GeometricWeightedMean => source.gwmean(),
                StatisticsAlgo::HarmonicMean => source.hmean(),
                StatisticsAlgo::HarmonicWeightedMean => source.hwmean(),
                StatisticsAlgo::HarmonicSpread => source.hmad(),
            };
            let res = match res_val {
                Ok(res) => res,
                Err(err) => {
                    bail!("{} returned: {}", &err_prefix, err);
                }
            };
            let _ = match op {
                StackOps::FromStack => vm.stack.push(Value::from_float(res as f64)),
                StackOps::FromWorkBench => {
                    vm.stack.push_to_workbench(Value::from_float(res as f64))
                }
            };
        }
        Err(err) => {
            bail!("{} returned: {}", &err_prefix, err);
        }
    }
    Ok(vm)
}

pub fn stdlib_stats_stack_consume_amean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::ArithmeticMean,
        "STAT.MEAN.ARITHMETICMEAN".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_amean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::ArithmeticMean,
        "STAT.MEAN.ARITHMETICMEAN.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_amean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::ArithmeticMean,
        "STAT.MEAN.ARITHMETICMEAN,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_amean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::ArithmeticMean,
        "STAT.MEAN.ARITHMETICMEAN.,".to_string(),
    )
}
//
// AWMEAN
//
pub fn stdlib_stats_stack_consume_awmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::ArithmeticWeightedMean,
        "STAT.MEAN.ARITHMETICWEIGHTEDMEAN".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_awmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::ArithmeticWeightedMean,
        "STAT.MEAN.ARITHMETICWEIGHTEDMEAN.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_awmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::ArithmeticWeightedMean,
        "STAT.MEAN.ARITHMETICWEIGHTEDMEAN,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_awmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::ArithmeticWeightedMean,
        "STAT.MEAN.ARITHMETICWEIGHTEDMEAN.,".to_string(),
    )
}
//
// HMEAN
//
pub fn stdlib_stats_stack_consume_hmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicMean,
        "STAT.MEAN.HARMONIC".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_hmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicMean,
        "STAT.MEAN.HARMONIC.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_hmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicMean,
        "STAT.MEAN.HARMONIC,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_hmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicMean,
        "STAT.MEAN.HARMONIC.,".to_string(),
    )
}
//
// HWMEAN
//
pub fn stdlib_stats_stack_consume_hwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicWeightedMean,
        "STAT.MEAN.HARMONICWEIGHTED".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_hwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicWeightedMean,
        "STAT.MEAN.HARMONICWEIGHTED.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_hwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicWeightedMean,
        "STAT.MEAN.HARMONICWEIGHTED,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_hwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicWeightedMean,
        "STAT.MEAN.HARMONICWEIGHTED.,".to_string(),
    )
}
//
// GMEAN
//
pub fn stdlib_stats_stack_consume_gmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::GeometricMean,
        "STAT.MEAN.GEOMETRIC".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_gmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::GeometricMean,
        "STAT.MEAN.GEOMETRIC.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_gmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::GeometricMean,
        "STAT.MEAN.GEOMETRIC,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_gmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::GeometricMean,
        "STAT.MEAN.GEOMETRIC.,".to_string(),
    )
}
//
// GWMEAN
//
pub fn stdlib_stats_stack_consume_gwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::GeometricWeightedMean,
        "STAT.MEAN.GEOMETRICWEIGHTED".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_gwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::GeometricWeightedMean,
        "STAT.MEAN.GEOMETRICWEIGHTED.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_gwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::GeometricWeightedMean,
        "STAT.MEAN.GEOMETRICWEIGHTED,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_gwmean(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::GeometricWeightedMean,
        "STAT.MEAN.GEOMETRICWEIGHTED.,".to_string(),
    )
}
//
// HSPREAD
//
pub fn stdlib_stats_stack_consume_hspread(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicSpread,
        "STAT.MEAN.HARMONICSPREAD".to_string(),
    )
}

pub fn stdlib_stats_wb_consume_hspread(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Consume,
        StatisticsAlgo::HarmonicSpread,
        "STAT.MEAN.HARMONICSPREAD.".to_string(),
    )
}

pub fn stdlib_stats_stack_keep_hspread(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromStack,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicSpread,
        "STAT.MEAN.HARMONICSPREAD,".to_string(),
    )
}

pub fn stdlib_stats_wb_keep_hspread(vm: &mut VM) -> Result<&mut VM, Error> {
    stats_statistics_base(
        vm,
        StackOps::FromWorkBench,
        statistics::SourceMode::Keep,
        StatisticsAlgo::HarmonicSpread,
        "STAT.MEAN.HARMONICSPREAD.,".to_string(),
    )
}

pub fn init_stdlib(vm: &mut Bund) -> Result<(), Error> {
    let _ = vm.vm.register_inline(
        "stat.mean.arithmetic".to_string(),
        stdlib_stats_stack_consume_amean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmetic.".to_string(),
        stdlib_stats_wb_consume_amean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmetic,".to_string(),
        stdlib_stats_stack_keep_amean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmetic.,".to_string(),
        stdlib_stats_wb_keep_amean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.arithmeticweighted".to_string(),
        stdlib_stats_stack_consume_awmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmeticweighted.".to_string(),
        stdlib_stats_wb_consume_awmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmeticweighted,".to_string(),
        stdlib_stats_stack_keep_awmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.arithmeticweighted.,".to_string(),
        stdlib_stats_wb_keep_awmean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.harmonic".to_string(),
        stdlib_stats_stack_consume_hmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonic.".to_string(),
        stdlib_stats_wb_consume_hmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonic,".to_string(),
        stdlib_stats_stack_keep_hmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonic.,".to_string(),
        stdlib_stats_wb_keep_hmean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.geometric".to_string(),
        stdlib_stats_stack_consume_gmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometric.".to_string(),
        stdlib_stats_wb_consume_gmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometric,".to_string(),
        stdlib_stats_stack_keep_gmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometric.,".to_string(),
        stdlib_stats_wb_keep_gmean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.geometricweighted".to_string(),
        stdlib_stats_stack_consume_gwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometricweighted.".to_string(),
        stdlib_stats_wb_consume_gwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometricweighted,".to_string(),
        stdlib_stats_stack_keep_gwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.geometricweighted.,".to_string(),
        stdlib_stats_wb_keep_gwmean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.harmonicweighted".to_string(),
        stdlib_stats_stack_consume_hwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicweighted.".to_string(),
        stdlib_stats_wb_consume_hwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicweighted,".to_string(),
        stdlib_stats_stack_keep_hwmean,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicweighted.,".to_string(),
        stdlib_stats_wb_keep_hwmean,
    );

    let _ = vm.vm.register_inline(
        "stat.mean.harmonicspread".to_string(),
        stdlib_stats_stack_consume_hspread,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicspread.".to_string(),
        stdlib_stats_wb_consume_hspread,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicspread,".to_string(),
        stdlib_stats_stack_keep_hspread,
    );
    let _ = vm.vm.register_inline(
        "stat.mean.harmonicspread.,".to_string(),
        stdlib_stats_wb_keep_hspread,
    );

    Ok(())
}
