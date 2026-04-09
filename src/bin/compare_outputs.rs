use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

const DEFAULT_LEFT_ROOT: &str = r"C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output-java";
const DEFAULT_RIGHT_ROOT: &str = r"C:\Users\91658\Desktop\Projects\Rust\hoi4-parser\output";
const COMPARE_SUBDIRS: [&str; 3] = ["common", "events", "history"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let left_root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LEFT_ROOT));
    let right_root = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_RIGHT_ROOT));

    if let Err(err) = compare_output_dirs(&left_root, &right_root) {
        eprintln!("比对失败: {err}");
        std::process::exit(1);
    }
}

fn compare_output_dirs(left_root: &Path, right_root: &Path) -> Result<(), String> {
    if !left_root.exists() {
        return Err(format!("左目录不存在: {}", left_root.display()));
    }
    if !right_root.exists() {
        return Err(format!("右目录不存在: {}", right_root.display()));
    }

    let left_files = collect_txt_files_map(left_root)
        .map_err(|e| format!("扫描左目录失败 {}: {e}", left_root.display()))?;
    let right_files = collect_txt_files_map(right_root)
        .map_err(|e| format!("扫描右目录失败 {}: {e}", right_root.display()))?;

    let all_relative_paths: BTreeSet<String> = left_files
        .keys()
        .cloned()
        .chain(right_files.keys().cloned())
        .collect();

    let work_items: Vec<String> = all_relative_paths.into_iter().collect();
    let outcomes: Vec<Result<DiffOutcome, String>> = work_items
        .par_iter()
        .map(
            |relative| match (left_files.get(relative), right_files.get(relative)) {
                (Some(left_path), Some(right_path)) => {
                    let left_text = read_normalized_text(left_path)
                        .map_err(|e| format!("读取左文件失败 {}: {e}", left_path.display()))?;
                    let right_text = read_normalized_text(right_path)
                        .map_err(|e| format!("读取右文件失败 {}: {e}", right_path.display()))?;

                    if left_text == right_text {
                        Ok(DiffOutcome::Same)
                    } else {
                        let first_diff = first_diff_line(&left_text, &right_text);
                        Ok(DiffOutcome::Diff {
                            relative: relative.clone(),
                            detail: first_diff,
                        })
                    }
                }
                (Some(_), None) => Ok(DiffOutcome::OnlyLeft(relative.clone())),
                (None, Some(_)) => Ok(DiffOutcome::OnlyRight(relative.clone())),
                (None, None) => Ok(DiffOutcome::Same),
            },
        )
        .collect();

    let mut only_left = Vec::new();
    let mut only_right = Vec::new();
    let mut diff_content = Vec::new();
    let mut same_content_count = 0usize;
    for outcome in outcomes {
        match outcome? {
            DiffOutcome::Same => same_content_count += 1,
            DiffOutcome::OnlyLeft(relative) => only_left.push(relative),
            DiffOutcome::OnlyRight(relative) => only_right.push(relative),
            DiffOutcome::Diff { relative, detail } => diff_content.push((relative, detail)),
        }
    }

    only_left.sort();
    only_right.sort();
    diff_content.sort_by(|a, b| a.0.cmp(&b.0));

    println!("文本比对完成");
    println!("左目录: {}", left_root.display());
    println!("右目录: {}", right_root.display());
    println!("左侧 txt 文件数: {}", left_files.len());
    println!("右侧 txt 文件数: {}", right_files.len());
    println!("内容一致文件数: {same_content_count}");
    println!("内容不一致文件数: {}", diff_content.len());
    println!("仅左侧存在文件数: {}", only_left.len());
    println!("仅右侧存在文件数: {}", only_right.len());

    if !diff_content.is_empty() {
        println!("\n内容不一致示例(最多前20个):");
        for (relative, diff) in diff_content.iter().take(20) {
            println!(" - {relative} -> {diff}");
        }
    }
    if !only_left.is_empty() {
        println!("\n仅左侧存在示例(最多前20个):");
        for relative in only_left.iter().take(20) {
            println!(" - {relative}");
        }
    }
    if !only_right.is_empty() {
        println!("\n仅右侧存在示例(最多前20个):");
        for relative in only_right.iter().take(20) {
            println!(" - {relative}");
        }
    }

    Ok(())
}

enum DiffOutcome {
    Same,
    OnlyLeft(String),
    OnlyRight(String),
    Diff { relative: String, detail: String },
}

fn collect_txt_files_map(root: &Path) -> std::io::Result<HashMap<String, PathBuf>> {
    let mut txt_files = Vec::new();
    let has_structured_layout = COMPARE_SUBDIRS
        .iter()
        .any(|subdir| root.join(subdir).exists());

    if has_structured_layout {
        for subdir in COMPARE_SUBDIRS {
            let dir = root.join(subdir);
            if dir.exists() {
                collect_txt_files(&dir, &mut txt_files)?;
            }
        }
    } else {
        // 兼容旧输出结构：根目录直接是 common 的内容。
        collect_txt_files(root, &mut txt_files)?;
    }

    let mut map = HashMap::with_capacity(txt_files.len());
    for path in txt_files {
        if let Ok(relative) = path.strip_prefix(root) {
            let relative = relative.to_string_lossy().replace('\\', "/");
            let key = if has_structured_layout {
                relative
            } else {
                format!("common/{relative}")
            };
            map.insert(key, path);
        }
    }
    Ok(map)
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

fn read_normalized_text(path: &Path) -> std::io::Result<String> {
    let source = fs::read_to_string(path)?;
    Ok(source.replace("\r\n", "\n"))
}

fn first_diff_line(left: &str, right: &str) -> String {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    let min_len = left_lines.len().min(right_lines.len());
    for idx in 0..min_len {
        if left_lines[idx] != right_lines[idx] {
            let line_no = idx + 1;
            return format!(
                "第 {line_no} 行不同 | 左: {:?} | 右: {:?}",
                left_lines[idx], right_lines[idx]
            );
        }
    }
    if left_lines.len() != right_lines.len() {
        return format!(
            "行数不同 | 左: {} 行 | 右: {} 行",
            left_lines.len(),
            right_lines.len()
        );
    }
    "存在差异(未定位到首个不同行)".to_string()
}
