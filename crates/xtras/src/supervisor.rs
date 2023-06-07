use crate::ActorName;
use futures::Future;
use futures::FutureExt;
use std::error::Error;
use std::fmt;
use std::ops::ControlFlow;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::time::Duration;
use tracing::Instrument;
use xtra::Address;
use xtra::Context;

/// A supervising actor reacts to messages from the actor it is supervising and restarts it based on
/// a given policy.
pub struct Supervisor<T, R> {
    context: Context<T>,
    ctor: Box<dyn Fn() -> T + Send + 'static>,
    restart_policy: AsyncClosure<R>,
    metrics: Metrics,
}

type AsyncClosure<R> = Box<
    dyn for<'a> FnMut(&'a R) -> Pin<Box<dyn Future<Output = bool> + 'a + Send + Sync>>
        + Send
        + Sync,
>;

/// Closure that configures the supervisor to restart on every kind of error
pub fn always_restart<E>() -> AsyncClosure<E>
where
    E: Error + Send + Sync + 'static,
{
    Box::new(|_: &E| Box::pin(async move { true }))
}

/// Closure that configures the supervisor to restart on every kind of error,
/// after waiting for the specified `wait_time`.
///
/// Useful for preventing tight loops.
pub fn always_restart_after<E>(wait_time: Duration) -> AsyncClosure<E>
where
    E: Error + Send + Sync + 'static,
{
    let wait_time = wait_time;
    Box::new(move |_: &E| {
        Box::pin(async move {
            tokio_extras::time::sleep(wait_time)
                .instrument(tracing::trace_span!("Wait before restarting actor"))
                .await;
            true
        })
    })
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Metrics {
    /// How many times the supervisor spawned an instance of the actor.
    pub num_spawns: u64,
    /// How many times the actor shut down due to a panic.
    pub num_panics: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct UnitReason {}

impl fmt::Display for UnitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Error for UnitReason {}

impl From<()> for UnitReason {
    fn from(_: ()) -> Self {
        UnitReason {}
    }
}

impl<T> Supervisor<T, UnitReason>
where
    T: xtra::Actor<Stop = ()>,
{
    /// Construct a new supervisor for an [`Actor`] with an [`xtra::Actor::Stop`] value of `()`.
    ///
    /// The actor will always be restarted if it stops. If you don't want this behaviour, don't use
    /// a supervisor. If you want more fine-granular control in which circumstances the actor
    /// should be restarted, set [`xtra::Actor::Stop`] to a more descriptive value and use
    /// [`Actor::with_policy`].
    pub fn new(ctor: impl (Fn() -> T) + Send + 'static) -> (Self, Address<T>) {
        let (address, context) = Context::new(None);

        let supervisor = Self {
            context,
            ctor: Box::new(ctor),
            restart_policy: always_restart(),
            metrics: Metrics::default(),
        };

        (supervisor, address)
    }
}

impl<T, R, S> Supervisor<T, R>
where
    T: xtra::Actor<Stop = S>,
    R: Error + Send + Sync + 'static,
    S: Into<R> + Send + 'static,
{
    /// Construct a new supervisor.
    ///
    /// The supervisor needs to know two things:
    /// 1. How to construct an instance of the actor.
    /// 2. When to construct an instance of the actor.
    pub fn with_policy(
        ctor: impl (Fn() -> T) + Send + 'static,
        restart_policy: AsyncClosure<R>,
    ) -> (Self, Address<T>) {
        let (address, context) = Context::new(None);

        let supervisor = Self {
            context,
            ctor: Box::new(ctor),
            restart_policy,
            metrics: Metrics::default(),
        };

        (supervisor, address)
    }

    pub async fn run_log_summary(self) {
        let weak = self.context.weak_address();
        let (exit, metrics) = self.run().await;

        tracing::info!(
            ?metrics,
            actor = %std::any::type_name::<T>(),
            reason = %format!("{:#}", exit), // Format entire chain of errors with alternate Display
            connected = %weak.is_connected(),
            "Supervisor exited"
        );
    }

    pub async fn run(mut self) -> (anyhow::Error, Metrics) {
        let mut actor = self.spawn_new().await;

        loop {
            if !self.context.running {
                let reason = actor.stopped().await.into();
                let restart = (self.restart_policy)(&reason).await;
                let err = anyhow::Error::new(reason);
                let connected = self.context.weak_address().is_connected();

                tracing::info!(
                    actor = %T::name(),
                    // Format entire chain of errors by using alternate Display (#)
                    reason = %format!("{:#}", err),
                    %restart,
                    %connected,
                    "Actor stopped"
                );

                if restart && connected {
                    // Spawn the actor and continue to check context.running again
                    actor = self.spawn_new().await;
                    continue;
                } else {
                    tracing::info!("Ending supervisor loop");
                    break (err, self.metrics);
                }
            }

            let msg = self.context.next_message().await;

            match AssertUnwindSafe(self.context.tick(msg, &mut actor))
                .catch_unwind()
                .await
            {
                Ok(ControlFlow::Continue(())) => (),
                Ok(ControlFlow::Break(())) => (), // This will run `if !self.context.running` above
                Err(error) => {
                    let actor_name = T::name();
                    let reason = match error.downcast::<&'static str>() {
                        Ok(reason) => *reason,
                        Err(_) => "unknown",
                    };

                    tracing::info!(actor = %&actor_name, %reason, restart = true, "Actor panicked");

                    self.metrics.num_panics += 1;
                    actor = self.spawn_new().await;
                }
            }
        }
    }

    async fn spawn_new(&mut self) -> T {
        let actor_name = T::name();
        tracing::info!(actor = %&actor_name, "Spawning new actor instance");
        self.metrics.num_spawns += 1;
        let mut actor = (self.ctor)();
        self.context.running = true;
        actor.started(&mut self.context).await;
        actor
    }
}

/// Return the metrics tracked by this supervisor.
///
/// Currently private because it is a feature only used for testing. If we want to expose metrics
/// about the supervisor, we should look into creating a [`tracing::Subscriber`] that processes the
/// events we are emitting.
#[derive(Debug)]
struct GetMetrics;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SendAsyncSafe;
    use async_trait::async_trait;
    use std::io;
    use std::time::Duration;
    use tokio_extras::Tasks;
    use tracing_subscriber::util::SubscriberInitExt;
    use xtra_productivity::xtra_productivity;

    #[tokio::test]
    async fn supervisor_tracks_spawn_metrics() {
        let _guard = tracing_subscriber::fmt().with_test_writer().set_default();
        let mut tasks = Tasks::default();

        let (supervisor, addr) =
            Supervisor::with_policy(|| RemoteShutdown, always_restart::<io::Error>());
        let task = supervisor.run();

        drop(addr);

        let (_, metrics) = task.await;

        assert_eq!(
            metrics.num_spawns, 1,
            "after initial spawn, should have 1 spawn"
        );

        let (supervisor, addr) =
            Supervisor::with_policy(|| RemoteShutdown, always_restart::<io::Error>());
        let task = supervisor.run();

        tasks.add(async move {
            let _ = addr.send(Shutdown).await;
            drop(addr);
        });

        let (_, metrics) = task.await;

        assert_eq!(metrics.num_spawns, 2, "with shutdown, should have 2 spawns");
    }

    #[tokio::test]
    async fn supervisor_can_delay_respawn() {
        let _guard = tracing_subscriber::fmt().with_test_writer().set_default();
        let mut tasks = Tasks::default();

        let wait_time_seconds = 2;
        let wait_time = Duration::from_secs(wait_time_seconds);

        let (supervisor, address) = Supervisor::with_policy(
            || RemoteShutdown,
            always_restart_after::<io::Error>(wait_time),
        );
        let task = supervisor.run();

        address.send_async_safe(Shutdown).await.unwrap();

        tasks.add(async move {
            tokio_extras::time::sleep(wait_time + Duration::from_secs(1)).await;
            drop(address);
        });

        let (_, metrics) = task.await;

        assert_eq!(
            metrics.num_spawns, 2,
            "after waiting longer than {wait_time_seconds}s, should have 2 spawns"
        );
    }

    #[tokio::test]
    async fn restarted_actor_is_usable() {
        let _guard = tracing_subscriber::fmt().with_test_writer().set_default();

        let (supervisor, address) =
            Supervisor::with_policy(|| RemoteShutdown, always_restart::<io::Error>());
        let task = supervisor.run();

        #[allow(clippy::disallowed_methods)]
        tokio::spawn(task);

        address.send(Shutdown).await.unwrap();

        let message = address.send(SayHello("World".to_owned())).await.unwrap();

        assert_eq!(message, "Hello World");
    }

    #[tokio::test]
    async fn supervisor_tracks_panic_metrics() {
        let _guard = tracing_subscriber::fmt().with_test_writer().set_default();

        std::panic::set_hook(Box::new(|_| ())); // Override hook to avoid panic printing to log.

        let (supervisor, address) =
            Supervisor::with_policy(|| PanickingActor, always_restart::<io::Error>());
        let task = supervisor.run();

        let _ = address.send(Panic).split_receiver().await;
        drop(address);

        let (_, metrics) = task.await;
        assert_eq!(metrics.num_spawns, 2, "after panic, should have 2 spawns");
        assert_eq!(metrics.num_panics, 1, "after panic, should have 1 panic");
    }

    #[tokio::test]
    async fn supervisor_can_supervise_unit_actor() {
        let _guard = tracing_subscriber::fmt().with_test_writer().set_default();

        let (supervisor, _) = Supervisor::new(|| UnitActor);
        let task = supervisor.run();

        task.await;
    }

    /// An actor that can be shutdown remotely.
    struct RemoteShutdown;

    #[derive(Debug)]
    struct Shutdown;

    struct SayHello(String);

    #[async_trait]
    impl xtra::Actor for RemoteShutdown {
        type Stop = io::Error;

        async fn stopped(self) -> Self::Stop {
            io::Error::new(io::ErrorKind::Other, "unknown")
        }
    }

    #[xtra_productivity]
    impl RemoteShutdown {
        fn handle(&mut self, _: Shutdown, ctx: &mut Context<Self>) {
            ctx.stop_self()
        }

        fn handle(&mut self, msg: SayHello) -> String {
            format!("Hello {}", msg.0)
        }
    }

    struct PanickingActor;

    #[derive(Debug)]
    struct Panic;

    #[async_trait]
    impl xtra::Actor for PanickingActor {
        type Stop = io::Error;

        async fn stopped(self) -> Self::Stop {
            io::Error::new(io::ErrorKind::Other, "unknown")
        }
    }

    #[xtra_productivity]
    impl PanickingActor {
        fn handle(&mut self, _: Panic) {
            panic!("Help!")
        }
    }

    struct UnitActor;

    #[async_trait]
    impl xtra::Actor for UnitActor {
        type Stop = ();

        async fn stopped(self) -> Self::Stop {}
    }
}
