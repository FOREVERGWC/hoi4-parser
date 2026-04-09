use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hoi4_parser::{generate, parse_owned};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

const INPUT_ROOT: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV";
const OUTPUT_ROOT: &str = r"C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output";
const DEFAULT_THREADS: usize = 16;
const THREADS_ENV: &str = "HOI4_PARSER_THREADS";
const INPUT_SUBDIRS: [&str; 3] = ["common", "events", "history"];

fn main() {
    if let Err(err) = run() {
        eprintln!("执行失败: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let threads = resolve_threads()?;
    configure_rayon_threads(threads)?;

    let start = Instant::now();
    let input_root = Path::new(INPUT_ROOT);
    if !input_root.exists() {
        return Err(format!("输入目录不存在: {}", input_root.display()));
    }

    let output_root = Path::new(OUTPUT_ROOT);
    fs::create_dir_all(output_root)
        .map_err(|e| format!("创建输出目录失败 {}: {e}", output_root.display()))?;

    let mut txt_files = Vec::new();
    for subdir in INPUT_SUBDIRS {
        let dir = input_root.join(subdir);
        if !dir.exists() {
            return Err(format!("输入子目录不存在: {}", dir.display()));
        }
        let mut files = collect_txt_files_parallel(&dir)
            .map_err(|e| format!("扫描目录失败 {}: {e}", dir.display()))?;
        txt_files.append(&mut files);
    }

    if txt_files.is_empty() {
        println!("未发现 txt 文件: {}", input_root.display());
        return Ok(());
    }
    prepare_output_dirs(input_root, output_root, &txt_files)
        .map_err(|e| format!("预创建输出目录失败: {e}"))?;
    println!("并行线程数: {threads}");

    let (success_count, skipped_count, mut failed) = txt_files
        .into_par_iter()
        .fold(
            || (0usize, 0usize, Vec::<(PathBuf, String)>::new()),
            |mut acc, file_path| {
                match process_one_file(input_root, output_root, &file_path) {
                    Ok(()) => acc.0 += 1,
                    Err(ProcessFileOutcome::SkippedEmpty)
                    | Err(ProcessFileOutcome::SkippedNonParadoxText) => acc.1 += 1,
                    Err(ProcessFileOutcome::Failed(e)) => acc.2.push((file_path, e)),
                }
                acc
            },
        )
        .reduce(
            || (0usize, 0usize, Vec::<(PathBuf, String)>::new()),
            |mut a, mut b| {
                a.0 += b.0;
                a.1 += b.1;
                a.2.append(&mut b.2);
                a
            },
        );
    failed.sort_by(|a, b| a.0.cmp(&b.0));

    println!(
        "处理完成: 成功 {success_count}, 跳过空文件 {skipped_count}, 失败 {}",
        failed.len()
    );
    let elapsed = start.elapsed();
    println!(
        "总耗时: {:.3} 秒 ({} ms)",
        elapsed.as_secs_f64(),
        elapsed.as_millis()
    );
    if !failed.is_empty() {
        println!("失败详情:");
        for (path, err) in failed {
            println!(" - {} -> {}", path.display(), err);
        }
    }

    Ok(())
}

enum ProcessFileOutcome {
    SkippedEmpty,
    SkippedNonParadoxText,
    Failed(String),
}

fn resolve_threads() -> Result<usize, String> {
    match std::env::var(THREADS_ENV) {
        Ok(raw) => {
            let parsed = raw
                .parse::<usize>()
                .map_err(|e| format!("环境变量 {THREADS_ENV} 解析失败: {e}"))?;
            if parsed == 0 {
                return Err(format!("环境变量 {THREADS_ENV} 必须大于 0"));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_THREADS),
        Err(e) => Err(format!("读取环境变量 {THREADS_ENV} 失败: {e}")),
    }
}

fn configure_rayon_threads(threads: usize) -> Result<(), String> {
    ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .map_err(|e| format!("初始化线程池失败: {e}"))
}

fn process_one_file(
    input_root: &Path,
    output_root: &Path,
    file_path: &Path,
) -> Result<(), ProcessFileOutcome> {
    let source = fs::read_to_string(file_path)
        .map_err(|e| ProcessFileOutcome::Failed(format!("读取失败: {e}")))?;
    if source.trim().is_empty() {
        return Err(ProcessFileOutcome::SkippedEmpty);
    }
    if is_plain_list_text(&source) {
        return Err(ProcessFileOutcome::SkippedNonParadoxText);
    }
    let document =
        parse_owned(source).map_err(|e| ProcessFileOutcome::Failed(format!("解析失败: {e}")))?;
    let generated =
        generate(&document).map_err(|e| ProcessFileOutcome::Failed(format!("还原失败: {e}")))?;

    let relative = file_path
        .strip_prefix(input_root)
        .map_err(|e| ProcessFileOutcome::Failed(format!("路径转换失败: {e}")))?;
    let output_path = output_root.join(relative);
    fs::write(&output_path, generated).map_err(|e| {
        ProcessFileOutcome::Failed(format!("写入失败 {}: {e}", output_path.display()))
    })?;
    Ok(())
}

fn prepare_output_dirs(
    input_root: &Path,
    output_root: &Path,
    files: &[PathBuf],
) -> Result<(), String> {
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    dirs.insert(output_root.to_path_buf());
    for subdir in INPUT_SUBDIRS {
        dirs.insert(output_root.join(subdir));
    }
    for file_path in files {
        let relative = file_path
            .strip_prefix(input_root)
            .map_err(|e| format!("路径转换失败 {}: {e}", file_path.display()))?;
        if let Some(parent) = relative.parent() {
            dirs.insert(output_root.join(parent));
        }
    }
    for dir in dirs {
        fs::create_dir_all(&dir).map_err(|e| format!("创建目录失败 {}: {e}", dir.display()))?;
    }
    Ok(())
}

fn collect_txt_files_parallel(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_txt_files_parallel_inner(dir, &mut out)?;
    Ok(out)
}

fn collect_txt_files_parallel_inner(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut subdirs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if is_txt_file(&path) {
            out.push(path);
        }
    }

    let nested_results: Vec<std::io::Result<Vec<PathBuf>>> = subdirs
        .par_iter()
        .map(|subdir| collect_txt_files_parallel(subdir))
        .collect();

    for result in nested_results {
        match result {
            Ok(mut files) => out.append(&mut files),
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

fn is_txt_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("txt"))
        .unwrap_or(false)
}

fn is_plain_list_text(source: &str) -> bool {
    let mut has_content = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        has_content = true;
        if trimmed.contains('=') || trimmed.contains('{') || trimmed.contains('}') {
            return false;
        }
    }
    has_content
}
