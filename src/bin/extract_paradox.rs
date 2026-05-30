use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hoi4_parser::{parse_loc, parse_owned, ObjectNode, Value};
use rayon::prelude::*;
use serde_json::{Map, Value as JsonValue};

#[derive(Clone, Copy, Debug)]
enum Mode {
    Script,
    Loc,
}

impl Mode {
    fn default_exts(self) -> &'static [&'static str] {
        match self {
            Mode::Script => &["txt", "gfx"],
            Mode::Loc => &["yml"],
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("执行失败: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err(format!(
            "用法: extract_paradox <input_path> <output_dir> [--mode script|loc] [--ext yml,txt]\n  传入文件 → 单文件解析\n  传入目录 → 递归镜像目录结构\n  当前接收: {args:?}"
        ));
    }

    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let mode = parse_mode_arg(&args[2..])?.unwrap_or(Mode::Script);
    let exts = parse_ext_arg(&args[2..])
        .unwrap_or_else(|| mode.default_exts().iter().map(|s| s.to_string()).collect());

    if !input.exists() {
        return Err(format!("输入路径不存在: {}", input.display()));
    }

    let started = Instant::now();
    fs::create_dir_all(&output)
        .map_err(|e| format!("创建输出目录失败 {}: {e}", output.display()))?;

    let files = if input.is_file() {
        vec![input.clone()]
    } else {
        collect_files(&input, &exts)
            .map_err(|e| format!("扫描目录失败 {}: {e}", input.display()))?
    };

    if files.is_empty() {
        println!("未发现匹配文件 (扩展名: {exts:?})");
        return Ok(());
    }

    let base = if input.is_file() {
        input.parent().unwrap_or_else(|| Path::new(""))
    } else {
        input.as_path()
    };

    let results: Vec<Result<(), (PathBuf, String)>> = files
        .par_iter()
        .map(|file| process_one(file, base, &output, mode))
        .collect();

    let mut ok = 0usize;
    let mut errors = Vec::new();
    for r in results {
        match r {
            Ok(()) => ok += 1,
            Err(e) => errors.push(e),
        }
    }

    let elapsed = started.elapsed();
    println!(
        "完成 [{:?}]: 成功 {ok}, 失败 {}, 耗时 {:.2}s",
        mode,
        errors.len(),
        elapsed.as_secs_f64()
    );
    if !errors.is_empty() {
        for (path, err) in &errors {
            eprintln!(" - {} -> {err}", path.display());
        }
        return Err(format!("{} 个文件解析失败", errors.len()));
    }
    Ok(())
}

fn parse_mode_arg(rest: &[String]) -> Result<Option<Mode>, String> {
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        if arg == "--mode" {
            let value = iter.next().ok_or_else(|| "--mode 需要参数".to_string())?;
            return match value.as_str() {
                "script" => Ok(Some(Mode::Script)),
                "loc" => Ok(Some(Mode::Loc)),
                other => Err(format!("未知 --mode {other}（支持 script | loc）")),
            };
        }
    }
    Ok(None)
}

fn parse_ext_arg(rest: &[String]) -> Option<Vec<String>> {
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        if arg == "--ext" {
            let value = iter.next()?;
            return Some(
                value
                    .split(',')
                    .map(|s| s.trim().trim_start_matches('.').to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect(),
            );
        }
    }
    None
}

fn process_one(
    file: &Path,
    base: &Path,
    output_root: &Path,
    mode: Mode,
) -> Result<(), (PathBuf, String)> {
    let source =
        fs::read_to_string(file).map_err(|e| (file.to_path_buf(), format!("读取失败: {e}")))?;
    if source.trim().is_empty() {
        return Ok(());
    }

    let json = match mode {
        Mode::Script => {
            let document =
                parse_owned(source).map_err(|e| (file.to_path_buf(), format!("解析失败: {e}")))?;
            value_to_json(document.root())
        }
        Mode::Loc => {
            let loc_file =
                parse_loc(&source).map_err(|e| (file.to_path_buf(), format!("解析失败: {e}")))?;
            serde_json::to_value(&loc_file)
                .map_err(|e| (file.to_path_buf(), format!("序列化失败: {e}")))?
        }
    };

    let relative = file.strip_prefix(base).unwrap_or(file);
    let mut out_path = output_root.join(relative);
    out_path.set_extension("json");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            (
                file.to_path_buf(),
                format!("创建目录失败 {}: {e}", parent.display()),
            )
        })?;
    }

    let text = serde_json::to_string_pretty(&json)
        .map_err(|e| (file.to_path_buf(), format!("序列化失败: {e}")))?;
    fs::write(&out_path, text).map_err(|e| {
        (
            file.to_path_buf(),
            format!("写入失败 {}: {e}", out_path.display()),
        )
    })?;
    Ok(())
}

fn collect_files(dir: &Path, exts: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_files_inner(dir, exts, &mut out)?;
    Ok(out)
}

fn collect_files_inner(dir: &Path, exts: &[String], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_inner(&path, exts, out)?;
        } else if matches_ext(&path, exts) {
            out.push(path);
        }
    }
    Ok(())
}

fn matches_ext(path: &Path, exts: &[String]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|target| target.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

fn value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Scalar(s) => JsonValue::String(s.clone()),
        Value::Array(arr) => JsonValue::Array(arr.iter().map(value_to_json).collect()),
        Value::Object(obj) | Value::AnonymousObject(obj) => object_to_json(obj),
    }
}

fn object_to_json(obj: &ObjectNode) -> JsonValue {
    let mut map = Map::new();
    for entry in obj.entries() {
        let key = entry.key();
        let final_key = if let Some(suffix) = &entry.metadata().duplicate_suffix {
            format!("{}_{}", key, suffix)
        } else if let Some(idx) = entry.metadata().duplicate_index {
            format!("{}_{}", key, idx)
        } else {
            key.to_string()
        };
        map.insert(final_key, value_to_json(entry.value()));
    }
    JsonValue::Object(map)
}
