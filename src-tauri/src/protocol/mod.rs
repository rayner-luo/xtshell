use std::sync::Arc;

use async_trait::async_trait;

use crate::error::AppResult;

/// 输出回调契约：实现须快速返回，不得同步/阻塞式获取 SessionManager 锁；
/// 需要访问 manager 时必须在闭包内 tokio::spawn 异步执行。
pub type OutputSink = Arc<dyn Fn(&[u8]) + Send + Sync + 'static>;

/// 连接断开通知，由协议内部读任务结束时调用（恰好一次）。
pub type ClosedSignal = Box<dyn FnOnce() + Send + 'static>;

#[async_trait]
pub trait TerminalStream: Send {
    async fn write(&mut self, data: &[u8]) -> AppResult<()>;
    async fn resize(&mut self, _cols: u32, _rows: u32) -> AppResult<()> { Ok(()) }
    async fn close(&mut self) -> AppResult<()>;
}

pub mod serial;
pub mod ssh;
pub mod telnet;
