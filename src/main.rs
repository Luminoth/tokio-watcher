use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use futures::FutureExt;
use rand::prelude::*;
use tokio::sync::watch;

static THING_IDS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
enum TaskStatus {
    #[default]
    NotStarted,
    Waiting,
    Running,
    Complete,
}

#[derive(Debug)]
struct TaskState {
    tx: watch::Sender<TaskStatus>,
    dependencies: Vec<watch::Receiver<TaskStatus>>,
}

impl TaskState {
    fn new(dependencies: Vec<watch::Receiver<TaskStatus>>) -> Self {
        let (tx, _) = watch::channel(TaskStatus::default());
        Self { tx, dependencies }
    }

    fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    fn update_status(&self, status: TaskStatus) -> TaskStatus {
        self.tx.send_replace(status)
    }
}

#[derive(Debug, Copy, Clone)]
enum TaskType {
    Destroy,
    Create,
    Update,
}

#[derive(Debug)]
enum Task<T>
where
    T: Thing,
{
    Destroy(T, TaskState),
    Create(T, TaskState),
    Update(T, T, TaskState),
}

impl<T> Task<T>
where
    T: Thing,
{
    fn get_id(&self) -> usize {
        match self {
            Self::Destroy(thing, _) => thing.get_id(),
            Self::Create(thing, _) => thing.get_id(),
            Self::Update(thing, _, _) => thing.get_id(),
        }
    }

    fn get_task_type(&self) -> TaskType {
        match self {
            Self::Destroy(_, _) => TaskType::Destroy,
            Self::Create(_, _) => TaskType::Create,
            Self::Update(_, _, _) => TaskType::Update,
        }
    }

    fn get_thing_type(&self) -> ThingType {
        match self {
            Self::Destroy(thing, _) => thing.get_type(),
            Self::Create(thing, _) => thing.get_type(),
            Self::Update(thing, _, _) => thing.get_type(),
        }
    }

    fn get_state(&self) -> &TaskState {
        match self {
            Self::Destroy(_, state) => state,
            Self::Create(_, state) => state,
            Self::Update(_, _, state) => state,
        }
    }

    fn get_state_mut(&mut self) -> &mut TaskState {
        match self {
            Self::Destroy(_, state) => state,
            Self::Create(_, state) => state,
            Self::Update(_, _, state) => state,
        }
    }

    fn subscribe(&self) -> watch::Receiver<TaskStatus> {
        self.get_state().tx.subscribe()
    }

    async fn wait_for_dependencies(&mut self) {
        let id = self.get_id();
        let task_type = self.get_task_type();
        let thing_type = self.get_thing_type();

        let mut tasks = vec![];
        for rx in self.get_state_mut().dependencies.iter_mut() {
            tasks.push(rx.wait_for(|status| *status == TaskStatus::Complete));
        }

        println!(
            "[{} {:?} {:?}]: waiting for dependencies ...",
            id, task_type, thing_type
        );
        futures::future::join_all(tasks).await;
        println!(
            "[{} {:?} {:?}]: dependencies finished!",
            id, task_type, thing_type
        );
    }

    async fn run(&mut self) {
        let id = self.get_id();
        let task_type = self.get_task_type();
        let thing_type = self.get_thing_type();

        if self.get_state().has_dependencies() {
            self.get_state().update_status(TaskStatus::Waiting);
            self.wait_for_dependencies().await;
        }

        self.get_state().update_status(TaskStatus::Running);

        let mut rng = rand::thread_rng();
        let wait = rng.gen_range(5..10);

        println!(
            "[{} {:?} {:?}]: running ({}) ...",
            id, task_type, thing_type, wait
        );

        tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;

        println!("[{} {:?} {:?}]: complete!", id, task_type, thing_type);

        self.get_state().update_status(TaskStatus::Complete);
    }
}

#[derive(Debug, Copy, Clone)]
enum ThingType {
    ThingOne,
    ThingTwo,
}

trait Thing {
    fn get_internal_id(&self) -> usize;

    fn get_external_id(&self) -> Option<usize>;

    fn get_type(&self) -> ThingType;
}

#[derive(Debug)]
struct ThingOne {
    id: usize,
}

impl Thing for ThingOne {
    fn get_internal_id(&self) -> usize {
        self.id
    }

    fn get_external_id(&self) -> Option<usize> {
        None
    }

    fn get_type(&self) -> ThingType {
        ThingType::ThingOne
    }
}

impl ThingOne {
    fn new() -> Self {
        Self {
            id: THING_IDS.fetch_add(1, SeqCst),
        }
    }
}

#[derive(Debug)]
struct ThingTwo {
    id: usize,
}

impl Thing for ThingTwo {
    fn get_internal_id(&self) -> usize {
        self.id
    }

    fn get_external_id(&self) -> Option<usize> {
        None
    }

    fn get_type(&self) -> ThingType {
        ThingType::ThingTwo
    }
}

impl ThingTwo {
    fn new() -> Self {
        Self {
            id: THING_IDS.fetch_add(1, SeqCst),
        }
    }
}

#[derive(Debug)]
struct App {
    thing_one_tasks: Vec<Task<ThingOne>>,
    thing_two_tasks: Vec<Task<ThingTwo>>,
}

impl Default for App {
    fn default() -> Self {
        let mut thing_one_tasks = vec![];
        for _ in 0..10 {
            let task = Task::Create(ThingOne::new(), TaskState::new(vec![]));
            thing_one_tasks.push(task);
        }

        let mut thing_two_tasks = vec![];
        for i in 0..10 {
            let task = Task::Create(
                ThingTwo::new(),
                TaskState::new(vec![thing_one_tasks[i].subscribe()]),
            );
            thing_two_tasks.push(task);
        }

        Self {
            thing_one_tasks,
            thing_two_tasks,
        }
    }
}

impl App {
    async fn run(&mut self) {
        let mut tasks = vec![];

        for task in &mut self.thing_one_tasks {
            tasks.push(task.run().boxed_local());
        }

        for task in &mut self.thing_two_tasks {
            tasks.push(task.run().boxed_local());
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
