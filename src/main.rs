use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hoi4_parser::{generate, parse};

const INPUT_ROOT: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV\common";
const OUTPUT_ROOT: &str = r"C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output";

fn main() {
    if let Err(err) = run() {
        eprintln!("执行失败: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let start = Instant::now();
    let input_root = Path::new(INPUT_ROOT);
    if !input_root.exists() {
        return Err(format!("输入目录不存在: {}", input_root.display()));
    }

    let output_root = Path::new(OUTPUT_ROOT);
    fs::create_dir_all(output_root)
        .map_err(|e| format!("创建输出目录失败 {}: {e}", output_root.display()))?;

    let mut txt_files = Vec::new();
    collect_txt_files(input_root, &mut txt_files)
        .map_err(|e| format!("扫描目录失败 {}: {e}", input_root.display()))?;

    if txt_files.is_empty() {
        println!("未发现 txt 文件: {}", input_root.display());
        return Ok(());
    }

    let mut success_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed: Vec<(PathBuf, String)> = Vec::new();

    for file_path in txt_files {
        match process_one_file(input_root, output_root, &file_path) {
            Ok(()) => success_count += 1,
            Err(ProcessFileOutcome::SkippedEmpty) => skipped_count += 1,
            Err(ProcessFileOutcome::SkippedNonParadoxText) => skipped_count += 1,
            Err(ProcessFileOutcome::Failed(e)) => failed.push((file_path, e)),
        }
    }

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
    let document = parse(&source).map_err(|e| ProcessFileOutcome::Failed(format!("解析失败: {e}")))?;
    let generated =
        generate(&document).map_err(|e| ProcessFileOutcome::Failed(format!("还原失败: {e}")))?;

    let relative = file_path
        .strip_prefix(input_root)
        .map_err(|e| ProcessFileOutcome::Failed(format!("路径转换失败: {e}")))?;
    let output_path = output_root.join(relative);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| {
                ProcessFileOutcome::Failed(format!(
                    "创建输出子目录失败 {}: {e}",
                    parent.display()
                ))
            })?;
    }
    fs::write(&output_path, generated).map_err(|e| {
        ProcessFileOutcome::Failed(format!("写入失败 {}: {e}", output_path.display()))
    })?;
    Ok(())
}

fn collect_txt_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_txt_files(&path, out)?;
        } else if is_txt_file(&path) {
            out.push(path);
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
