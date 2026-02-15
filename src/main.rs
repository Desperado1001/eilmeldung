// 声明模块：在 Rust 中，mod 关键字用于声明当前目录下或同名目录中的其他 .rs 文件。
// 类似于 C 语言中的 #include，但它定义了模块层级，而不仅仅是文本替换。
mod cli; // 对应 cli.rs 或 cli/mod.rs
mod config; // 对应 config.rs
mod connectivity; // 对应 connectivity.rs
mod input; // 对应 input.rs
mod logging; // 对应 logging.rs
mod login; // 对应 login.rs
mod messages; // 对应 messages.rs
mod newsflash_utils; // 对应 newsflash_utils.rs
mod query; // 对应 query.rs
mod ui; // 对应 ui.rs
mod utils; // 对应 utils.rs

// 使用 "use" 关键字将特定路径下的类型或函数引入当前作用域，类似于 C++ 的 using namespace。
// std 是 Rust 的标准库。
use std::{path::Path, sync::Arc, time::Duration};

// 引入外部 crate (库) 的功能。
use clap::Parser; // 用于解析命令行参数。
use log::{debug, error, info}; // 结构化日志。
use news_flash::{NewsFlash, models::LoginData}; // 核心业务逻辑库。
use tokio::{sync::mpsc::unbounded_channel, task::spawn_blocking}; // 异步运行时和任务调度。

mod prelude; // 引入 prelude 模块，通常存放常用的类型声明。
// crate 代表当前项目的根。
use crate::{connectivity::ConnectivityMonitor, prelude::*};

