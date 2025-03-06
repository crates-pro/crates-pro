# Crates-pro各模块架构

## crates_pro模块

crates_pro模块为项目的入口，包含了程序的入口main.rs，在main.rs中调用接口core_controller.run()来启动crates-pro的运行。
core_controller.rs内定义了三个异步任务，分别为import，analysis，package。

import任务用来读取kafka消息队列中的消息，解析crate信息。

analysis任务用来对crate进行一些分析作业。

package任务用来打包解析出的数据到pg数据库中。

这三个异步任务通过设置环境变量来控制其是否执行，其中package对应的环境变量还用来标识前后端的连接，当环境变量CRATES_PRO_PACKAGE设置为1时，调用接口run_api_server()来连接前端。

该模块目前逻辑较为完备，短期无需添加代码及功能。

## data_transporter模块

data_transporter模块为连接前后端的桥梁，前面提到接口run_api_server()用来连接前端，该接口位于data_transporter模块的入口lib.rs中，其中定义着一些后端api及对应路由，这些接口对应着前端的一个个功能实现，下面是这些接口的解释。

### /api/search

用于进行crate的搜索，该接口接受前端的查询参数，调用模块search中的搜索和排序api，从pg数据库中按名称搜索crate并按名称相关度排序，搜索结果以json格式返回。

### /api/crates/{nsfront}/{nsbehind}/{cratename}/{version}

nsfront和nsfront分别表示namespace的前半部分和后半部分，cratename和version分别crate的名字和版本。

该接口用于查询特定名称和版本的crate信息，在repo_import模块中我们解析了crate信息，并且存到tugraph中。在该接口中我们会用这些参数到tugraph中查询解析出的crate信息，包括cve信息，dependency和dependent信息等，查询结果以json格式返回。

### /api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies

该接口用于获取crate的依赖项信息，对于一个crate，在其Cargo.toml文件中注明了该crate的依赖项信息，包含直接依赖和间接依赖，直接依赖为该crate的Cargo.toml文件中注明的依赖项，间接依赖为该依赖项crate的依赖，例如a依赖于b，b依赖于c，那么c是a的间接依赖。我们根据参数到tugraph中查询，查询结果以json格式返回。

### /api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependencies/graph

该接口首先获取当前crate的依赖项信息，并且将依赖项信息构建成依赖树，将依赖树以json格式返回，前端获取数据通过构建工具展示为依赖图。

### /api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/dependents

该接口用于获取crate的被依赖项信息，即哪些crate依赖于当前crate，包含直接和间接，直接为哪些crate的Cargo.toml中包含当前crate，以此类推间接。同样在tugraph中查询，结果以json格式返回。

### /api/crates/{nsfront}/{nsbehind}/{cratename}/{version}/versions

该接口用于获取当前crate的所有历史版本，同时从pg数据库中获取每个版本的相关信息，包括published_time，downloads，以及被依赖项信息。

### 补充说明

lib.rs中定义了上述众多api和路由，前端通过get或post调用这些后端api，触发api的执行，获取数据并且展示出来，这些api的实现位于handler.rs中，所有数据均从tugraph和pg数据库中获取，tugraph和pg的读取操作分别位于data_reader.rs和db.rs中。

对于data_packer.rs，其内定义的api目的是将解析出的数据导入到pg数据库中，导入pg数据库的目的是用作search模块进行搜索的数据库。

该模块主要用于实现前端所需功能，后续有新的功能需要继续添加代码及功能。

### 添加前后端功能流程

首先需要由前端设计接口及路由，请求类型，请求参数，返回值类型，在接口设计好之后，后端在lib.rs中添加路由，之后实现api，最后进行前后端联调。

## model模块

model: 对于一个crate及其版本进行建模。
  - general_model: 在对crate进行分析时候的模型，与Tugraph无关。
  - repo_sync_model: 从Kafka导入数据的格式，一个message代表一个Model实例。
  - tugraph_model: 导入和导出tugraph的模型，对应了Tugraph的模型，repo_import将其写为json格式。具体的tugraph的数据结构可以参考import.config.tmp。

本模块定义的数据结构被多个模块调用，包括repo_import，在该模块中读取kafka消息并解析，kafka消息队列的格式在本模块定义，以及对于data_transporter模块，tugraph相关的一些模型在本模块定义。

## repo_import模块

repo_import模块可以说是crates-pro的地基，在本模块中，我们进行了kafka消息队列消息的读取，解析并存储crate，所有获取的crate信息和数据均基于此模块，下面对此模块进行详细的解读。

### lib.rs

lib.rs是该模块的入口，其中主要封装了解析kafka消息队列消息的函数import_from_mq_for_a_message。

kafka消息队列是一种分布式的消息队列系统，通过追加的方式向消息队列中发送消息，根据offset顺序读取数据。

在函数import_from_mq_for_a_message中，按序读取每一条消息，消息的格式参见model模块，解析消息可以获取一些与crate相关的信息，包括其所在url(我们的crate存在mega数据库中，根据消息中的mega的url访问到crate的文件，再解析这些文件)，我们需要根据url从mega中将当前消息对应的crate文件clone到本地存起来，再对这些文件进行解析。

mega中的crate文件是一个git仓库，其内包含以往的tag，我们按tag将git仓库拆分成多个文件夹，其中一个crate版本对应一个文件夹，再对每个版本进行解析，在解析的过程中，我们主要访问根目录下的Cargo.toml文件，其内包含了众多crate相关信息，我们解析每一个Cargo.toml文件信息并存储，目前kafka的全量消息大约有17w条，未来的增量数据会不断增加。

### crate_info.rs

主要包含解析Cargo.toml文件的逻辑。

### git.rs

主要包含一些git操作对应的函数，如clone等。

### utils.rs

包含一些辅助函数

### version_info.rs

主要负责处理解析单个crate版本信息，以及处理并存储crate之间的依赖关系。

### 补充说明

该模块负责crate的解析工作，目前解析逻辑完备，数据全面，后续如需要添加crate的分析测试等功能，可能需要在本模块添加功能调用。

## search模块

该模块主要功能是实现crate的搜索，前面提到我们目前的搜索逻辑是根据crate名字进行搜索并按名字相关度排序，后续可以引入大模型实现更多的排序策略，该模块在/api/search接口中被调用。

## sync_tool模块

该模块主要负责处理kafka消息队列的增量逻辑，crates-pro解析时的数据来源于kafka消息队列，我们可以根据每一条消息定位到对应的crate文件夹，其中增量部分的消息生成逻辑在本模块实现。

### 补充说明

该模块负责生成kafka消息，独立于crates-pro之外，该模块单独部署。

## 进行后端开发的流程

目前crates-pro已经进展成为包含众多功能的初步版本，但仍有许多功能待实现，前面已经提到进行后端api开发的流程(主要为前后端功能的实现及联调)，下面介绍如何进行后端功能开发。

以search模块为例子，search模块为最新添加的模块，主要负责crate搜索功能。

在进行开发之前需要开发者掌握Rust如何进行项目管理，在项目的根目录下，有一个Cargo.toml文件，在其中进行整个项目的管理，根目录下的每一个文件夹都可以视为一个lib类型的crate(crates_pro除外)，每个crate可以被其他的crate调用，故开发者可以单独将一个功能设计为一个crate，如上述的search功能，再结合Rust的项目管理机制将该功能集成到整个项目中。