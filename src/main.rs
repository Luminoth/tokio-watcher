use futures::FutureExt;
use rand::prelude::*;
use tokio::sync::watch;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Status {
    NotStarted,
    Running,
    Complete,
}

#[derive(Debug)]
struct Runner {
    tx: watch::Sender<Status>,
}

impl Default for Runner {
    fn default() -> Self {
        let (tx, _) = watch::channel(Status::NotStarted);
        Self { tx }
    }
}

impl Runner {
    fn subscribe(&self) -> watch::Receiver<Status> {
        self.tx.subscribe()
    }

    async fn run(&self) {
        let mut rng = rand::thread_rng();
        let wait = rng.gen_range(5..10);

        self.tx.send_replace(Status::Running);
        println!("running ({}) ...", wait);

        tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;

        println!("complete!");
        self.tx.send_replace(Status::Complete);
    }
}

#[derive(Debug)]
struct Waiter {
    dependencies: Vec<watch::Receiver<Status>>,
}

impl Waiter {
    fn new(dependencies: Vec<watch::Receiver<Status>>) -> Self {
        Self { dependencies }
    }

    async fn wait_for_dependencies(&mut self) {
        let mut tasks = vec![];
        for rx in self.dependencies.iter_mut() {
            tasks.push(rx.wait_for(|status| *status == Status::Complete));
        }

        println!("waiting for dependencies ...");
        futures::future::join_all(tasks).await;
        println!("dependencies finished!");
    }
}

#[derive(Debug)]
struct App {
    runners: Vec<Runner>,
    waiter: Waiter,
}

impl Default for App {
    fn default() -> Self {
        let mut runners = vec![];
        let mut dependencies = vec![];

        for _ in 0..10 {
            let runner = Runner::default();
            dependencies.push(runner.subscribe());
            runners.push(runner);
        }

        Self {
            runners,
            waiter: Waiter::new(dependencies),
        }
    }
}

impl App {
    async fn run(&mut self) {
        let mut tasks = vec![];
        for runner in &self.runners {
            tasks.push(runner.run().boxed_local());
        }
        tasks.push(self.waiter.wait_for_dependencies().boxed_local());

        futures::future::join_all(tasks).await;
    }
}

#[tokio::main]
async fn main() {
    let mut app = App::default();
    app.run().await;

    println!("Done!");
}
