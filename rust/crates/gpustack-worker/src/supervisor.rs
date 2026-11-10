//! Lifecycle supervision for critical Worker tasks.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use futures_util::stream::{FuturesUnordered, StreamExt};
use tokio::sync::watch;

pub(crate) struct SupervisedTaskExit<E> {
    pub name: &'static str,
    pub result: Result<(), E>,
}

pub(crate) struct WorkerSupervisor<'a, E> {
    shutdown: watch::Sender<bool>,
    tasks: FuturesUnordered<Pin<Box<dyn Future<Output = SupervisedTaskExit<E>> + Send + 'a>>>,
}

impl<'a, E: 'a> WorkerSupervisor<'a, E> {
    pub fn new() -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            shutdown,
            tasks: FuturesUnordered::new(),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }

    pub fn add<F>(&mut self, name: &'static str, task: F)
    where
        F: Future<Output = Result<(), E>> + Send + 'a,
    {
        self.tasks.push(Box::pin(async move {
            SupervisedTaskExit {
                name,
                result: task.await,
            }
        }));
    }

    pub async fn next_exit(&mut self) -> Option<SupervisedTaskExit<E>> {
        self.tasks.next().await
    }

    pub fn request_shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    pub async fn shutdown(
        mut self,
        grace: Duration,
    ) -> Result<Vec<SupervisedTaskExit<E>>, tokio::time::error::Elapsed> {
        self.request_shutdown();
        tokio::time::timeout(grace, async move {
            let mut exits = Vec::new();
            while let Some(exit) = self.tasks.next().await {
                exits.push(exit);
            }
            exits
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn observes_critical_exit_and_cancels_remaining_tasks() {
        let mut supervisor = WorkerSupervisor::<&'static str>::new();
        let mut shutdown = supervisor.subscribe();
        supervisor.add("critical", async { Err("failed") });
        supervisor.add("peer", async move {
            while !*shutdown.borrow() {
                shutdown.changed().await.map_err(|_| "closed")?;
            }
            Ok(())
        });

        let exit = supervisor.next_exit().await.expect("critical exit");
        assert_eq!(exit.name, "critical");
        assert_eq!(exit.result, Err("failed"));

        let exits = supervisor
            .shutdown(Duration::from_secs(1))
            .await
            .expect("bounded shutdown");
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].name, "peer");
        assert_eq!(exits[0].result, Ok(()));
    }

    #[tokio::test]
    async fn shutdown_deadline_is_enforced() {
        let mut supervisor = WorkerSupervisor::<()>::new();
        supervisor.add("blocked", std::future::pending());

        assert!(
            supervisor
                .shutdown(Duration::from_millis(10))
                .await
                .is_err()
        );
    }
}
