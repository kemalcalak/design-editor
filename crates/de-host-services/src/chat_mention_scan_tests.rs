use super::*;

fn temp_project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "de-mention-scan-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn scan_lists_visible_files_and_skips_hidden_and_build_folders() {
    let root = temp_project("scan");
    fs::create_dir_all(root.join("docs/deep")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::create_dir_all(root.join("node_modules/x")).unwrap();
    fs::write(root.join("brief.md"), "hi").unwrap();
    fs::write(root.join("docs/deep/spec.txt"), "spec").unwrap();
    fs::write(root.join(".env"), "SECRET=1").unwrap();
    fs::write(root.join(".git/HEAD"), "ref").unwrap();
    fs::write(root.join("node_modules/x/index.js"), "").unwrap();
    assert_eq!(
        scan_project_files(&root, None),
        ProjectScan {
            files: vec!["brief.md".to_string(), "docs/deep/spec.txt".to_string()],
            limited: false,
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scan_stops_at_the_depth_limit_and_says_so() {
    let root = temp_project("depth");
    let mut deep = root.clone();
    for level in 0..=MAX_SCAN_DEPTH {
        deep = deep.join(format!("d{level}"));
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("too-deep.txt"), "x").unwrap();
    let scan = scan_project_files(&root, None);
    assert!(scan.files.is_empty());
    assert!(scan.limited);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scan_stops_after_the_folder_limit() {
    let root = temp_project("dirs");
    for i in 0..MAX_SCAN_DIRS + 5 {
        let dir = root.join(format!("d{i:04}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("f.txt"), "x").unwrap();
    }
    let scan = scan_project_files(&root, None);
    assert!(scan.limited);
    assert_eq!(scan.files.len(), MAX_SCAN_DIRS - 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_home_folder_root_is_listed_one_level_deep_with_a_warning() {
    let home = temp_project("home");
    fs::create_dir_all(home.join("Documents")).unwrap();
    fs::write(home.join("Documents/private.txt"), "x").unwrap();
    fs::write(home.join("design.de"), "{}").unwrap();
    let scan = scan_project_files(&home, Some(&home));
    assert_eq!(scan.files, vec!["design.de".to_string()]);
    assert!(scan.limited);
    // A folder above home is just as broad.
    let parent = home.parent().unwrap().to_path_buf();
    assert!(scan_project_files(&parent, Some(&home)).limited);
    // A project folder inside home is listed normally.
    let scan = scan_project_files(&home.join("Documents"), Some(&home));
    assert_eq!(scan.files, vec!["private.txt".to_string()]);
    assert!(!scan.limited);
    fs::remove_dir_all(&home).ok();
}

fn wait_for(scanner: &mut ProjectFileScanner) -> ProjectFileList {
    for _ in 0..500 {
        if let Some(list) = scanner.poll() {
            return list;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the background listing never finished");
}

#[test]
fn scanner_lists_in_the_background_then_answers_from_its_cache() {
    let root = temp_project("bg");
    fs::write(root.join("a.md"), "x").unwrap();
    let mut scanner = ProjectFileScanner::new();
    assert!(scanner.request(&root).is_none(), "nothing cached yet");
    assert!(scanner.is_pending());
    let list = wait_for(&mut scanner);
    assert_eq!(list.root, root);
    assert_eq!(&*list.files, ["a.md".to_string()]);
    assert!(!list.warn);
    assert!(!scanner.is_pending());
    // Fresh cache: answered at once, no new listing.
    let cached = scanner.request(&root).unwrap();
    assert_eq!(&*cached.files, ["a.md".to_string()]);
    assert!(!scanner.is_pending());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_cut_listing_warns_once_per_folder() {
    let root = temp_project("warn");
    let mut deep = root.clone();
    for level in 0..=MAX_SCAN_DEPTH {
        deep = deep.join(format!("d{level}"));
    }
    fs::create_dir_all(&deep).unwrap();
    let mut scanner = ProjectFileScanner::new();
    scanner.request(&root);
    assert!(wait_for(&mut scanner).warn);
    scanner.cache = None;
    scanner.request(&root);
    assert!(!wait_for(&mut scanner).warn);
    fs::remove_dir_all(&root).ok();
}

fn cut_project(tag: &str) -> PathBuf {
    let root = temp_project(tag);
    let mut deep = root.clone();
    for level in 0..=MAX_SCAN_DEPTH {
        deep = deep.join(format!("d{level}"));
    }
    fs::create_dir_all(&deep).unwrap();
    root
}

#[test]
fn switching_back_to_a_warned_folder_does_not_warn_again() {
    let a = cut_project("warn-a");
    let b = cut_project("warn-b");
    let mut scanner = ProjectFileScanner::new();
    scanner.request(&a);
    assert!(wait_for(&mut scanner).warn);
    scanner.request(&b);
    assert!(wait_for(&mut scanner).warn);
    scanner.request(&a);
    assert!(!wait_for(&mut scanner).warn, "A → B → A warns once for A");
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}
