use git2::{build::CheckoutBuilder, ObjectType, Repository};

pub(crate) fn hard_reset_to_head(repo: &Repository) -> Result<(), git2::Error> {
    // 获取当前HEAD指向的提交
    let head = repo.head()?;
    let commit = repo.find_commit(
        head.target()
            .ok_or(git2::Error::from_str("HEAD does not point to a commit"))?,
    )?;

    // 获取当前提交的树
    let tree = commit.tree()?;

    // 创建CheckoutBuilder，设置为强制检出，以确保工作目录的变更
    let mut checkout_opts = CheckoutBuilder::new();
    checkout_opts.force();

    // 正确地将tree转换为Object再进行检出
    let tree_obj = tree.into_object();
    repo.checkout_tree(&tree_obj as &git2::Object, Some(&mut checkout_opts))?;
    Ok(())
}

pub(crate) fn print_all_tags(repo: &Repository) {
    let tags = repo.tag_names(None).unwrap();
    for tag_name in tags.iter().flatten() {
        let tag_ref = repo
            .find_reference(&format!("refs/tags/{}", tag_name))
            .unwrap();
        // 解析标签指向的对象
        if let Ok(tag_object) = tag_ref.peel_to_tag() {
            // Annotated 标签
            let target_commit = tag_object.target().unwrap().peel_to_commit().unwrap();
            println!(
                "Annotated Tag: {}, Commit: {}, Message: {}",
                tag_name,
                target_commit.id(),
                tag_object.message().unwrap_or("No message")
            );
        } else {
            // 轻量级标签可能不能直接转换为 annotated 标签对象
            // 直接获取引用指向的提交
            let commit_object = tag_ref.peel(ObjectType::Commit).unwrap();
            let commit = commit_object
                .into_commit()
                .expect("Failed to peel into commit");
            println!("Lightweight Tag: {}, Commit: {}", tag_name, commit.id());
            // 轻量级标签没有存储消息
        }
    }
}
