use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use futures::FutureExt;
use rand::prelude::*;
use tokio::sync::watch;

static RUNNER_IDS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
enum Status {
    #[default]
    NotStarted,
    Waiting,
    Running,
    Complete,
}

#[derive(Debug)]
enum TaskType {
    Create,
    Destroy,
    Update,
}

#[derive(Debug)]
struct Runner {
    id: usize,

    task_type: TaskType,

    tx: watch::Sender<Status>,
    dependencies: Vec<watch::Receiver<Status>>,
}

impl Runner {
    fn new(task_type: TaskType, dependencies: Vec<watch::Receiver<Status>>) -> Self {
        let (tx, _) = watch::channel(Status::default());
        Self {
            id: RUNNER_IDS.fetch_add(1, SeqCst),
            task_type,
            tx,
            dependencies,
        }
    }

    fn subscribe(&self) -> watch::Receiver<Status> {
        self.tx.subscribe()
    }

    async fn wait_for_dependencies(&mut self) {
        let mut tasks = vec![];
        for rx in self.dependencies.iter_mut() {
            tasks.push(rx.wait_for(|status| *status == Status::Complete));
        }

        println!(
            "[{} {:?}]: waiting for dependencies ...",
            self.id, self.task_type
        );
        futures::future::join_all(tasks).await;
        println!("[{} {:?}]: dependencies finished!", self.id, self.task_type);
    }

    async fn run(&mut self) {
        self.tx.send_replace(Status::Waiting);
        self.wait_for_dependencies().await;

        let mut rng = rand::thread_rng();
        let wait = rng.gen_range(5..10);

        self.tx.send_replace(Status::Running);
        println!("[{} {:?}]: running ({}) ...", self.id, self.task_type, wait);

        tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;

        println!("[{} {:?}]: complete!", self.id, self.task_type);
        self.tx.send_replace(Status::Complete);
    }
}

#[derive(Debug)]
struct App {
    runners: Vec<Runner>,
}

impl Default for App {
    fn default() -> Self {
        let mut runners = vec![];
        //let mut dependencies = vec![];

        for _ in 0..10 {
            let runner = Runner::new(TaskType::Create, vec![]);
            //dependencies.push(runner.subscribe());
            runners.push(runner);
        }

        Self { runners }
    }
}

impl App {
    async fn run(&mut self) {
        let mut tasks = vec![];
        for runner in &mut self.runners {
            tasks.push(runner.run().boxed_local());
        }

        futures::future::join_all(tasks).await;
    }
}

#[tokio::main]
async fn main() {
    let mut app = App::default();
    app.run().await;

    println!("Done!");
}