// #[tokio::main] 是一个宏属性。它在编译时会将异步的 main 函数包装成同步的，并启动异步运行时。
// async 指示这是一个异步函数，它不会立即执行，而是返回一个 Future 对象。
// color_eyre::Result<()> 返回一个结果类型，Ok(()) 表示成功，错误信息会被 color_eyre 格式化。
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    // 调用日志宏，记录程序启动。
    info!("Starting eilmeldung application");

    info!("parsing CLI args");
    // 解析命令行参数。let 用于绑定变量，默认是不可变的 (immutable)。
    let cli_args = CliArgs::parse();

    // 获取配置目录。Rust 中 Option 类型处理可能为空的情况，取代了 C 的 NULL 指针。
    let config_dir = cli_args
        .config_dir() // 返回 Option<&String>
        .as_ref() // 将 Option<String> 转为 Option<&String> (借用)
        .map(Path::new) // 如果有值，映射为 Path 对象
        .unwrap_or(PROJECT_DIRS.config_dir()); // 如果没值，使用默认项目配置目录。

    // 类似地获取状态目录。
    let state_dir = cli_args
        .state_dir()
        .as_ref()
        .map(Path::new)
        .unwrap_or(PROJECT_DIRS.state_dir().unwrap_or(PROJECT_DIRS.data_dir()));

    // 初始化错误处理框架。? 操作符表示：如果出错，立即返回 Err；如果成功，解出 Result 中的值。
    color_eyre::install()?;
    // 初始化日志系统。
    crate::logging::init_logging(&cli_args)?;
    debug!("Error handling and logging initialized");

    info!("Loading configuration");
    // 加载配置。Arc 是 "Atomic Reference Counted" 的缩写，类似于 C++11 的 shared_ptr，
    // 用于在多个线程间安全地共享数据所有权。
    let config = Arc::new(load_config(config_dir)?);

    info!("Initializing NewsFlash");
    // 使用构建器模式初始化 NewsFlash 对象。
    let news_flash_attempt = NewsFlash::builder()
        .config_dir(config_dir)
        .data_dir(state_dir)
        .try_load(); // 这里返回一个 Result，尝试从磁盘加载现有状态。

    // 创建 HTTP 客户端。Duration::from_secs 将秒转换成 Rust 的持续时间结构。
    let client = build_client(Duration::from_secs(config.network_timeout_seconds))?;

    // 使用 match 进行模式匹配，这是 Rust 处理枚举 (Enum) 的核心方式，比 C 的 switch 更强大且安全。
    let news_flash = match news_flash_attempt {
        // 如果加载成功 (Ok 变体)
        Ok(news_flash) => {
            // 尝试重新登录以刷新会话令牌。
            // .await 用于等待异步操作完成。
            if let Some(login_data) = news_flash.get_login_data().await {
                info!("Re-logging in to refresh session");
                if let Err(e) = news_flash.login(login_data, &client).await {
                    error!("Failed to re-login: {}. Session may have expired.", e);
                }
            }
            news_flash // 返回这个值，赋值给外部的 news_flash 变量。
        }
        // 如果加载失败 (Err 变体)，通常意味着是第一次运行。
        Err(_) => {
            info!("no profile found => ask user or try config");
            // mut 关键字表示变量是可变的。
            let mut logged_in = false;
            // 检查配置中是否有登录信息。
            let mut skip_asking_for_login = config.login_setup.is_some();

            // 链式处理：转换、检查并获取登录数据。
            let mut login_data: Option<LoginData> = config
                .login_setup
                .as_ref()
                .inspect(|_| info!("login configuration found"))
                .map(|login_configuration| login_configuration.to_login_data())
                .transpose()?; // 处理 Result 中的 Option。

            let login_setup = LoginSetup::new();
            let mut news_flash: Option<NewsFlash> = None;

            // 循环直到登录成功。
            while !logged_in {
                // 如果没有数据或需要詢問，则交互式获取登录信息。
                login_data = if login_data.is_none() || !skip_asking_for_login {
                    skip_asking_for_login = false;
                    Some(login_setup.inquire_login_data(&login_data).await?)
                } else {
                    login_data
                };
                // 创建 NewsFlash 实例。
                news_flash = Some(
                    NewsFlash::builder()
                        .data_dir(state_dir)
                        .config_dir(config_dir)
                        .plugin(login_data.as_ref().unwrap().id())
                        .create()?,
                );
                // 执行登录和初始同步。
                logged_in = login_setup
                    .login_and_initial_sync(
                        news_flash.as_ref().unwrap(), // .unwrap() 会取出 Option 里的值，如果为空则 panic (崩溃)。
                        login_data.as_ref().unwrap(),
                        &client,
                    )
                    .await?;
            }
            news_flash.unwrap() // 循环结束必定有值，安全解包。
        }
    };

    // 执行命令行特定操作，如果返回 true 则直接退出程序。
    if execute_cli_actions(&config, &cli_args, &news_flash, &client).await? {
        return Ok(());
    }

    // 设置应用内部通信。unbounded_channel 创建一个多生产者单消费者 (mpsc) 管道。
    // 返回一个发送端 (sender) 和接收端 (receiver)。
    let (message_sender, message_receiver) = unbounded_channel::<Message>();
    // .clone() 会创建发送端的副本，用于传递给不同组件。
    let input_reader_message_sender = message_sender.clone();

    // 将 news_flash 包装在 Arc 中，这样多个组件可以并发访问。
    let news_flash_utils = Arc::new(NewsFlashUtils::new(
        news_flash,
        client,
        config.clone(),
        message_sender.clone(),
    ));

    // 初始化网络连接状态监控。
    let connectivity_monitor =
        ConnectivityMonitor::new(news_flash_utils.clone(), message_sender.clone());

    // 创建主应用状态机。
    let app = App::new(config, news_flash_utils.clone(), message_sender);

    info!("Initializing terminal");
    // 初始化 TUI (终端用户界面) 库。
    let terminal = ratatui::init();

    // 启动一个后台线程专门读取键盘事件。
    // spawn_blocking 用于运行可能会阻塞当前异步线程的任务。
    // move 关键字表示将闭包中捕获的变量的所有权转移进闭包内。
    let _input_reader_handle = spawn_blocking(move || {
        // 在后台线程循环读取输入并发送给主频道。
        if let Err(err) = input_reader(input_reader_message_sender) {
            error!("input reader got an error: {err}");
        }
    });

    // 启动网络状态监控任务。
    let _connecitivty_monitor_handle = connectivity_monitor.spawn()?;

    info!("Starting application main loop");
    // 进入主循环，等待消息接收并渲染界面。
    let result = app.run(message_receiver, terminal).await;

    info!("Application loop ended, restoring terminal");
    // 恢复终端状态，避免程序退出后终端终端样式错乱。
    ratatui::restore();

    // 根据运行结果记录日志。
    match &result {
        Ok(_) => info!("Application exited successfully"),
        Err(e) => error!("Application exited with error: {}", e),
    }

    // 返回最后的结果，Ok(()) 或 Err。
    result
}
