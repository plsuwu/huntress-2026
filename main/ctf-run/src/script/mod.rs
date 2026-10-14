//! The script-analysis toolkit: descriptor model and rubric, label-driven decoding, and the
//! per-language analysers that cannot be label-driven (cmd.exe model, JScript decoder evaluator,
//! PowerShell samples, VBScript control flow, advanced Bash, polyglot literals).

pub mod batch;
pub mod descriptor;
pub mod families;
pub mod jscript;
pub mod labelled;
