




# TuGraph模型设计

## 存储结构设计目标

我们的目的是创建一个图数据库模型，在TuGraph中有效地代表Rust的包管理系统中crate及其版本之间的依赖关系。这个模型旨在解决两个核心问题：
- 存储：表示Crate及其版本信息。通过两种类型的节点（CrateMaster和CrateVersion）明确区分crate的元信息和特定版本的信息。
- 计算：跟踪依赖关系。利用边（edges）来代表不同crate版本之间的依赖关系，使得能够追踪和查询一个crate的某个版本是否直接或间接依赖于另一个crate的特定版本。

## 节点设计

- CrateMaster：表示一个crate的主节点，存储与此crate相关的元信息。
  - 属性：
    - crate_name：String，crate的名称。
    - description：String，crate的简介或描述。
    - repository_url：String，crate的源代码仓库地址。
    - license：String，表明crate的许可证。
  - 用途：使用户能够通过crate名查询到主节点，获取crate的概览信息。
- CrateVersion：表示crate的一个具体版本，包含此版本特有的信息。
  - 属性：
    - version：String，此crate版本的版本号。
    - deps_spec：String，此版本依赖的规格描述，可以是版本号或版本范围。
    - published_date：DateTime，版本发布日期。
    - features：String Array，此crate版本的特性列表。
    - ...
  - 用途：存储每个版本的具体信息，支持识别不同版本间的细微差异。

## 边设计

- has_version：
  - 方向：从CrateMaster到CrateVersion。
  - 描述：表示crate主节点拥有的版本节点。
  - 属性无或者仅有版本发布的顺序信息（方便遍历）。
- depends_on：
  - 方向：从一个CrateVersion指向另一个CrateVersion。
  - 描述：表示一个crate版本依赖于另一个crate的特定版本。
  - 属性：
    - dependency_type：String，依赖的类型，比如dev, build, regular。
    - optional：Boolean，此依赖是否是可选的。
    - default_features：Boolean，是否使用了依赖的默认特性。
    - features：String Array，被激活的特性列表。
- updated_to:
  - 方向：从旧版本到新版本
  - 描述：
    - 通过遍历update_to边，我们可以高效地查询到一个crate的任何版本的完整更新历史。
    - 可以通过寻找没有指向其他版本的update_to出边的CrateVersion节点来快速定位到最新版本，这比比较日期或版本号字符串更直接且高效。
    - 当新版本发布时，仅需添加一个新的CrateVersion节点并建立一个指向它的UpdatedTo边。这种变化可以触发通知机制，通知所有订阅了旧版本更新信息的用户。
  - 属性：
    - updated_date: 更新日期，记录新版本发布的日期
    - change_log: 可选，记录从上一个版本到当前版本的主要更改摘要
